package notify

import (
	"context"
	"database/sql"
	"path/filepath"
	"testing"

	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/eventbus"
	"github.com/zitadel/zitadel/internal/settings"
)

func newTestService(t *testing.T) *Service {
	t.Helper()

	dir := t.TempDir()
	db, err := database.Open("sqlite://" + filepath.Join(dir, "notify.db"))
	if err != nil {
		t.Fatalf("database.Open() error = %v", err)
	}
	t.Cleanup(func() {
		_ = db.Close()
	})

	if err := database.Migrate(db); err != nil {
		t.Fatalf("database.Migrate() error = %v", err)
	}

	svc := NewService(db.SQL(), db.Dialect(), eventbus.New(), nil, "https://example.com")
	if err := svc.EnsureSchema(t.Context()); err != nil {
		t.Fatalf("EnsureSchema() error = %v", err)
	}
	return svc
}

func TestResolveChannel_DefaultAndDisabled(t *testing.T) {
	t.Parallel()

	svc := NewService(&sql.DB{}, "sqlite", eventbus.New(), nil, "https://example.com")
	cfg := defaultNotificationConfig()

	channelID, channel, err := svc.resolveChannel(cfg, MediumEmail, "")
	if err != nil {
		t.Fatalf("resolveChannel() error = %v", err)
	}
	if channelID != "dev_stdout" {
		t.Fatalf("channelID = %q, want dev_stdout", channelID)
	}
	if channel.Driver != DriverStdout {
		t.Fatalf("driver = %q, want %q", channel.Driver, DriverStdout)
	}

	disabled := false
	cfg.Email.Channels["disabled"] = channelConfig{Enabled: &disabled, Driver: DriverStdout}
	if _, _, err := svc.resolveChannel(cfg, MediumEmail, "disabled"); err == nil {
		t.Fatal("resolveChannel() expected error for disabled channel")
	}
}

func TestApplyLegacyConfig_PromotesLegacyFields(t *testing.T) {
	t.Parallel()

	cfg := notificationConfig{
		Legacy: legacyConfig{
			EmailFrom:  "legacy@example.com",
			SMTPHost:   "smtp.example.com",
			WebhookURL: "https://sms.example.com",
		},
	}
	applyLegacyConfig(&cfg)
	ensureChannelDefaults(&cfg)

	if cfg.Email.DefaultChannel != "legacy_smtp" {
		t.Fatalf("Email.DefaultChannel = %q, want legacy_smtp", cfg.Email.DefaultChannel)
	}
	if cfg.SMS.DefaultChannel != "legacy_custom_http" {
		t.Fatalf("SMS.DefaultChannel = %q, want legacy_custom_http", cfg.SMS.DefaultChannel)
	}
}

func TestTemplateForLocale_FallsBackToLanguagePrefix(t *testing.T) {
	t.Parallel()

	cfg := notificationTemplatesConfig{
		Templates: map[string]templateOverrideEnvelope{
			"invite": {
				Locales: map[string]messageTemplate{
					"en": {Subject: "Invite"},
				},
			},
		},
	}

	got := templateForLocale(cfg, "invite", "en-US")
	if got.Subject != "Invite" {
		t.Fatalf("Subject = %q, want Invite", got.Subject)
	}
}

func TestRender_UsesBaseURLAndOrgOverride(t *testing.T) {
	svc := newTestService(t)
	ctx := t.Context()

	err := settings.Put(ctx, svc.db, "notification_templates", "org", "org-1", map[string]any{
		"default_locale": "en",
		"templates": map[string]any{
			"invite": map[string]any{
				"locales": map[string]any{
					"en": map[string]any{
						"subject": "Invite {{ .base_url }}",
					},
				},
			},
		},
	})
	if err != nil {
		t.Fatalf("settings.Put() error = %v", err)
	}

	rendered, err := svc.render(ctx, defaultNotificationConfig(), "org-1", MediumEmail, "", "invite", "", map[string]any{})
	if err != nil {
		t.Fatalf("render() error = %v", err)
	}
	if rendered.Subject != "Invite https://example.com" {
		t.Fatalf("Subject = %q, want org override with base URL", rendered.Subject)
	}
}

func TestPayloadRoundTrip(t *testing.T) {
	svc := newTestService(t)

	payload := map[string]any{"name": "Ada"}
	sealed, err := svc.sealPayload(payload)
	if err != nil {
		t.Fatalf("sealPayload() error = %v", err)
	}

	got, err := svc.openPayload(sealed.Ciphertext, sealed.Nonce, sealed.KeyID)
	if err != nil {
		t.Fatalf("openPayload() error = %v", err)
	}
	if got["name"] != "Ada" {
		t.Fatalf("payload name = %v, want Ada", got["name"])
	}
}

func TestProcessDueRequests_SendsQueuedNotification(t *testing.T) {
	svc := newTestService(t)
	ctx := context.Background()

	tx, err := svc.db.BeginTx(ctx, nil)
	if err != nil {
		t.Fatalf("BeginTx() error = %v", err)
	}

	requestID, err := svc.EnqueueTx(ctx, tx, RequestSpec{
		OrgID:       "org-1",
		Recipient:   "user@example.com",
		TemplateKey: "invite",
		Medium:      MediumEmail,
		Payload: map[string]any{
			"link":       "https://example.com/invite",
			"expires_at": "tomorrow",
		},
	})
	if err != nil {
		t.Fatalf("EnqueueTx() error = %v", err)
	}
	if err := tx.Commit(); err != nil {
		t.Fatalf("Commit() error = %v", err)
	}

	if err := svc.processDueRequests(ctx); err != nil {
		t.Fatalf("processDueRequests() error = %v", err)
	}

	var state string
	if err := svc.db.QueryRowContext(ctx, `SELECT state FROM notification_requests WHERE id = ?`, requestID).Scan(&state); err != nil {
		t.Fatalf("query state: %v", err)
	}
	if state != requestStateSent {
		t.Fatalf("state = %q, want %q", state, requestStateSent)
	}
}

func TestProcessOne_RetriesFailedDelivery(t *testing.T) {
	svc := newTestService(t)
	ctx := t.Context()

	err := settings.Put(ctx, svc.db, "notification", "org", "org-1", map[string]any{
		"email": map[string]any{
			"default_channel": "webhook",
			"channels": map[string]any{
				"webhook": map[string]any{
					"driver": "custom_http",
					"url":    "http://127.0.0.1:1/fail",
				},
			},
		},
	})
	if err != nil {
		t.Fatalf("settings.Put() error = %v", err)
	}

	tx, err := svc.db.BeginTx(ctx, nil)
	if err != nil {
		t.Fatalf("BeginTx() error = %v", err)
	}
	requestID, err := svc.EnqueueTx(ctx, tx, RequestSpec{
		OrgID:       "org-1",
		Recipient:   "user@example.com",
		TemplateKey: "invite",
		Medium:      MediumEmail,
		Payload: map[string]any{
			"link":       "https://example.com/invite",
			"expires_at": "tomorrow",
		},
	})
	if err != nil {
		t.Fatalf("EnqueueTx() error = %v", err)
	}
	if err := tx.Commit(); err != nil {
		t.Fatalf("Commit() error = %v", err)
	}

	if err := svc.processDueRequests(ctx); err != nil {
		t.Fatalf("processDueRequests() error = %v", err)
	}

	var state string
	var attempts int
	if err := svc.db.QueryRowContext(ctx, `SELECT state, attempts FROM notification_requests WHERE id = ?`, requestID).Scan(&state, &attempts); err != nil {
		t.Fatalf("query retry state: %v", err)
	}
	if state != requestStateRetry {
		t.Fatalf("state = %q, want %q", state, requestStateRetry)
	}
	if attempts != 1 {
		t.Fatalf("attempts = %d, want 1", attempts)
	}
}
