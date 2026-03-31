package notify

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"net/http"
	"net/smtp"
	"strings"
	"time"

	"github.com/zitadel/zitadel/internal/logging"
)

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

func (s *Service) httpTransport() *http.Client {
	if s.httpClient != nil {
		return s.httpClient
	}
	s.httpClient = &http.Client{Timeout: 10 * time.Second}
	return s.httpClient
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

	resp, err := s.httpTransport().Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	if resp.StatusCode >= 300 {
		return fmt.Errorf("notify: custom_http status %d", resp.StatusCode)
	}
	return nil
}
