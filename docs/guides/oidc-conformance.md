# OIDC Protocol Compliance

The repository uses a dedicated `OIDC Protocol Compliance` lane for official protocol validation. In the current repo, that means the OpenID Provider conformance lane only.

```bash
just conformance-oidc
just oidc-conformance-op
just conformance-oidc-clean
```

## What Each Target Does

- `just conformance-oidc`
  Runs the canonical OIDC protocol compliance lane. Today this resolves to the OP conformance target.
- `just oidc-conformance-op`
  Runs the official OpenID Foundation conformance suite against Zitadel as an OpenID Provider. The current repo target is the Core Basic static-client profile.
- `just conformance-oidc-clean`
  Stops and removes the local Dockerized conformance stack.

Protocol conformance is intentionally outside the required PR and release walls. CI runs it in `oidc-conformance-daily.yml` on a nightly schedule and exposes the same workflow through `workflow_dispatch` for manual reruns or certification-style checks.

OIDC browser regression coverage now lives in the Journeys family instead of Conformance:

```bash
just journeys-oidc
just journeys-oidc-op
just journeys-oidc-rp
```

In CI, protocol compliance is wired as:

- `Prepare Conformance Image`
- `OIDC Protocol Compliance`

That boundary is intentional: journeys protect Zitadel-specific product behavior, while conformance protects official standards behavior.

## Requirements

- Docker with Compose support
- `git`

The OP lane clones the pinned OIDF suite release into `${XDG_CACHE_HOME:-$HOME/.cache}/hodor/oidc-conformance/` by default, builds the suite JAR with Docker, starts the suite stack, then starts a dedicated Zitadel container on the same Docker network.

## Local Usage

Run only the OP conformance lane:

```bash
just oidc-conformance-op
```

Run the canonical protocol lane:

```bash
just conformance-oidc
```

## Artifacts And Debugging

By default, artifacts are written under:

```text
artifacts/oidc-conformance/
```

Useful environment variables:

- `OIDC_CONFORMANCE_ARTIFACTS_DIR`
- `OIDC_CONFORMANCE_CACHE_DIR`
- `OIDC_CONFORMANCE_KEEP_STACK=1`
- `OIDC_CONFORMANCE_SUITE_REF`
- `OIDC_CONFORMANCE_PROJECT`
- `OIDC_CONFORMANCE_ZITADEL_IMAGE`

`OIDC_CONFORMANCE_ZITADEL_IMAGE` lets CI or local callers provide a prebuilt Docker image for the OP lane instead of rebuilding via `docker compose --build`.

When `OIDC_CONFORMANCE_KEEP_STACK=1` is set, the OP Docker stack stays up after the run for manual debugging.
