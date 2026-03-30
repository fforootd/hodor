/**
 * Web Component entry point for <zitadel-app-list>.
 *
 * Usage:
 *   <zitadel-app-list
 *     api-base-url="https://auth.acme.com"
 *     dark-mode="auto"
 *     show-search
 *   ></zitadel-app-list>
 *
 * Events:
 *   - 'app-selected' — { detail: { id, clientId, name } }
 *   - 'app-error'    — { detail: { error } }
 */

import { defineCustomElement } from 'vue'
import ZitadelAppListCe from './zitadel-app-list.ce.vue'

const ZitadelAppList = defineCustomElement(ZitadelAppListCe)

customElements.define('zitadel-app-list', ZitadelAppList)

export { ZitadelAppList }
