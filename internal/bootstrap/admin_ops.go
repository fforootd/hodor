package bootstrap

import (
	"bufio"
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"strings"

	"golang.org/x/term"

	"github.com/zitadel/zitadel/internal/auth"
	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/id"
	"github.com/zitadel/zitadel/internal/uniqueness"
)

var (
	ErrUsersAlreadyExist       = errors.New("users already exist")
	ErrRecoveryTargetNotFound  = errors.New("recovery target not found")
	ErrInteractivePasswordOnly = errors.New("password required: use --password, --password-stdin, or run in an interactive terminal")
)

type OwnerProvisioner interface {
	EnsureInstanceOwner(ctx context.Context, userID string) error
}

type CreateAdminOptions struct {
	Username    string
	Email       string
	Password    string
	DisplayName string
	Passwords   *auth.Passwords
	Owners      OwnerProvisioner
}

type RecoverAdminOptions struct {
	UserID          string
	Identifier      string
	Email           string
	Password        string
	CreateIfMissing bool
	Passwords       *auth.Passwords
	Owners          OwnerProvisioner
}

type AdminRecord struct {
	UserID     string
	Identifier string
	Email      string
	Created    bool
}

func SeedSystem(ctx context.Context, db *database.DB) error {
	if err := seedSchemas(ctx, db); err != nil {
		return fmt.Errorf("seed schemas: %w", err)
	}
	if err := seedDefaultLoginFlow(ctx, db); err != nil {
		return fmt.Errorf("seed default login flow: %w", err)
	}
	if err := seedConsoleClient(ctx, db); err != nil {
		return fmt.Errorf("seed console client: %w", err)
	}
	return nil
}

func HasAnyUsers(ctx context.Context, db *database.DB) (bool, error) {
	var count int
	if err := db.SQL().QueryRowContext(ctx, `SELECT COUNT(*) FROM users`).Scan(&count); err != nil {
		return false, fmt.Errorf("count users: %w", err)
	}
	return count > 0, nil
}

func CreateAdmin(ctx context.Context, db *database.DB, opts CreateAdminOptions) (*AdminRecord, error) {
	username := strings.TrimSpace(opts.Username)
	if username == "" {
		return nil, fmt.Errorf("username is required")
	}
	email := strings.TrimSpace(opts.Email)
	if email == "" {
		return nil, fmt.Errorf("email is required")
	}
	if err := validatePassword(opts.Password); err != nil {
		return nil, err
	}

	passwords := opts.Passwords
	if passwords == nil {
		passwords = auth.NewPasswords(db)
	}

	displayName := strings.TrimSpace(opts.DisplayName)
	if displayName == "" {
		displayName = "Admin"
	}

	userID, err := insertAdminUser(ctx, db, username, email, displayName)
	if err != nil {
		return nil, err
	}
	if err := passwords.SetPassword(ctx, userID, opts.Password); err != nil {
		return nil, fmt.Errorf("set admin password: %w", err)
	}
	if opts.Owners != nil {
		if err := opts.Owners.EnsureInstanceOwner(ctx, userID); err != nil {
			return nil, fmt.Errorf("ensure instance owner: %w", err)
		}
	}

	return &AdminRecord{
		UserID:     userID,
		Identifier: username,
		Email:      email,
		Created:    true,
	}, nil
}

func RecoverAdmin(ctx context.Context, db *database.DB, opts RecoverAdminOptions) (*AdminRecord, error) {
	if err := validatePassword(opts.Password); err != nil {
		return nil, err
	}

	passwords := opts.Passwords
	if passwords == nil {
		passwords = auth.NewPasswords(db)
	}

	record, err := lookupAdminRecord(ctx, db, opts.UserID, opts.Identifier)
	if err != nil {
		return nil, err
	}
	if record == nil {
		if !opts.CreateIfMissing {
			return nil, ErrRecoveryTargetNotFound
		}

		identifier := strings.TrimSpace(opts.Identifier)
		if identifier == "" {
			return nil, fmt.Errorf("identifier is required when creating a missing recovery admin")
		}
		email := strings.TrimSpace(opts.Email)
		if email == "" {
			email = fmt.Sprintf("%s@zitadel.local", identifier)
		}
		created, err := CreateAdmin(ctx, db, CreateAdminOptions{
			Username:    identifier,
			Email:       email,
			Password:    opts.Password,
			DisplayName: "Recovered Admin",
			Passwords:   passwords,
			Owners:      opts.Owners,
		})
		if err != nil {
			return nil, err
		}
		return created, nil
	}

	if err := passwords.SetPassword(ctx, record.UserID, opts.Password); err != nil {
		return nil, fmt.Errorf("set admin password: %w", err)
	}
	if err := activateUser(ctx, db, record.UserID); err != nil {
		return nil, err
	}
	if opts.Owners != nil {
		if err := opts.Owners.EnsureInstanceOwner(ctx, record.UserID); err != nil {
			return nil, fmt.Errorf("ensure instance owner: %w", err)
		}
	}
	return record, nil
}

func IsInteractive(file *os.File) bool {
	if file == nil {
		return false
	}
	return term.IsTerminal(int(file.Fd()))
}

func ReadPasswordFromStdin(r io.Reader) (string, error) {
	if r == nil {
		return "", fmt.Errorf("password stdin is not available")
	}
	reader := bufio.NewReader(r)
	line, err := reader.ReadString('\n')
	if err != nil && !errors.Is(err, io.EOF) {
		return "", fmt.Errorf("read password stdin: %w", err)
	}
	password := strings.TrimSpace(line)
	if password == "" {
		return "", fmt.Errorf("password stdin is empty")
	}
	if err := validatePassword(password); err != nil {
		return "", err
	}
	return password, nil
}

func PromptPassword(file *os.File, out io.Writer, prompt, confirmPrompt string) (string, error) {
	password, err := readPromptedSecret(file, out, prompt)
	if err != nil {
		return "", err
	}
	if confirmPrompt != "" {
		confirm, err := readPromptedSecret(file, out, confirmPrompt)
		if err != nil {
			return "", err
		}
		if confirm != password {
			return "", fmt.Errorf("passwords do not match")
		}
	}
	if err := validatePassword(password); err != nil {
		return "", err
	}
	return password, nil
}

func insertAdminUser(ctx context.Context, db *database.DB, username, email, displayName string) (string, error) {
	tx, err := db.SQL().BeginTx(ctx, nil)
	if err != nil {
		return "", fmt.Errorf("begin tx: %w", err)
	}
	defer tx.Rollback()

	if err := checkIdentifierConflict(ctx, db, tx, username); err != nil {
		return "", err
	}
	if err := checkEmailConflict(ctx, db, tx, email); err != nil {
		return "", err
	}

	metadataJSON, err := adminMetadataJSON(email)
	if err != nil {
		return "", err
	}

	userID := id.New()
	query := fmt.Sprintf(
		`INSERT INTO users (id, org_id, identifier, display_name, user_type, state, schema_id, metadata, created_at, updated_at)
		 VALUES (%s, %s, %s, %s, 'human', 'active', 'human_user_v1', %s, %s, %s)`,
		db.Placeholder(1), db.Placeholder(2), db.Placeholder(3), db.Placeholder(4), db.Placeholder(5), db.TimestampNow(), db.TimestampNow(),
	)
	if _, err := tx.ExecContext(ctx, query, userID, "", username, displayName, metadataJSON); err != nil {
		return "", fmt.Errorf("insert admin user: %w", err)
	}
	if err := uniqueness.EnforceFromIdentifier(ctx, tx, userID, "", username); err != nil {
		return "", err
	}
	if err := uniqueness.Enforce(ctx, tx, userID, "",
		[]uniqueness.FieldConstraint{{FieldName: "email", Scope: uniqueness.ScopeInstance}},
		map[string]any{"email": email},
	); err != nil {
		return "", err
	}
	if err := tx.Commit(); err != nil {
		return "", fmt.Errorf("commit admin user: %w", err)
	}
	return userID, nil
}

func lookupAdminRecord(ctx context.Context, db *database.DB, userID, identifier string) (*AdminRecord, error) {
	var (
		query string
		arg   string
	)
	switch {
	case strings.TrimSpace(userID) != "":
		query = fmt.Sprintf(`SELECT id, identifier, metadata FROM users WHERE id = %s LIMIT 1`, db.Placeholder(1))
		arg = strings.TrimSpace(userID)
	case strings.TrimSpace(identifier) != "":
		query = fmt.Sprintf(`SELECT id, identifier, metadata FROM users WHERE identifier = %s LIMIT 1`, db.Placeholder(1))
		arg = strings.TrimSpace(identifier)
	default:
		return nil, fmt.Errorf("either user ID or identifier is required")
	}

	var (
		idValue         string
		identifierValue string
		metadataRaw     []byte
	)
	if err := db.SQL().QueryRowContext(ctx, query, arg).Scan(&idValue, &identifierValue, &metadataRaw); err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return nil, nil
		}
		return nil, fmt.Errorf("lookup admin: %w", err)
	}

	return &AdminRecord{
		UserID:     idValue,
		Identifier: identifierValue,
		Email:      parseEmailMetadata(metadataRaw),
	}, nil
}

func activateUser(ctx context.Context, db *database.DB, userID string) error {
	query := fmt.Sprintf(`UPDATE users SET state = 'active', updated_at = %s WHERE id = %s`, db.TimestampNow(), db.Placeholder(1))
	if _, err := db.SQL().ExecContext(ctx, query, userID); err != nil {
		return fmt.Errorf("reactivate user: %w", err)
	}
	return nil
}

func checkIdentifierConflict(ctx context.Context, db *database.DB, tx *sql.Tx, username string) error {
	var count int
	query := fmt.Sprintf(`SELECT COUNT(*) FROM users WHERE org_id = '' AND identifier = %s`, db.Placeholder(1))
	if err := tx.QueryRowContext(ctx, query,
		username,
	).Scan(&count); err != nil {
		return fmt.Errorf("check identifier conflict: %w", err)
	}
	if count > 0 {
		return &uniqueness.ViolationError{
			Field: "identifier",
			Value: username,
			Scope: string(uniqueness.ScopeInstance),
		}
	}
	return nil
}

func checkEmailConflict(ctx context.Context, db *database.DB, tx *sql.Tx, email string) error {
	var count int
	query := fmt.Sprintf(`SELECT COUNT(*) FROM unique_fields WHERE scope_id = '' AND field_name = 'email' AND normalized_value = %s`, db.Placeholder(1))
	if err := tx.QueryRowContext(ctx, query,
		uniqueness.Normalize(email),
	).Scan(&count); err != nil {
		return fmt.Errorf("check email conflict: %w", err)
	}
	if count > 0 {
		return &uniqueness.ViolationError{
			Field: "email",
			Value: email,
			Scope: string(uniqueness.ScopeInstance),
		}
	}
	return nil
}

func adminMetadataJSON(email string) (string, error) {
	payload, err := json.Marshal(map[string]any{"email": email})
	if err != nil {
		return "", fmt.Errorf("marshal admin metadata: %w", err)
	}
	return string(payload), nil
}

func parseEmailMetadata(raw []byte) string {
	if len(raw) == 0 {
		return ""
	}
	var parsed map[string]any
	if err := json.Unmarshal(raw, &parsed); err != nil {
		return ""
	}
	if email, ok := parsed["email"].(string); ok {
		return email
	}
	return ""
}

func readPromptedSecret(file *os.File, out io.Writer, prompt string) (string, error) {
	if !IsInteractive(file) {
		return "", ErrInteractivePasswordOnly
	}
	if out == nil {
		out = io.Discard
	}
	fmt.Fprint(out, prompt)
	secret, err := term.ReadPassword(int(file.Fd()))
	fmt.Fprintln(out)
	if err == nil {
		return strings.TrimSpace(string(secret)), nil
	}

	reader := bufio.NewReader(file)
	visible, readErr := reader.ReadString('\n')
	if readErr != nil && !errors.Is(readErr, io.EOF) {
		return "", fmt.Errorf("read password: %w", readErr)
	}
	return strings.TrimSpace(visible), nil
}

func validatePassword(password string) error {
	if len(strings.TrimSpace(password)) < 6 {
		return fmt.Errorf("password must be at least 6 characters")
	}
	return nil
}
