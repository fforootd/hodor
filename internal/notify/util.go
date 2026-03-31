package notify

import (
	"strings"
	"time"
)

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
