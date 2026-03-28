package logging

import (
	"fmt"
	"os"
	"strings"
)

// --- Backward compatibility layer ---
//
// These functions provide drop-in replacements for Go's stdlib log.Printf,
// log.Println, and log.Fatalf. They route through the logging infrastructure
// (redaction, fan-out, circuit breakers) while accepting the same signatures.
//
// Use these during migration. New code should prefer the structured API:
//
//	logger := logging.New(logging.StreamRuntime)
//	logger.Info("message", "key", "value")

// Printf logs a formatted message at Info level on the runtime stream.
// Drop-in replacement for log.Printf.
func Printf(format string, args ...any) {
	msg := fmt.Sprintf(format, args...)
	// Strip common prefix tags like "[scheduler]" and use them as component attrs.
	component, cleaned := extractTag(msg)
	logger := New(streamForComponent(component))
	if component != "" {
		logger.Info(cleaned, "component", component)
	} else {
		logger.Info(cleaned)
	}
}

// Println logs a message at Info level on the runtime stream.
// Drop-in replacement for log.Println.
func Println(args ...any) {
	msg := fmt.Sprint(args...)
	component, cleaned := extractTag(msg)
	logger := New(streamForComponent(component))
	if component != "" {
		logger.Info(cleaned, "component", component)
	} else {
		logger.Info(cleaned)
	}
}

// Fatalf logs a formatted message at Error level and exits.
// Drop-in replacement for log.Fatalf.
func Fatalf(format string, args ...any) {
	msg := fmt.Sprintf(format, args...)
	New(StreamRuntime).Error(msg)
	os.Exit(1)
}

// --- Helpers ---

// extractTag pulls out a bracketed prefix like "[scheduler]" from a log message.
// Returns the tag (without brackets) and the remaining message.
func extractTag(msg string) (string, string) {
	if !strings.HasPrefix(msg, "[") {
		return "", msg
	}
	idx := strings.Index(msg, "]")
	if idx < 0 {
		return "", msg
	}
	tag := msg[1:idx]
	rest := strings.TrimSpace(msg[idx+1:])
	return tag, rest
}

// streamForComponent maps common component tags to streams.
func streamForComponent(component string) Stream {
	switch strings.ToLower(component) {
	case "scheduler", "session_gc", "event_gc":
		return StreamJobs
	case "flow", "login", "sso", "magic-link", "claims":
		return StreamRequest
	case "ratelimit", "actions":
		return StreamRequest
	default:
		return StreamRuntime
	}
}
