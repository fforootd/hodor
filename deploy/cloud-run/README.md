# Cloud Run Deploy

This folder contains the shortest Cloud Run path for a Spanner-backed Zitadel test deploy.

## Approach

Use Cloud Run service identity for Google Cloud access. Do not set `GOOGLE_APPLICATION_CREDENTIALS` inside the service.

Run schema migration and bootstrap once before deployment, then keep Cloud Run runtime startup read-only:

- `ZITADEL_STORAGE__STATEFUL__MIGRATE=check`
- `ZITADEL_STORAGE__STATEFUL__BOOTSTRAP=skip`

For Spanner, pre-create the target database out of band. Zitadel manages schema
inside that database, but does not create or drop the Spanner database
resource.

For your current Spanner setup, keep Cloud Run in `us-west1` if the Spanner instance is also in `us-west1`.

## One-Time Setup

Create the runtime service account:

```bash
PROJECT_ID="ffo-test-27661"
SERVICE_ACCOUNT_NAME="zitadel-run"

gcloud iam service-accounts create "${SERVICE_ACCOUNT_NAME}" \
  --project "${PROJECT_ID}" \
  --display-name "Zitadel Cloud Run"
```

Grant runtime database access:

```bash
PROJECT_ID="ffo-test-27661"
SPANNER_INSTANCE="test"
SPANNER_DATABASE="a1"
SERVICE_ACCOUNT_NAME="zitadel-run"

gcloud spanner databases add-iam-policy-binding "${SPANNER_DATABASE}" \
  --project "${PROJECT_ID}" \
  --instance "${SPANNER_INSTANCE}" \
  --member "serviceAccount:${SERVICE_ACCOUNT_NAME}@${PROJECT_ID}.iam.gserviceaccount.com" \
  --role "roles/spanner.databaseUser"
```

If you still need to initialize schema and bootstrap data, do that separately before Cloud Run deployment:

```bash
cargo run -p zitadel -- db migrate -c fixtures/zitadel.spanner.local.toml --bootstrap
```

## Runtime Env File

By default the deploy script derives `deploy/cloud-run/runtime.env.yaml` from [fixtures/zitadel.spanner.local.toml](/home/ffo/git/fforootd/hodor/fixtures/zitadel.spanner.local.toml).

```bash
PROJECT_ID="ffo-test-27661" \
SERVICE_ACCOUNT_EMAIL="zitadel-run@ffo-test-27661.iam.gserviceaccount.com" \
REGION="us-west1" \
bash deploy/cloud-run/deploy.sh
```

That generation intentionally keeps the shared values from the fixture:

- `server.port`
- `server.cookie_secrets`
- `storage.stateful.backend`
- `storage.stateful.database`

and overrides the local-only parts for Cloud Run:

- `storage.stateful.migrate=check`
- `storage.stateful.bootstrap=skip`
- `observability.cache_path=/tmp/zitadel-cache.db`

The deploy script also sets `ZITADEL_SERVER__PUBLIC_ORIGIN` and `ZITADEL_SERVER__EXTERNAL_DOMAIN` from the final Cloud Run URL after deployment.

For `cookie_secrets`, the generated env uses the flat override `ZITADEL_COOKIE_SECRETS`, not
`ZITADEL_SERVER__COOKIE_SECRETS`. The flat form is required because `server.cookie_secrets` is a
list in the config model.

If you want to override the generated file, either:

```bash
cp deploy/cloud-run/runtime.env.example.yaml deploy/cloud-run/runtime.env.yaml
```

or point the script at a different source TOML:

```bash
SOURCE_CONFIG=/absolute/path/to/zitadel.toml bash deploy/cloud-run/deploy.sh
```

## Deploy

Run:

```bash
PROJECT_ID="YOUR_PROJECT_ID" \
SERVICE_ACCOUNT_EMAIL="zitadel-run@YOUR_PROJECT_ID.iam.gserviceaccount.com" \
SERVICE_NAME="zitadel-test" \
REGION="us-west1" \
bash deploy/cloud-run/deploy.sh
```

Useful optional overrides:

- `ALLOW_UNAUTHENTICATED=false`
- `MIN_INSTANCES=1`
- `CPU=1`
- `MEMORY=1Gi`
- `ENV_FILE=/absolute/path/to/runtime.env.yaml`
- `SOURCE_CONFIG=/absolute/path/to/zitadel.toml`

## What The Script Does

- deploys from the repo root using the checked-in [Dockerfile](/home/ffo/git/fforootd/hodor/Dockerfile)
- uses the provided Cloud Run service account as the runtime identity
- applies the env file with Spanner config
- reads the deployed Cloud Run URL
- updates `ZITADEL_SERVER__PUBLIC_ORIGIN` and `ZITADEL_SERVER__EXTERNAL_DOMAIN` to match that URL

## Notes

- Cloud Run source deploy uses the Dockerfile when present.
- `MIN_INSTANCES=1` is recommended for latency testing so cold starts do not dominate the results.
- If you later want to test cloud-only domain provisioning, add the `ZITADEL_CLOUD__...` variables to `runtime.env.yaml`.
- If you generated `deploy/cloud-run/runtime.env.yaml` before this fix, delete it once so the script regenerates it with `ZITADEL_COOKIE_SECRETS`.
