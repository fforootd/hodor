# OIDC Daily Coverage

The repository now has a dedicated daily OpenID coverage path with Make targets:

```bash
make oidc-conformance-op
make oidc-conformance-rp
make oidc-conformance
make oidc-conformance-clean
```

## What Each Target Does

- `make oidc-conformance-op`
  Runs the official OpenID Foundation conformance suite against Zitadel as an OpenID Provider. The current repo target is the Core Basic static-client profile.
- `make oidc-conformance-rp`
  Runs the current RP-focused daily regression lane using the repository's Playwright OIDC RP suite.
- `make oidc-conformance`
  Runs both surfaces by default.
- `make oidc-conformance-clean`
  Stops and removes the local Dockerized conformance stack.

The OP lane is the official protocol conformance lane. The current daily profile is intentionally narrower than full certification: it runs the Core Basic static-client plan through an HTTPS reverse proxy plus a plain HTML conformance login surface. The RP lane is currently the best reproducible daily regression path for the Zitadel RP/broker flow. A dedicated OIDF RP harness and broader OP profiles can be added later without changing the top-level Make entrypoints.

## Requirements

- Docker with Compose support
- `git`
- `npm ci` completed for the workspace when running the RP lane

The OP lane clones the pinned OIDF suite release into `${XDG_CACHE_HOME:-$HOME/.cache}/hodor/oidc-conformance/` by default, builds the suite JAR with Docker, starts the suite stack, then starts a dedicated Zitadel container on the same Docker network.

## Local Usage

Run only the OP conformance lane:

```bash
make oidc-conformance-op
```

Run only the RP daily lane:

```bash
make oidc-conformance-rp
```

Run both:

```bash
make oidc-conformance
```

You can also select the aggregate surface explicitly:

```bash
OIDC_CONFORMANCE_SURFACE=op make oidc-conformance
OIDC_CONFORMANCE_SURFACE=rp make oidc-conformance
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

When `OIDC_CONFORMANCE_KEEP_STACK=1` is set, the OP Docker stack stays up after the run for manual debugging.
