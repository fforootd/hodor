# Design Decisions

Resolved architecture and product decisions, with rationale.

## Architecture

### D-001: OpenFGA vs Permify → OpenFGA ✅

OpenFGA (CNCF incubating) — vendor-neutral, no competitor supply-chain risk (Permify was acquired by FusionAuth). In-process embedding with `MINIMIZE_LATENCY` mode handles hot-path performance. ~5μs per check, shares the same DB connection.

### D-002: API Versioning → `v1` ✅

Start at `v1`. The unified identity model is fundamentally different from current Zitadel — calling it `v3` sets wrong expectations. Clean break, new product.

### D-003: Multi-Tenancy Model → Shared DB ✅

Shared DB with `org_id` row filtering. DB-per-tenant deferred to when an enterprise contract demands hard isolation.

### D-004: SAML Library → Fork `crewjam/saml` ✅

Writing SAML from scratch is a tar pit of XML canonicalization nightmares. Fork and own. Budget extra time — enterprise SAML quirks always take longer. Run fuzz tests aggressively.

### D-005: Web Components → Lit ✅

Lit over vanilla JS. 5KB overhead is a rounding error. Vanilla JS Web Components become unmaintainable with dynamic state (MFA flows). Google-backed stability.

## Product

### D-006: Passkeys → Both (capability-based) ✅

Passkeys as primary or second factor is an org-level policy toggle in `expr`. The capability model supports both natively.

### D-007: Login Whitelabeling → CSS vars + custom CSS ✅

No full HTML template overrides. Every Zitadel update that adds a new auth method would break custom templates. CSS gives brand control; we keep structural control.

### D-008: SQLite as Production → Yes, first-class ✅

First-class production target. WAL mode handles read-heavy identity workloads. Litestream for replication. This is the DevRel superpower — easiest IAM to self-host.

### D-009: SCIM Scope → Provisioning only ✅

SCIM for inbound user provisioning from enterprise IdPs. REST API for everything else.

### D-010: SLM for Threat Detection → BYOM ✅

Do NOT embed LLM weights in the auth binary. It violates the "single lightweight binary" principle. Provide an OpenAI-compatible endpoint interface. Ship a `docker-compose.yml` with Zitadel + Ollama sidecar. Keep the core binary pure.

### D-011: FGA + expr N+1 Query Trap → Pre-fetch ✅

`expr` policies invoking `fga.check()` on every request creates N+1 queries. **Solution:** pre-fetch authorization context (batch check) BEFORE `expr` evaluation. The `expr` environment receives a pre-computed `authz` map. Zero FGA calls during policy execution.

### D-012: SCIM → Unified Identity Mapping ✅

SCIM is a "dumb translation proxy." When Okta pushes to `/Users`, Zitadel creates an identity with appropriate capabilities. SCIM concepts never leak into the core domain model.

### D-013: Agent Circuit Breakers → First-class ✅

AI agents in infinite loops can cause infrastructure damage. Circuit breakers are a first-class feature:
- Hard quota: "Pause agent if actions > N/hour" (configurable per-agent)
- Auto-revoke on sustained rate limit violations
- Dashboard alerting + webhook notification
- Prominently surfaced in Console UI
