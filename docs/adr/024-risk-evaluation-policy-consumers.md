# ADR-024: Risk Evaluation and Policy Consumers

**Status**: Accepted  
**Date**: 2026-03-30  
**Builds on**: ADR-009 (Settings & Engine Pipeline), ADR-021 (Login Flow Schema), ADR-023 (Wide Events)

## Context

ZITADEL already models `pre_auth` and `post_auth` hook points, adaptive CAPTCHA modes, fingerprinting, and wide events. What it does not yet define is the contract between:

1. The component that **evaluates risk**
2. The component that **applies policy**
3. The event stream that makes those decisions observable and queryable over time

If the evaluator also owns allow/deny behavior, we lose reuse and make future consumers harder to add. If UI components interpret the result directly, we push security policy into the wrong place.

## Decision

### 1. Risk Evaluation Is a Built-In Core Runtime Capability

Risk evaluation lives in the core binary as a deterministic local capability. It is not a marketplace plugin and it does not require a network dependency in the request path.

The evaluator normalizes inputs from:

- request context
- login flow signals
- device fingerprint
- auth method and provider context
- trust context such as reauthentication
- bounded historical features from local OLTP tables and wide events

### 2. The Evaluator Returns a Reusable Result, Not a Final Verdict

The evaluator returns a stable `RiskResult`:

```go
type RiskResult struct {
    Score               float64
    Level               "low" | "medium" | "high" | "unknown"
    Reasons             []RiskReason
    RecommendedNextStep RiskRecommendation
    Stage               "pre_auth" | "post_auth"
    EvaluatorVersion    string
}
```

`RecommendedNextStep` is advisory output intended for reuse across multiple consumers:

- `allow`
- `allow_and_log`
- `require_captcha`
- `require_step_up`
- `require_reauth`
- `block`
- `shadow_only`

### 3. Policy Lives With the Hook Consumer

The **hook consumer** applies enforcement policy.

- `pre_auth` consumer: adaptive CAPTCHA gating
- `post_auth` consumer: session metadata enrichment and future step-up decisions
- later consumers may exist at `on_token` or async intelligence stages

UI components remain dumb renderers. They receive server-derived state such as `captcha_required`; they do not interpret `RiskResult` into allow/deny behavior.

### 4. Marketplace Provides Policy Data, Not Auth-Path Code

Marketplace content may ship:

- threshold profiles
- scoring presets
- policy mappings
- action/catalog templates

Marketplace content does **not** ship executable auth-path logic. The core evaluator owns trusted signal normalization and request-path execution.

### 5. Every Completed Evaluation Emits an Observation Event

Every completed evaluation emits:

- `signal.risk_evaluated`

This is an **observation** event, not a confirmed security verdict. The payload includes:

- `score`
- `level`
- `reasons`
- `recommended_next_step`
- `stage`
- `evaluator_version`
- optional policy metadata such as `policy_name` and `policy_version`

Correlation comes from the existing wide-event columns:

- `request_id`
- `session_id`
- `flow_id`
- `fingerprint`
- `actor_id`
- `client_id`
- `token_id`

Later confirmed detections may emit `threat.*` events such as `threat.detected` or `threat.classified`.

### 6. Chained Evaluation Is Reserved by the Contract

The contract supports a future chained model:

1. fast built-in evaluator in the request path
2. optional deeper evaluator for gray-zone or high-impact cases
3. shadow-mode rollout before any stronger enforcement

Future chained evaluators may include local secondary engines or external SLM-backed verifiers, but v1 remains fully local and deterministic.

## Consequences

- **Reusable contract**: one evaluator can serve CAPTCHA, session posture, and later step-up/reauth consumers
- **Correct boundary**: policy stays server-side, not in UI
- **Safe request path**: no external dependency required for v1
- **Historical explainability**: every evaluation is queryable via `signal.risk_evaluated`
- **Marketplace extensibility**: profiles and templates can evolve without changing the runtime boundary
