package notify

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"strings"
	"time"

	zcrypto "github.com/zitadel/zitadel/internal/crypto"
	"github.com/zitadel/zitadel/internal/eventbus"
	"github.com/zitadel/zitadel/internal/id"
	"github.com/zitadel/zitadel/internal/logging"
	"github.com/zitadel/zitadel/internal/telemetry"
)

// NewService creates a notification service.
func NewService(db *sql.DB, dialect string, bus *eventbus.Bus, box *zcrypto.SecretBox, externalBase string) *Service {
	if box == nil {
		box, _ = zcrypto.NewSecretBox("", nil)
	}
	return &Service{
		db:           db,
		dialect:      dialect,
		bus:          bus,
		box:          box,
		externalBase: strings.TrimRight(externalBase, "/"),
		httpClient:   nil,
		pollInterval: 2 * time.Second,
	}
}

// EnsureSchema bootstraps the lightweight notification queue table for the POC.
func (s *Service) EnsureSchema(ctx context.Context) error {
	ddl := make([]string, 0, 4)
	if s.dialect == "postgres" {
		ddl = append(ddl,
			`CREATE TABLE IF NOT EXISTS notification_requests (
				id TEXT PRIMARY KEY,
				org_id TEXT NOT NULL DEFAULT '0',
				aggregate_id TEXT DEFAULT '',
				aggregate_type TEXT DEFAULT '',
				event_type TEXT NOT NULL DEFAULT 'notification.requested',
				medium TEXT NOT NULL,
				channel_id TEXT NOT NULL DEFAULT '',
				recipient TEXT NOT NULL,
				template_key TEXT NOT NULL,
				locale TEXT NOT NULL DEFAULT '',
				state TEXT NOT NULL DEFAULT 'pending',
				attempts INTEGER NOT NULL DEFAULT 0,
				max_attempts INTEGER NOT NULL DEFAULT 3,
				last_error TEXT NOT NULL DEFAULT '',
				payload_ciphertext BYTEA NOT NULL,
				payload_nonce BYTEA,
				payload_key_id TEXT NOT NULL DEFAULT '',
				next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
				last_attempt_at TIMESTAMPTZ,
				sent_at TIMESTAMPTZ,
				created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
				updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
			)`,
		)
	} else {
		ddl = append(ddl,
			`CREATE TABLE IF NOT EXISTS notification_requests (
				id TEXT PRIMARY KEY,
				org_id TEXT NOT NULL DEFAULT '0',
				aggregate_id TEXT DEFAULT '',
				aggregate_type TEXT DEFAULT '',
				event_type TEXT NOT NULL DEFAULT 'notification.requested',
				medium TEXT NOT NULL,
				channel_id TEXT NOT NULL DEFAULT '',
				recipient TEXT NOT NULL,
				template_key TEXT NOT NULL,
				locale TEXT NOT NULL DEFAULT '',
				state TEXT NOT NULL DEFAULT 'pending',
				attempts INTEGER NOT NULL DEFAULT 0,
				max_attempts INTEGER NOT NULL DEFAULT 3,
				last_error TEXT NOT NULL DEFAULT '',
				payload_ciphertext BLOB NOT NULL,
				payload_nonce BLOB,
				payload_key_id TEXT NOT NULL DEFAULT '',
				next_attempt_at TEXT NOT NULL DEFAULT (datetime('now')),
				last_attempt_at TEXT,
				sent_at TEXT,
				created_at TEXT NOT NULL DEFAULT (datetime('now')),
				updated_at TEXT NOT NULL DEFAULT (datetime('now'))
			)`,
		)
	}
	ddl = append(ddl,
		`CREATE INDEX IF NOT EXISTS idx_notification_requests_state_next
		 ON notification_requests(state, next_attempt_at)`,
		`CREATE INDEX IF NOT EXISTS idx_notification_requests_org_created
		 ON notification_requests(org_id, created_at)`,
		`CREATE INDEX IF NOT EXISTS idx_notification_requests_template
		 ON notification_requests(template_key, created_at)`,
	)
	for _, stmt := range ddl {
		if _, err := s.db.ExecContext(ctx, stmt); err != nil {
			return fmt.Errorf("notify: ensure schema: %w", err)
		}
	}
	return nil
}

// EnqueueTx stores a notification request inside an existing transaction.
func (s *Service) EnqueueTx(ctx context.Context, tx *sql.Tx, spec RequestSpec) (string, error) {
	if tx == nil {
		return "", errors.New("notify: transaction is required")
	}
	if spec.Recipient == "" {
		return "", errors.New("notify: recipient is required")
	}
	if spec.TemplateKey == "" {
		return "", errors.New("notify: template_key is required")
	}
	if spec.Medium == "" {
		return "", errors.New("notify: medium is required")
	}
	if spec.EventType == "" {
		spec.EventType = "notification.requested"
	}
	if spec.MaxAttempts <= 0 {
		spec.MaxAttempts = 3
	}

	payload := cloneMap(spec.Payload)
	if payload == nil {
		payload = map[string]any{}
	}
	sealed, err := s.sealPayload(payload)
	if err != nil {
		return "", err
	}

	now := time.Now().UTC().Format(time.RFC3339)
	requestID := id.New()
	if _, err := tx.ExecContext(ctx,
		`INSERT INTO notification_requests
		 (id, org_id, aggregate_id, aggregate_type, event_type, medium, channel_id, recipient, template_key, locale, state, attempts, max_attempts, payload_ciphertext, payload_nonce, payload_key_id, next_attempt_at, created_at, updated_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?, ?, ?, ?, ?, ?)`,
		requestID, defaultOrg(spec.OrgID), spec.AggregateID, spec.AggregateType, spec.EventType, spec.Medium, spec.ChannelID, spec.Recipient, spec.TemplateKey, spec.Locale, requestStatePending, spec.MaxAttempts, sealed.Ciphertext, sealed.Nonce, sealed.KeyID, now, now, now,
	); err != nil {
		return "", fmt.Errorf("notify: insert request: %w", err)
	}

	s.emitEvent(ctx, tx, spec.EventType, spec.AggregateID, "notification_request", map[string]any{
		"notification_request_id": requestID,
		"medium":                  spec.Medium,
		"channel_id":              spec.ChannelID,
		"template_key":            spec.TemplateKey,
		"locale":                  spec.Locale,
	})
	return requestID, nil
}

// Preview renders a notification using the current effective settings.
func (s *Service) Preview(ctx context.Context, req PreviewRequest) (*RenderedMessage, error) {
	cfg, err := s.resolveConfig(ctx, req.OrgID)
	if err != nil {
		return nil, err
	}
	return s.render(ctx, cfg, req.OrgID, req.Medium, "", req.TemplateKey, req.Locale, cloneMap(req.Payload))
}

// SendTest renders and delivers a notification immediately without queueing it.
func (s *Service) SendTest(ctx context.Context, req TestRequest) (*RenderedMessage, error) {
	cfg, err := s.resolveConfig(ctx, req.OrgID)
	if err != nil {
		return nil, err
	}
	rendered, err := s.render(ctx, cfg, req.OrgID, req.Medium, req.ChannelID, req.TemplateKey, req.Locale, cloneMap(req.Payload))
	if err != nil {
		return nil, err
	}
	channelID, channel, err := s.resolveChannel(cfg, req.Medium, req.ChannelID)
	if err != nil {
		return nil, err
	}
	rendered.ChannelID = channelID
	if err := s.deliver(ctx, channelID, channel, req.Recipient, rendered); err != nil {
		return nil, err
	}
	return rendered, nil
}

// Start launches background workers.
func (s *Service) Start(ctx context.Context, workers int) {
	if err := s.EnsureSchema(ctx); err != nil {
		logging.Printf("[notify] schema init failed: %v", err)
		return
	}
	if workers <= 0 {
		workers = 1
	}
	for i := 0; i < workers; i++ {
		consumer := s.bus.Register(fmt.Sprintf("notifications_%d", i))
		go s.runWorker(ctx, consumer)
	}
}

func (s *Service) runWorker(ctx context.Context, consumer *eventbus.Consumer) {
	ticker := time.NewTicker(s.pollInterval)
	defer ticker.Stop()
	for {
		if err := s.processDueRequests(ctx); err != nil && !errors.Is(err, context.Canceled) {
			logging.Printf("[notify] worker error: %v", err)
		}
		select {
		case <-ctx.Done():
			return
		case <-ticker.C:
		case <-consumer.Chan():
		}
	}
}

func (s *Service) processDueRequests(ctx context.Context) error {
	rows, err := s.db.QueryContext(ctx,
		`SELECT id, org_id, aggregate_id, aggregate_type, event_type, medium, channel_id, recipient, template_key, locale, state, attempts, max_attempts, last_error, payload_ciphertext, payload_nonce, payload_key_id
		 FROM notification_requests
		 WHERE state IN (?, ?) AND next_attempt_at <= ?
		 ORDER BY created_at ASC
		 LIMIT 25`,
		requestStatePending, requestStateRetry, time.Now().UTC().Format(time.RFC3339),
	)
	if err != nil {
		return fmt.Errorf("notify: query due requests: %w", err)
	}
	defer rows.Close()

	var items []requestRow
	for rows.Next() {
		var row requestRow
		if err := rows.Scan(&row.ID, &row.OrgID, &row.AggregateID, &row.AggregateType, &row.EventType, &row.Medium, &row.ChannelID, &row.Recipient, &row.TemplateKey, &row.Locale, &row.State, &row.Attempts, &row.MaxAttempts, &row.LastError, &row.PayloadCiphertext, &row.PayloadNonce, &row.PayloadKeyID); err != nil {
			return err
		}
		items = append(items, row)
	}
	if err := rows.Err(); err != nil {
		return err
	}
	for _, row := range items {
		if err := s.processOne(ctx, row); err != nil && !errors.Is(err, context.Canceled) {
			logging.Printf("[notify] request %s failed: %v", row.ID, err)
		}
	}
	return nil
}

func (s *Service) processOne(ctx context.Context, row requestRow) error {
	res, err := s.db.ExecContext(ctx,
		`UPDATE notification_requests
		 SET state = ?, last_attempt_at = ?, updated_at = ?
		 WHERE id = ? AND state IN (?, ?)`,
		requestStateProcessing, nowRFC3339(), nowRFC3339(), row.ID, requestStatePending, requestStateRetry,
	)
	if err != nil {
		return err
	}
	affected, _ := res.RowsAffected()
	if affected == 0 {
		return nil
	}

	payload, err := s.openPayload(row.PayloadCiphertext, row.PayloadNonce, row.PayloadKeyID)
	if err != nil {
		return s.markFailed(ctx, row, err)
	}
	cfg, err := s.resolveConfig(ctx, row.OrgID)
	if err != nil {
		return s.markFailed(ctx, row, err)
	}
	rendered, err := s.render(ctx, cfg, row.OrgID, row.Medium, row.ChannelID, row.TemplateKey, row.Locale, payload)
	if err != nil {
		return s.markFailed(ctx, row, err)
	}
	channelID, channel, err := s.resolveChannel(cfg, row.Medium, row.ChannelID)
	if err != nil {
		return s.markFailed(ctx, row, err)
	}
	rendered.ChannelID = channelID
	if err := s.deliver(ctx, channelID, channel, row.Recipient, rendered); err != nil {
		return s.markFailed(ctx, row, err)
	}

	if _, err := s.db.ExecContext(ctx,
		`UPDATE notification_requests
		 SET state = ?, sent_at = ?, last_error = '', updated_at = ?
		 WHERE id = ?`,
		requestStateSent, nowRFC3339(), nowRFC3339(), row.ID,
	); err != nil {
		return err
	}
	s.emitEvent(ctx, s.db, "notification.sent", row.AggregateID, row.AggregateType, map[string]any{
		"notification_request_id": row.ID,
		"medium":                  row.Medium,
		"channel_id":              channelID,
		"template_key":            row.TemplateKey,
	})
	s.bus.Signal()
	return nil
}

func (s *Service) markFailed(ctx context.Context, row requestRow, cause error) error {
	attempts := row.Attempts + 1
	now := nowRFC3339()
	if attempts >= maxInt(row.MaxAttempts, 1) {
		if _, err := s.db.ExecContext(ctx,
			`UPDATE notification_requests
			 SET state = ?, attempts = ?, last_error = ?, updated_at = ?
			 WHERE id = ?`,
			requestStateFailed, attempts, cause.Error(), now, row.ID,
		); err != nil {
			return err
		}
		s.emitEvent(ctx, s.db, "notification.failed", row.AggregateID, row.AggregateType, map[string]any{
			"notification_request_id": row.ID,
			"medium":                  row.Medium,
			"template_key":            row.TemplateKey,
			"error":                   cause.Error(),
		})
		s.bus.Signal()
		return cause
	}

	nextAttempt := time.Now().UTC().Add(backoffForAttempt(attempts)).Format(time.RFC3339)
	if _, err := s.db.ExecContext(ctx,
		`UPDATE notification_requests
		 SET state = ?, attempts = ?, last_error = ?, next_attempt_at = ?, updated_at = ?
		 WHERE id = ?`,
		requestStateRetry, attempts, cause.Error(), nextAttempt, now, row.ID,
	); err != nil {
		return err
	}
	s.emitEvent(ctx, s.db, "notification.retried", row.AggregateID, row.AggregateType, map[string]any{
		"notification_request_id": row.ID,
		"medium":                  row.Medium,
		"template_key":            row.TemplateKey,
		"attempt":                 attempts,
		"next_attempt_at":         nextAttempt,
		"error":                   cause.Error(),
	})
	s.bus.Signal()
	return cause
}

func (s *Service) emitEvent(ctx context.Context, db execer, eventType, aggregateID, aggregateType string, payload map[string]any) {
	eventID := id.New()
	payloadJSON := "{}"
	if len(payload) > 0 {
		b, _ := json.Marshal(payload)
		payloadJSON = string(b)
	}
	requestID := telemetry.RequestIDFromContext(ctx)
	sessionID := telemetry.SessionIDFromContext(ctx)
	flowID := telemetry.FlowIDFromContext(ctx)
	fingerprint := telemetry.FingerprintFromContext(ctx)
	clientID := telemetry.ClientIDFromContext(ctx)
	tokenID := telemetry.TokenIDFromContext(ctx)
	delegationType := telemetry.DelegationTypeFromContext(ctx)
	sdkName := telemetry.SDKNameFromContext(ctx)
	sdkVersion := telemetry.SDKVersionFromContext(ctx)
	if _, err := db.ExecContext(ctx,
		`INSERT INTO events (id, event_type, category, org_id, actor_id, actor_type, aggregate_id, aggregate_type, payload, metadata, request_id, session_id, flow_id, fingerprint, client_id, token_id, delegation_type, sdk_name, sdk_version, created_at)
		 VALUES (?, ?, ?, '0', '', '', ?, ?, ?, '{}', ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))`,
		eventID, eventType, eventCategory(eventType), aggregateID, aggregateType, payloadJSON, requestID, sessionID, flowID, fingerprint, clientID, tokenID, delegationType, sdkName, sdkVersion); err != nil {
		logging.Printf("notify: emit event %s failed: %v", eventType, err)
	}
}
