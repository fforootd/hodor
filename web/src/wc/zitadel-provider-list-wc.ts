/**
 * Web Component entry point for <zitadel-provider-list>.
 *
 * Usage:
 *   <zitadel-provider-list
 *     api-base-url="https://auth.acme.com"
 *     dark-mode="auto"
 *     show-search
 *     show-create
 *   ></zitadel-provider-list>
 *
 * Events:
 *   - 'provider-selected' — { detail: { id, name, protocol, enabled } }
 *   - 'provider-created'  — { detail: { id, name } }
 *   - 'provider-toggled'  — { detail: { id, enabled } }
 *   - 'provider-deleted'  — { detail: { id } }
 *   - 'provider-error'    — { detail: { error } }
 */

import { defineCustomElement } from 'vue'
import ZitadelProviderListCe from './zitadel-provider-list.ce.vue'

const ZitadelProviderList = defineCustomElement(ZitadelProviderListCe)

customElements.define('zitadel-provider-list', ZitadelProviderList)

export { ZitadelProviderList }
