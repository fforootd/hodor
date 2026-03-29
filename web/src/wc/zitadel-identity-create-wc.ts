/**
 * Web Component entry point for <zitadel-identity-create>.
 *
 * Usage:
 *   <zitadel-identity-create
 *     api-base-url="https://auth.acme.com"
 *     schema-type="human_user"
 *     dark-mode="auto"
 *   ></zitadel-identity-create>
 *
 * Events:
 *   - 'identity-created' — { detail: { id, identifier } }
 *   - 'create-cancelled' — { detail: undefined }
 *   - 'create-error'     — { detail: { error } }
 */

import { defineCustomElement } from 'vue'
import ZitadelIdentityCreateCe from './zitadel-identity-create.ce.vue'

const ZitadelIdentityCreate = defineCustomElement(ZitadelIdentityCreateCe)

customElements.define('zitadel-identity-create', ZitadelIdentityCreate)

export { ZitadelIdentityCreate }
