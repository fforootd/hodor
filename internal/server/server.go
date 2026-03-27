// Package server provides the HTTP server that muxes REST API handlers,
// health checks, and Templ-rendered UI.
package server

import (
	"context"
	"embed"
	"fmt"
	"io/fs"
	"log"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/zitadel/zitadel/internal/analytics"
	"github.com/zitadel/zitadel/internal/api"
	"github.com/zitadel/zitadel/internal/auth"
	"github.com/zitadel/zitadel/internal/config"
	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/eventbus"
	"github.com/zitadel/zitadel/internal/fga"
	"github.com/zitadel/zitadel/internal/jobs"
	"github.com/zitadel/zitadel/internal/login"
	"github.com/zitadel/zitadel/internal/oidcop"
	"github.com/zitadel/zitadel/internal/session"
	"github.com/zitadel/zitadel/internal/ui"
)

//go:embed all:webdist
var webAssets embed.FS

// Server is the main ZITADEL HTTP server.
type Server struct {
	cfg       *config.Config
	db        *database.DB
	bus       *eventbus.Bus
	http      *http.Server
	api       *api.API
	fga       *fga.Service
	analytics *analytics.Engine
}

// New creates a new Server with all routes registered.
func New(cfg *config.Config, db *database.DB, bus *eventbus.Bus) *Server {
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
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte("ready"))
	})

	// Create hardened cookie config.
	cookieCfg := session.NewCookieConfig(cfg.Server.CookieSecrets, cfg.Server.ExternalDomain)

	// Mount REST API — identity, schema, session, event CRUD + dynamic OpenAPI.
	restAPI := api.New(db, bus, cookieCfg)
	restAPI.RegisterRoutes(mux)

	// Mount analytics engine (queries OLTP database directly — pure Go, no DuckDB).
	oltpBackend := analytics.NewOLTPBackend(db.SQL(), db.Dialect())
	analyticsEngine := analytics.New(oltpBackend)
	analyticsEngine.RegisterRoutes(mux)

	// Mount login flow API (serves <zitadel-login> web component).
	passwords := auth.NewPasswords(db)
	loginAPI := login.New(db, passwords, restAPI, cookieCfg)
	loginAPI.Register(mux)

	// Serve web assets (JS/CSS) from go:embed.
	webFS, err := fs.Sub(webAssets, "webdist")
	if err == nil {
		mux.Handle("GET /assets/", http.FileServer(http.FS(webFS)))
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

	// Mount OIDC Provider (OP) — handles /.well-known/openid-configuration,
	// /authorize, /oauth/token, /userinfo, /keys, /end_session etc.
	issues := fmt.Sprintf("http://%s:%d", cfg.Server.ExternalDomain, cfg.Server.Port)
	oidcStorage := oidcop.NewStorage(db)
	opHandler, err := oidcop.SetupProvider(oidcStorage, issues, nil)
	if err != nil {
		log.Printf("WARN: OIDC Provider setup failed: %v", err)
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
		log.Printf("OIDC Provider ready (issuer=%s)", issues)
	}

	// Wrap the mux with OTel middleware for trace_id injection.
	wrappedHandler := OTelMiddleware(mux)

	httpSrv := &http.Server{
		Addr:         fmt.Sprintf(":%d", cfg.Server.Port),
		Handler:      wrappedHandler,
		ReadTimeout:  15 * time.Second,
		WriteTimeout: 15 * time.Second,
		IdleTimeout:  60 * time.Second,
	}

	return &Server{
		cfg:       cfg,
		db:        db,
		bus:       bus,
		http:      httpSrv,
		api:       restAPI,
		analytics: analyticsEngine,
	}
}

// Handler returns the HTTP handler for testing purposes.
func (s *Server) Handler() http.Handler {
	return s.http.Handler
}

// ListenAndServe starts the HTTP server and blocks until shutdown.
// It handles SIGTERM/SIGINT for graceful shutdown.
func (s *Server) ListenAndServe() error {
	ctx := context.Background()

	// Initialise embedded OpenFGA (shares our SQLite/Postgres DB).
	fgaSvc, err := fga.New(ctx, s.db.SQL(), s.db.Dialect())
	if err != nil {
		return fmt.Errorf("init fga: %w", err)
	}
	s.fga = fgaSvc

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
	go func() {
		log.Printf("listening on %s", s.http.Addr)
		if s.cfg.Server.TLSCert != "" && s.cfg.Server.TLSKey != "" {
			errCh <- s.http.ListenAndServeTLS(s.cfg.Server.TLSCert, s.cfg.Server.TLSKey)
		} else {
			errCh <- s.http.ListenAndServe()
		}
	}()

	select {
	case err := <-errCh:
		if err != nil && err != http.ErrServerClosed {
			return fmt.Errorf("server error: %w", err)
		}
	case sig := <-sigCh:
		log.Printf("received %v, shutting down gracefully...", sig)
		shutdownCtx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		if err := s.http.Shutdown(shutdownCtx); err != nil {
			return fmt.Errorf("shutdown: %w", err)
		}
		log.Println("shutdown complete")
	}

	return nil
}
