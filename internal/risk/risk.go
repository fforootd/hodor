package risk

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"math"
	"net"
	"strings"
)

const EvaluatorVersion = "builtin-risk/v1"

type Stage string

const (
	StagePreAuth  Stage = "pre_auth"
	StagePostAuth Stage = "post_auth"
)

type Level string

const (
	LevelUnknown Level = "unknown"
	LevelLow     Level = "low"
	LevelMedium  Level = "medium"
	LevelHigh    Level = "high"
)

type Recommendation string

const (
	RecommendationAllow          Recommendation = "allow"
	RecommendationAllowAndLog    Recommendation = "allow_and_log"
	RecommendationRequireCaptcha Recommendation = "require_captcha"
	RecommendationRequireStepUp  Recommendation = "require_step_up"
	RecommendationRequireReauth  Recommendation = "require_reauth"
	RecommendationBlock          Recommendation = "block"
	RecommendationShadowOnly     Recommendation = "shadow_only"
)

type Reason string

const (
	ReasonTrustedReauth          Reason = "trusted_reauth"
	ReasonPasskeyAuth            Reason = "passkey_auth"
	ReasonLowAssuranceAuthMethod Reason = "low_assurance_auth_method"
	ReasonNewFingerprint         Reason = "new_fingerprint"
	ReasonMissingFingerprint     Reason = "missing_fingerprint"
	ReasonHighRecentFailures     Reason = "high_recent_failures"
	ReasonRecentSessionRevoked   Reason = "recent_session_revocations"
	ReasonRecentTokenRevoked     Reason = "recent_token_revocations"
	ReasonSuspiciousPoWTiming    Reason = "suspicious_pow_timing"
	ReasonNewIPOrUA              Reason = "new_ip_or_ua"
)

type Signals struct {
	CaptchaProvider  string  `json:"captcha_provider,omitempty"`
	CaptchaVerified  bool    `json:"captcha_verified,omitempty"`
	CaptchaScore     float64 `json:"captcha_score,omitempty"`
	PoWCompleted     bool    `json:"pow_completed,omitempty"`
	PoWDurationMs    float64 `json:"pow_duration_ms,omitempty"`
	VisitorID        string  `json:"visitor_id,omitempty"`
	FingerprintHash  string  `json:"fingerprint_hash,omitempty"`
	RequestID        string  `json:"request_id,omitempty"`
	DocumentLoadMs   float64 `json:"document_load_ms,omitempty"`
	InteractionCount int     `json:"interaction_count,omitempty"`
}

type Input struct {
	Stage          Stage
	UserID         string
	AuthMethod     string
	ProviderID     string
	ProviderKind   string
	LoginFlowID    string
	IPAddress      string
	UserAgent      string
	TrustedSession bool
	Reauth         bool
	Signals        Signals
}

type Result struct {
	Score               float64        `json:"score"`
	Level               Level          `json:"level"`
	Reasons             []Reason       `json:"reasons,omitempty"`
	RecommendedNextStep Recommendation `json:"recommended_next_step"`
	Stage               Stage          `json:"stage"`
	EvaluatorVersion    string         `json:"evaluator_version"`
}

type Evaluator interface {
	Evaluate(ctx context.Context, input Input) (*Result, error)
}

type Engine struct {
	db *sql.DB
}

func NewEvaluator(db *sql.DB) Evaluator {
	return &Engine{db: db}
}

type historySnapshot struct {
	recentLoginFailures  int
	recentSessionRevokes int
	recentTokenRevokes   int
	knownFingerprint     bool
	newIPOrUA            bool
}

func (e *Engine) Evaluate(ctx context.Context, input Input) (*Result, error) {
	if e.db == nil {
		return nil, errors.New("risk evaluator requires a database")
	}

	history, err := e.loadHistory(ctx, input)
	if err != nil {
		return nil, err
	}

	return evaluate(input, history), nil
}

func (e *Engine) loadHistory(ctx context.Context, input Input) (historySnapshot, error) {
	history := historySnapshot{}
	if input.UserID == "" && input.Signals.VisitorID == "" {
		return history, nil
	}

	var err error
	switch {
	case input.UserID != "":
		history.recentLoginFailures, err = queryCount(ctx, e.db,
			`SELECT COUNT(*) FROM events
			 WHERE event_type = 'auth.login_failed'
			   AND actor_id = ?
			   AND created_at > datetime('now', '-1 hour')`,
			input.UserID,
		)
	default:
		history.recentLoginFailures, err = queryCount(ctx, e.db,
			`SELECT COUNT(*) FROM events
			 WHERE event_type = 'auth.login_failed'
			   AND fingerprint = ?
			   AND created_at > datetime('now', '-1 hour')`,
			input.Signals.VisitorID,
		)
	}
	if err != nil {
		return history, fmt.Errorf("load recent failures: %w", err)
	}

	if input.UserID == "" {
		return history, nil
	}

	history.recentSessionRevokes, err = queryCount(ctx, e.db,
		`SELECT COUNT(*) FROM sessions
		 WHERE user_id = ?
		   AND revoked_at IS NOT NULL
		   AND revoked_at > datetime('now', '-7 day')`,
		input.UserID,
	)
	if err != nil {
		return history, fmt.Errorf("load session revocations: %w", err)
	}

	history.recentTokenRevokes, err = queryCount(ctx, e.db,
		`SELECT COUNT(*) FROM tokens
		 WHERE user_id = ?
		   AND revoked_at IS NOT NULL
		   AND revoked_at > datetime('now', '-7 day')`,
		input.UserID,
	)
	if err != nil {
		return history, fmt.Errorf("load token revocations: %w", err)
	}

	if input.Signals.VisitorID != "" {
		history.knownFingerprint, err = queryExists(ctx, e.db,
			`SELECT 1 FROM events
			 WHERE actor_id = ?
			   AND fingerprint = ?
			   AND created_at > datetime('now', '-30 day')
			 LIMIT 1`,
			input.UserID, input.Signals.VisitorID,
		)
		if err != nil {
			return history, fmt.Errorf("load known fingerprint: %w", err)
		}
	}

	history.newIPOrUA, err = isNewIPOrUA(ctx, e.db, input.UserID, normalizeIPAddress(input.IPAddress), input.UserAgent)
	if err != nil {
		return history, fmt.Errorf("load ip/ua posture: %w", err)
	}

	return history, nil
}

func evaluate(input Input, history historySnapshot) *Result {
	score := 0.50
	var reasons []Reason

	if input.Signals.CaptchaVerified {
		score += 0.15
	}
	if input.Signals.PoWCompleted {
		score += 0.10
	}
	if input.Signals.PoWDurationMs >= 100 && input.Signals.PoWDurationMs <= 30000 {
		score += 0.05
	}
	if input.Signals.PoWDurationMs > 0 && input.Signals.PoWDurationMs < 50 {
		score -= 0.25
		reasons = append(reasons, ReasonSuspiciousPoWTiming)
	}

	if input.Signals.VisitorID != "" {
		score += 0.10
	} else {
		score -= 0.15
		reasons = append(reasons, ReasonMissingFingerprint)
	}
	if input.Signals.RequestID != "" {
		score += 0.05
	}

	if input.Reauth || input.TrustedSession {
		score += 0.15
		reasons = append(reasons, ReasonTrustedReauth)
	}

	switch input.AuthMethod {
	case "passkey":
		score += 0.15
		reasons = append(reasons, ReasonPasskeyAuth)
	case "password", "magic_link", "registration":
		score -= 0.10
		reasons = append(reasons, ReasonLowAssuranceAuthMethod)
	}

	if input.UserID != "" && input.Signals.VisitorID != "" && !history.knownFingerprint {
		score -= 0.10
		reasons = append(reasons, ReasonNewFingerprint)
	}
	if history.recentLoginFailures >= 3 {
		score -= 0.20
		reasons = append(reasons, ReasonHighRecentFailures)
	}
	if history.recentSessionRevokes > 0 {
		score -= 0.10
		reasons = append(reasons, ReasonRecentSessionRevoked)
	}
	if history.recentTokenRevokes > 0 {
		score -= 0.10
		reasons = append(reasons, ReasonRecentTokenRevoked)
	}
	if history.newIPOrUA {
		score -= 0.10
		reasons = append(reasons, ReasonNewIPOrUA)
	}

	score = clamp(score, 0, 1)
	score = math.Round(score*1000) / 1000
	level := levelForScore(score)

	return &Result{
		Score:               score,
		Level:               level,
		Reasons:             dedupeReasons(reasons),
		RecommendedNextStep: recommendationFor(input.Stage, level),
		Stage:               input.Stage,
		EvaluatorVersion:    EvaluatorVersion,
	}
}

func FailureResult(stage Stage, recommendation Recommendation) *Result {
	return &Result{
		Score:               0,
		Level:               LevelUnknown,
		RecommendedNextStep: recommendation,
		Stage:               stage,
		EvaluatorVersion:    EvaluatorVersion,
	}
}

func levelForScore(score float64) Level {
	switch {
	case score >= 0.65:
		return LevelLow
	case score >= 0.40:
		return LevelMedium
	default:
		return LevelHigh
	}
}

func recommendationFor(stage Stage, level Level) Recommendation {
	switch stage {
	case StagePreAuth:
		if level == LevelLow {
			return RecommendationAllow
		}
		return RecommendationRequireCaptcha
	case StagePostAuth:
		if level == LevelLow {
			return RecommendationAllowAndLog
		}
		return RecommendationRequireStepUp
	default:
		return RecommendationShadowOnly
	}
}

func clamp(value, min, max float64) float64 {
	if value < min {
		return min
	}
	if value > max {
		return max
	}
	return value
}

func queryCount(ctx context.Context, db *sql.DB, query string, args ...any) (int, error) {
	var count int
	if err := db.QueryRowContext(ctx, query, args...).Scan(&count); err != nil {
		return 0, err
	}
	return count, nil
}

func queryExists(ctx context.Context, db *sql.DB, query string, args ...any) (bool, error) {
	var value int
	err := db.QueryRowContext(ctx, query, args...).Scan(&value)
	if errors.Is(err, sql.ErrNoRows) {
		return false, nil
	}
	if err != nil {
		return false, err
	}
	return true, nil
}

func isNewIPOrUA(ctx context.Context, db *sql.DB, userID, ipAddress, userAgent string) (bool, error) {
	if userID == "" || (ipAddress == "" && userAgent == "") {
		return false, nil
	}

	var existing int
	if err := db.QueryRowContext(ctx,
		`SELECT COUNT(*) FROM sessions
		 WHERE user_id = ?
		   AND created_at > datetime('now', '-30 day')`,
		userID,
	).Scan(&existing); err != nil {
		return false, err
	}
	if existing == 0 {
		return false, nil
	}

	var matched int
	if err := db.QueryRowContext(ctx,
		`SELECT COUNT(*) FROM sessions
		 WHERE user_id = ?
		   AND created_at > datetime('now', '-30 day')
		   AND (ip_address = ? OR user_agent = ?)`,
		userID, ipAddress, userAgent,
	).Scan(&matched); err != nil {
		return false, err
	}
	return matched == 0, nil
}

func normalizeIPAddress(addr string) string {
	host, _, err := net.SplitHostPort(addr)
	if err == nil {
		addr = host
	}
	return strings.TrimSpace(addr)
}

func dedupeReasons(reasons []Reason) []Reason {
	if len(reasons) == 0 {
		return nil
	}

	seen := make(map[Reason]struct{}, len(reasons))
	deduped := make([]Reason, 0, len(reasons))
	for _, reason := range reasons {
		if _, ok := seen[reason]; ok {
			continue
		}
		seen[reason] = struct{}{}
		deduped = append(deduped, reason)
	}
	return deduped
}
