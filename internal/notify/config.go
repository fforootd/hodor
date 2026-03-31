package notify

import (
	"context"
	"encoding/json"
	"fmt"
	"sort"
	"strings"

	"github.com/zitadel/zitadel/internal/settings"
)

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
	if len(cfg.Email.Channels) == 0 {
		defaults := defaultNotificationConfig()
		cfg.Email = defaults.Email
	}
	if len(cfg.SMS.Channels) == 0 {
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
