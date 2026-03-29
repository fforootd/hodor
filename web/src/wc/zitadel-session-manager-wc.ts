/**
 * Web Component entry point for <zitadel-session-manager>.
 *
 * Usage:
 *   <zitadel-session-manager
 *     api-base-url="https://auth.acme.com"
 *     dark-mode="auto"
 *   ></zitadel-session-manager>
 *
 * Events:
 *   - 'session-revoked'       — { detail: { session_id } }
 *   - 'all-sessions-revoked'  — { detail: undefined }
 */

import { defineCustomElement } from 'vue'
import ZitadelSessionManagerCe from './zitadel-session-manager.ce.vue'

const ZitadelSessionManager = defineCustomElement(ZitadelSessionManagerCe)

customElements.define('zitadel-session-manager', ZitadelSessionManager)

export { ZitadelSessionManager }
