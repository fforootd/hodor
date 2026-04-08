#!/usr/bin/env bash
# One-time bootstrap for Google Cloud Infrastructure Manager.
# Run with owner access before the first automated apply.
set -euo pipefail

PROJECT_ID="${PROJECT_ID:?Set PROJECT_ID}"
REGION="${REGION:-us-central1}"
GITHUB_REPO="${GITHUB_REPO:?Set GITHUB_REPO (owner/repo)}"
ENV="${ENV:-dev}"
STATE_BUCKET="${STATE_BUCKET:-${PROJECT_ID}-tofu-state}"

echo "==> Bootstrapping Infrastructure Manager for ${PROJECT_ID} (${ENV})"

echo "==> Enabling APIs..."
gcloud services enable \
  config.googleapis.com \
  cloudbuild.googleapis.com \
  serviceusage.googleapis.com \
  iam.googleapis.com \
  cloudresourcemanager.googleapis.com \
  --project="${PROJECT_ID}"

echo "==> Creating state bucket gs://${STATE_BUCKET}..."
gcloud storage buckets create "gs://${STATE_BUCKET}" \
  --project="${PROJECT_ID}" \
  --location="${REGION}" \
  --uniform-bucket-level-access \
  2>/dev/null || echo "    (bucket already exists)"

IM_SA="infra-manager"
IM_SA_EMAIL="${IM_SA}@${PROJECT_ID}.iam.gserviceaccount.com"

echo "==> Creating service account ${IM_SA}..."
gcloud iam service-accounts create "${IM_SA}" \
  --project="${PROJECT_ID}" \
  --display-name="Infrastructure Manager" \
  2>/dev/null || echo "    (SA already exists)"

echo "==> Granting roles to ${IM_SA_EMAIL}..."
for role in roles/editor roles/iam.securityAdmin roles/storage.admin roles/spanner.admin; do
  gcloud projects add-iam-policy-binding "${PROJECT_ID}" \
    --member="serviceAccount:${IM_SA_EMAIL}" \
    --role="${role}" \
    --condition=None \
    --quiet
done

cat <<MANUAL

==> Bootstrap complete (automated steps).

==> MANUAL STEPS REQUIRED:

1. Create a 2nd-gen Cloud Build GitHub connection:
   https://console.cloud.google.com/cloud-build/repositories/2nd-gen?project=${PROJECT_ID}

2. Create the Infrastructure Manager deployment:

   gcloud infra-manager deployments apply zitadel-infra-${ENV} \\
     --project="${PROJECT_ID}" \\
     --location="${REGION}" \\
     --service-account="projects/${PROJECT_ID}/serviceAccounts/${IM_SA_EMAIL}" \\
     --git-source-repo="https://github.com/${GITHUB_REPO}" \\
     --git-source-directory="infra" \\
     --git-source-ref="main" \\
     --input-values="project_id=${PROJECT_ID},environment=${ENV},region=${REGION}" \\
     --tf-version-constraint=">=1.6"

3. Add GitHub repository secrets:
   - OP_SERVICE_ACCOUNT_TOKEN: 1Password service account token

4. Add GitHub repository variables:
   - GCP_PROJECT_ID: ${PROJECT_ID}
   - GCP_WIF_PROVIDER: (from 'tofu output wif_provider')
   - GCP_WIF_SERVICE_ACCOUNT: (from 'tofu output github_deploy_sa_email')
   - AR_REGION: ${REGION}

==> To apply infrastructure locally before Infrastructure Manager is set up:

   cd infra
   tofu init -backend-config="bucket=${STATE_BUCKET}" -backend-config="prefix=infra/${ENV}"
   tofu plan -var-file=environments/${ENV}.tfvars
   tofu apply -var-file=environments/${ENV}.tfvars

MANUAL
