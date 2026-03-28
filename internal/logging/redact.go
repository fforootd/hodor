package logging

import (
	"crypto/sha256"
	"fmt"
	"log/slog"
	"net"
	"strings"
)

// IPMode controls how IP addresses are handled in log output.
type IPMode string

const (
	IPKeep   IPMode = "keep"   // no change
	IPRedact IPMode = "redact" // replace with mask string
	IPHash   IPMode = "hash"   // SHA-256 → hex[:16] (consistent, non-reversible)
	IPMask   IPMode = "mask"   // 192.168.1.100 → 192.168.x.x / ::1 → ::x
)

// ipKeyFragments are substrings that identify IP address fields.
var ipKeyFragments = []string{"ip", "remote_addr", "x_forwarded", "client_ip"}

// Redactor masks values of sensitive fields in log records.
// It inspects every attribute's key and, if it matches a configured sensitive
// key, replaces its value with the mask string.
//
// Matching is case-insensitive and checks for substring containment,
// so "client_secret" matches a key named "oidc_client_secret_hash".
type Redactor struct {
	keys   map[string]bool // lowercased sensitive key fragments
	mask   string
	ipMode IPMode
}

// NewRedactor creates a Redactor from a list of sensitive key fragments.
// If mask is empty, "***REDACTED***" is used.
func NewRedactor(keys []string, mask string) *Redactor {
	if mask == "" {
		mask = "***REDACTED***"
	}
	m := make(map[string]bool, len(keys))
	for _, k := range keys {
		m[strings.ToLower(strings.TrimSpace(k))] = true
	}
	return &Redactor{keys: m, mask: mask, ipMode: IPKeep}
}

// NewRedactorWithIP creates a Redactor with IP redaction mode.
func NewRedactorWithIP(keys []string, mask string, ipMode string) *Redactor {
	r := NewRedactor(keys, mask)
	switch IPMode(ipMode) {
	case IPRedact, IPHash, IPMask:
		r.ipMode = IPMode(ipMode)
	default:
		r.ipMode = IPKeep
	}
	return r
}

// IsSensitive checks if a key matches any sensitive key fragment.
func (r *Redactor) IsSensitive(key string) bool {
	if len(r.keys) == 0 {
		return false
	}
	lower := strings.ToLower(key)
	for k := range r.keys {
		if strings.Contains(lower, k) {
			return true
		}
	}
	return false
}

// isIPField checks if a key looks like it contains an IP address.
func isIPField(key string) bool {
	lower := strings.ToLower(key)
	for _, frag := range ipKeyFragments {
		if strings.Contains(lower, frag) {
			return true
		}
	}
	return false
}

// RedactIP applies the configured IP redaction mode to an IP address string.
func (r *Redactor) RedactIP(ip string) string {
	switch r.ipMode {
	case IPRedact:
		return r.mask
	case IPHash:
		h := sha256.Sum256([]byte(ip))
		return fmt.Sprintf("%x", h[:8]) // 16 hex chars
	case IPMask:
		return maskIP(ip)
	default:
		return ip
	}
}

// maskIP replaces the last meaningful octets with "x".
// IPv4: 192.168.1.100 → 192.168.x.x
// IPv6: 2001:db8::1 → 2001:db8::x
func maskIP(ip string) string {
	parsed := net.ParseIP(ip)
	if parsed == nil {
		// Not a valid IP — mask the whole thing.
		return "x.x.x.x"
	}

	if parsed.To4() != nil {
		// IPv4: mask last 2 octets.
		parts := strings.Split(ip, ".")
		if len(parts) == 4 {
			return parts[0] + "." + parts[1] + ".x.x"
		}
		return "x.x.x.x"
	}

	// IPv6: mask everything after the first 4 groups.
	parts := strings.Split(parsed.String(), ":")
	if len(parts) > 4 {
		result := strings.Join(parts[:4], ":")
		return result + "::x"
	}
	return "::x"
}

// RedactValue returns the masked value if the key is sensitive,
// otherwise returns the original value.
func (r *Redactor) RedactValue(key string, val slog.Value) slog.Value {
	if r.IsSensitive(key) {
		return slog.StringValue(r.mask)
	}

	// Handle IP redaction.
	if r.ipMode != IPKeep && isIPField(key) && val.Kind() == slog.KindString {
		return slog.StringValue(r.RedactIP(val.String()))
	}

	// Recurse into groups.
	if val.Kind() == slog.KindGroup {
		attrs := val.Group()
		redacted := make([]slog.Attr, len(attrs))
		for i, a := range attrs {
			redacted[i] = slog.Attr{
				Key:   a.Key,
				Value: r.RedactValue(a.Key, a.Value),
			}
		}
		return slog.GroupValue(redacted...)
	}

	return val
}

// RedactRecord returns a copy of the record with sensitive fields masked.
func (r *Redactor) RedactRecord(record slog.Record) slog.Record {
	if len(r.keys) == 0 && r.ipMode == IPKeep {
		return record
	}

	redacted := slog.NewRecord(record.Time, record.Level, record.Message, record.PC)
	record.Attrs(func(a slog.Attr) bool {
		redacted.AddAttrs(slog.Attr{
			Key:   a.Key,
			Value: r.RedactValue(a.Key, a.Value),
		})
		return true
	})
	return redacted
}
