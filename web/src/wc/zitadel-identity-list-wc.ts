/**
 * Web Component entry point for <zitadel-identity-list>.
 *
 * Usage:
 *   <zitadel-identity-list
 *     api-base-url="https://auth.acme.com"
 *     schema-type="human_user"
 *     page-size="20"
 *     dark-mode="auto"
 *   ></zitadel-identity-list>
 *
 * Events emitted (native CustomEvent, bubbles + composed):
 *   - 'identity-selected' — { detail: { id, identifier, schema_type } }
 *   - 'identity-create'   — { detail: { schema_type } }
 */

import { defineCustomElement } from 'vue'
import ZitadelIdentityListCe from './zitadel-identity-list.ce.vue'

const ZitadelIdentityList = defineCustomElement(ZitadelIdentityListCe)

customElements.define('zitadel-identity-list', ZitadelIdentityList)

export { ZitadelIdentityList }
