/**
 * Web Component entry point for <zitadel-user-invite>.
 *
 * Usage:
 *   <zitadel-user-invite
 *     api-base-url="https://auth.acme.com"
 *     schema-type="human_user"
 *     dark-mode="auto"
 *   ></zitadel-user-invite>
 *
 * Events:
 *   - 'invite-sent'   — { detail: { email, user_id, purpose } }
 *   - 'invite-error'  — { detail: { error } }
 */

import { defineCustomElement } from 'vue'
import ZitadelUserInviteCe from './zitadel-user-invite.ce.vue'

const ZitadelUserInvite = defineCustomElement(ZitadelUserInviteCe)

customElements.define('zitadel-user-invite', ZitadelUserInvite)

export { ZitadelUserInvite }
