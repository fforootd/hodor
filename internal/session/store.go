package session

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"time"

	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/events"
)

type Store struct {
	db *database.DB
}

func NewStore(db *database.DB) *Store {
	return &Store{db: db}
}

type Record struct {
	ID           string
	UserID       string
	OrgID        string
	AuthMethod   string
	ProviderID   string
	ProviderKind string
	LoginFlowID  string
	Metadata     map[string]any
	UserAgent    string
	IPAddress    string
	CreatedAt    string
	ExpiresAt    string
	RevokedAt    *string
}

type CreateParams struct {
	SessionID             string
	TokenID               string
	UserID                string
	OrgID                 string
	TokenHash             string
	UserAgent             string
	IPAddress             string
	Fingerprint           string
	Metadata              map[string]any
	CreatedAt             time.Time
	ExpiresAt             time.Time
	SessionCreatedPayload map[string]any
	RiskEvaluatedPayload  map[string]any
}

func (s *Store) Create(ctx context.Context, params CreateParams) (Record, error) {
	scoped := s.db.Scoped(ctx)
	tx, err := scoped.BeginTx(ctx, nil)
	if err != nil {
		return Record{}, fmt.Errorf("begin tx: %w", err)
	}
	defer tx.Rollback()

	var exists int
	err = tx.QueryRowContext(ctx, tx.Rebind(`SELECT 1 FROM users WHERE id = ? AND instance_id = ?`), params.UserID, tx.InstanceID()).Scan(&exists)
	if err == sql.ErrNoRows {
		return Record{}, fmt.Errorf("identity %s not found", params.UserID)
	}
	if err != nil {
		return Record{}, fmt.Errorf("check identity: %w", err)
	}

	if params.OrgID == "" {
		params.OrgID = "_global"
	}

	metadataJSON, err := marshalMetadata(params.Metadata)
	if err != nil {
		return Record{}, fmt.Errorf("marshal session metadata: %w", err)
	}

	createdAt := params.CreatedAt.UTC().Format(time.RFC3339)
	expiresAt := params.ExpiresAt.UTC().Format(time.RFC3339)

	if _, err := tx.ExecContext(ctx,
		tx.Rebind(`INSERT INTO sessions (id, instance_id, user_id, org_id, token_hash, user_agent, ip_address, metadata, created_at, expires_at, fingerprint)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`),
		params.SessionID,
		tx.InstanceID(),
		params.UserID,
		params.OrgID,
		params.TokenHash,
		params.UserAgent,
		params.IPAddress,
		metadataJSON,
		createdAt,
		expiresAt,
		params.Fingerprint,
	); err != nil {
		return Record{}, fmt.Errorf("insert session: %w", err)
	}

	if _, err := tx.ExecContext(ctx,
		tx.Rebind(`INSERT INTO tokens (id, instance_id, type, token_hash, user_id, session_id, scopes, expires_at, created_at)
		 VALUES (?, ?, 'session', ?, ?, ?, '[]', ?, ?)`),
		params.TokenID,
		tx.InstanceID(),
		params.TokenHash,
		params.UserID,
		params.SessionID,
		expiresAt,
		createdAt,
	); err != nil {
		return Record{}, fmt.Errorf("insert token: %w", err)
	}

	if err := events.Append(ctx, tx, "session.created", params.UserID, params.SessionID, "session", params.SessionCreatedPayload); err != nil {
		return Record{}, err
	}
	if len(params.RiskEvaluatedPayload) > 0 {
		if err := events.Append(ctx, tx, "signal.risk_evaluated", params.UserID, params.SessionID, "session", params.RiskEvaluatedPayload); err != nil {
			return Record{}, err
		}
	}

	if err := tx.Commit(); err != nil {
		return Record{}, fmt.Errorf("commit: %w", err)
	}

	return recordFromParts(params.SessionID, params.UserID, params.OrgID, params.UserAgent, params.IPAddress, createdAt, expiresAt, nil, metadataJSON), nil
}

func (s *Store) Get(ctx context.Context, sessionID string) (Record, error) {
	scoped := s.db.Scoped(ctx)
	return scanRecord(scoped.QueryRowContext(ctx,
		scoped.Rebind(`SELECT id, user_id, org_id, user_agent, ip_address, COALESCE(metadata,'{}'), created_at, expires_at, revoked_at
		 FROM sessions WHERE id = ? AND instance_id = ?`),
		sessionID,
		scoped.InstanceID(),
	))
}

func (s *Store) List(ctx context.Context, userID string, limit int) ([]Record, error) {
	scoped := s.db.Scoped(ctx)
	if limit <= 0 {
		limit = 50
	}

	query := `SELECT id, user_id, org_id, user_agent, ip_address, COALESCE(metadata,'{}'), created_at, expires_at, revoked_at
	          FROM sessions WHERE instance_id = ? ORDER BY created_at DESC LIMIT ?`
	args := []any{scoped.InstanceID(), limit}
	if userID != "" {
		query = `SELECT id, user_id, org_id, user_agent, ip_address, COALESCE(metadata,'{}'), created_at, expires_at, revoked_at
		         FROM sessions WHERE instance_id = ? AND user_id = ? ORDER BY created_at DESC LIMIT ?`
		args = []any{scoped.InstanceID(), userID, limit}
	}

	rows, err := scoped.QueryContext(ctx, scoped.Rebind(query), args...)
	if err != nil {
		return nil, fmt.Errorf("list sessions: %w", err)
	}
	defer rows.Close()

	var items []Record
	for rows.Next() {
		record, err := scanRecord(rows)
		if err != nil {
			return nil, err
		}
		items = append(items, record)
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("iterate sessions: %w", err)
	}
	if items == nil {
		items = []Record{}
	}
	return items, nil
}

func (s *Store) Revoke(ctx context.Context, sessionID, reason string) error {
	scoped := s.db.Scoped(ctx)
	tx, err := scoped.BeginTx(ctx, nil)
	if err != nil {
		return fmt.Errorf("begin tx: %w", err)
	}
	defer tx.Rollback()

	if reason == "" {
		reason = "api_revoke"
	}
	now := time.Now().UTC().Format(time.RFC3339)
	result, err := tx.ExecContext(ctx,
		tx.Rebind(`UPDATE sessions SET revoked_at = ? WHERE id = ? AND instance_id = ? AND revoked_at IS NULL`),
		now,
		sessionID,
		tx.InstanceID(),
	)
	if err != nil {
		return fmt.Errorf("revoke session: %w", err)
	}
	rows, _ := result.RowsAffected()
	if rows == 0 {
		return fmt.Errorf("session %s not found or already revoked", sessionID)
	}

	if _, err := tx.ExecContext(ctx,
		tx.Rebind(`UPDATE tokens SET revoked_at = ? WHERE session_id = ? AND instance_id = ? AND revoked_at IS NULL`),
		now,
		sessionID,
		tx.InstanceID(),
	); err != nil {
		return fmt.Errorf("revoke session tokens: %w", err)
	}

	var userID string
	if err := tx.QueryRowContext(ctx, tx.Rebind(`SELECT user_id FROM sessions WHERE id = ? AND instance_id = ?`), sessionID, tx.InstanceID()).Scan(&userID); err != nil {
		return fmt.Errorf("load revoked session user: %w", err)
	}

	if err := events.Append(ctx, tx, "session.revoked", userID, sessionID, "session", map[string]any{
		"user_id": userID,
		"reason":  reason,
	}); err != nil {
		return err
	}

	if err := tx.Commit(); err != nil {
		return fmt.Errorf("commit: %w", err)
	}
	return nil
}

type scanner interface {
	Scan(dest ...any) error
}

func scanRecord(s scanner) (Record, error) {
	var record Record
	var metadataJSON string
	if err := s.Scan(&record.ID, &record.UserID, &record.OrgID, &record.UserAgent, &record.IPAddress, &metadataJSON, &record.CreatedAt, &record.ExpiresAt, &record.RevokedAt); err != nil {
		return Record{}, err
	}
	return recordFromParts(record.ID, record.UserID, record.OrgID, record.UserAgent, record.IPAddress, record.CreatedAt, record.ExpiresAt, record.RevokedAt, metadataJSON), nil
}

func recordFromParts(id, userID, orgID, userAgent, ipAddress, createdAt, expiresAt string, revokedAt *string, metadataJSON string) Record {
	record := Record{
		ID:        id,
		UserID:    userID,
		OrgID:     orgID,
		UserAgent: userAgent,
		IPAddress: ipAddress,
		CreatedAt: createdAt,
		ExpiresAt: expiresAt,
		RevokedAt: revokedAt,
	}
	applyMetadata(&record, metadataJSON)
	return record
}

func applyMetadata(record *Record, metadataJSON string) {
	if record == nil {
		return
	}
	var metadata map[string]any
	if err := json.Unmarshal([]byte(metadataJSON), &metadata); err != nil {
		return
	}
	record.Metadata = metadata
	record.AuthMethod = stringOr(metadata["auth_method"])
	record.ProviderID = stringOr(metadata["provider_id"])
	record.ProviderKind = stringOr(metadata["provider_kind"])
	record.LoginFlowID = stringOr(metadata["login_flow_id"])
}

func marshalMetadata(metadata map[string]any) (string, error) {
	if len(metadata) == 0 {
		return "{}", nil
	}
	b, err := json.Marshal(metadata)
	if err != nil {
		return "", err
	}
	return string(b), nil
}

func stringOr(value any) string {
	if str, ok := value.(string); ok {
		return str
	}
	return ""
}
