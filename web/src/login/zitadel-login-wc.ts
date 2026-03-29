/**
 * Web Component entry point for <zitadel-login>.
 *
 * Usage:
 *   <script type="module" src="/assets/zitadel-login.js"></script>
 *   <zitadel-login
 *     api-base-url="https://auth.acme.com"
 *     redirect-uri="/callback"
 *   ></zitadel-login>
 *
 * Events emitted (native CustomEvent, bubbles + composed):
 *   - 'login-complete'  — { detail: { session_id, redirect_uri } }
 *   - 'login-error'     — { detail: { code, message } }
 *   - 'login-redirect'  — { detail: { redirect_url } }
 *
 * Build:
 *   Included in the Vite build as a separate entry. The component
 *   self-registers as <zitadel-login>.
 *
 * ADR-019: Server-Driven Login UI + Web Components
 */

import { defineCustomElement } from 'vue'
import LoginApp from './LoginApp.ce.vue'

const ZitadelLogin = defineCustomElement(LoginApp)

// Register globally
customElements.define('zitadel-login', ZitadelLogin)

export { ZitadelLogin }
