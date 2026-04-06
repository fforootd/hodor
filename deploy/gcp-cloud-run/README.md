# Deploy Zitadel Cloud Mode to Google Cloud Run + Spanner

This deploy target provisions a single-region Zitadel cloud-mode stack on Google Cloud with:

- one Cloud Run service for the app
- one Cloud Run job for `zitadel db migrate --bootstrap`
- one Spanner instance and database for stateful data plus cloud routing
- one global external HTTPS load balancer in front of Cloud Run
- wildcard certificate management for `*.${platform_domain}` through Google Certificate Manager
- one Cloud DNS managed zone with a wildcard `A` record pointing at the load balancer
- a bootstrap script that makes `root.${platform_domain}` reachable and creates `demo.${platform_domain}`

The runtime intentionally leaves `server.public_origin` blank so OIDC, login, and `/openapi.json` derive their origin from the incoming request host in cloud mode.

## Architecture

```text
Client
  -> Global external HTTPS load balancer
     -> Serverless NEG
        -> Cloud Run service (zitadel start)
           -> Spanner database

Bootstrap flow
  -> Cloud Run job (zitadel db migrate --bootstrap)
  -> Spanner row insert for root.<platform_domain> -> default
  -> Root API call to create demo.<platform_domain>
```

## Prerequisites

- A Google Cloud project with billing enabled
- `gcloud`, `terraform`, `jq`, and `curl`
- Docker for building the repo image
- Control of the registrar or parent DNS for `platform_domain`

## Files

- [main.tf](/Users/ffo/git/fforootd/hodor/deploy/gcp-cloud-run/main.tf): GCP infrastructure
- [variables.tf](/Users/ffo/git/fforootd/hodor/deploy/gcp-cloud-run/variables.tf): deploy inputs
- [terraform.tfvars.example](/Users/ffo/git/fforootd/hodor/deploy/gcp-cloud-run/terraform.tfvars.example): example values
- [scripts/bootstrap-cloud.sh](/Users/ffo/git/fforootd/hodor/deploy/gcp-cloud-run/scripts/bootstrap-cloud.sh): migration + root mapping + demo instance bootstrap
- [scripts/create-demo-instance.sh](/Users/ffo/git/fforootd/hodor/deploy/gcp-cloud-run/scripts/create-demo-instance.sh): idempotent demo child creation

## Quick Start

Use a two-pass apply:

1. create APIs plus Artifact Registry
2. build and push the image
3. run the full apply for Cloud Run, Spanner, load balancing, DNS, and certificates

### 1. Create your vars file

```bash
cd deploy/gcp-cloud-run
cp terraform.tfvars.example terraform.tfvars
```

Set at least:

- `project_id`
- `region`
- `platform_domain`
- `spanner_instance_config`
- `cookie_secrets`
- `management_secret`
- `admin_password`
- `admin_pat`

Use a regional Spanner config for v1 unless you explicitly want multi-region spend.

### 2. Create the Artifact Registry repository

```bash
terraform init
terraform apply \
  -target=google_project_service.required \
  -target=google_artifact_registry_repository.images
```

After that, either:

- read `artifact_registry_repository_url` from `terraform output`, or
- derive it as `${region}-docker.pkg.dev/${project_id}/${artifact_registry_repository_id}`

### 3. Build and push the image

Push the repo image before the full apply, because Cloud Run needs the image to exist.

Example:

```bash
PROJECT_ID="my-gcp-project"
REGION="us-central1"
REPO="zitadel"
TAG="$(git rev-parse --short HEAD)"

gcloud auth configure-docker "${REGION}-docker.pkg.dev"
docker build -t "${REGION}-docker.pkg.dev/${PROJECT_ID}/${REPO}/zitadel:${TAG}" .
docker push "${REGION}-docker.pkg.dev/${PROJECT_ID}/${REPO}/zitadel:${TAG}"
```

Then either:

- set `image_tag = "${TAG}"` in `terraform.tfvars`, or
- set `container_image` to the full pushed image URL

### 4. Apply Terraform

```bash
terraform apply
```

Terraform outputs:

- `artifact_registry_repository_url`
- the load balancer IP
- Cloud DNS name servers for the managed zone
- the Cloud Run service and job names
- the Spanner instance and database IDs
- `root_url` and `demo_url`

### 5. Delegate DNS

Update your registrar so `platform_domain` uses the `platform_name_servers` output from Terraform.

Terraform already manages:

- the wildcard `A` record for `*.${platform_domain}`
- the DNS authorization record needed for the wildcard certificate

Certificate issuance will not complete until the zone is delegated and DNS has propagated.

### 6. Run the bootstrap flow

```bash
PROJECT_ID="my-gcp-project" \
REGION="us-central1" \
PLATFORM_DOMAIN="zitadel.example.com" \
SPANNER_INSTANCE_ID="zitadel" \
SPANNER_DATABASE_NAME="zitadel" \
MIGRATOR_JOB_NAME="zitadel-db-migrate" \
ADMIN_PAT_SECRET_NAME="zitadel-admin-pat" \
./scripts/bootstrap-cloud.sh
```

That script:

1. executes the Cloud Run migration job
2. inserts `root.${platform_domain} -> default` into Spanner if it does not exist
3. waits for `https://root.${platform_domain}` to report the correct issuer
4. creates `demo.${platform_domain}` through the root API if it does not already exist

## Operator Checklist

When you come back to run this later, the shortest safe sequence is:

```bash
cd deploy/gcp-cloud-run
cp terraform.tfvars.example terraform.tfvars
# edit terraform.tfvars

terraform init
terraform apply \
  -target=google_project_service.required \
  -target=google_artifact_registry_repository.images

# build and push the tag referenced by image_tag or container_image

terraform apply

# delegate DNS to the platform_name_servers output

PROJECT_ID="..." \
REGION="..." \
PLATFORM_DOMAIN="..." \
SPANNER_INSTANCE_ID="zitadel" \
SPANNER_DATABASE_NAME="zitadel" \
MIGRATOR_JOB_NAME="zitadel-db-migrate" \
ADMIN_PAT_SECRET_NAME="zitadel-admin-pat" \
./scripts/bootstrap-cloud.sh
```

## Manual Acceptance

After bootstrap:

```bash
curl -fsS "https://root.${PLATFORM_DOMAIN}/.well-known/openid-configuration" | jq .issuer
curl -fsS "https://demo.${PLATFORM_DOMAIN}/.well-known/openid-configuration" | jq .issuer
```

Expected values:

- `https://root.${platform_domain}`
- `https://demo.${platform_domain}`

## Secret Model

Terraform creates Secret Manager secrets for:

- `ZITADEL_COOKIE_SECRETS`
- `ZITADEL_MANAGEMENT_SECRET`
- `ZITADEL_ADMIN_PASSWORD`
- `ZITADEL_ADMIN_PAT`
- optional `ZITADEL_ENCRYPTION_KEYS`

The Cloud Run service gets the runtime secrets it needs:

- cookie secrets
- management secret
- optional encryption keys

The migration job additionally receives:

- admin password
- admin PAT

`fixtures/prod-seed.yaml` is already baked into the container at `/etc/zitadel/seed.yaml`, so the deploy only needs `ZITADEL_SEED_FILE=/etc/zitadel/seed.yaml`.

## Runtime Notes

- The Cloud Run service runs with `cloud.enabled=true`.
- The Cloud Run service uses `storage.stateful.backend=spanner`.
- `cloud.control_plane.url` stays empty, so routing data is read from the same Spanner database.
- The service uses `migrate=check` and `bootstrap=skip`.
- The job uses `zitadel db migrate --bootstrap` for first-run schema + seed work.
- `server.public_origin` is intentionally unset so the runtime can advertise the current request host.

## Limitations

- This is a single-region Cloud Run + Spanner example.
- The deploy target assumes platform-controlled subdomains only.
- Customer-owned domain onboarding is intentionally out of scope here.
- Certificate issuance depends on correct DNS delegation and propagation.
- The example grants Spanner roles at the project level for simplicity.
- Local `terraform fmt/init/validate` was intentionally left for the operator run later.
