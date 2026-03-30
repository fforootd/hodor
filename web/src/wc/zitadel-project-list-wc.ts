/**
 * Web Component entry point for <zitadel-project-list>.
 *
 * Usage:
 *   <zitadel-project-list
 *     api-base-url="https://auth.acme.com"
 *     dark-mode="auto"
 *     show-search
 *     show-create
 *   ></zitadel-project-list>
 *
 * Events:
 *   - 'project-selected' — { detail: { id, name } }
 *   - 'project-created'  — { detail: { id, name } }
 *   - 'project-error'    — { detail: { error } }
 */

import { defineCustomElement } from 'vue'
import ZitadelProjectListCe from './zitadel-project-list.ce.vue'

const ZitadelProjectList = defineCustomElement(ZitadelProjectListCe)

customElements.define('zitadel-project-list', ZitadelProjectList)

export { ZitadelProjectList }
