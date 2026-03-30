/**
 * Web Component entry point for <zitadel-group-list>.
 *
 * Usage:
 *   <zitadel-group-list
 *     api-base-url="https://auth.acme.com"
 *     dark-mode="auto"
 *     show-search
 *     show-create
 *   ></zitadel-group-list>
 *
 * Events:
 *   - 'group-selected' — { detail: { id, name } }
 *   - 'group-created'  — { detail: { id, name } }
 *   - 'group-error'    — { detail: { error } }
 */

import { defineCustomElement } from 'vue'
import ZitadelGroupListCe from './zitadel-group-list.ce.vue'

const ZitadelGroupList = defineCustomElement(ZitadelGroupListCe)

customElements.define('zitadel-group-list', ZitadelGroupList)

export { ZitadelGroupList }
