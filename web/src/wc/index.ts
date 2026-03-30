/**
 * @zitadel/ui — Barrel export for all Zitadel web components.
 *
 * Programmatic usage (for customers who import via NPM):
 *
 *   import '@zitadel/ui'  // registers all custom elements
 *
 * Or import individual components:
 *
 *   import '@zitadel/ui/identity-list'
 *
 * CDN usage:
 *
 *   <script type="module" src="https://cdn.zitadel.com/ui/zitadel-ui.js"></script>
 */

// Login (existing)
export { ZitadelLogin } from '../login/zitadel-login-wc'

// Identity CRUD
export { ZitadelIdentityList } from './zitadel-identity-list-wc'
export { ZitadelIdentityDetail } from './zitadel-identity-detail-wc'
export { ZitadelIdentityCreate } from './zitadel-identity-create-wc'

// Self-service
export { ZitadelAccount } from './zitadel-account-wc'
export { ZitadelSessionManager } from './zitadel-session-manager-wc'
export { ZitadelUserInvite } from './zitadel-user-invite-wc'

// Admin: Orgs, Sessions, Providers
export { ZitadelOrgList } from './zitadel-org-list-wc'
export { ZitadelOrgCreate } from './zitadel-org-create-wc'
export { ZitadelOrgDetail } from './zitadel-org-detail-wc'
export { ZitadelSessionList } from './zitadel-session-list-wc'
export { ZitadelProviderList } from './zitadel-provider-list-wc'

// Resource management: Apps, Groups, Projects
export { ZitadelAppList } from './zitadel-app-list-wc'
export { ZitadelGroupList } from './zitadel-group-list-wc'
export { ZitadelProjectList } from './zitadel-project-list-wc'

// Shared utilities (for advanced usage)
export { createSharedStyleSheet, getSharedStyleSheet } from './base-styles'
export { createWCApiClient, WCApiError } from './wc-api-client'
export type { WCApiClient } from './wc-api-client'
