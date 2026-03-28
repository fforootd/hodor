// Package id provides unique ID generation using UUIDv7 (RFC 9562).
// All IDs are time-ordered, globally unique strings requiring no
// coordination (no machine ID, no global state).
//
// This is the SINGLE source of truth for all ID generation in Zitadel.
// Every new resource ID — entities, sessions, tokens, events, flows —
// must be minted through this package.
package id

import "github.com/google/uuid"

// New generates a new UUIDv7 string.
// UUIDv7 embeds a millisecond-precision Unix timestamp, ensuring
// chronological sort order and B-tree insert locality.
func New() string {
	return uuid.Must(uuid.NewV7()).String()
}

// --- Typed constructors for prefixed IDs ---
// These centralise the prefix conventions so callers don't scatter
// fmt.Sprintf("flow_%d", ...) patterns across the codebase.

// NewFlow returns a login flow ID: "flow_<uuid>".
func NewFlow() string { return "flow_" + New() }

// NewLoginSession returns a login-session ID: "ls_<uuid>".
func NewLoginSession() string { return "ls_" + New() }

// NewSSEConsumer returns an SSE consumer label: "sse-<uuid>".
func NewSSEConsumer() string { return "sse-" + New() }
