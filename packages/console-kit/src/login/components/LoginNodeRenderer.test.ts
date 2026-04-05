import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import LoginNodeRenderer from './LoginNodeRenderer.vue'
import type { FlowStep } from '@/api/branding'

const baseStep: FlowStep = {
  flow_id: 'flow_123',
  step: 'identifier',
  branding: {
    heading: 'Welcome back',
    description: 'Sign in to your account',
    logo_url: '',
    org_name: 'Acme Corp',
    colors: {},
    font_family: 'system-ui',
    font_url: '',
    texts: {},
    custom_css: '',
    hide_zitadel_branding: false,
    layout: 'centered',
    dark_mode: 'light',
    cover_image: '',
    logo_dark: '',
    favicon: '',
    border_radius: 'md',
    terms_url: '',
    privacy_url: '',
    social_position: 'bottom',
    consent: [],
  },
  nodes: [],
  errors: [],
  messages: [],
}

const stubs = {
  Alert: { template: '<div><slot /></div>' },
  AlertDescription: { template: '<div><slot /></div>' },
  Avatar: { template: '<div><slot /></div>' },
  AvatarFallback: { template: '<div><slot /></div>' },
  Button: {
    props: ['disabled', 'type', 'variant'],
    template: '<button :disabled="disabled" :type="type"><slot /></button>',
  },
  CaptchaWidget: { template: '<div class="captcha-widget" />' },
  Input: {
    props: ['modelValue', 'type', 'disabled'],
    emits: ['update:modelValue'],
    template:
      '<input :value="modelValue" :type="type" :disabled="disabled" @input="$emit(\'update:modelValue\', $event.target.value)" />',
  },
  Label: { template: '<label><slot /></label>' },
  Separator: { template: '<hr />' },
  Spinner: { template: '<span class="spinner" />' },
  AlertCircle: { template: '<span class="alert-circle" />' },
}

function mountRenderer(nodes: FlowStep['nodes'], props: Record<string, unknown> = {}) {
  return mount(LoginNodeRenderer, {
    props: {
      flowStep: {
        ...baseStep,
        nodes,
      },
      formData: {},
      confirmPasswords: {},
      ...props,
    },
    global: {
      stubs,
    },
  })
}

describe('LoginNodeRenderer captcha gating', () => {
  it('disables protected actions until captcha is verified', () => {
    const wrapper = mountRenderer(
      [
        { type: 'submit', label: 'Continue', action: 'identifier' },
        { type: 'sso_button', label: 'Continue with Google', action: 'sso', provider_id: 'google' },
      ],
      {
        captchaRequired: true,
        captchaSolved: false,
      },
    )

    const buttons = wrapper.findAll('button')
    expect(buttons).toHaveLength(2)
    expect(buttons[0].attributes('disabled')).toBeDefined()
    expect(buttons[1].attributes('disabled')).toBeDefined()
  })

  it('keeps navigation actions enabled while captcha is required', () => {
    const wrapper = mountRenderer(
      [
        { type: 'link', label: 'Back', action: 'back' },
        { type: 'registration_link', label: 'Create an account', action: 'register' },
      ],
      {
        captchaRequired: true,
        captchaSolved: false,
      },
    )

    const buttons = wrapper.findAll('button')
    expect(buttons).toHaveLength(2)
    expect(buttons[0].attributes('disabled')).toBeUndefined()
    expect(buttons[1].attributes('disabled')).toBeUndefined()
  })

  it('renders captcha immediately after the submit action', () => {
    const wrapper = mountRenderer([
      { type: 'input', name: 'identifier', label: 'Email' },
      { type: 'submit', label: 'Continue', action: 'identifier' },
      { type: 'divider' },
      { type: 'registration_link', label: 'Create an account', action: 'register' },
      { type: 'captcha_checkbox', name: 'captcha', attributes: { provider: 'turnstile' } },
    ])

    const html = wrapper.find('form').html()
    expect(html.indexOf('Continue')).toBeLessThan(html.indexOf('captcha-widget'))
    expect(html.indexOf('captcha-widget')).toBeLessThan(html.indexOf('Create an account'))
  })

  it('emits form data updates for text fields', async () => {
    const wrapper = mountRenderer([
      { type: 'input', name: 'identifier', label: 'Email', input_type: 'email' },
    ])

    await wrapper.find('input[type="email"]').setValue('james@example.com')

    expect(wrapper.emitted('update:form-data')).toEqual([
      [{ identifier: 'james@example.com' }],
    ])
  })

  it('emits checkbox updates for consent fields', async () => {
    const wrapper = mountRenderer([
      { type: 'consent_checkbox', name: 'terms', label: 'I agree' },
    ])

    await wrapper.find('input[type="checkbox"]').setValue(true)

    expect(wrapper.emitted('update:form-data')).toEqual([
      [{ terms: true }],
    ])
  })

  it('emits confirm password updates during registration', async () => {
    const wrapper = mount(LoginNodeRenderer, {
      props: {
        flowStep: {
          ...baseStep,
          step: 'register',
          nodes: [{ type: 'input', name: 'password', label: 'Password', input_type: 'password' }],
        },
        formData: {},
        confirmPasswords: {},
      },
      global: {
        stubs,
      },
    })

    const inputs = wrapper.findAll('input[type="password"]')
    expect(inputs).toHaveLength(2)

    await inputs[1].setValue('super-secret')

    expect(wrapper.emitted('update:confirm-passwords')).toEqual([
      [{ password: 'super-secret' }],
    ])
  })

  it('keeps preview inputs read-only', () => {
    const wrapper = mountRenderer(
      [{ type: 'input', name: 'identifier', label: 'Email', input_type: 'email' }],
      { preview: true },
    )

    expect(wrapper.find('input[type="email"]').attributes('disabled')).toBeDefined()
  })
})
