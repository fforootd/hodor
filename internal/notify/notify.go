// Package notify provides queued notification delivery for Zitadel.
package notify

import (
	"context"
	"database/sql"
	"net/http"
	"time"

	zcrypto "github.com/zitadel/zitadel/internal/crypto"
	"github.com/zitadel/zitadel/internal/eventbus"
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
