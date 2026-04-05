import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import LoginShell from './LoginShell.vue'
import type { FlowBranding } from '@/api/branding'

const branding: FlowBranding = {
  heading: 'Welcome back',
  description: 'Sign in to your account',
  logo_url: '/logo-light.svg',
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
  logo_dark: '/logo-dark.svg',
  favicon: '',
  border_radius: 'md',
  terms_url: '',
  privacy_url: '',
  social_position: 'bottom',
  consent: [],
}

function mountShell(overrides: Partial<FlowBranding> = {}, props: Record<string, unknown> = {}) {
  return mount(LoginShell, {
    props: {
      branding: {
        ...branding,
        ...overrides,
      },
      ...props,
    },
    slots: {
      default: '<div class="shell-body">Body</div>',
    },
  })
}

describe('LoginShell', () => {
  it('shows the Zitadel footer by default', () => {
    const wrapper = mountShell()

    expect(wrapper.text()).toContain('Powered by Zitadel')
  })

  it('hides the Zitadel footer when branding disables it', () => {
    const wrapper = mountShell({ hide_zitadel_branding: true })

    expect(wrapper.text()).not.toContain('Powered by Zitadel')
  })

  it('uses the dark logo when dark mode is active', () => {
    const wrapper = mountShell({ dark_mode: 'dark' })

    const logo = wrapper.get('img')
    expect(logo.attributes('src')).toBe('/logo-dark.svg')
  })

  it('falls back to the centered layout when split has no cover image', () => {
    const wrapper = mountShell({ layout: 'split', cover_image: '' })

    expect(wrapper.find('.login-layout-centered').exists()).toBe(true)
    expect(wrapper.find('.login-layout-split').exists()).toBe(false)
  })

  it('avoids duplicating the brand name inside muted layout cards', () => {
    const wrapper = mountShell({ layout: 'muted', logo_url: '' })
    const brandMatches = wrapper.text().match(/Acme Corp/g) || []

    expect(brandMatches).toHaveLength(1)
  })
})
