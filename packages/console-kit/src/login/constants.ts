/** Actions that require interactive captcha verification before submission. */
export const PROTECTED_CAPTCHA_ACTIONS = new Set([
  'identifier',
  'password',
  'magic_link',
  'sso',
  'register_submit',
  'send_reset',
])
