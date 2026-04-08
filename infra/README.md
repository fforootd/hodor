# Zitadel GCP Infrastructure

OpenTofu modules managed by [Google Cloud Infrastructure Manager](https://cloud.google.com/infrastructure-manager/docs).

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Google Cloud Project                      │
│                                                              │
│  ┌──────────┐  ┌───────────────┐  ┌────────────────────┐   │
│  │ Spanner  │  │  Cloud Run    │  │  External ALB       │   │
│  │ Instance │  │  Service      │◄─│  + CDN              │   │
│  │          │  │  (zitadel)    │  │  + Certificate Map  │   │
│  │  ┌────┐  │  ├───────────────┤  └────────────────────┘   │
│  │  │ DB │◄─┼──│  Cloud Run    │                            │
│  │  └────┘  │  │  Job          │  ┌────────────────────┐   │
│  └──────────┘  │  (migrate)    │  │  Cloud DNS         │   │
│                └───────────────┘  └────────────────────┘   │
│                                                              │
│  ┌──────────────────┐  ┌───────────────────────────────┐   │
│  │ Artifact Registry │  │  IAM + Workload Identity Fed  │   │
│  └──────────────────┘  └───────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

## Modules

| Module | Purpose |
|--------|---------|
| `project` | Enable required GCP APIs |
| `artifact-registry` | Docker image repository |
| `spanner` | Spanner instance + database shell |
| `iam` | Service accounts + role bindings |
| `workload-identity` | WIF pool/provider for GitHub Actions |
| `cloud-run` | Service (runtime) + Job (migrations) |
| `load-balancer` | External ALB + CDN + serverless NEG |
| `certificate-map` | Certificate Manager map (runtime-populated) |
| `dns` | Cloud DNS zone + A records |

## Resource Ownership

| Resource | Owner | Lifecycle |
|----------|-------|-----------|
| Spanner, LB, CDN, IAM, DNS | OpenTofu (this repo) | Infra changes |
| Cloud Run image + secret env vars | GitHub Actions deploy workflow | App releases |
| Customer certs + host rules | Zitadel runtime (`infra.rs`) | Tenant onboarding |

## Usage

```bash
# First-time setup
export PROJECT_ID=my-project GITHUB_REPO=owner/repo
./bootstrap.sh

# Local apply (before Infrastructure Manager)
cd infra
tofu init -backend-config="bucket=${PROJECT_ID}-tofu-state" -backend-config="prefix=infra/dev"
tofu apply -var-file=environments/dev.tfvars
```

After Infrastructure Manager is set up: PR → preview, merge → auto-apply.

## Secrets

Secrets flow from 1Password at deploy time, not through OpenTofu.
See `deploy/config/secrets.env.tpl` for the 1Password item references.
