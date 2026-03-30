import { describe, expect, it } from 'vitest'
import { buildPreviewFlowStep } from './preview'
import type { FlowBranding } from '@/api/branding'

const branding: FlowBranding = {
  heading: 'Welcome back',
  description: 'Sign in to your account',
  logo_url: '',
  org_name: 'Acme Corp',
  colors: {
    primary: '#111827',
    primary_foreground: '#ffffff',
    background: '#f8fafc',
    surface: '#ffffff',
    text: '#111827',
    muted: '#e5e7eb',
    accent: '#111827',
    border: '#d1d5db',
    error: '#ef4444',
  },
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
}

describe('buildPreviewFlowStep', () => {
  it('builds a password-first preview with submit and captcha nodes', () => {
    const step = buildPreviewFlowStep({
      strategy: 'identifier_first',
      branding,
      captchaEnabled: true,
      captchaProvider: 'altcha',
    })

    expect(step.step).toBe('preview')
    expect(step.nodes.some((node) => node.type === 'input' && node.name === 'identifier')).toBe(true)
    expect(step.nodes.some((node) => node.type === 'input' && node.name === 'password')).toBe(true)
    expect(step.nodes.some((node) => node.type === 'captcha_altcha')).toBe(true)
    expect(step.nodes.some((node) => node.type === 'submit')).toBe(true)
  })

  it('builds an sso-only preview without a submit button', () => {
    const step = buildPreviewFlowStep({
      strategy: 'sso_only',
      branding,
      captchaEnabled: false,
      captchaProvider: 'altcha',
    })

    expect(step.nodes.some((node) => node.type === 'social_group')).toBe(true)
    expect(step.nodes.some((node) => node.type === 'submit')).toBe(false)
    expect(step.nodes.some((node) => node.type === 'registration_link')).toBe(true)
  })
})
