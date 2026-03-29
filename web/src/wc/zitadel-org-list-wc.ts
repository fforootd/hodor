/**
 * Web Component entry point for <zitadel-org-list>.
 *
 * Usage:
 *   <zitadel-org-list
 *     api-base-url="https://auth.acme.com"
 *     dark-mode="auto"
 *     show-search
 *     show-create
 *   ></zitadel-org-list>
 *
 * Events:
 *   - 'org-selected' — { detail: { id, name } }
 *   - 'org-created'  — { detail: { id, name } }
 *   - 'org-error'    — { detail: { error } }
 */

import { defineCustomElement } from 'vue'
import ZitadelOrgListCe from './zitadel-org-list.ce.vue'

const ZitadelOrgList = defineCustomElement(ZitadelOrgListCe)

customElements.define('zitadel-org-list', ZitadelOrgList)

export { ZitadelOrgList }
