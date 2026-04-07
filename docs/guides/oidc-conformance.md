# OIDC Protocol Compliance

The repository uses a dedicated `OIDC Protocol Compliance` lane for official protocol validation. In the current repo, that means the OpenID Provider conformance lane only.

```bash
./conformance/oidc/scripts/run-op.sh      # run OP conformance
./conformance/oidc/scripts/clean.sh        # stop and remove the conformance stack
```

## What Each Target Does

- `./conformance/oidc/scripts/run-op.sh`
  Runs the official OpenID Foundation conformance suite against Zitadel as an OpenID Provider. The current repo target is the Core Basic static-client profile. This is also the canonical OIDC protocol compliance lane.
- `./conformance/oidc/scripts/clean.sh`
  Stops and removes the local Dockerized conformance stack.

Protocol conformance is intentionally outside the required PR and release walls. CI runs it in `oidc-conformance-daily.yml` on a nightly schedule and exposes the same workflow through `workflow_dispatch` for manual reruns or certification-style checks.

OIDC browser regression coverage now lives in the Journeys family instead of Conformance:

```bash
npm test -w browser-tests -- --project=journeys-login-oidc                                          # all OIDC journeys
npm test -w browser-tests -- --project=journeys-login-oidc journeys/login/oidc-code-pkce.spec.ts    # OP journeys
npm test -w browser-tests -- --project=journeys-login-oidc journeys/login/oidc-rp.spec.ts           # RP journeys
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

Run the OP conformance lane:

```bash
./conformance/oidc/scripts/run-op.sh
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
