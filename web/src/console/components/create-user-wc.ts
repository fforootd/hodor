/**
 * Web Component entry point for <zitadel-create-user>.
 *
 * Usage:
 *   <zitadel-create-user
 *     schema-type="human_user"
 *     org-id="acme"
 *     api-base-url="https://auth.example.com"
 *   ></zitadel-create-user>
 *
 * Events emitted:
 *   - 'user-created'  — { detail: { entityId: string } }
 *   - 'wizard-closed'  — no detail
 *   - 'wizard-error'   — { detail: { error: string } }
 *
 * Build:
 *   Include a Vite entry that imports this file. The component
 *   self-registers as <zitadel-create-user>.
 */

import { defineCustomElement } from 'vue'
import CreateUserWizard from './CreateUserWizard.ce.vue'

const ZitadelCreateUser = defineCustomElement(CreateUserWizard)

// Register globally
customElements.define('zitadel-create-user', ZitadelCreateUser)

export { ZitadelCreateUser }
