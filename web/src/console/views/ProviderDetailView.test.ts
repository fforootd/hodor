import { flushPromises, shallowMount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'

const mocked = vi.hoisted(() => ({
  apiDelete: vi.fn(),
  apiGet: vi.fn(),
  apiPatch: vi.fn(),
  routerPush: vi.fn(),
}))

vi.mock('@/api/client', () => ({
  api: {
    delete: mocked.apiDelete,
    get: mocked.apiGet,
    patch: mocked.apiPatch,
  },
}))

vi.mock('vue-router', () => ({
  useRoute: () => ({
    params: {
      id: 'prov-1',
    },
  }),
  useRouter: () => ({
    push: mocked.routerPush,
  }),
}))

vi.mock('@/lib/notify', () => ({
  notifyMutationError: vi.fn(),
  notifyMutationSuccess: vi.fn(),
}))

import ProviderDetailView from './ProviderDetailView.vue'

describe('ProviderDetailView', () => {
  beforeEach(() => {
    mocked.apiDelete.mockReset()
    mocked.apiGet.mockReset()
    mocked.apiPatch.mockReset()
    mocked.routerPush.mockReset()
  })

  it('loads and renders provider details', async () => {
    mocked.apiGet.mockResolvedValue({
      id: 'prov-1',
      display_name: 'Mock OIDC',
      enabled: true,
      kind: 'custom',
      protocol: 'oidc',
      target: { schema_type: 'human_user' },
      linking: { mode: 'create_or_link', match_by: 'verified_email' },
      connection: {
        issuer: 'https://issuer.example.com',
        client_id: 'client-id',
        client_secret: 'secret',
      },
      mapping: {
        claims: {
          email: 'claims.email',
        },
      },
      created_at: '2026-04-02T00:00:00Z',
      updated_at: '2026-04-02T01:00:00Z',
    })

    const wrapper = shallowMount(ProviderDetailView, {
      global: {
        stubs: {
          RouterLink: {
            template: '<a><slot /></a>',
          },
        },
      },
    })
    await flushPromises()

    expect(mocked.apiGet).toHaveBeenCalledWith('/v1/providers/prov-1')
    expect(wrapper.text()).toContain('Mock OIDC')
    expect(wrapper.text()).toContain('custom')
  })

  it('shows a load error when the provider fetch fails', async () => {
    mocked.apiGet.mockRejectedValue(new Error('Provider fetch failed'))

    const wrapper = shallowMount(ProviderDetailView, {
      global: {
        stubs: {
          RouterLink: {
            template: '<a><slot /></a>',
          },
        },
      },
    })
    await flushPromises()

    expect(wrapper.text()).toContain('Provider fetch failed')
  })
})
