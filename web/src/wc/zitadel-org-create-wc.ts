/**
 * Web Component entry point for <zitadel-org-create>.
 *
 * Usage:
 *   <zitadel-org-create
 *     api-base-url="https://auth.acme.com"
 *     dark-mode="auto"
 *   ></zitadel-org-create>
 *
 * Events:
 *   - 'org-created'      — { detail: { id, name } }
 *   - 'create-cancelled' — no detail
 *   - 'org-error'        — { detail: { error } }
 */

import { defineCustomElement } from 'vue'
import ZitadelOrgCreateCe from './zitadel-org-create.ce.vue'

const ZitadelOrgCreate = defineCustomElement(ZitadelOrgCreateCe)

customElements.define('zitadel-org-create', ZitadelOrgCreate)

export { ZitadelOrgCreate }
