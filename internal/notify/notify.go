// Package notify provides queued notification delivery for Zitadel.
package notify

import (
	"bytes"
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"html"
	"net/http"
	"net/smtp"
	"sort"
	"strings"
	"text/template"
	"time"

	zcrypto "github.com/zitadel/zitadel/internal/crypto"
	"github.com/zitadel/zitadel/internal/eventbus"
	"github.com/zitadel/zitadel/internal/id"
	"github.com/zitadel/zitadel/internal/logging"
	"github.com/zitadel/zitadel/internal/settings"
	"github.com/zitadel/zitadel/internal/telemetry"
)

const (
	MediumEmail = "email"
	MediumSMS   = "sms"

	DriverStdout     = "stdout"
	DriverSMTP       = "smtp"
	DriverCustomHTTP = "custom_http"

	requestStatePending    = "pending"
	requestStateProcessing = "processing"
	requestStateRetry      = "retry"
	requestStateSent       = "sent"
	requestStateFailed     = "failed"
)

type execer interface {
	ExecContext(context.Context, string, ...any) (sql.Result, error)
}

// Service manages notification rendering, queueing, and delivery.
type Service struct {
	db           *sql.DB
	dialect      string
	bus          *eventbus.Bus
	box          *zcrypto.SecretBox
	externalBase string
	httpClient   *http.Client
	pollInterval time.Duration
}

// RequestSpec describes a queued notification request.
type RequestSpec struct {
	OrgID         string
	AggregateID   string
	AggregateType string
	EventType     string
	Medium        string
	ChannelID     string
	Recipient     string
	TemplateKey   string
	Locale        string
	Payload       map[string]any
	MaxAttempts   int
}

// PreviewRequest renders a notification without delivering it.
type PreviewRequest struct {
	OrgID       string
	Medium      string
	TemplateKey string
	Locale      string
	Payload     map[string]any
}

// TestRequest renders and delivers a notification immediately.
type TestRequest struct {
	OrgID       string
	Medium      string
	ChannelID   string
	Recipient   string
	TemplateKey string
	Locale      string
	Payload     map[string]any
}

// RenderedMessage is the final rendered notification body.
type RenderedMessage struct {
	Medium      string            `json:"medium"`
	ChannelID   string            `json:"channel_id,omitempty"`
	TemplateKey string            `json:"template_key"`
	Locale      string            `json:"locale"`
	Subject     string            `json:"subject,omitempty"`
	TextBody    string            `json:"text_body"`
	HTMLBody    string            `json:"html_body,omitempty"`
	Metadata    map[string]string `json:"metadata,omitempty"`
}

// Preset defines a suggested channel configuration.
type Preset struct {
	ID          string         `json:"id"`
	Label       string         `json:"label"`
	Medium      string         `json:"medium"`
	Driver      string         `json:"driver"`
	Description string         `json:"description"`
	Config      map[string]any `json:"config"`
}

type notificationConfig struct {
	DefaultLocale string       `json:"default_locale"`
	Email         mediumConfig `json:"email"`
	SMS           mediumConfig `json:"sms"`
	Legacy        legacyConfig `json:"-"`
}

type mediumConfig struct {
	DefaultChannel string                   `json:"default_channel"`
	Channels       map[string]channelConfig `json:"channels"`
}

type channelConfig struct {
	Enabled      *bool             `json:"enabled,omitempty"`
	Driver       string            `json:"driver,omitempty"`
	Preset       string            `json:"preset,omitempty"`
	From         string            `json:"from,omitempty"`
	FromName     string            `json:"from_name,omitempty"`
	Host         string            `json:"host,omitempty"`
	Port         int               `json:"port,omitempty"`
	Username     string            `json:"username,omitempty"`
	Password     string            `json:"password,omitempty"`
	TLS          *bool             `json:"tls,omitempty"`
	URL          string            `json:"url,omitempty"`
	Method       string            `json:"method,omitempty"`
	ContentType  string            `json:"content_type,omitempty"`
	Headers      map[string]string `json:"headers,omitempty"`
	BodyTemplate string            `json:"body_template,omitempty"`
}

type legacyConfig struct {
	EmailFrom     string         `json:"email_from,omitempty"`
	EmailFromName string         `json:"email_from_name,omitempty"`
	SMTPHost      string         `json:"smtp_host,omitempty"`
	SMTPPort      int            `json:"smtp_port,omitempty"`
	SMTPUser      string         `json:"smtp_user,omitempty"`
	SMTPPassword  string         `json:"smtp_password,omitempty"`
	SMTPTLS       *bool          `json:"smtp_tls,omitempty"`
	SMSProvider   string         `json:"sms_provider,omitempty"`
	SMSConfig     map[string]any `json:"sms_config,omitempty"`
	WebhookURL    string         `json:"webhook_url,omitempty"`
}

type notificationTemplatesConfig struct {
	DefaultLocale string                              `json:"default_locale"`
	Templates     map[string]templateOverrideEnvelope `json:"templates"`
}

type templateOverrideEnvelope struct {
	Locales map[string]messageTemplate `json:"locales"`
}

type messageTemplate struct {
	Subject  string `json:"subject,omitempty"`
	TextBody string `json:"text_body,omitempty"`
	HTMLBody string `json:"html_body,omitempty"`
}

type requestRow struct {
	ID                string
	OrgID             string
	AggregateID       string
	AggregateType     string
	EventType         string
	Medium            string
	ChannelID         string
	Recipient         string
	TemplateKey       string
	Locale            string
	State             string
	Attempts          int
	MaxAttempts       int
	LastError         string
	PayloadCiphertext []byte
	PayloadNonce      []byte
	PayloadKeyID      string
}

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
		httpClient:   &http.Client{Timeout: 10 * time.Second},
		pollInterval: 2 * time.Second,
	}
}

// EnsureSchema bootstraps the lightweight notification queue table for the POC.
func (s *Service) EnsureSchema(ctx context.Context) error {
	var ddl []string
	if s.dialect == "postgres" {
		ddl = []string{
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
		}
	} else {
		ddl = []string{
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
		}
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

// Presets returns the built-in preset packs exposed in the Console.
func Presets() []Preset {
	return []Preset{
		{
			ID:          "sendgrid_smtp",
			Label:       "SendGrid SMTP",
			Medium:      MediumEmail,
			Driver:      DriverSMTP,
			Description: "SMTP settings for SendGrid using API key auth.",
			Config: map[string]any{
				"driver": "smtp", "host": "smtp.sendgrid.net", "port": 587, "username": "apikey", "tls": true,
			},
		},
		{
			ID:          "amazon_ses_smtp",
			Label:       "Amazon SES SMTP",
			Medium:      MediumEmail,
			Driver:      DriverSMTP,
			Description: "SMTP settings for Amazon SES. Adjust host to your SES region.",
			Config: map[string]any{
				"driver": "smtp", "host": "email-smtp.us-east-1.amazonaws.com", "port": 587, "tls": true,
			},
		},
		{
			ID:          "twilio_sms",
			Label:       "Twilio SMS",
			Medium:      MediumSMS,
			Driver:      DriverCustomHTTP,
			Description: "POST form-encoded SMS messages to Twilio's Messages API.",
			Config: map[string]any{
				"driver":        "custom_http",
				"method":        "POST",
				"content_type":  "application/x-www-form-urlencoded",
				"url":           "https://api.twilio.com/2010-04-01/Accounts/<account_sid>/Messages.json",
				"body_template": "To={{ .Recipient }}&From={{ index .Metadata \"from\" }}&Body={{ .TextBody }}",
			},
		},
		{
			ID:          "generic_json_http",
			Label:       "Generic JSON HTTP",
			Medium:      MediumSMS,
			Driver:      DriverCustomHTTP,
			Description: "POST notification payloads as JSON to a custom endpoint.",
			Config: map[string]any{
				"driver":       "custom_http",
				"method":       "POST",
				"content_type": "application/json",
			},
		},
	}
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

func (s *Service) render(ctx context.Context, cfg notificationConfig, orgID, medium, channelID, templateKey, locale string, payload map[string]any) (*RenderedMessage, error) {
	if medium == "" {
		medium = MediumEmail
	}
	templateData := cloneMap(payload)
	if templateData == nil {
		templateData = map[string]any{}
	}
	if _, ok := templateData["base_url"]; !ok {
		templateData["base_url"] = s.externalBase
	}

	tmpl := s.resolveTemplate(ctx, orgID, templateKey, locale)
	subject, err := renderTemplateString(tmpl.Subject, templateData)
	if err != nil {
		return nil, err
	}
	textBody, err := renderTemplateString(tmpl.TextBody, templateData)
	if err != nil {
		return nil, err
	}
	htmlBody, err := renderTemplateString(tmpl.HTMLBody, templateData)
	if err != nil {
		return nil, err
	}
	resolvedLocale := locale
	if resolvedLocale == "" {
		resolvedLocale = defaultString(cfg.DefaultLocale, "en")
	}
	return &RenderedMessage{
		Medium:      medium,
		TemplateKey: templateKey,
		Locale:      resolvedLocale,
		Subject:     strings.TrimSpace(subject),
		TextBody:    strings.TrimSpace(textBody),
		HTMLBody:    strings.TrimSpace(htmlBody),
		Metadata:    map[string]string{},
	}, nil
}

func (s *Service) resolveConfig(ctx context.Context, orgID string) (notificationConfig, error) {
	data, err := settings.Resolve(ctx, s.db, "notification", orgID, "")
	if err != nil {
		return notificationConfig{}, err
	}
	cfg := defaultNotificationConfig()
	if len(data) > 0 {
		raw, err := json.Marshal(data)
		if err != nil {
			return notificationConfig{}, err
		}
		if err := json.Unmarshal(raw, &cfg); err != nil {
			return notificationConfig{}, err
		}
	}
	applyLegacyConfig(&cfg)
	ensureChannelDefaults(&cfg)
	return cfg, nil
}

func (s *Service) resolveChannel(cfg notificationConfig, medium, channelID string) (string, channelConfig, error) {
	group := cfg.Email
	if medium == MediumSMS {
		group = cfg.SMS
	}
	if group.Channels == nil {
		return "", channelConfig{}, fmt.Errorf("notify: no %s channels configured", medium)
	}
	if channelID == "" {
		channelID = group.DefaultChannel
	}
	channel, ok := group.Channels[channelID]
	if !ok {
		keys := make([]string, 0, len(group.Channels))
		for key := range group.Channels {
			keys = append(keys, key)
		}
		sort.Strings(keys)
		return "", channelConfig{}, fmt.Errorf("notify: channel %q not found for %s (available: %s)", channelID, medium, strings.Join(keys, ", "))
	}
	if channel.Enabled != nil && !*channel.Enabled {
		return "", channelConfig{}, fmt.Errorf("notify: channel %q is disabled", channelID)
	}
	if channel.Driver == "" {
		channel.Driver = DriverStdout
	}
	return channelID, channel, nil
}

func (s *Service) deliver(ctx context.Context, channelID string, channel channelConfig, recipient string, rendered *RenderedMessage) error {
	switch channel.Driver {
	case DriverSMTP:
		return deliverSMTP(channel, recipient, rendered)
	case DriverCustomHTTP:
		return s.deliverHTTP(ctx, channel, recipient, rendered)
	default:
		deliverStdout(channelID, recipient, rendered)
		return nil
	}
}

func deliverStdout(channelID, recipient string, rendered *RenderedMessage) {
	logging.Printf("[notify] channel=%s medium=%s to=%s template=%s", channelID, rendered.Medium, recipient, rendered.TemplateKey)
	if rendered.Subject != "" {
		logging.Printf("[notify] subject=%s", rendered.Subject)
	}
	logging.Printf("[notify] body=%s", rendered.TextBody)
}

func deliverSMTP(channel channelConfig, recipient string, rendered *RenderedMessage) error {
	if channel.Host == "" {
		return errors.New("notify: smtp host is required")
	}
	if channel.Port == 0 {
		channel.Port = 587
	}
	from := defaultString(channel.From, "no-reply@localhost")
	addr := fmt.Sprintf("%s:%d", channel.Host, channel.Port)
	host := channel.Host
	var auth smtp.Auth
	if channel.Username != "" {
		auth = smtp.PlainAuth("", channel.Username, channel.Password, host)
	}

	headers := []string{
		fmt.Sprintf("From: %s", formatFrom(from, channel.FromName)),
		fmt.Sprintf("To: %s", recipient),
		fmt.Sprintf("Subject: %s", rendered.Subject),
		"MIME-Version: 1.0",
	}
	body := rendered.TextBody
	if rendered.HTMLBody != "" {
		boundary := "zitadel-boundary"
		headers = append(headers, fmt.Sprintf("Content-Type: multipart/alternative; boundary=%q", boundary))
		body = fmt.Sprintf("--%s\r\nContent-Type: text/plain; charset=UTF-8\r\n\r\n%s\r\n--%s\r\nContent-Type: text/html; charset=UTF-8\r\n\r\n%s\r\n--%s--", boundary, rendered.TextBody, boundary, rendered.HTMLBody, boundary)
	} else {
		headers = append(headers, "Content-Type: text/plain; charset=UTF-8")
	}
	msg := strings.Join(headers, "\r\n") + "\r\n\r\n" + body
	return smtp.SendMail(addr, auth, from, []string{recipient}, []byte(msg))
}

func (s *Service) deliverHTTP(ctx context.Context, channel channelConfig, recipient string, rendered *RenderedMessage) error {
	if channel.URL == "" {
		return errors.New("notify: custom_http url is required")
	}
	method := defaultString(channel.Method, http.MethodPost)
	contentType := defaultString(channel.ContentType, "application/json")
	body := channel.BodyTemplate
	payload := map[string]any{
		"recipient":    recipient,
		"subject":      rendered.Subject,
		"text_body":    rendered.TextBody,
		"html_body":    rendered.HTMLBody,
		"template_key": rendered.TemplateKey,
		"locale":       rendered.Locale,
		"metadata":     rendered.Metadata,
	}
	if body == "" {
		raw, _ := json.Marshal(payload)
		body = string(raw)
	} else {
		renderedBody, err := renderTemplateString(body, map[string]any{
			"Recipient":   recipient,
			"Subject":     rendered.Subject,
			"TextBody":    rendered.TextBody,
			"HTMLBody":    rendered.HTMLBody,
			"TemplateKey": rendered.TemplateKey,
			"Locale":      rendered.Locale,
			"Metadata":    rendered.Metadata,
		})
		if err != nil {
			return err
		}
		body = renderedBody
	}

	req, err := http.NewRequestWithContext(ctx, method, channel.URL, strings.NewReader(body))
	if err != nil {
		return err
	}
	req.Header.Set("Content-Type", contentType)
	for key, value := range channel.Headers {
		req.Header.Set(key, value)
	}

	resp, err := s.httpClient.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	if resp.StatusCode >= 300 {
		return fmt.Errorf("notify: custom_http status %d", resp.StatusCode)
	}
	return nil
}

func (s *Service) resolveTemplate(ctx context.Context, orgID, templateKey, locale string) messageTemplate {
	result := builtInTemplates()[templateKey]
	instance := s.loadTemplateOverrides(ctx, "", "instance")
	org := s.loadTemplateOverrides(ctx, orgID, "org")

	result = mergeTemplate(result, templateForLocale(instance, templateKey, defaultString(instance.DefaultLocale, "en")))
	result = mergeTemplate(result, templateForLocale(instance, templateKey, locale))
	result = mergeTemplate(result, templateForLocale(org, templateKey, defaultString(org.DefaultLocale, "")))
	result = mergeTemplate(result, templateForLocale(org, templateKey, locale))
	return result
}

func (s *Service) loadTemplateOverrides(ctx context.Context, orgID, scope string) notificationTemplatesConfig {
	scopeID := ""
	if scope == "org" {
		scopeID = orgID
		if scopeID == "" {
			return notificationTemplatesConfig{}
		}
	}
	data, err := settings.Get(ctx, s.db, "notification_templates", scope, scopeID)
	if err != nil {
		return notificationTemplatesConfig{}
	}
	raw, err := json.Marshal(data)
	if err != nil {
		return notificationTemplatesConfig{}
	}
	var cfg notificationTemplatesConfig
	if err := json.Unmarshal(raw, &cfg); err != nil {
		return notificationTemplatesConfig{}
	}
	return cfg
}

func (s *Service) sealPayload(payload map[string]any) (*zcrypto.SealedSecret, error) {
	raw, err := json.Marshal(payload)
	if err != nil {
		return nil, fmt.Errorf("notify: marshal payload: %w", err)
	}
	return s.box.Seal(raw)
}

func (s *Service) openPayload(ciphertext, nonce []byte, keyID string) (map[string]any, error) {
	raw, err := s.box.Open(ciphertext, nonce, keyID)
	if err != nil {
		return nil, fmt.Errorf("notify: decrypt payload: %w", err)
	}
	var payload map[string]any
	if len(raw) == 0 {
		return map[string]any{}, nil
	}
	if err := json.Unmarshal(raw, &payload); err != nil {
		return nil, fmt.Errorf("notify: decode payload: %w", err)
	}
	return payload, nil
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
	db.ExecContext(ctx,
		`INSERT INTO events (id, event_type, category, org_id, actor_id, actor_type, aggregate_id, aggregate_type, payload, metadata, request_id, session_id, flow_id, fingerprint, client_id, token_id, delegation_type, sdk_name, sdk_version, created_at)
		 VALUES (?, ?, ?, '0', '', '', ?, ?, ?, '{}', ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))`,
		eventID, eventType, eventCategory(eventType), aggregateID, aggregateType, payloadJSON, requestID, sessionID, flowID, fingerprint, clientID, tokenID, delegationType, sdkName, sdkVersion)
}

func defaultNotificationConfig() notificationConfig {
	stdout := boolPtr(true)
	return notificationConfig{
		DefaultLocale: "en",
		Email: mediumConfig{
			DefaultChannel: "dev_stdout",
			Channels: map[string]channelConfig{
				"dev_stdout": {Enabled: stdout, Driver: DriverStdout, From: "no-reply@localhost", FromName: "Zitadel Dev"},
			},
		},
		SMS: mediumConfig{
			DefaultChannel: "dev_stdout",
			Channels: map[string]channelConfig{
				"dev_stdout": {Enabled: stdout, Driver: DriverStdout},
			},
		},
	}
}

func applyLegacyConfig(cfg *notificationConfig) {
	if cfg.Email.Channels == nil {
		cfg.Email.Channels = map[string]channelConfig{}
	}
	if cfg.SMS.Channels == nil {
		cfg.SMS.Channels = map[string]channelConfig{}
	}
	if cfg.Legacy.SMTPHost != "" && len(cfg.Email.Channels) == 0 {
		cfg.Email.Channels["legacy_smtp"] = channelConfig{
			Driver:   DriverSMTP,
			From:     cfg.Legacy.EmailFrom,
			FromName: cfg.Legacy.EmailFromName,
			Host:     cfg.Legacy.SMTPHost,
			Port:     defaultInt(cfg.Legacy.SMTPPort, 587),
			Username: cfg.Legacy.SMTPUser,
			Password: cfg.Legacy.SMTPPassword,
			TLS:      cfg.Legacy.SMTPTLS,
		}
		cfg.Email.DefaultChannel = "legacy_smtp"
	}
	if cfg.Legacy.WebhookURL != "" && len(cfg.SMS.Channels) == 0 {
		cfg.SMS.Channels["legacy_custom_http"] = channelConfig{
			Driver: DriverCustomHTTP,
			URL:    cfg.Legacy.WebhookURL,
		}
		cfg.SMS.DefaultChannel = "legacy_custom_http"
	}
}

func ensureChannelDefaults(cfg *notificationConfig) {
	if cfg.Email.Channels == nil || len(cfg.Email.Channels) == 0 {
		defaults := defaultNotificationConfig()
		cfg.Email = defaults.Email
	}
	if cfg.SMS.Channels == nil || len(cfg.SMS.Channels) == 0 {
		defaults := defaultNotificationConfig()
		cfg.SMS = defaults.SMS
	}
	if cfg.Email.DefaultChannel == "" {
		for key := range cfg.Email.Channels {
			cfg.Email.DefaultChannel = key
			break
		}
	}
	if cfg.SMS.DefaultChannel == "" {
		for key := range cfg.SMS.Channels {
			cfg.SMS.DefaultChannel = key
			break
		}
	}
}

func builtInTemplates() map[string]messageTemplate {
	return map[string]messageTemplate{
		"magic_link_login": {
			Subject:  "Sign in to Zitadel",
			TextBody: "Click the link below to sign in:\n\n{{ .link }}\n\nThis link expires at {{ .expires_at }}.",
			HTMLBody: "<p>Click the link below to sign in:</p><p><a href=\"{{ .link }}\">{{ .link }}</a></p><p>This link expires at {{ .expires_at }}.</p>",
		},
		"magic_link_register": {
			Subject:  "Complete your Zitadel registration",
			TextBody: "Complete your registration by opening the link below:\n\n{{ .link }}\n\nThis link expires at {{ .expires_at }}.",
			HTMLBody: "<p>Complete your registration by opening the link below:</p><p><a href=\"{{ .link }}\">{{ .link }}</a></p><p>This link expires at {{ .expires_at }}.</p>",
		},
		"invite": {
			Subject:  "You're invited to Zitadel",
			TextBody: "Use this invitation link to finish joining:\n\n{{ .link }}\n\nThis link expires at {{ .expires_at }}.",
			HTMLBody: "<p>Use this invitation link to finish joining:</p><p><a href=\"{{ .link }}\">{{ .link }}</a></p><p>This link expires at {{ .expires_at }}.</p>",
		},
		"password_reset": {
			Subject:  "Reset your Zitadel password",
			TextBody: "Reset your password with the link below:\n\n{{ .link }}\n\nThis link expires at {{ .expires_at }}.",
			HTMLBody: "<p>Reset your password with the link below:</p><p><a href=\"{{ .link }}\">{{ .link }}</a></p><p>This link expires at {{ .expires_at }}.</p>",
		},
		"email_verification": {
			Subject:  "Verify your email for Zitadel",
			TextBody: "Verify your email address by opening the link below:\n\n{{ .link }}\n\nThis link expires at {{ .expires_at }}.",
			HTMLBody: "<p>Verify your email address by opening the link below:</p><p><a href=\"{{ .link }}\">{{ .link }}</a></p><p>This link expires at {{ .expires_at }}.</p>",
		},
	}
}

func templateForLocale(cfg notificationTemplatesConfig, templateKey, locale string) messageTemplate {
	if locale == "" || cfg.Templates == nil {
		return messageTemplate{}
	}
	entry, ok := cfg.Templates[templateKey]
	if !ok || entry.Locales == nil {
		return messageTemplate{}
	}
	if tmpl, ok := entry.Locales[locale]; ok {
		return tmpl
	}
	if idx := strings.Index(locale, "-"); idx > 0 {
		if tmpl, ok := entry.Locales[locale[:idx]]; ok {
			return tmpl
		}
	}
	return messageTemplate{}
}

func mergeTemplate(base, overlay messageTemplate) messageTemplate {
	if overlay.Subject != "" {
		base.Subject = overlay.Subject
	}
	if overlay.TextBody != "" {
		base.TextBody = overlay.TextBody
	}
	if overlay.HTMLBody != "" {
		base.HTMLBody = overlay.HTMLBody
	}
	return base
}

func renderTemplateString(src string, payload map[string]any) (string, error) {
	if src == "" {
		return "", nil
	}
	tmpl, err := template.New("notification").Option("missingkey=zero").Parse(src)
	if err != nil {
		return "", fmt.Errorf("notify: parse template: %w", err)
	}
	var buf bytes.Buffer
	if err := tmpl.Execute(&buf, payload); err != nil {
		return "", fmt.Errorf("notify: execute template: %w", err)
	}
	return buf.String(), nil
}

func formatFrom(email, name string) string {
	if strings.TrimSpace(name) == "" {
		return email
	}
	return fmt.Sprintf("%s <%s>", strings.TrimSpace(name), email)
}

func eventCategory(eventType string) string {
	switch {
	case strings.HasPrefix(eventType, "notification."):
		return "system"
	case strings.HasPrefix(eventType, "auth."):
		return "auth"
	case strings.HasPrefix(eventType, "session."):
		return "session"
	case strings.HasPrefix(eventType, "signal."):
		return "signal"
	default:
		return "system"
	}
}

func nowRFC3339() string { return time.Now().UTC().Format(time.RFC3339) }

func backoffForAttempt(attempt int) time.Duration {
	switch attempt {
	case 1:
		return 10 * time.Second
	case 2:
		return 60 * time.Second
	default:
		return 5 * time.Minute
	}
}

func cloneMap(src map[string]any) map[string]any {
	if src == nil {
		return nil
	}
	dst := make(map[string]any, len(src))
	for key, value := range src {
		dst[key] = value
	}
	return dst
}

func defaultString(v, fallback string) string {
	if strings.TrimSpace(v) == "" {
		return fallback
	}
	return v
}

func defaultInt(v, fallback int) int {
	if v == 0 {
		return fallback
	}
	return v
}

func defaultOrg(orgID string) string {
	if strings.TrimSpace(orgID) == "" {
		return "0"
	}
	return orgID
}

func boolPtr(v bool) *bool { return &v }

func maxInt(v, fallback int) int {
	if v <= 0 {
		return fallback
	}
	return v
}

// EscapeHTML is used by the Console when previewing raw text inside HTML cards.
func EscapeHTML(v string) string {
	return html.EscapeString(v)
}
