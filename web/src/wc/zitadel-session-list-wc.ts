/**
 * Web Component entry point for <zitadel-session-list>.
 *
 * Usage:
 *   <zitadel-session-list
 *     api-base-url="https://auth.acme.com"
 *     dark-mode="auto"
 *     show-search
 *     user-id="user_abc123"
 *   ></zitadel-session-list>
 *
 * Events:
 *   - 'session-selected' — { detail: { id, entity_id, state } }
 *   - 'session-revoked'  — { detail: { session_id } }
 */

import { defineCustomElement } from 'vue'
import ZitadelSessionListCe from './zitadel-session-list.ce.vue'

const ZitadelSessionList = defineCustomElement(ZitadelSessionListCe)

customElements.define('zitadel-session-list', ZitadelSessionList)

export { ZitadelSessionList }
