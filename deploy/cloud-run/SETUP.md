# Cloud Run CI/CD Setup Guide

## Architecture

```
                    OpenTofu (infra/)
                    managed by Infrastructure Manager
                         │
         ┌───────────────┼────────────────────┐
         ▼               ▼                    ▼
    Spanner         Cloud Run           External ALB
    Instance     Service + Job         + CDN + Certs
         │               │                    │
         └───────┬───────┘                    │
                 │                            │
    push to main ──→ build-image.yml ──→ Artifact Registry
                                              │
             workflow_dispatch                │
                  │                           ▼
         deploy-cloud-run.yml ──→ 1Password (secrets)
                               ──→ Cloud Run Job (migrate)
                               ──→ Cloud Run Service (deploy)
                               ──→ Health check
```

## Setup

1. Run `infra/bootstrap.sh` — creates state bucket, enables APIs, creates IM service account
2. Apply infra locally: `tofu apply -var-file=environments/dev.tfvars`
3. Set up 1Password vault (see `deploy/config/secrets.env.tpl`)
4. Configure GitHub secrets/variables (see bootstrap output)
5. First deploy: **Actions > Deploy / Cloud Run > Run workflow**
6. Set up Infrastructure Manager for auto-apply (see bootstrap output)

## 1Password Vault

| Item | Fields |
|------|--------|
| `cookie-secrets-{env}` | `secret` |
| `encryption-key-{env}` | `key-id`, `secret` |
| `cloud-license-{env}` | `key` |
| `management-secret-{env}` | `secret` |

## GitHub Secrets

| Secret | Value |
|--------|-------|
| `OP_SERVICE_ACCOUNT_TOKEN` | 1Password service account token |

## GitHub Variables

| Variable | Source |
|----------|--------|
| `GCP_PROJECT_ID` | Your project ID |
| `GCP_WIF_PROVIDER` | `tofu output wif_provider` |
| `GCP_WIF_SERVICE_ACCOUNT` | `tofu output github_deploy_sa_email` |
