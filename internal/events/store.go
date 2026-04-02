package events

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"strings"
	"time"

	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/id"
	"github.com/zitadel/zitadel/internal/telemetry"
)

type writer interface {
	ExecContext(context.Context, string, ...any) (sql.Result, error)
	InstanceID() string
	Rebind(string) string
}

type Store struct {
	db *database.DB
}

func NewStore(db *database.DB) *Store {
	return &Store{db: db}
}

type Event struct {
	ID             string
	EventType      string
	OrgID          string
	ActorID        string
	ActorType      string
	AggregateID    string
	AggregateType  string
	RequestID      string
	SessionID      string
	FlowID         string
	Fingerprint    string
	ClientID       string
	TokenID        string
	DelegationType string
	SDKName        string
	SDKVersion     string
	Payload        any
	Metadata       any
	CreatedAt      string
}

type AggregateRow struct {
	Dimensions map[string]string
	Count      int64
}

type Filter struct {
	Cursor         string
	Limit          int
	OrgID          string
	AggregateType  string
	AggregateID    string
	SessionID      string
	Fingerprint    string
	ClientID       string
	DelegationType string
	Since          string
	Types          []string
}

func Append(ctx context.Context, db writer, eventType, actorID, aggregateID, aggregateType string, payload map[string]any) error {
	eventID := id.New()
	payloadJSON := "{}"
	if len(payload) > 0 {
		b, err := json.Marshal(payload)
		if err != nil {
			return fmt.Errorf("marshal event payload: %w", err)
		}
		payloadJSON = string(b)
	}

	createdAt := time.Now().UTC().Format(time.RFC3339)
	_, err := db.ExecContext(ctx, db.Rebind(
		`INSERT INTO events (instance_id, id, event_type, category, org_id, actor_id, actor_type, aggregate_id, aggregate_type, payload, metadata, request_id, session_id, flow_id, fingerprint, client_id, token_id, delegation_type, sdk_name, sdk_version, created_at)
		 VALUES (?, ?, ?, ?, '0', ?, '', ?, ?, ?, '{}', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`),
		db.InstanceID(),
		eventID,
		eventType,
		Category(eventType),
		actorID,
		aggregateID,
		aggregateType,
		payloadJSON,
		telemetry.RequestIDFromContext(ctx),
		telemetry.SessionIDFromContext(ctx),
		telemetry.FlowIDFromContext(ctx),
		telemetry.FingerprintFromContext(ctx),
		telemetry.ClientIDFromContext(ctx),
		telemetry.TokenIDFromContext(ctx),
		telemetry.DelegationTypeFromContext(ctx),
		telemetry.SDKNameFromContext(ctx),
		telemetry.SDKVersionFromContext(ctx),
		createdAt,
	)
	if err != nil {
		return fmt.Errorf("append event: %w", err)
	}
	return nil
}

func Category(eventType string) string {
	for i := 0; i < len(eventType); i++ {
		if eventType[i] == '.' {
			prefix := eventType[:i]
			switch prefix {
			case "entity", "identity", "provider", "settings", "schema":
				return "entity"
			case "auth":
				return "auth"
			case "session":
				return "session"
			case "token":
				return "token"
			case "request", "api":
				return "request"
			case "log":
				return "log"
			case "signal":
				return "signal"
			case "threat":
				return "threat"
			case "notification":
				return "system"
			}
			return prefix
		}
	}
	return "system"
}

func (s *Store) List(ctx context.Context, filter Filter) ([]Event, string, error) {
	scoped := s.db.Scoped(ctx)
	limit := filter.Limit
	if limit <= 0 {
		limit = 100
	}
	if limit > 1000 {
		limit = 1000
	}

	query := `SELECT id, event_type, org_id, actor_id, actor_type,
	                 aggregate_id, aggregate_type, payload, metadata, created_at,
	                 request_id, session_id, flow_id, fingerprint,
	                 client_id, token_id, delegation_type, sdk_name, sdk_version
	          FROM events WHERE instance_id = ? AND id > ?`
	args := []any{scoped.InstanceID(), filter.Cursor}

	params := map[string]string{
		"org_id":          filter.OrgID,
		"aggregate_type":  filter.AggregateType,
		"aggregate_id":    filter.AggregateID,
		"session_id":      filter.SessionID,
		"fingerprint":     filter.Fingerprint,
		"client_id":       filter.ClientID,
		"delegation_type": filter.DelegationType,
		"created_at >= ?": filter.Since,
	}
	for col, val := range params {
		if val == "" {
			continue
		}
		if strings.Contains(col, "?") {
			query += " AND " + col
		} else {
			query += fmt.Sprintf(" AND %s = ?", col)
		}
		args = append(args, val)
	}

	if len(filter.Types) > 0 {
		placeholders := make([]string, 0, len(filter.Types))
		for _, eventType := range filter.Types {
			eventType = strings.TrimSpace(eventType)
			if eventType == "" {
				continue
			}
			placeholders = append(placeholders, "?")
			args = append(args, eventType)
		}
		if len(placeholders) > 0 {
			query += fmt.Sprintf(" AND event_type IN (%s)", strings.Join(placeholders, ","))
		}
	}

	query += ` ORDER BY id ASC LIMIT ?`
	args = append(args, limit+1)

	rows, err := scoped.QueryContext(ctx, scoped.Rebind(query), args...)
	if err != nil {
		return nil, "", fmt.Errorf("query events: %w", err)
	}
	defer rows.Close()

	items, err := scanRows(rows)
	if err != nil {
		return nil, "", err
	}
	nextCursor := ""
	if len(items) > limit {
		nextCursor = items[limit-1].ID
		items = items[:limit]
	}
	return items, nextCursor, nil
}

func (s *Store) ListAfter(ctx context.Context, cursor string, limit int) ([]Event, error) {
	items, _, err := s.List(ctx, Filter{Cursor: cursor, Limit: limit})
	return items, err
}

func (s *Store) AggregateCountsByEventType(ctx context.Context, orgID string) ([]AggregateRow, error) {
	scoped := s.db.Scoped(ctx)
	rows, err := scoped.QueryContext(ctx,
		scoped.Rebind(`SELECT event_type, COUNT(*) as cnt FROM events WHERE instance_id = ? AND org_id = ? GROUP BY event_type`),
		scoped.InstanceID(),
		orgID,
	)
	if err != nil {
		return nil, fmt.Errorf("aggregate events: %w", err)
	}
	defer rows.Close()

	var result []AggregateRow
	for rows.Next() {
		var key string
		var count int64
		if err := rows.Scan(&key, &count); err != nil {
			return nil, fmt.Errorf("scan aggregate row: %w", err)
		}
		result = append(result, AggregateRow{
			Dimensions: map[string]string{"event_type": key},
			Count:      count,
		})
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("iterate aggregate rows: %w", err)
	}
	return result, nil
}

func (s *Store) MaxID(ctx context.Context) (string, error) {
	scoped := s.db.Scoped(ctx)
	var cursor string
	if err := scoped.QueryRowContext(ctx, scoped.Rebind(`SELECT COALESCE(MAX(id), '') FROM events WHERE instance_id = ?`), scoped.InstanceID()).Scan(&cursor); err != nil {
		return "", fmt.Errorf("max event id: %w", err)
	}
	return cursor, nil
}

func scanRows(rows *sql.Rows) ([]Event, error) {
	var items []Event
	for rows.Next() {
		event, err := scanRow(rows)
		if err != nil {
			return nil, err
		}
		items = append(items, event)
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("iterate event rows: %w", err)
	}
	if items == nil {
		items = []Event{}
	}
	return items, nil
}

func scanRow(rows *sql.Rows) (Event, error) {
	var event Event
	var payloadStr, metadataStr string
	var requestID, sessionID, flowID, fingerprint *string
	var clientID, tokenID, delegationType, sdkName, sdkVersion *string
	if err := rows.Scan(
		&event.ID,
		&event.EventType,
		&event.OrgID,
		&event.ActorID,
		&event.ActorType,
		&event.AggregateID,
		&event.AggregateType,
		&payloadStr,
		&metadataStr,
		&event.CreatedAt,
		&requestID,
		&sessionID,
		&flowID,
		&fingerprint,
		&clientID,
		&tokenID,
		&delegationType,
		&sdkName,
		&sdkVersion,
	); err != nil {
		return event, fmt.Errorf("scan event row: %w", err)
	}
	if requestID != nil {
		event.RequestID = *requestID
	}
	if sessionID != nil {
		event.SessionID = *sessionID
	}
	if flowID != nil {
		event.FlowID = *flowID
	}
	if fingerprint != nil {
		event.Fingerprint = *fingerprint
	}
	if clientID != nil {
		event.ClientID = *clientID
	}
	if tokenID != nil {
		event.TokenID = *tokenID
	}
	if delegationType != nil {
		event.DelegationType = *delegationType
	}
	if sdkName != nil {
		event.SDKName = *sdkName
	}
	if sdkVersion != nil {
		event.SDKVersion = *sdkVersion
	}
	_ = json.Unmarshal([]byte(payloadStr), &event.Payload)
	_ = json.Unmarshal([]byte(metadataStr), &event.Metadata)
	return event, nil
}
