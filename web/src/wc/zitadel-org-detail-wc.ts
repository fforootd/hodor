/**
 * Web Component entry point for <zitadel-org-detail>.
 *
 * Usage:
 *   <zitadel-org-detail
 *     api-base-url="https://auth.acme.com"
 *     org-id="org_abc123"
 *     editable
 *     dark-mode="auto"
 *   ></zitadel-org-detail>
 *
 * Events:
 *   - 'org-updated' — { detail: { id, changes } }
 *   - 'org-deleted' — { detail: { id } }
 *   - 'org-error'   — { detail: { error } }
 */

import { defineCustomElement } from 'vue'
import ZitadelOrgDetailCe from './zitadel-org-detail.ce.vue'

const ZitadelOrgDetail = defineCustomElement(ZitadelOrgDetailCe)

customElements.define('zitadel-org-detail', ZitadelOrgDetail)

export { ZitadelOrgDetail }
