import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { createRouter, createWebHistory } from 'vue-router'
import IdentityListView from './IdentityListView.vue'

// Mock the API layer.
vi.mock('@/api/resources', () => ({
  identityApi: { list: vi.fn() },
}))

import { identityApi } from '@/api/resources'

// Minimal router for tests.
function makeRouter() {
  return createRouter({
    history: createWebHistory(),
    routes: [
      { path: '/', component: { template: '<div />' } },
      { path: '/identities/:id', component: { template: '<div />' } },
      { path: '/identities/new', component: { template: '<div />' } },
    ],
  })
}

describe('IdentityListView', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('renders "No identities found" when empty', async () => {
    ;(identityApi.list as any).mockResolvedValue([])

    const router = makeRouter()
    await router.push('/')
    await router.isReady()

    const wrapper = mount(IdentityListView, {
      global: { plugins: [router] },
    })
    await flushPromises()

    expect(wrapper.find('.empty').text()).toBe('No identities found')
    expect(wrapper.find('h3').text()).toBe('0 identities')
  })

  it('renders identity rows from API', async () => {
    ;(identityApi.list as any).mockResolvedValue([
      { id: '1', identifier: 'admin@test.com', display_name: 'Admin', state: 'active', created_at: '2026-01-01T00:00:00Z' },
      { id: '2', identifier: 'user@test.com', display_name: 'User', state: 'deactivated', created_at: '2026-01-02T00:00:00Z' },
    ])

    const router = makeRouter()
    await router.push('/')
    await router.isReady()

    const wrapper = mount(IdentityListView, {
      global: { plugins: [router] },
    })
    await flushPromises()

    expect(wrapper.find('h3').text()).toBe('2 identities')
    const rows = wrapper.findAll('tbody tr')
    expect(rows).toHaveLength(2)

    // First row.
    expect(rows[0].find('.identifier').text()).toBe('admin@test.com')
    expect(rows[0].find('.badge').text()).toBe('active')
    expect(rows[0].find('.badge').classes()).toContain('active')

    // Second row.
    expect(rows[1].find('.identifier').text()).toBe('user@test.com')
    expect(rows[1].find('.badge').classes()).toContain('deactivated')
  })

  it('has a "New Identity" link', async () => {
    ;(identityApi.list as any).mockResolvedValue([])

    const router = makeRouter()
    await router.push('/')
    await router.isReady()

    const wrapper = mount(IdentityListView, {
      global: { plugins: [router] },
    })

    const newButton = wrapper.find('.btn-primary')
    expect(newButton.text()).toBe('+ New Identity')
    expect(newButton.attributes('href')).toBe('/identities/new')
  })
})
