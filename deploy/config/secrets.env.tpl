# 1Password secret references for Zitadel deployment.
# Used by GitHub Actions: 1password/load-secrets-action
# Replace ${ENV} with the target environment (dev, prod).

ZITADEL_COOKIE_SECRETS=op://Zitadel/cookie-secrets-${ENV}/secret
ZITADEL_ENCRYPTION__ACTIVE_KEY_ID=op://Zitadel/encryption-key-${ENV}/key-id
ZITADEL_ENCRYPTION__KEYS__0__ID=op://Zitadel/encryption-key-${ENV}/key-id
ZITADEL_ENCRYPTION__KEYS__0__SECRET=op://Zitadel/encryption-key-${ENV}/secret
ZITADEL_CLOUD__LICENSE_KEY=op://Zitadel/cloud-license-${ENV}/key
ZITADEL_SERVER__MANAGEMENT_SECRET=op://Zitadel/management-secret-${ENV}/secret
