package loginflow

import (
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"time"

	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/id"
)

type Store struct {
	db *database.DB
}

func NewStore(db *database.DB) *Store {
	return &Store{db: db}
}

type Record struct {
	ID          string
	OrgID       string
	SchemaID    string
	Name        string
	Strategy    string
	IsDefault   bool
	Enabled     bool
	State       string
	Priority    int
	Audience    any
	AuthMethods any
	Config      any
	Metadata    any
	CreatedAt   string
	UpdatedAt   string
}

type WriteParams struct {
	ID              string
	OrgID           string
	SchemaID        string
	Name            string
	Strategy        string
	IsDefault       bool
	Enabled         bool
	State           string
	Priority        int
	AudienceJSON    string
	AuthMethodsJSON string
	ConfigJSON      string
	MetadataJSON    string
	CreatedAt       string
	UpdatedAt       string
}

type Asset struct {
	ID          string
	LoginFlowID string
	Slot        string
	URL         string
	Filename    string
	ContentType string
	SizeBytes   int64
	ETag        string
}

type AssetData struct {
	ContentType string
	ETag        string
	Payload     []byte
}

func (s *Store) Create(ctx context.Context, params WriteParams) (Record, error) {
	scoped := s.db.Scoped(ctx)
	if params.MetadataJSON == "" {
		params.MetadataJSON = "{}"
	}
	if _, err := scoped.ExecContext(ctx, scoped.Rebind(
		`INSERT INTO login_flows (instance_id, id, org_id, name, strategy, config, is_default, enabled, state, priority, audience, auth_methods, schema_id, metadata, created_at, updated_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`),
		scoped.InstanceID(),
		params.ID,
		params.OrgID,
		params.Name,
		params.Strategy,
		params.ConfigJSON,
		params.IsDefault,
		params.Enabled,
		params.State,
		params.Priority,
		params.AudienceJSON,
		params.AuthMethodsJSON,
		params.SchemaID,
		params.MetadataJSON,
		params.CreatedAt,
		params.UpdatedAt,
	); err != nil {
		return Record{}, fmt.Errorf("create login flow: %w", err)
	}
	return s.Get(ctx, params.ID)
}

func (s *Store) Get(ctx context.Context, flowID string) (Record, error) {
	scoped := s.db.Scoped(ctx)
	return scanRecord(scoped.QueryRowContext(ctx,
		scoped.Rebind(`SELECT id, COALESCE(org_id,''), COALESCE(schema_id,''), name, strategy, config,
		        CASE WHEN COALESCE(is_default, false) THEN 1 ELSE 0 END,
		        CASE WHEN COALESCE(enabled, true) THEN 1 ELSE 0 END, state, priority,
		        COALESCE(audience,'{}'), COALESCE(auth_methods,'{}'),
		        COALESCE(metadata,'{}'), created_at, updated_at
		 FROM login_flows WHERE instance_id = ? AND id = ?`),
		scoped.InstanceID(),
		flowID,
	))
}

func (s *Store) List(ctx context.Context, stateFilter string) ([]Record, error) {
	scoped := s.db.Scoped(ctx)
	args := []any{scoped.InstanceID()}
	query := `SELECT id, COALESCE(org_id,''), COALESCE(schema_id,''), name, strategy, config,
	                  CASE WHEN COALESCE(is_default, false) THEN 1 ELSE 0 END,
	                  CASE WHEN COALESCE(enabled, true) THEN 1 ELSE 0 END, state, priority,
	                  COALESCE(audience,'{}'), COALESCE(auth_methods,'{}'),
	                  COALESCE(metadata,'{}'), created_at, updated_at
	           FROM login_flows WHERE instance_id = ?`
	if stateFilter != "" {
		query += ` AND state = ?`
		args = append(args, stateFilter)
	}
	query += ` ORDER BY CASE WHEN COALESCE(is_default, false) THEN 1 ELSE 0 END DESC, priority DESC, created_at DESC`

	rows, err := scoped.QueryContext(ctx, scoped.Rebind(query), args...)
	if err != nil {
		return nil, fmt.Errorf("list login flows: %w", err)
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
		return nil, fmt.Errorf("iterate login flows: %w", err)
	}
	if items == nil {
		items = []Record{}
	}
	return items, nil
}

func (s *Store) Update(ctx context.Context, params WriteParams) (Record, error) {
	scoped := s.db.Scoped(ctx)
	result, err := scoped.ExecContext(ctx, scoped.Rebind(
		`UPDATE login_flows
		 SET name = ?, strategy = ?, config = ?, is_default = ?, enabled = ?, state = ?, priority = ?, audience = ?, auth_methods = ?, schema_id = ?, metadata = ?, updated_at = ?
		 WHERE instance_id = ? AND id = ?`),
		params.Name,
		params.Strategy,
		params.ConfigJSON,
		params.IsDefault,
		params.Enabled,
		params.State,
		params.Priority,
		params.AudienceJSON,
		params.AuthMethodsJSON,
		params.SchemaID,
		params.MetadataJSON,
		params.UpdatedAt,
		scoped.InstanceID(),
		params.ID,
	)
	if err != nil {
		return Record{}, fmt.Errorf("update login flow: %w", err)
	}
	rows, _ := result.RowsAffected()
	if rows == 0 {
		return Record{}, sql.ErrNoRows
	}
	return s.Get(ctx, params.ID)
}

func (s *Store) Delete(ctx context.Context, flowID string) error {
	scoped := s.db.Scoped(ctx)
	var isDefault int
	if err := scoped.QueryRowContext(ctx,
		scoped.Rebind(`SELECT CASE WHEN COALESCE(is_default, false) THEN 1 ELSE 0 END FROM login_flows WHERE instance_id = ? AND id = ?`),
		scoped.InstanceID(),
		flowID,
	).Scan(&isDefault); err != nil {
		if err == sql.ErrNoRows {
			return err
		}
		return fmt.Errorf("load login flow delete state: %w", err)
	}
	if isDefault != 0 {
		return fmt.Errorf("cannot delete the default login flow")
	}
	result, err := scoped.ExecContext(ctx, scoped.Rebind(`DELETE FROM login_flows WHERE instance_id = ? AND id = ?`), scoped.InstanceID(), flowID)
	if err != nil {
		return fmt.Errorf("delete login flow: %w", err)
	}
	rows, _ := result.RowsAffected()
	if rows == 0 {
		return sql.ErrNoRows
	}
	return nil
}

func (s *Store) Promote(ctx context.Context, flowID string) (string, string, error) {
	scoped := s.db.Scoped(ctx)
	var currentState string
	if err := scoped.QueryRowContext(ctx,
		scoped.Rebind(`SELECT state FROM login_flows WHERE instance_id = ? AND id = ?`),
		scoped.InstanceID(),
		flowID,
	).Scan(&currentState); err != nil {
		return "", "", err
	}

	var nextState string
	switch currentState {
	case "draft":
		nextState = "testing"
	case "testing":
		nextState = "active"
	case "active":
		return currentState, "", fmt.Errorf("flow is already active")
	case "archived":
		return currentState, "", fmt.Errorf("cannot promote archived flow; create a new version")
	default:
		return currentState, "", fmt.Errorf("unknown state: %s", currentState)
	}

	if _, err := scoped.ExecContext(ctx,
		scoped.Rebind(`UPDATE login_flows SET state = ?, updated_at = ? WHERE instance_id = ? AND id = ?`),
		nextState,
		time.Now().UTC().Format(time.RFC3339),
		scoped.InstanceID(),
		flowID,
	); err != nil {
		return currentState, "", fmt.Errorf("promote login flow: %w", err)
	}
	return currentState, nextState, nil
}

func (s *Store) Archive(ctx context.Context, flowID string) error {
	scoped := s.db.Scoped(ctx)
	result, err := scoped.ExecContext(ctx,
		scoped.Rebind(`UPDATE login_flows SET state = 'archived', updated_at = ? WHERE instance_id = ? AND id = ?`),
		time.Now().UTC().Format(time.RFC3339),
		scoped.InstanceID(),
		flowID,
	)
	if err != nil {
		return fmt.Errorf("archive login flow: %w", err)
	}
	rows, _ := result.RowsAffected()
	if rows == 0 {
		return sql.ErrNoRows
	}
	return nil
}

func (s *Store) ReplaceAsset(ctx context.Context, flowID, slot, filename, contentType string, payload []byte) (Asset, error) {
	scoped := s.db.Scoped(ctx)
	tx, err := scoped.BeginTx(ctx, nil)
	if err != nil {
		return Asset{}, fmt.Errorf("database error")
	}
	defer tx.Rollback()

	orgID, config, err := s.loadConfigForAsset(ctx, tx, flowID)
	if err != nil {
		return Asset{}, err
	}

	if _, err = tx.ExecContext(ctx,
		tx.Rebind(`DELETE FROM login_flow_assets WHERE instance_id = ? AND login_flow_id = ? AND slot = ?`),
		tx.InstanceID(),
		flowID,
		slot,
	); err != nil {
		return Asset{}, fmt.Errorf("failed to replace existing asset")
	}

	assetID := id.New()
	sum := sha256.Sum256(payload)
	etag := fmt.Sprintf(`"%s"`, hex.EncodeToString(sum[:]))
	now := time.Now().UTC().Format(time.RFC3339)
	if _, err = tx.ExecContext(ctx,
		tx.Rebind(`INSERT INTO login_flow_assets (instance_id, id, org_id, login_flow_id, slot, filename, content_type, size_bytes, sha256, etag, data, metadata, created_at, updated_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`),
		tx.InstanceID(),
		assetID,
		orgID,
		flowID,
		slot,
		filename,
		contentType,
		len(payload),
		hex.EncodeToString(sum[:]),
		etag,
		payload,
		"{}",
		now,
		now,
	); err != nil {
		return Asset{}, fmt.Errorf("failed to save asset")
	}

	assetURL := "/assets/login/" + assetID
	setBrandingField(config, slot, assetURL)
	if err := s.updateConfig(ctx, tx, flowID, config); err != nil {
		return Asset{}, fmt.Errorf("failed to update login flow config")
	}
	if err := tx.Commit(); err != nil {
		return Asset{}, fmt.Errorf("commit failed")
	}

	return Asset{
		ID:          assetID,
		LoginFlowID: flowID,
		Slot:        slot,
		URL:         assetURL,
		Filename:    filename,
		ContentType: contentType,
		SizeBytes:   int64(len(payload)),
		ETag:        etag,
	}, nil
}

func (s *Store) DeleteAsset(ctx context.Context, flowID, assetID string) (string, error) {
	scoped := s.db.Scoped(ctx)
	tx, err := scoped.BeginTx(ctx, nil)
	if err != nil {
		return "", fmt.Errorf("database error")
	}
	defer tx.Rollback()

	var slot string
	if err := tx.QueryRowContext(ctx,
		tx.Rebind(`SELECT slot FROM login_flow_assets WHERE instance_id = ? AND id = ? AND login_flow_id = ?`),
		tx.InstanceID(),
		assetID,
		flowID,
	).Scan(&slot); err != nil {
		return "", err
	}

	if _, err := tx.ExecContext(ctx,
		tx.Rebind(`DELETE FROM login_flow_assets WHERE instance_id = ? AND id = ? AND login_flow_id = ?`),
		tx.InstanceID(),
		assetID,
		flowID,
	); err != nil {
		return "", fmt.Errorf("delete login flow asset: %w", err)
	}

	_, config, err := s.loadConfigForAsset(ctx, tx, flowID)
	if err != nil {
		return "", err
	}
	setBrandingField(config, slot, "")
	if err := s.updateConfig(ctx, tx, flowID, config); err != nil {
		return "", err
	}
	if err := tx.Commit(); err != nil {
		return "", fmt.Errorf("commit failed")
	}
	return slot, nil
}

func (s *Store) GetAsset(ctx context.Context, assetID string) (AssetData, error) {
	scoped := s.db.Scoped(ctx)
	var data AssetData
	if err := scoped.QueryRowContext(ctx,
		scoped.Rebind(`SELECT content_type, etag, data FROM login_flow_assets WHERE instance_id = ? AND id = ?`),
		scoped.InstanceID(),
		assetID,
	).Scan(&data.ContentType, &data.ETag, &data.Payload); err != nil {
		return AssetData{}, err
	}
	return data, nil
}

type recordScanner interface {
	Scan(dest ...any) error
}

func scanRecord(s recordScanner) (Record, error) {
	var record Record
	var configStr, audienceStr, authMethodsStr, metadataStr string
	var isDefault, enabled int
	if err := s.Scan(
		&record.ID,
		&record.OrgID,
		&record.SchemaID,
		&record.Name,
		&record.Strategy,
		&configStr,
		&isDefault,
		&enabled,
		&record.State,
		&record.Priority,
		&audienceStr,
		&authMethodsStr,
		&metadataStr,
		&record.CreatedAt,
		&record.UpdatedAt,
	); err != nil {
		return Record{}, err
	}

	record.IsDefault = isDefault == 1 || isDefault != 0
	record.Enabled = enabled == 1 || enabled != 0
	_ = json.Unmarshal([]byte(configStr), &record.Config)
	_ = json.Unmarshal([]byte(audienceStr), &record.Audience)
	_ = json.Unmarshal([]byte(authMethodsStr), &record.AuthMethods)
	_ = json.Unmarshal([]byte(metadataStr), &record.Metadata)
	return record, nil
}

func (s *Store) loadConfigForAsset(ctx context.Context, tx *database.ScopedTx, flowID string) (string, map[string]any, error) {
	var orgID string
	var configJSON string
	if err := tx.QueryRowContext(ctx,
		tx.Rebind(`SELECT COALESCE(org_id, '1'), COALESCE(config, '{}') FROM login_flows WHERE instance_id = ? AND id = ?`),
		tx.InstanceID(),
		flowID,
	).Scan(&orgID, &configJSON); err != nil {
		return "", nil, err
	}

	var config map[string]any
	if err := json.Unmarshal([]byte(configJSON), &config); err != nil || config == nil {
		config = map[string]any{}
	}
	return orgID, config, nil
}

func (s *Store) updateConfig(ctx context.Context, tx *database.ScopedTx, flowID string, config map[string]any) error {
	configBytes, err := json.Marshal(config)
	if err != nil {
		return err
	}
	_, err = tx.ExecContext(ctx,
		tx.Rebind(`UPDATE login_flows SET config = ?, updated_at = ? WHERE instance_id = ? AND id = ?`),
		string(configBytes),
		time.Now().UTC().Format(time.RFC3339),
		tx.InstanceID(),
		flowID,
	)
	return err
}

func setBrandingField(config map[string]any, field, value string) {
	branding, ok := config["branding"].(map[string]any)
	if !ok || branding == nil {
		branding = map[string]any{}
	}
	if value == "" {
		delete(branding, field)
	} else {
		branding[field] = value
	}
	config["branding"] = branding
}
