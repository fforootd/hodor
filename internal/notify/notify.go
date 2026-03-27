// Package notify provides notification channels for Zitadel.
// The default channel logs to stdout, but can be swapped for
// SMTP, webhook, or any other implementation.
package notify

import (
	"fmt"
	"log"
	"time"
)

// Channel is the interface for sending notifications.
type Channel interface {
	Send(to, subject, body string) error
}

// StdoutChannel logs notifications to stdout. Default for development.
type StdoutChannel struct{}

func (s *StdoutChannel) Send(to, subject, body string) error {
	log.Printf("[notify] ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")
	log.Printf("[notify]  To:      %s", to)
	log.Printf("[notify]  Subject: %s", subject)
	log.Printf("[notify]  Body:    %s", body)
	log.Printf("[notify] ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")
	return nil
}

// NewStdout returns the default stdout notification channel.
func NewStdout() Channel {
	return &StdoutChannel{}
}

// FormatMagicLink builds a magic link email body.
func FormatMagicLink(baseURL, token string, expiresAt time.Time) string {
	link := fmt.Sprintf("%s/v1/auth/magic-link/verify?token=%s", baseURL, token)
	return fmt.Sprintf(
		"Click the link below to sign in:\n\n  %s\n\nThis link expires at %s.",
		link,
		expiresAt.Format(time.RFC3339),
	)
}
