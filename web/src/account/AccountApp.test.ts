import { describe, expect, it, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import AccountApp from './AccountApp.vue'

vi.mock('@/api/client', () => ({
  api: {
    get: vi.fn(),
    patch: vi.fn(),
    post: vi.fn(),
  },
}))

vi.mock('@/api/branding', () => ({
  brandingApi: {
    get: vi.fn(),
  },
}))

import { api } from '@/api/client'
import { brandingApi } from '@/api/branding'

describe('AccountApp bootstrap', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('shows the bootstrap screen instead of partially rendering the account page', async () => {
    vi.mocked(brandingApi.get).mockResolvedValue({
      org_id: '',
      org_name: 'Zitadel',
      logo_url: '',
      heading: '',
      description: '',
      colors: {},
      font_family: '',
      hide_zitadel_branding: false,
    } as any)
    vi.mocked(api.get).mockImplementation((path: string) => {
      if (path === '/v1/account/profile') {
        return new Promise(() => {})
      }
      return Promise.resolve({ events: [], sessions: [] } as any)
    })

    const wrapper = mount(AccountApp, {
      global: {
        stubs: {
          AppBootstrapScreen: {
            props: ['appName', 'state'],
            template: '<div class="bootstrap-screen">{{ appName }}:{{ state }}</div>',
          },
        },
      },
    })

    await flushPromises()

    expect(wrapper.find('.bootstrap-screen').text()).toContain('account:initializing')
    expect(wrapper.text()).not.toContain('Sign out')
  })
})
