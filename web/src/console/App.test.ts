import { describe, expect, it, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { createRouter, createWebHistory } from 'vue-router'
import App from './App.vue'

if (!globalThis.localStorage || typeof globalThis.localStorage.getItem !== 'function') {
  const store: Record<string, string> = {}
  Object.defineProperty(globalThis, 'localStorage', {
    value: {
      getItem: (key: string) => store[key] ?? null,
      setItem: (key: string, value: string) => {
        store[key] = value
      },
      removeItem: (key: string) => {
        delete store[key]
      },
      clear: () => {
        Object.keys(store).forEach((key) => delete store[key])
      },
    },
    writable: true,
    configurable: true,
  })
}

vi.mock('@/api/client', () => ({
  api: {
    get: vi.fn(),
  },
}))

vi.mock('@/api/resources', () => ({
  searchApi: {
    search: vi.fn(),
  },
}))

import { api } from '@/api/client'

describe('Console bootstrap', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  async function mountView() {
    const router = createRouter({
      history: createWebHistory(),
      routes: [{ path: '/', name: 'dashboard', component: { template: '<div />' } }],
    })
    await router.push('/')
    await router.isReady()

    return mount(App, {
      global: {
        plugins: [router],
        stubs: {
          AppBootstrapScreen: {
            props: ['appName', 'state'],
            template: '<div class="bootstrap-screen">{{ appName }}:{{ state }}</div>',
          },
          Toaster: true,
        },
      },
    })
  }

  it('shows the bootstrap screen instead of rendering the console shell early', async () => {
    vi.mocked(api.get).mockImplementation(() => new Promise(() => {}))

    const wrapper = await mountView()
    await flushPromises()

    expect(wrapper.find('.bootstrap-screen').text()).toContain('console:initializing')
    expect(wrapper.text()).not.toContain('Dashboard')
  })
})
