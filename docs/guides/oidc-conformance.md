# Compliance OIDC

The repository uses a dedicated `Compliance OIDC` lane for protocol-focused coverage. The local entrypoints stay the same:

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
  Runs the current RP-focused regression lane using the repository's Playwright OIDC RP suite.
- `make oidc-conformance`
  Runs both surfaces by default.
- `make oidc-conformance-clean`
  Stops and removes the local Dockerized conformance stack.

The OP lane is the official protocol conformance lane. The current profile is intentionally narrower than full certification: it runs the Core Basic static-client plan through an HTTPS reverse proxy plus a plain HTML conformance login surface. The RP lane is the current reproducible regression path for the Zitadel RP and broker flow. A dedicated OIDF RP harness and broader OP profiles can be added later without changing the top-level Make entrypoints.

In CI, the lane is wired as:
- `Prepare Rust Binary` for the RP surface
- `Prepare Conformance Image` for the OP surface
- `Compliance OIDC` for the final aggregated run, summary, and artifacts

This keeps protocol coverage under one domain name while still letting the OP and RP surfaces consume the correct prepared output.

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
- `OIDC_CONFORMANCE_ZITADEL_IMAGE`
- `ZITADEL_E2E_BINARY`

`OIDC_CONFORMANCE_ZITADEL_IMAGE` lets CI or local callers provide a prebuilt Docker image for the OP lane instead of rebuilding via `docker compose --build`.

`ZITADEL_E2E_BINARY` lets CI or local callers provide a prepared Zitadel binary for the RP lane instead of rebuilding with `cargo build -p zitadel`.

When `OIDC_CONFORMANCE_KEEP_STACK=1` is set, the OP Docker stack stays up after the run for manual debugging.
