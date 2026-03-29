# ADR-023: Wide Events as Internal Observability Primitive

**Status**: Accepted  
**Date**: 2026-03-29  
**Builds on**: ADR-010 (Three-Tier Data), ADR-004 (Apps as Identities), ADR-017 (SDK Generation)  
**Supersedes**: The implicit OTEL-as-internal-model pattern

## Context

ZITADEL needs to answer questions like:

- "Which AI agent accessed user X's data using a delegated token?"
- "Show me all activity from device fingerprint Y across sessions"
- "What did application Z do in the last hour?"
- "Reconstruct the full timeline for this user — direct and delegated access"

OpenTelemetry distributed tracing (spans, parent-child trees) solves a different problem: **latency debugging across microservices**. It can't directly answer business-dimension queries. Our `events` table is already structurally a wide event store — this ADR formalizes that fact and extends it.

## Decision

### 1. OTEL Is Tier 3 Only

OTEL remains exclusively an **export format** for forwarding to an operator's collector (Splunk, Datadog, Grafana, etc.). ZITADEL never reads from OTEL. OTEL naming (`trace_id`, `span_id`) does not appear in the internal data model.

```
Wide Event (internal)  ──→  OTEL Span (Tier 3 export)
                              └─ Customer's collector → their tools
```

### 2. Every Mutation Produces a Wide Event

A wide event is a single flat record with all relevant context attached at write time. No joins required for common queries:

```sql
CREATE TABLE events (
    id              TEXT PRIMARY KEY,
    event_type      TEXT NOT NULL,       -- 'identity.updated', 'request.api', etc.
    category        TEXT NOT NULL,       -- 'entity', 'auth', 'session', 'request', 'signal'
    org_id          TEXT NOT NULL,

    -- WHO
    actor_id        TEXT,                -- user who triggered this
    actor_type      TEXT,                -- 'human', 'service', 'system'

    -- WHAT
    aggregate_id    TEXT,                -- resource affected
    aggregate_type  TEXT,                -- 'identity', 'session', 'org'
    resource_type   TEXT,

    -- HOW (delegation context)
    client_id       TEXT DEFAULT '',     -- OIDC client_id / app that made the call
    token_id        TEXT DEFAULT '',     -- specific token used
    delegation_type TEXT DEFAULT '',     -- 'direct', 'delegated', 'pat_shared', 'exchanged'
    sdk_name        TEXT DEFAULT '',     -- 'zitadel-js', 'zitadel-go'
    sdk_version     TEXT DEFAULT '',     -- '1.4.0'

    -- WHERE (device context)
    fingerprint     TEXT DEFAULT '',     -- device fingerprint

    -- WHEN (correlation scopes)
    request_id      TEXT,                -- all events from one HTTP request (W3C traceparent compatible)
    session_id      TEXT,                -- all events from one user session
    flow_id         TEXT,                -- all events from one login flow

    -- Arbitrary data
    payload         TEXT DEFAULT '{}',   -- event-specific structured data
    metadata        TEXT DEFAULT '{}',   -- overflow: span_id, parent_span_id, SDK context

    sequence        INTEGER,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    shipped_at      TEXT
);
```

### 3. Six Orthogonal Correlation Scopes

Every event participates in six independent grouping dimensions:

| Scope | Column | Groups |
|-------|--------|--------|
| **Request** | `request_id` | All events from a single HTTP request |
| **Session** | `session_id` | All events from a user session |
| **Flow** | `flow_id` | All events from a login flow |
| **Device** | `fingerprint` | All events from a device |
| **App** | `client_id` | All events from an application or AI agent |
| **User** | `actor_id` | All events by a user |

Admin queries use these scopes directly:

```sql
-- What did the AI agent do?
SELECT * FROM events WHERE client_id = 'agent_copilot' ORDER BY created_at;

-- All delegated access for user X
SELECT * FROM events WHERE actor_id = 'user_123' AND delegation_type != 'direct';

-- All activity from this device
SELECT * FROM events WHERE fingerprint = 'fp_abc123' ORDER BY created_at;

-- Everything in this login flow
SELECT * FROM events WHERE flow_id = 'flow_xyz' ORDER BY created_at;
```

### 4. OTEL Concepts Demoted from Top-Level

| Old column | Disposition |
|------------|------------|
| `trace_id` | **Renamed** to `request_id` — same 128-bit hex value, W3C traceparent compatible |
| `span_id` | **Demoted** to `metadata` JSON — the event `id` is the unit of work |
| `parent_span_id` | **Demoted** to `metadata` JSON — span trees are an OTEL export concern |
| `X-Trace-Id` header | **Renamed** to `X-Request-Id` |

When exporting to OTEL (Tier 3), the projector maps: `request_id → trace_id`, `event.id → span_id`, `metadata.parent_span_id → parent_span_id`.

### 5. SDK Reports via Token Resolution, Not Self-Declaration

The SDK does **not** declare its own identity. All context is resolved server-side from the token:

| Dimension | Source | Validation |
|-----------|--------|------------|
| `client_id` | `tokens.application_id` | Guaranteed by token issuance |
| `actor_id` | `tokens.user_id` | Guaranteed by token issuance |
| `delegation_type` | Inferred from token structure (see §6) | Automatic |
| `sdk_name`, `sdk_version` | SDK sends `X-SDK-Name` / `X-SDK-Version` headers | Informational only, not trusted for authz |

The SDK's `X-Client-Id` header is **ignored** — the server resolves `client_id` from the token itself. This prevents spoofing.

### 6. Three Delegation Models

All three delegation mechanisms are supported, each producing a different `delegation_type`:

| Mechanism | `delegation_type` | `client_id` source | `actor_id` source |
|-----------|-------------------|-------------------|-------------------|
| OIDC with `act` claim | `delegated` | `tokens.application_id` | `act.sub` claim |
| PAT shared with agent | `pat_shared` | `tokens.on_behalf_of_app` | `tokens.user_id` |
| Token Exchange (RFC 8693) | `exchanged` | Requesting client's `client_id` | Original `subject_token`'s user |
| Direct (no delegation) | `direct` | `tokens.application_id` | `tokens.user_id` |

`delegation_type` is inferred from the token automatically — no SDK configuration needed.

For PATs, an optional `on_behalf_of_app` column is added to the `tokens` table, validated against `apps.client_id` at PAT creation time.

### 7. OTelMiddleware Becomes RequestContextMiddleware

The middleware renames to reflect its actual purpose — enriching the request context, not injecting OTEL concepts:

```go
func RequestContextMiddleware(next http.Handler) http.Handler {
    return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
        ctx := r.Context()

        // Extract or generate request ID (W3C traceparent compatible).
        requestID, incomingSpanID := extractTraceparent(r)
        if requestID == "" {
            requestID = generateRequestID() // 128-bit hex
        }
        ctx = telemetry.WithRequestID(ctx, requestID)

        // Stash incoming span info in metadata context (for OTEL export only).
        if incomingSpanID != "" {
            ctx = telemetry.WithMetadata(ctx, "parent_span_id", incomingSpanID)
        }

        // Session, flow, fingerprint — unchanged.
        if sessionID := r.Header.Get("X-Session-Id"); sessionID != "" {
            ctx = telemetry.WithSessionID(ctx, sessionID)
        }
        if flowID := r.Header.Get("X-Flow-Id"); flowID != "" {
            ctx = telemetry.WithFlowID(ctx, flowID)
        }
        if fp := r.Header.Get("X-Fingerprint"); fp != "" {
            ctx = telemetry.WithFingerprint(ctx, fp)
        }

        // SDK info (informational, not validated).
        if sdk := r.Header.Get("X-SDK-Name"); sdk != "" {
            ctx = telemetry.WithSDKName(ctx, sdk)
            ctx = telemetry.WithSDKVersion(ctx, r.Header.Get("X-SDK-Version"))
        }

        w.Header().Set("X-Request-Id", requestID)
        next.ServeHTTP(w, r.WithContext(ctx))
    })
}
```

### 8. AuthGate Enriches Delegation Context

AuthGate resolves the token and injects delegation dimensions into context:

```go
// In AuthGate, after resolveToken():
ctx = telemetry.WithClientID(ctx, info.ApplicationID)
ctx = telemetry.WithTokenID(ctx, info.TokenID)
ctx = telemetry.WithDelegationType(ctx, info.DelegationType) // inferred from token
```

`emitEvent()` then reads all dimensions from context — no per-handler changes needed.

## Consequences

- **Business-dimension queries** work with simple WHERE clauses — no trace tree traversal
- **AI agent activity is visible** — `WHERE client_id = ? AND delegation_type = 'delegated'`
- **Zero OTEL vocabulary** in the internal data model — OTEL is a Tier 3 export format
- **schema change**: `trace_id` → `request_id`, `span_id`/`parent_span_id` → `metadata` JSON
- **Two dropped columns**, five new columns (net +3 top-level columns)
- **SDK reports passively** — no special event endpoint needed for API calls
- **W3C compat preserved** — `request_id` is still a 128-bit hex from traceparent, OTEL export maps it back
- **`tokens` table change** — optional `on_behalf_of_app` column for PAT delegation
