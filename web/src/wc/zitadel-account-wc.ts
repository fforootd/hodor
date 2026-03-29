/**
 * Web Component entry point for <zitadel-account>.
 *
 * Usage:
 *   <zitadel-account
 *     api-base-url="https://auth.acme.com"
 *     dark-mode="auto"
 *     show-sessions
 *     show-activity
 *   ></zitadel-account>
 *
 * Events:
 *   - 'profile-updated'   — { detail: { changes } }
 *   - 'session-revoked'   — { detail: { session_id } }
 *   - 'sign-out'          — { detail: undefined }
 */

import { defineCustomElement } from 'vue'
import ZitadelAccountCe from './zitadel-account.ce.vue'

const ZitadelAccount = defineCustomElement(ZitadelAccountCe)

customElements.define('zitadel-account', ZitadelAccount)

export { ZitadelAccount }
