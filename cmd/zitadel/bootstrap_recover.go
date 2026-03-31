package main

import (
	"context"
	"errors"
	"fmt"
	"io"
	"os"

	"github.com/spf13/cobra"

	"github.com/zitadel/zitadel/internal/auth"
	"github.com/zitadel/zitadel/internal/bootstrap"
	"github.com/zitadel/zitadel/internal/config"
	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/fga"
	"github.com/zitadel/zitadel/internal/logging"
)

type cliIO struct {
	in        io.Reader
	out       io.Writer
	errOut    io.Writer
	stdinFile *os.File
}

func defaultCLIIO() cliIO {
	return cliIO{
		in:        os.Stdin,
		out:       os.Stdout,
		errOut:    os.Stderr,
		stdinFile: os.Stdin,
	}
}

func newRootCmd() *cobra.Command {
	return newRootCmdWithIO(defaultCLIIO())
}

func newRootCmdWithIO(stdio cliIO) *cobra.Command {
	root := &cobra.Command{
		Use:   "zitadel",
		Short: "Identity infrastructure for humans and AI",
		Long:  "Zitadel — open-source identity platform with unified auth, fine-grained authorization, and AI agent governance.",
	}
	if stdio.out != nil {
		root.SetOut(stdio.out)
	}
	if stdio.errOut != nil {
		root.SetErr(stdio.errOut)
	}

	root.AddCommand(startCmd())
	root.AddCommand(migrateCmd())
	root.AddCommand(seedCmd())
	root.AddCommand(bootstrapCmd(stdio))
	root.AddCommand(recoverCmd(stdio))
	root.AddCommand(versionCmd())
	root.AddCommand(openapiExportCmd())

	return root
}

func bootstrapCmd(stdio cliIO) *cobra.Command {
	cmd := &cobra.Command{
		Use:   "bootstrap",
		Short: "Bootstrap a self-hosted Zitadel instance",
	}
	cmd.AddCommand(bootstrapAdminCmd(stdio))
	return cmd
}

func bootstrapAdminCmd(stdio cliIO) *cobra.Command {
	var configPath string
	var username string
	var email string
	var password string
	var passwordStdin bool

	cmd := &cobra.Command{
		Use:   "admin",
		Short: "Create the first local admin user",
		RunE: func(cmd *cobra.Command, args []string) error {
			resolvedPassword, err := resolveExplicitPassword(stdio, password, passwordStdin)
			if err != nil {
				return err
			}

			cfg, storageResolution, err := loadCommandConfig(configPath)
			if err != nil {
				return err
			}
			initCommandLogging(cfg)
			logLegacyStorageWarning(storageResolution)

			db, err := database.Open(cfg.Database.URL)
			if err != nil {
				return fmt.Errorf("open database: %w", err)
			}
			defer db.Close()

			if err := database.Migrate(db); err != nil {
				return fmt.Errorf("run migrations: %w", err)
			}

			ctx := context.Background()
			fgaSvc, closeFGA, err := initLocalFGAService(ctx, cfg)
			if err != nil {
				return err
			}
			defer closeFGA()

			if err := bootstrap.SeedSystem(ctx, db); err != nil {
				return fmt.Errorf("seed system: %w", err)
			}

			hasUsers, err := bootstrap.HasAnyUsers(ctx, db)
			if err != nil {
				return err
			}
			if hasUsers {
				return fmt.Errorf("%w: bootstrap admin only runs on an unclaimed instance", bootstrap.ErrUsersAlreadyExist)
			}

			record, err := bootstrap.CreateAdmin(ctx, db, bootstrap.CreateAdminOptions{
				Username:  username,
				Email:     email,
				Password:  resolvedPassword,
				Passwords: passwordsForConfig(db, cfg),
				Owners:    fgaSvc,
			})
			if err != nil {
				return fmt.Errorf("create admin: %w", err)
			}

			logging.Printf("[bootstrap] action=bootstrap_admin identifier=%s user_id=%s email=%s", record.Identifier, record.UserID, record.Email)
			fmt.Fprintf(stdio.out, "Bootstrap complete.\nAdmin identifier: %s\nAdmin email: %s\nOpen /console to sign in.\n", record.Identifier, record.Email)
			return nil
		},
	}

	cmd.Flags().StringVarP(&configPath, "config", "c", "", "path to zitadel.toml config file")
	cmd.Flags().StringVar(&username, "username", "admin", "identifier for the first admin")
	cmd.Flags().StringVar(&email, "email", "admin@zitadel.local", "email for the first admin")
	cmd.Flags().StringVar(&password, "password", "", "admin password")
	cmd.Flags().BoolVar(&passwordStdin, "password-stdin", false, "read the admin password from stdin")

	return cmd
}

func recoverCmd(stdio cliIO) *cobra.Command {
	cmd := &cobra.Command{
		Use:   "recover",
		Short: "Recover local break-glass access",
	}
	cmd.AddCommand(recoverAdminCmd(stdio))
	return cmd
}

func recoverAdminCmd(stdio cliIO) *cobra.Command {
	var configPath string
	var userID string
	var identifier string
	var email string
	var password string
	var passwordStdin bool
	var createIfMissing bool

	cmd := &cobra.Command{
		Use:   "admin",
		Short: "Reset an existing admin or create a new local break-glass admin",
		RunE: func(cmd *cobra.Command, args []string) error {
			resolvedPassword, err := resolveExplicitPassword(stdio, password, passwordStdin)
			if err != nil {
				return err
			}

			cfg, storageResolution, err := loadCommandConfig(configPath)
			if err != nil {
				return err
			}
			initCommandLogging(cfg)
			logLegacyStorageWarning(storageResolution)

			db, err := database.Open(cfg.Database.URL)
			if err != nil {
				return fmt.Errorf("open database: %w", err)
			}
			defer db.Close()

			if err := database.CheckVersion(db); err != nil {
				return fmt.Errorf("schema version check: %w", err)
			}

			ctx := context.Background()
			fgaSvc, closeFGA, err := initLocalFGAService(ctx, cfg)
			if err != nil {
				return err
			}
			defer closeFGA()

			record, err := bootstrap.RecoverAdmin(ctx, db, bootstrap.RecoverAdminOptions{
				UserID:          userID,
				Identifier:      identifier,
				Email:           email,
				Password:        resolvedPassword,
				CreateIfMissing: createIfMissing,
				Passwords:       passwordsForConfig(db, cfg),
				Owners:          fgaSvc,
			})
			if err != nil {
				if errors.Is(err, bootstrap.ErrRecoveryTargetNotFound) {
					return fmt.Errorf("%w: rerun with --create-if-missing to create a new break-glass admin", err)
				}
				return fmt.Errorf("recover admin: %w", err)
			}

			action := "recovered"
			if record.Created {
				action = "created"
			}
			logging.Printf("[bootstrap] action=recover_admin result=%s identifier=%s user_id=%s", action, record.Identifier, record.UserID)
			if record.Email != "" {
				fmt.Fprintf(stdio.out, "Recovery complete.\nAdmin identifier: %s\nAdmin email: %s\n", record.Identifier, record.Email)
			} else {
				fmt.Fprintf(stdio.out, "Recovery complete.\nAdmin identifier: %s\n", record.Identifier)
			}
			return nil
		},
	}

	cmd.Flags().StringVarP(&configPath, "config", "c", "", "path to zitadel.toml config file")
	cmd.Flags().StringVar(&userID, "user-id", "", "recover a specific user ID")
	cmd.Flags().StringVar(&identifier, "identifier", "admin", "recover by identifier when user ID is not provided")
	cmd.Flags().StringVar(&email, "email", "", "email for a newly created recovery admin")
	cmd.Flags().BoolVar(&createIfMissing, "create-if-missing", false, "create a new break-glass admin when no target user exists")
	cmd.Flags().StringVar(&password, "password", "", "new admin password")
	cmd.Flags().BoolVar(&passwordStdin, "password-stdin", false, "read the new admin password from stdin")

	return cmd
}

func resolveExplicitPassword(stdio cliIO, password string, passwordStdin bool) (string, error) {
	switch {
	case password != "" && passwordStdin:
		return "", fmt.Errorf("use exactly one of --password or --password-stdin")
	case password != "":
		return password, nil
	case passwordStdin:
		return bootstrap.ReadPasswordFromStdin(stdio.in)
	case bootstrap.IsInteractive(stdio.stdinFile):
		return bootstrap.PromptPassword(stdio.stdinFile, stdio.out, "Admin password: ", "Confirm password: ")
	default:
		return "", bootstrap.ErrInteractivePasswordOnly
	}
}

func loadCommandConfig(configPath string) (*config.Config, *config.LocalStorageResolution, error) {
	cfg, err := config.Load(configPath)
	if err != nil {
		return nil, nil, fmt.Errorf("load config: %w", err)
	}
	storageResolution, err := cfg.ResolveLocalStorage(configPath)
	if err != nil {
		return nil, nil, fmt.Errorf("resolve local storage: %w", err)
	}
	return cfg, storageResolution, nil
}

func initCommandLogging(cfg *config.Config) {
	logging.Init(logging.Config{
		Level:  cfg.Observability.LogLevel,
		Format: cfg.Observability.LogFormat,
	})
}

func passwordsForConfig(db *database.DB, cfg *config.Config) *auth.Passwords {
	if cfg.Dev.MockOIDC || cfg.Dev.SeedFile != "" {
		return auth.NewPasswordsDev(db)
	}
	return auth.NewPasswords(db)
}

func initLocalFGAService(ctx context.Context, cfg *config.Config) (*fga.Service, func(), error) {
	fgaDB, err := database.Open(cfg.Database.URL)
	if err != nil {
		return nil, nil, fmt.Errorf("open dedicated FGA database: %w", err)
	}

	fgaSvc, err := fga.New(ctx, fgaDB.SQL(), fgaDB.Dialect())
	if err != nil {
		fgaDB.Close()
		return nil, nil, fmt.Errorf("init local FGA: %w", err)
	}

	return fgaSvc, func() {
		_ = fgaDB.Close()
	}, nil
}
