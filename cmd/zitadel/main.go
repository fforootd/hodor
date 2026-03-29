// Package main is the entry point for the zitadel binary.
// A single binary serves as both the identity server and CLI client.
package main

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"time"

	"github.com/spf13/cobra"

	"github.com/zitadel/zitadel/internal/api"
	"github.com/zitadel/zitadel/internal/bootstrap"
	"github.com/zitadel/zitadel/internal/cli"
	"github.com/zitadel/zitadel/internal/config"
	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/eventbus"
	"github.com/zitadel/zitadel/internal/logging"
	"github.com/zitadel/zitadel/internal/mockoidc"
	"github.com/zitadel/zitadel/internal/seed"
	"github.com/zitadel/zitadel/internal/server"
)

// version is set by goreleaser at build time.
var version = "dev"

func main() {
	root := &cobra.Command{
		Use:   "zitadel",
		Short: "Identity infrastructure for humans and AI",
		Long:  "Zitadel — open-source identity platform with unified auth, fine-grained authorization, and AI agent governance.",
	}

	root.AddCommand(startCmd())
	root.AddCommand(migrateCmd())
	root.AddCommand(versionCmd())
	root.AddCommand(openapiExportCmd())

	if err := root.Execute(); err != nil {
		os.Exit(1)
	}
}

// startCmd starts the Zitadel identity server.
// Migration and bootstrap behavior depends on config and flags:
//   - Default: auto-migrate + auto-bootstrap (consistent for all dialects)
//   - migrate=check: version check only, fail if behind
//   - migrate=skip: no check, no migration (fastest cold-start)
//   - --auto-migrate flag: forces migrate=auto + bootstrap=auto
func startCmd() *cobra.Command {
	var configPath string
	var enableMockOIDC bool
	var seedFile string
	var autoMigrate bool

	cmd := &cobra.Command{
		Use:   "start",
		Short: "Start the Zitadel identity server",
		RunE: func(cmd *cobra.Command, args []string) error {
			cfg, err := config.Load(configPath)
			if err != nil {
				return fmt.Errorf("load config: %w", err)
			}

			// CLI flags override config/env.
			if enableMockOIDC {
				cfg.Dev.MockOIDC = true
			}
			if seedFile != "" {
				cfg.Dev.SeedFile = seedFile
			}
			if autoMigrate {
				cfg.Database.Migrate = "auto"
				cfg.Database.Bootstrap = "auto"
			}

			// Initialize logging — streams, sinks, redaction.
			// DB is nil here; analytics drainer activates after DB open.
			logging.Init(logging.Config{
				Level:     cfg.Observability.LogLevel,
				Format:    cfg.Observability.LogFormat,
				CachePath: cfg.Observability.CachePath,
				CacheMax:  cfg.Observability.CacheMax,
				Streams: logging.StreamRouting{
					Runtime:     logging.StreamConfig{Sinks: cfg.Observability.Streams.Runtime.Sinks, Mode: cfg.Observability.Streams.Runtime.Mode, SampleRate: cfg.Observability.Streams.Runtime.SampleRate},
					Request:     logging.StreamConfig{Sinks: cfg.Observability.Streams.Request.Sinks, Mode: cfg.Observability.Streams.Request.Mode, SampleRate: cfg.Observability.Streams.Request.SampleRate},
					Jobs:        logging.StreamConfig{Sinks: cfg.Observability.Streams.Jobs.Sinks, Mode: cfg.Observability.Streams.Jobs.Mode, SampleRate: cfg.Observability.Streams.Jobs.SampleRate},
					EventPusher: logging.StreamConfig{Sinks: cfg.Observability.Streams.EventPusher.Sinks, Mode: cfg.Observability.Streams.EventPusher.Mode, SampleRate: cfg.Observability.Streams.EventPusher.SampleRate},
				},
				Sinks: logging.SinksConfig{
					OTEL: logging.OTELSinkConfig{
						Endpoint: cfg.Observability.Sinks.OTEL.Endpoint,
						Protocol: cfg.Observability.Sinks.OTEL.Protocol,
					},
					Analytics: logging.AnalyticsSinkConfig{
						Enabled:       cfg.Observability.Sinks.Analytics.Enabled,
						DrainInterval: cfg.Observability.Sinks.Analytics.DrainInterval,
						DrainBatch:    cfg.Observability.Sinks.Analytics.DrainBatch,
					},
				},
				Redaction: logging.RedactionConfig{
					Keys:   cfg.Observability.Redaction.Keys,
					Mask:   cfg.Observability.Redaction.Mask,
					IPMode: cfg.Observability.Redaction.IPMode,
				},
			})

			// Open database with pool config.
			poolLifetime, _ := time.ParseDuration(cfg.Database.ConnMaxLifetime)
			if poolLifetime == 0 {
				poolLifetime = time.Hour
			}
			db, err := database.OpenWithConfig(cfg.Database.URL, database.PoolConfig{
				MaxOpenConns:    cfg.Database.MaxOpenConns,
				MaxIdleConns:    cfg.Database.MaxIdleConns,
				ConnMaxLifetime: poolLifetime,
			})
			if err != nil {
				return fmt.Errorf("open database: %w", err)
			}
			defer db.Close()

			// Activate analytics drainer now that DB is available.
			// logging.Init was called without DB; this starts the cache→events pipeline.
			logging.ActivateDrainer(db.SQL())

			// Schema migration — behavior depends on config.
			migrateMode := cfg.Database.ResolveMigrateMode()
			switch migrateMode {
			case "auto":
				if err := database.Migrate(db); err != nil {
					return fmt.Errorf("run migrations: %w", err)
				}
			case "check":
				if err := database.CheckVersion(db); err != nil {
					return fmt.Errorf("schema version check: %w", err)
				}
			case "skip":
				logging.Printf("schema migration skipped (migrate=skip)")
			}

			// Bootstrap — behavior depends on config.
			bootstrapMode := cfg.Database.ResolveBootstrapMode()
			if bootstrapMode == "auto" {
				if err := bootstrap.EnsureAdmin(context.Background(), db, cfg.Dev.SeedFile); err != nil {
					return fmt.Errorf("bootstrap: %w", err)
				}
			} else {
				logging.Printf("bootstrap skipped (bootstrap=skip)")
			}

			bus := eventbus.New()

			// Start mock OIDC identity provider when enabled via flag, env, or config.
			if cfg.Dev.MockOIDC {
				mockCfg := mockoidc.DefaultConfig()
				if cfg.Dev.MockOIDCPort > 0 {
					mockCfg.Port = cfg.Dev.MockOIDCPort
				}
				mock := mockoidc.New(mockCfg)
				mock.Start()

				// Auto-provision mock provider if not exists.
				var count int
				db.SQL().QueryRow(`SELECT COUNT(*) FROM users WHERE id = 'prov_mock_oidc'`).Scan(&count)
				if count == 0 {
					mockConfig := fmt.Sprintf(`{"issuer":"%s","client_id":"%s","client_secret":"%s","scopes":"openid email profile"}`,
						mock.Issuer(), mock.ClientID(), mock.ClientSecret())
					data := fmt.Sprintf(`{"protocol":"oidc","template":"custom","config":%s,"claim_overrides":{},"auto_register":true,"enabled":true,"display_order":99}`,
						mockConfig)
					db.SQL().Exec(
						`INSERT INTO users (id, org_id, schema_id, identifier, display_name, state, data, created_at, updated_at)
						 VALUES ('prov_mock_oidc', '1', 'provider_v1', 'Mock OIDC (dev)', 'Mock OIDC (dev)', 'active', ?, datetime('now'), datetime('now'))`,
						data,
					)
					logging.Printf("[mock-oidc] provider auto-provisioned (id: prov_mock_oidc)")
				}
			}

			// Apply seed file if configured.
			if cfg.Dev.SeedFile != "" {
				if err := seed.LoadAndApply(context.Background(), db.SQL(), cfg.Dev.SeedFile); err != nil {
					return fmt.Errorf("apply seed: %w", err)
				}
			}

			cli.PrintLogo()
			logging.Printf("Zitadel %s starting on :%d", version, cfg.Server.Port)
			logging.Printf("Database: %s", cfg.Database.URL)
			logging.Printf("Migration mode: %s | Bootstrap mode: %s", migrateMode, bootstrapMode)

			srv := server.New(cfg, db, bus)
			return srv.ListenAndServe()
		},
	}

	cmd.Flags().StringVarP(&configPath, "config", "c", "", "path to zitadel.toml config file")
	cmd.Flags().BoolVar(&enableMockOIDC, "mock-oidc", false, "start an embedded mock OIDC identity provider for testing")
	cmd.Flags().StringVar(&seedFile, "seed", "", "path to YAML seed file to load on startup")
	cmd.Flags().BoolVar(&autoMigrate, "auto-migrate", false, "force auto-migrate and auto-bootstrap regardless of config")

	return cmd
}

// migrateCmd runs schema migrations and exits.
// Use this as a K8s init container or CI/CD job to prepare the database
// before starting the server with 'zitadel start'.
func migrateCmd() *cobra.Command {
	var configPath string
	var doBootstrap bool
	var seedFile string

	cmd := &cobra.Command{
		Use:   "migrate",
		Short: "Run database schema migrations",
		Long: `Run all pending schema migrations for the configured database dialect.
Use this as a pre-deployment step (K8s init container, CI/CD job) to
prepare the database before starting the server with 'zitadel start'.

For Postgres, an advisory lock ensures safe concurrent execution.`,
		RunE: func(cmd *cobra.Command, args []string) error {
			cfg, err := config.Load(configPath)
			if err != nil {
				return fmt.Errorf("load config: %w", err)
			}

			// Minimal logging for migration command.
			logging.Init(logging.Config{
				Level:  cfg.Observability.LogLevel,
				Format: cfg.Observability.LogFormat,
			})

			db, err := database.Open(cfg.Database.URL)
			if err != nil {
				return fmt.Errorf("open database: %w", err)
			}
			defer db.Close()

			logging.Printf("running migrations (dialect=%s)...", db.Dialect())

			if err := database.Migrate(db); err != nil {
				return fmt.Errorf("run migrations: %w", err)
			}

			// Optional: bootstrap admin after migration.
			if doBootstrap {
				sf := seedFile
				if sf == "" {
					sf = cfg.Dev.SeedFile
				}
				if err := bootstrap.EnsureAdmin(context.Background(), db, sf); err != nil {
					return fmt.Errorf("bootstrap: %w", err)
				}
			}

			logging.Printf("migrations complete")
			return nil
		},
	}

	cmd.Flags().StringVarP(&configPath, "config", "c", "", "path to zitadel.toml config file")
	cmd.Flags().BoolVar(&doBootstrap, "bootstrap", false, "also run admin bootstrap after migrations")
	cmd.Flags().StringVar(&seedFile, "seed", "", "path to YAML seed file (implies --bootstrap)")

	// Add status subcommand.
	cmd.AddCommand(migrateStatusCmd())

	return cmd
}

// migrateStatusCmd prints the current schema version info.
func migrateStatusCmd() *cobra.Command {
	var configPath string

	cmd := &cobra.Command{
		Use:   "status",
		Short: "Print schema migration status",
		RunE: func(cmd *cobra.Command, args []string) error {
			cfg, err := config.Load(configPath)
			if err != nil {
				return fmt.Errorf("load config: %w", err)
			}

			db, err := database.Open(cfg.Database.URL)
			if err != nil {
				return fmt.Errorf("open database: %w", err)
			}
			defer db.Close()

			current, target, err := database.VersionInfo(db)
			if err != nil {
				return fmt.Errorf("get version info: %w", err)
			}

			status := "✓ up to date"
			if current < target {
				status = fmt.Sprintf("⚠ behind (run 'zitadel migrate' to apply %d pending)", target-current)
			} else if current > target {
				status = "⚠ ahead of binary (binary may be outdated)"
			}

			fmt.Printf("Dialect:  %s\n", db.Dialect())
			fmt.Printf("Current:  %d\n", current)
			fmt.Printf("Target:   %d\n", target)
			fmt.Printf("Status:   %s\n", status)
			fmt.Println()

			// Also print per-migration status.
			return database.MigrateStatus(db)
		},
	}

	cmd.Flags().StringVarP(&configPath, "config", "c", "", "path to zitadel.toml config file")

	return cmd
}

func versionCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "version",
		Short: "Print the Zitadel version",
		Run: func(cmd *cobra.Command, args []string) {
			fmt.Printf("zitadel %s\n", version)
		},
	}
}

func openapiExportCmd() *cobra.Command {
	var pretty bool
	cmd := &cobra.Command{
		Use:   "openapi-export",
		Short: "Export the OpenAPI 3.1 spec to stdout",
		Long:  "Generate the complete OpenAPI 3.1 specification from registered API operations. No server or database required.",
		RunE: func(cmd *cobra.Command, args []string) error {
			// Create a minimal API instance — just need the registry.
			a := api.New(nil, nil, nil)
			// Populate only the OpenAPI operations (no HTTP handlers, no DB needed).
			a.RegisterOpenAPIOnly()

			if pretty {
				data, err := a.Spec().SpecJSON()
				if err != nil {
					return fmt.Errorf("marshal spec: %w", err)
				}
				os.Stdout.Write(data)
				fmt.Println()
			} else {
				data, err := json.Marshal(a.Spec().Spec())
				if err != nil {
					return fmt.Errorf("marshal spec: %w", err)
				}
				os.Stdout.Write(data)
				fmt.Println()
			}
			return nil
		},
	}
	cmd.Flags().BoolVar(&pretty, "pretty", true, "pretty-print the JSON output")
	return cmd
}
