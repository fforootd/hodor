// Package main is the entry point for the zitadel binary.
// A single binary serves as both the identity server and CLI client.
package main

import (
	"context"
	"fmt"
	"os"

	"github.com/spf13/cobra"

	"github.com/zitadel/zitadel/internal/bootstrap"
	"github.com/zitadel/zitadel/internal/config"
	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/eventbus"
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
		Long:  "ZITADEL — open-source identity platform with unified auth, fine-grained authorization, and AI agent governance.",
	}

	root.AddCommand(serveCmd())
	root.AddCommand(versionCmd())

	if err := root.Execute(); err != nil {
		os.Exit(1)
	}
}

func serveCmd() *cobra.Command {
	var configPath string
	var enableMockOIDC bool
	var seedFile string

	cmd := &cobra.Command{
		Use:   "serve",
		Short: "Start the ZITADEL identity server",
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

			db, err := database.Open(cfg.Database.URL)
			if err != nil {
				return fmt.Errorf("open database: %w", err)
			}
			defer db.Close()

			if err := database.EnsureSchema(db); err != nil {
				return fmt.Errorf("run migrations: %w", err)
			}

			// Bootstrap admin identity on first start.
			if err := bootstrap.EnsureAdmin(context.Background(), db); err != nil {
				return fmt.Errorf("bootstrap: %w", err)
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
				db.SQL().QueryRow(`SELECT COUNT(*) FROM providers WHERE id = 'prov_mock_oidc'`).Scan(&count)
				if count == 0 {
					db.SQL().Exec(
						`INSERT INTO providers (id, org_id, name, protocol, template, config, claim_overrides, auto_register, enabled, display_order, created_at, updated_at)
						 VALUES ('prov_mock_oidc', 1, 'Mock OIDC (dev)', 'oidc', 'custom', ?, '{}', 1, 1, 99, datetime('now'), datetime('now'))`,
						fmt.Sprintf(`{"issuer":"%s","client_id":"%s","client_secret":"%s","scopes":"openid email profile"}`,
							mock.Issuer(), mock.ClientID(), mock.ClientSecret()),
					)
					fmt.Printf("[mock-oidc] provider auto-provisioned (id: prov_mock_oidc)\n")
				}
			}

			// Apply seed file if configured.
			if cfg.Dev.SeedFile != "" {
				if err := seed.LoadAndApply(context.Background(), db.SQL(), cfg.Dev.SeedFile); err != nil {
					return fmt.Errorf("apply seed: %w", err)
				}
			}

			fmt.Printf("ZITADEL %s starting on :%d\n", version, cfg.Server.Port)
			fmt.Printf("Database: %s\n", cfg.Database.URL)

			srv := server.New(cfg, db, bus)
			return srv.ListenAndServe()
		},
	}

	cmd.Flags().StringVarP(&configPath, "config", "c", "", "path to zitadel.toml config file")
	cmd.Flags().BoolVar(&enableMockOIDC, "mock-oidc", false, "start an embedded mock OIDC identity provider for testing")
	cmd.Flags().StringVar(&seedFile, "seed", "", "path to YAML seed file to load on startup")

	return cmd
}

func versionCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "version",
		Short: "Print the ZITADEL version",
		Run: func(cmd *cobra.Command, args []string) {
			fmt.Printf("zitadel %s\n", version)
		},
	}
}
