# Cloud Run CI/CD Setup Guide

This guide walks through the one-time GCP and GitHub setup needed for the automated build and deploy workflows.

## What You'll Set Up

```
push to main ──→ build-image.yml ──→ Artifact Registry
                                          │
         workflow_dispatch                 │
              │                            ▼
     deploy-cloud-run.yml ──→ Cloud Run Job (migrate)
                           ──→ Cloud Run Service (deploy)
                           ──→ Health check
```

## Prerequisites

- `gcloud` CLI authenticated with owner/editor access to the GCP project
- GitHub repository admin access (for repo variables and environments)

---

## Step 1: Set Variables

Edit these for your project. Every command below uses them.

```bash
export PROJECT_ID="ffo-test-27661"
export PROJECT_NUMBER="$(gcloud projects describe ${PROJECT_ID} --format='value(projectNumber)')"
export REGION="us-west1"
export GITHUB_ORG="fforootd"          # GitHub org or username
export GITHUB_REPO="hodor"            # GitHub repo name
export SPANNER_INSTANCE="test"
export SPANNER_DATABASE="a1"
```

---

## Step 2: Artifact Registry

Create the Docker repository where container images are stored.

```bash
gcloud artifacts repositories create zitadel \
  --repository-format=docker \
  --location="${REGION}" \
  --description="Zitadel container images" \
  --project="${PROJECT_ID}"
```

Verify:
```bash
gcloud artifacts repositories describe zitadel \
  --location="${REGION}" \
  --project="${PROJECT_ID}"
```

---

## Step 3: Service Accounts

### 3a. Runtime service account (Cloud Run service)

This already exists if you followed the original README. If not:

```bash
gcloud iam service-accounts create zitadel-run \
  --project="${PROJECT_ID}" \
  --display-name="Zitadel Cloud Run Runtime"

# Spanner read/write
gcloud spanner databases add-iam-policy-binding "${SPANNER_DATABASE}" \
  --project="${PROJECT_ID}" \
  --instance="${SPANNER_INSTANCE}" \
  --member="serviceAccount:zitadel-run@${PROJECT_ID}.iam.gserviceaccount.com" \
  --role="roles/spanner.databaseUser"
```

### 3b. Migrator service account (Cloud Run Job)

Needs schema admin, not just read/write.

```bash
gcloud iam service-accounts create zitadel-migrator \
  --project="${PROJECT_ID}" \
  --display-name="Zitadel Schema Migrator"

# Spanner admin (DDL)
gcloud spanner databases add-iam-policy-binding "${SPANNER_DATABASE}" \
  --project="${PROJECT_ID}" \
  --instance="${SPANNER_INSTANCE}" \
  --member="serviceAccount:zitadel-migrator@${PROJECT_ID}.iam.gserviceaccount.com" \
  --role="roles/spanner.databaseAdmin"
```

### 3c. GitHub Actions service account (CI/CD)

Used by Workload Identity Federation. Needs to push images, deploy services, and act as the runtime SA.

```bash
gcloud iam service-accounts create github-deploy \
  --project="${PROJECT_ID}" \
  --display-name="GitHub Actions Deploy"

# Push images to Artifact Registry
gcloud artifacts repositories add-iam-policy-binding zitadel \
  --location="${REGION}" \
  --project="${PROJECT_ID}" \
  --member="serviceAccount:github-deploy@${PROJECT_ID}.iam.gserviceaccount.com" \
  --role="roles/artifactregistry.writer"

# Deploy Cloud Run services and execute jobs
gcloud projects add-iam-policy-binding "${PROJECT_ID}" \
  --member="serviceAccount:github-deploy@${PROJECT_ID}.iam.gserviceaccount.com" \
  --role="roles/run.developer"

# Impersonate the runtime + migrator service accounts when deploying
gcloud iam service-accounts add-iam-policy-binding \
  "zitadel-run@${PROJECT_ID}.iam.gserviceaccount.com" \
  --project="${PROJECT_ID}" \
  --member="serviceAccount:github-deploy@${PROJECT_ID}.iam.gserviceaccount.com" \
  --role="roles/iam.serviceAccountUser"

gcloud iam service-accounts add-iam-policy-binding \
  "zitadel-migrator@${PROJECT_ID}.iam.gserviceaccount.com" \
  --project="${PROJECT_ID}" \
  --member="serviceAccount:github-deploy@${PROJECT_ID}.iam.gserviceaccount.com" \
  --role="roles/iam.serviceAccountUser"
```

---

## Step 4: Workload Identity Federation

This lets GitHub Actions authenticate to GCP without a service account key file.

### 4a. Create the identity pool

```bash
gcloud iam workload-identity-pools create github-actions \
  --location="global" \
  --display-name="GitHub Actions" \
  --project="${PROJECT_ID}"
```

### 4b. Create the OIDC provider (bound to your repo)

```bash
gcloud iam workload-identity-pools providers create-oidc github \
  --location="global" \
  --workload-identity-pool="github-actions" \
  --issuer-uri="https://token.actions.githubusercontent.com" \
  --attribute-mapping="google.subject=assertion.sub,attribute.repository=assertion.repository" \
  --attribute-condition="assertion.repository=='${GITHUB_ORG}/${GITHUB_REPO}'" \
  --project="${PROJECT_ID}"
```

### 4c. Allow the pool to impersonate the deploy service account

```bash
gcloud iam service-accounts add-iam-policy-binding \
  "github-deploy@${PROJECT_ID}.iam.gserviceaccount.com" \
  --project="${PROJECT_ID}" \
  --member="principalSet://iam.googleapis.com/projects/${PROJECT_NUMBER}/locations/global/workloadIdentityPools/github-actions/attribute.repository/${GITHUB_ORG}/${GITHUB_REPO}" \
  --role="roles/iam.workloadIdentityUser"
```

### 4d. Get the provider resource name (you'll need this for GitHub)

```bash
gcloud iam workload-identity-pools providers describe github \
  --location="global" \
  --workload-identity-pool="github-actions" \
  --project="${PROJECT_ID}" \
  --format="value(name)"
```

This prints something like:
```
projects/1234/locations/global/workloadIdentityPools/github-actions/providers/github
```

Save this — it goes in the `GCP_WIF_PROVIDER` GitHub variable.

---

## Step 5: Observability IAM (Optional)

If using the OTEL export to GCP (traces + metrics):

```bash
# Traces
gcloud projects add-iam-policy-binding "${PROJECT_ID}" \
  --member="serviceAccount:zitadel-run@${PROJECT_ID}.iam.gserviceaccount.com" \
  --role="roles/cloudtrace.agent"

# Metrics
gcloud projects add-iam-policy-binding "${PROJECT_ID}" \
  --member="serviceAccount:zitadel-run@${PROJECT_ID}.iam.gserviceaccount.com" \
  --role="roles/monitoring.metricWriter"
```

---

## Step 6: Cloud Run Migration Job

Create the one-time migration job. It uses a placeholder image initially — the deploy workflow updates it before each execution.

```bash
# Build and push an initial image first (or use the build-image workflow)
IMAGE="${REGION}-docker.pkg.dev/${PROJECT_ID}/zitadel/zitadel:latest"

gcloud run jobs create zitadel-migrate \
  --image="${IMAGE}" \
  --service-account="zitadel-migrator@${PROJECT_ID}.iam.gserviceaccount.com" \
  --command="zitadel" \
  --args="db,migrate,--bootstrap" \
  --set-env-vars="\
ZITADEL_STORAGE__STATEFUL__BACKEND=spanner,\
ZITADEL_STORAGE__STATEFUL__DATABASE=projects/${PROJECT_ID}/instances/${SPANNER_INSTANCE}/databases/${SPANNER_DATABASE},\
ZITADEL_STORAGE__STATEFUL__MIGRATE=auto,\
ZITADEL_STORAGE__STATEFUL__BOOTSTRAP=auto" \
  --memory="1Gi" \
  --region="${REGION}" \
  --project="${PROJECT_ID}"
```

---

## Step 7: GitHub Repository Variables

Go to **Settings → Secrets and variables → Actions → Variables** and add:

| Variable | Value | Required |
|----------|-------|----------|
| `GCP_PROJECT_ID` | `ffo-test-27661` | Yes |
| `GCP_WIF_PROVIDER` | Output from step 4d | Yes |
| `GCP_WIF_SERVICE_ACCOUNT` | `github-deploy@ffo-test-27661.iam.gserviceaccount.com` | Yes |
| `AR_REGION` | `us-west1` | No (default) |
| `AR_REPOSITORY` | `zitadel` | No (default) |
| `CLOUD_RUN_SERVICE` | `zitadel-test` | No (default) |
| `CLOUD_RUN_REGION` | `us-west1` | No (default) |
| `CLOUD_RUN_MIGRATE_JOB` | `zitadel-migrate` | No (default) |

### GitHub Environments (optional but recommended)

For production deploys with approval gates:

1. Go to **Settings → Environments**
2. Create `test` and `production` environments
3. On `production`, enable **Required reviewers** and add approvers
4. Optionally set environment-specific variable overrides (e.g. different `CLOUD_RUN_SERVICE`)

---

## Step 8: Verify

### Test the build workflow

Push to `main` or trigger manually:
```
Actions → Build / Container Image → Run workflow
```

Check the image landed:
```bash
gcloud artifacts docker images list \
  "${REGION}-docker.pkg.dev/${PROJECT_ID}/zitadel/zitadel" \
  --project="${PROJECT_ID}"
```

### Test the deploy workflow

```
Actions → Deploy / Cloud Run → Run workflow
  image_tag: latest
  environment: test
  run_migrations: true
```

Check the service:
```bash
gcloud run services describe zitadel-test \
  --region="${REGION}" \
  --project="${PROJECT_ID}" \
  --format="value(status.url)"
```

---

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `PERMISSION_DENIED` on AR push | WIF binding missing | Re-run step 4c |
| `Image not found` in deploy | Build didn't run or wrong tag | Check build-image workflow, verify image in AR |
| Migration job fails | Wrong Spanner config or SA missing admin | Check env vars on the job, verify step 3b |
| `403` on Cloud Run deploy | `github-deploy` SA missing `run.developer` | Re-run step 3c |
| Health check fails | Service not ready, bad config | Check Cloud Run logs: `gcloud run services logs read zitadel-test --region=${REGION}` |
