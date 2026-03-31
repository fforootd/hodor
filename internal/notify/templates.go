package notify

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"html"
	"strings"
	"text/template"

	"github.com/zitadel/zitadel/internal/settings"
)

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

// EscapeHTML is used by the Console when previewing raw text inside HTML cards.
func EscapeHTML(v string) string {
	return html.EscapeString(v)
}
