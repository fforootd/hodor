import type { FlowBranding, FlowStep, UINode } from '@/api/branding'

export interface LoginFlowPreviewOptions {
  strategy: string
  branding: FlowBranding
  captchaEnabled: boolean
  captchaProvider: string
}

function buildAuthNodes(strategy: string): UINode[] {
  switch (strategy) {
    case 'passkey_first':
      return [
        { type: 'button', label: 'Continue with passkey', action: 'passkey' },
        { type: 'divider' },
        {
          type: 'input',
          name: 'identifier',
          label: 'Email',
          input_type: 'email',
          placeholder: 'name@example.com',
          required: true,
        },
        {
          type: 'input',
          name: 'password',
          label: 'Password',
          input_type: 'password',
          placeholder: '••••••••',
          required: true,
        },
      ]
    case 'sso_only':
      return [
        {
          type: 'social_group',
          children: [
            {
              type: 'sso_button',
              label: 'Continue with Google',
              action: 'sso',
              provider_id: 'google',
              template: 'google',
            },
            {
              type: 'sso_button',
              label: 'Continue with Microsoft',
              action: 'sso',
              provider_id: 'entraid',
              template: 'entraid',
            },
          ],
        },
      ]
    default:
      return [
        {
          type: 'input',
          name: 'identifier',
          label: 'Email',
          input_type: 'email',
          placeholder: 'name@example.com',
          required: true,
        },
        {
          type: 'input',
          name: 'password',
          label: 'Password',
          input_type: 'password',
          placeholder: '••••••••',
          required: true,
        },
      ]
  }
}

export function buildPreviewFlowStep(options: LoginFlowPreviewOptions): FlowStep {
  const nodes: UINode[] = [
    { type: 'heading', text: options.branding.heading || 'Welcome back' },
    { type: 'description', text: options.branding.description || 'Sign in to your account' },
    ...buildAuthNodes(options.strategy),
  ]

  if (options.captchaEnabled) {
    nodes.push(
      options.captchaProvider === 'altcha'
        ? { type: 'captcha_altcha' }
        : {
            type: 'captcha_checkbox',
            name: 'captcha',
            attributes: { provider: options.captchaProvider },
          },
    )
  }

  if (options.strategy !== 'sso_only') {
    nodes.push({ type: 'submit', label: 'Sign in', action: 'password' })
  }

  nodes.push({
    type: 'registration_link',
    label: options.strategy === 'sso_only' ? 'Need another option?' : 'Create an account',
    action: 'register',
  })

  return {
    flow_id: 'preview',
    step: 'preview',
    nodes,
    branding: options.branding,
    captcha_required: options.captchaEnabled,
    captcha_verified: false,
    errors: [],
    messages: [],
  }
}
