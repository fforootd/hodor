/**
 * Web Component entry point for <zitadel-identity-detail>.
 *
 * Usage:
 *   <zitadel-identity-detail
 *     api-base-url="https://auth.acme.com"
 *     identity-id="user_abc123"
 *     editable
 *     dark-mode="auto"
 *   ></zitadel-identity-detail>
 *
 * Events:
 *   - 'identity-updated' — { detail: { id, changes } }
 *   - 'identity-deleted' — { detail: { id } }
 */

import { defineCustomElement } from 'vue'
import ZitadelIdentityDetailCe from './zitadel-identity-detail.ce.vue'

const ZitadelIdentityDetail = defineCustomElement(ZitadelIdentityDetailCe)

customElements.define('zitadel-identity-detail', ZitadelIdentityDetail)

export { ZitadelIdentityDetail }
