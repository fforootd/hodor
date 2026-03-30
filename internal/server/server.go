// Package server provides the HTTP server that muxes REST API handlers,
// health checks, and Templ-rendered UI.
package server

import (
	"context"
	"crypto/rand"
	cryptotls "crypto/tls"
	"embed"
	"fmt"
	"io/fs"
	"net"
	"net/http"
	"os"
	"os/signal"
	"strconv"
	"strings"
	"sync/atomic"
	"syscall"
	"time"

	"github.com/zitadel/zitadel/internal/analytics"
	"github.com/zitadel/zitadel/internal/api"
	"github.com/zitadel/zitadel/internal/auth"
	"github.com/zitadel/zitadel/internal/catalog"
	"github.com/zitadel/zitadel/internal/config"
	zcrypto "github.com/zitadel/zitadel/internal/crypto"
	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/eventbus"
	"github.com/zitadel/zitadel/internal/fga"
	"github.com/zitadel/zitadel/internal/jobs"
	"github.com/zitadel/zitadel/internal/login"
	"github.com/zitadel/zitadel/internal/loginflow"
	"github.com/zitadel/zitadel/internal/logging"
	"github.com/zitadel/zitadel/internal/mgmt"
	"github.com/zitadel/zitadel/internal/oidcop"
	"github.com/zitadel/zitadel/internal/ratelimit"
	"github.com/zitadel/zitadel/internal/session"
	"github.com/zitadel/zitadel/internal/ui"
	"github.com/zitadel/zitadel/internal/ztls"
)

//go:embed all:webdist
var webAssets embed.FS

// Server is the main Zitadel HTTP server.
type Server struct {
	cfg       *config.Config
	db        *database.DB
	bus       *eventbus.Bus
	http      *http.Server
	api       *api.API
	fga       *fga.Service
	analytics *analytics.Engine
	tlsMgr    *ztls.Manager
	ready     atomic.Bool
}

// New creates a new Server with all routes registered.
func New(cfg *config.Config, db *database.DB, bus *eventbus.Bus) *Server {
	srv := &Server{
		cfg: cfg,
		db:  db,
		bus: bus,
	}
	mux := http.NewServeMux()

	// Health check — always first.
	mux.HandleFunc("GET /healthz", func(w http.ResponseWriter, r *http.Request) {
		if err := db.SQL().PingContext(r.Context()); err != nil {
			http.Error(w, "database unhealthy", http.StatusServiceUnavailable)
			return
		}
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte("ok"))
	})

	// Readiness check.
	mux.HandleFunc("GET /readyz", func(w http.ResponseWriter, r *http.Request) {
		if !srv.ready.Load() {
			http.Error(w, "starting", http.StatusServiceUnavailable)
			return
		}
		if err := db.SQL().PingContext(r.Context()); err != nil {
			http.Error(w, "database unhealthy", http.StatusServiceUnavailable)
			return
		}
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte("ready"))
	})

	// Create hardened cookie config.
	cookieCfg := session.NewCookieConfig(cfg.Server.CookieSecrets, cfg.Server.ExternalDomain)

	// Mount REST API — identity, schema, session, event CRUD + dynamic OpenAPI.
	restAPI := api.New(db, bus, cookieCfg)
	restAPI.RegisterRoutes(mux)

	// Mount template catalog API (ADR-015).
	catalogSvc := catalog.New(cfg.Catalog, db.SQL())
	restAPI.SetCatalogService(catalogSvc)
	api.RegisterCatalogRoutes(mux, catalogSvc, db.SQL())
	logging.Printf("Catalog ready (%d embedded templates)", catalogSvc.EmbeddedCount())

	// Mount analytics engine (queries OLTP database directly — pure Go, no DuckDB).
	oltpBackend := analytics.NewOLTPBackend(db.SQL(), db.Dialect())
	analyticsEngine := analytics.New(oltpBackend)
	analyticsEngine.RegisterRoutes(mux)

	// Mount login flow API (serves <zitadel-login> web component).
	// Use fast argon2id params in dev mode to avoid 30s+ login latency.
	var passwords *auth.Passwords
	if cfg.Dev.MockOIDC || cfg.Dev.SeedFile != "" {
		passwords = auth.NewPasswordsDev(db)
	} else {
		passwords = auth.NewPasswords(db)
	}
	loginAPI := login.New(db, passwords, restAPI, cookieCfg, loginflow.NewResolver(db))
	loginAPI.Register(mux)

	// Serve web assets (JS/CSS) from go:embed.
	webFS, err := fs.Sub(webAssets, "webdist")
	if err == nil {
		mux.Handle("GET /assets/", http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			w.Header().Set("Cache-Control", "public, max-age=31536000, immutable")
			http.FileServer(http.FS(webFS)).ServeHTTP(w, r)
		}))
	}

	// Vue login page — server-rendered shell enhanced by Vue.
	serveLogin := func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/html; charset=utf-8")
		data, err := webAssets.ReadFile("webdist/src/login/index.html")
		if err != nil {
			http.Error(w, "login page not found", http.StatusNotFound)
			return
		}
		_, _ = w.Write(data)
	}
	mux.HandleFunc("GET /login", serveLogin)

	// Vue account (self-service profile) page.
	serveAccount := func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/html; charset=utf-8")
		data, err := webAssets.ReadFile("webdist/src/account/index.html")
		if err != nil {
			http.Error(w, "account page not found", http.StatusNotFound)
			return
		}
		w.Write(data)
	}
	mux.HandleFunc("/account", serveAccount)
	mux.HandleFunc("/account/", serveAccount)

	// Root redirect → login.
	mux.HandleFunc("GET /{$}", func(w http.ResponseWriter, r *http.Request) {
		http.Redirect(w, r, "/login", http.StatusTemporaryRedirect)
	})

	// Vue console SPA — catch-all for /console/* routes.
	mux.HandleFunc("/console", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/html; charset=utf-8")
		data, err := webAssets.ReadFile("webdist/src/console/index.html")
		if err != nil {
			http.Error(w, "console not found", http.StatusNotFound)
			return
		}
		w.Write(data)
	})
	mux.HandleFunc("/console/", func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "text/html; charset=utf-8")
		data, err := webAssets.ReadFile("webdist/src/console/index.html")
		if err != nil {
			http.Error(w, "console not found", http.StatusNotFound)
			return
		}
		w.Write(data)
	})

	// Mount UI routes — login, logout, admin console.
	uiHandlers := ui.New(db, bus, restAPI, cookieCfg)
	uiHandlers.RegisterRoutes(mux)

	// --- Initialize Application-Level Encryption (ALE) ---
	// SecretBox provides AES-256-GCM envelope encryption for all secrets at rest.
	// In dev mode with no keys configured, operates in plaintext passthrough.
	secretBox, err := zcrypto.NewSecretBox(cfg.Encryption.ActiveKeyID, cfg.Encryption.KeyMap())
	if err != nil {
		logging.Fatalf("encryption init: %v", err)
	}
	if secretBox.Plaintext() {
		logging.Println("[WARN] encryption: no keys configured — secrets stored in plaintext (dev mode)")
	} else {
		logging.Printf("[encryption] active_key=%s, ring_size=%d", cfg.Encryption.ActiveKeyID, len(cfg.Encryption.Keys))
	}
	secretStore := zcrypto.NewSecretStore(db.SQL(), secretBox)

	// Mount OIDC Provider (OP) — handles /.well-known/openid-configuration,
	// /authorize, /oauth/token, /userinfo, /keys, /end_session etc.
	issues := issuerURL(cfg)
	oidcStorage := oidcop.NewStorage(db, secretStore)
	var firstCookieSecret string
	if len(cfg.Server.CookieSecrets) > 0 {
		firstCookieSecret = cfg.Server.CookieSecrets[0]
	}
	// The OIDC encryption key now lives in the secrets table (type 'oidc_encryption').
	// On first boot it's auto-generated and stored encrypted.
	oidcEncKey := getOrCreateOIDCEncryptionKey(secretStore)
	opHandler, err := oidcop.SetupProvider(oidcStorage, issues, nil, oidcEncKey, firstCookieSecret)
	if err != nil {
		logging.Printf("WARN: OIDC Provider setup failed: %v", err)
	} else {
		// The OP handler is mounted as a fallback: the mux tries registered patterns first,
		// and if none match, falls through to the OP which handles OIDC-specific paths.
		mux.Handle("/authorize", opHandler)
		mux.Handle("/oauth/", opHandler)
		mux.Handle("/userinfo", opHandler)
		mux.Handle("/end_session", opHandler)
		mux.Handle("/keys", opHandler)
		mux.Handle("/revoke", opHandler)
		mux.Handle("/devicecode", opHandler)
		// Discovery endpoint (appended — the existing well-known in api.go is a redirect)
		mux.Handle("GET /.well-known/openid-configuration", opHandler)
		logging.Printf("OIDC Provider ready (issuer=%s)", issues)
	}

	// --- Resolve paths for route prefixing ---
	paths := cfg.Server.ResolvePaths()
	logging.Printf("Path config: base=%q console=%q api=%q oidc=%q", paths.Base, paths.Console, paths.API, paths.OIDC)

	// --- Build middleware chain (outermost first) ---
	// 1. RealIP — resolve true client IP from proxy headers.
	var realIPCfg *RealIPConfig
	if len(cfg.Server.TrustedProxies) > 0 {
		cidrs, err := ParseTrustedProxies(cfg.Server.TrustedProxies)
		if err != nil {
			logging.Printf("WARN: failed to parse trusted proxies: %v", err)
		} else {
			realIPCfg = &RealIPConfig{
				TrustedCIDRs: cidrs,
				Mode:         cfg.Server.ProxyHeaderMode,
				CustomHeader: cfg.Server.RealIPHeader,
			}
			logging.Printf("RealIP middleware: mode=%q trusted_cidrs=%d", realIPCfg.Mode, len(cidrs))
		}
	}

	// 2. Security headers.
	isSecure := cfg.Server.TLSCert != "" || (cfg.Server.ExternalDomain != "" && cfg.Server.ExternalDomain != "localhost")

	// Initialise embedded OpenFGA on its OWN dedicated DB connection.
	// Root cause of slowness: SQLite has a single write lock. When FGA's
	// Write() competed with app transactions on the shared pool, it deadlocked
	// and returned Internal Server Error (4000) after the SQLite busy timeout.
	// A separate connection lets both write concurrently via WAL mode.
	ctx := context.Background()
	fgaDB, err := database.Open(cfg.Database.URL)
	if err != nil {
		logging.Fatalf("fga: open dedicated db connection: %v", err)
	}
	fgaSvc, err := fga.New(ctx, fgaDB.SQL(), fgaDB.Dialect())
	if err != nil {
		logging.Fatalf("fga init: %v", err)
	}

	// Set the global FGA service reference for the API layer.
	api.FGAService = fgaSvc

	// Post-init FGA bootstrap: if admin exists but has no FGA tuples, seed them now.
	// This handles the case where EnsureAdmin ran before FGA was initialized.
	{
		var adminID string
		if err := db.SQL().QueryRowContext(ctx,
			`SELECT id FROM users WHERE identifier = 'admin' LIMIT 1`,
		).Scan(&adminID); err == nil && adminID != "" {
			// Check if tuples already exist for this admin.
			tuples, _ := fgaSvc.ReadTuples(ctx, "", "", "instance:self")
			if len(tuples) == 0 {
				if err := fgaSvc.OnBootstrap(ctx, adminID); err != nil {
					logging.Printf("WARN: FGA post-init bootstrap failed: %v", err)
				} else {
					logging.Printf("[fga] post-init bootstrap: admin=%s", adminID)
				}
			}
		}
	}

	// Build FGA middleware.
	fgaMiddleware := fga.NewMiddleware(fgaSvc)

	// Initialize rate limiter — backend from config (memory | sql | redis).
	var rlStore ratelimit.Store
	gcInterval := time.Duration(cfg.RateLimit.GCInterval) * time.Second
	if gcInterval == 0 {
		gcInterval = 60 * time.Second
	}
	switch cfg.RateLimit.Backend {
	case "sql":
		logging.Printf("Rate limiter: sql backend (batch_write=%v) — not yet implemented, using memory", cfg.RateLimit.BatchWrite)
		rlStore = ratelimit.NewMemoryStore(gcInterval)
	case "redis":
		logging.Printf("Rate limiter: redis backend (%s) — not yet implemented, using memory", cfg.RateLimit.RedisURL)
		rlStore = ratelimit.NewMemoryStore(gcInterval)
	default: // "memory"
		rlStore = ratelimit.NewMemoryStore(gcInterval)
		logging.Printf("Rate limiter ready (backend=memory, gc=%s)", gcInterval)
	}
	rateLimiter := ratelimit.New(rlStore, db.SQL())

	// Start catalog background refresh (after boot, non-blocking).
	catalogSvc.StartBackground()

	// Wrap the mux with middleware: RealIP → SecurityHeaders → AppGate → RateLimit → AuthGate → FGAGate → RequestLog → OTel.
	mgmtCfg := &mgmt.Config{Secret: cfg.Server.ManagementSecret}
	var handler http.Handler = mux
	handler = api.RequestLogMiddleware()(handler)
	handler = fgaMiddleware.Gate(handler)
	handler = api.AuthGate(cookieCfg, db.SQL(), mgmtCfg)(handler)
	handler = ratelimit.Middleware(rateLimiter, FromContext)(handler)
	handler = AppGate(paths, &cfg.Server.AppAccess)(handler)
	handler = SecurityHeaders(cfg.Server.SecurityHeaders, isSecure)(handler)
	handler = RealIP(realIPCfg)(handler)
	handler = OTelMiddleware(handler)

	httpSrv := &http.Server{
		Addr:        fmt.Sprintf(":%d", cfg.Server.Port),
		Handler:     handler,
		ReadTimeout: 15 * time.Second,
		// WriteTimeout is intentionally omitted: SSE streams (/v1/events/stream)
		// are long-lived connections and need unbounded write time.
		// Individual handlers use r.Context() cancellation for their own timeout.
		IdleTimeout: 60 * time.Second,
	}

	// Initialize TLS manager.
	isDev := cfg.Dev.MockOIDC || cfg.Dev.SeedFile != ""
	tlsMgr, err := ztls.NewManager(cfg.TLS, &cfg.Server, db.SQL(), secretBox, isDev)
	if err != nil {
		logging.Printf("WARN: TLS manager init failed: %v", err)
	}

	srv.http = httpSrv
	srv.api = restAPI
	srv.fga = fgaSvc
	srv.analytics = analyticsEngine
	srv.tlsMgr = tlsMgr
	srv.ready.Store(true)

	return srv
}

// Handler returns the HTTP handler for testing purposes.
func (s *Server) Handler() http.Handler {
	return s.http.Handler
}

// ListenAndServe starts the HTTP server and blocks until shutdown.
// It handles SIGTERM/SIGINT for graceful shutdown.
func (s *Server) ListenAndServe() error {
	// Start job scheduler with registered jobs.
	schedCtx, schedCancel := context.WithCancel(context.Background())
	defer schedCancel()

	sched := jobs.New(s.db)
	sched.Register("session_gc", jobs.SessionGC(s.db, s.bus))
	sched.Register("event_gc", jobs.EventGC(s.db, s.bus))
	go sched.Run(schedCtx)

	// Graceful shutdown on signals — use a channel, not NotifyContext.
	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, os.Interrupt, syscall.SIGTERM)

	errCh := make(chan error, 1)

	tlsMode := "off"
	if s.tlsMgr != nil {
		tlsMode = s.tlsMgr.Mode()
	}

	go func() {
		switch tlsMode {
		case "auto":
			// Auto-TLS via CertMagic.
			httpsPort := s.cfg.TLS.ResolveHTTPSPort()
			httpPort := s.cfg.TLS.ResolveHTTPPort()

			// Start HTTP listener for ACME challenges + redirect.
			go func() {
				httpAddr := fmt.Sprintf(":%d", httpPort)
				logging.Printf("[tls] HTTP listener on %s (ACME challenges + HTTPS redirect)", httpAddr)
				srv := &http.Server{
					Addr:              httpAddr,
					Handler:           s.tlsMgr.HTTPChallengeHandler(httpsPort),
					ReadTimeout:       5 * time.Second,
					ReadHeaderTimeout: 5 * time.Second,
				}
				if err := srv.ListenAndServe(); err != nil && err != http.ErrServerClosed {
					logging.Printf("WARN: HTTP listener error: %v", err)
				}
			}()

			// Start HTTPS listener.
			httpsAddr := fmt.Sprintf(":%d", httpsPort)
			logging.Printf("listening on %s (auto-TLS)", httpsAddr)
			ln, err := cryptotls.Listen("tcp", httpsAddr, s.tlsMgr.TLSConfig())
			if err != nil {
				errCh <- fmt.Errorf("tls listen: %w", err)
				return
			}
			errCh <- s.http.Serve(ln)

		case "manual":
			logging.Printf("listening on %s (manual TLS)", s.http.Addr)
			errCh <- s.http.ListenAndServeTLS(s.cfg.Server.TLSCert, s.cfg.Server.TLSKey)

		default: // "external", "off"
			logging.Printf("listening on %s", s.http.Addr)
			errCh <- s.http.ListenAndServe()
		}
	}()

	select {
	case err := <-errCh:
		if err != nil && err != http.ErrServerClosed {
			return fmt.Errorf("server error: %w", err)
		}
	case sig := <-sigCh:
		logging.Printf("received %v, shutting down gracefully...", sig)
		shutdownCtx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		if err := s.http.Shutdown(shutdownCtx); err != nil {
			return fmt.Errorf("shutdown: %w", err)
		}
		logging.Println("shutdown complete")
	}

	return nil
}

func issuerURL(cfg *config.Config) string {
	host := strings.TrimSpace(cfg.Server.ExternalDomain)
	if host == "" {
		host = "localhost"
	}

	isDev := cfg.Dev.MockOIDC || cfg.Dev.SeedFile != ""
	tlsMode := cfg.TLS.ResolveMode(&cfg.Server, isDev)

	scheme := "http"
	port := cfg.Server.Port
	switch tlsMode {
	case "auto", "manual":
		scheme = "https"
		port = cfg.TLS.ResolveHTTPSPort()
	case "external":
		scheme = "https"
		port = 443
	}

	if _, _, err := net.SplitHostPort(host); err == nil {
		return scheme + "://" + host
	}
	if (scheme == "http" && port == 80) || (scheme == "https" && port == 443) {
		return scheme + "://" + host
	}
	return scheme + "://" + net.JoinHostPort(host, strconv.Itoa(port))
}

// getOrCreateOIDCEncryptionKey retrieves the OIDC encryption key from the
// secrets table, or generates and stores a new one. Returns a 64-char hex
// string suitable for op.Config.CryptoKey.
func getOrCreateOIDCEncryptionKey(store *zcrypto.SecretStore) string {
	ctx := context.Background()

	// Try to load existing OIDC encryption key.
	_, keyBytes, err := store.GetByType(ctx, "oidc_encryption")
	if err == nil {
		return fmt.Sprintf("%x", keyBytes)
	}

	// Generate a new 32-byte key.
	key := make([]byte, 32)
	if _, err := rand.Read(key); err != nil {
		logging.Fatalf("generate OIDC encryption key: %v", err)
	}

	// Store it (envelope-encrypted by the key ring).
	id := fmt.Sprintf("oidc_enc_%d", time.Now().UnixMilli())
	if err := store.Put(ctx, id, "oidc_encryption", key, zcrypto.WithAlgorithm("AES256")); err != nil {
		logging.Fatalf("store OIDC encryption key: %v", err)
	}
	logging.Println("[secrets] generated and stored OIDC encryption key")

	return fmt.Sprintf("%x", key)
}
