import { beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import type { FlowStep } from '@/api/branding'
import LoginApp from './LoginApp.vue'

vi.mock('@/api/branding', () => ({
  flowApi: {
    create: vi.fn(),
    submit: vi.fn(),
    get: vi.fn(),
    ready: vi.fn(),
  },
}))

vi.mock('@/lib/fingerprint', () => ({
  collectFingerprint: vi.fn(),
}))

vi.mock('@/lib/telemetry', () => ({
  initTelemetry: vi.fn(),
  shutdownTelemetry: vi.fn(),
  traceStepTransition: vi.fn(),
  traceFormSubmit: vi.fn(() => null),
  setFlowId: vi.fn(),
}))

import { flowApi } from '@/api/branding'
import { collectFingerprint } from '@/lib/fingerprint'

const branding = {
  heading: 'Welcome back',
  description: 'Sign in to your account',
  logo_url: '',
  org_name: 'Zitadel',
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
}

describe('LoginApp adaptive captcha refresh', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    // Ensure fingerprint collection is not skipped in the test environment.
    Object.defineProperty(navigator, 'webdriver', { value: false, configurable: true })
  })

  it('adopts the refreshed flow after silent fingerprint submission', async () => {
    const initialStep: FlowStep = {
      flow_id: 'flow_123',
      step: 'identifier',
      branding,
      captcha_required: true,
      captcha_verified: false,
      nodes: [
        { type: 'captcha_altcha', name: 'altcha_payload' },
        { type: 'fingerprint_collect', name: 'visitor_id' },
      ],
      errors: [],
      messages: [],
    }
    const refreshedStep: FlowStep = {
      flow_id: 'flow_123',
      step: 'identifier',
      branding,
      captcha_required: false,
      captcha_verified: false,
      nodes: [{ type: 'submit', label: 'Continue', action: 'identifier' }],
      errors: [],
      messages: [],
    }

    vi.mocked(flowApi.create).mockResolvedValue(initialStep)
    vi.mocked(flowApi.submit).mockResolvedValue(refreshedStep)
    vi.mocked(collectFingerprint).mockResolvedValue({
      visitorId: 'fp_123',
      components: {},
      confidence: 0.995,
      collectedAt: Date.now(),
    })

    const wrapper = mount(LoginApp, {
      global: {
        stubs: {
          AppBootstrapScreen: {
            props: ['state', 'error'],
            template: '<div class="bootstrap-screen">{{ state }}</div>',
          },
          LoginShell: {
            template: '<div class="login-shell"><slot /></div>',
          },
          LoginNodeRenderer: {
            props: ['flowStep', 'captchaRequired'],
            template:
              '<div class="renderer"><span class="captcha-required">{{ captchaRequired ? "required" : "not-required" }}</span><span class="node-count">{{ flowStep?.nodes?.length || 0 }}</span></div>',
          },
        },
      },
    })

    await flushPromises()
    await flushPromises()

    expect(flowApi.submit).toHaveBeenCalledWith('', 'flow_123', 'fingerprint_submit', {
      visitor_id: 'fp_123',
      fingerprint_hash: 'fp_123',
    })
    expect(wrapper.find('.captcha-required').text()).toBe('not-required')
    expect(wrapper.find('.node-count').text()).toBe('1')
  })
})
