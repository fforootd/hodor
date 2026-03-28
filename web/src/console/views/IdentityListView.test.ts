import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { createRouter, createWebHistory } from 'vue-router'
import IdentityListView from './IdentityListView.vue'

// Stub all shadcn-vue components as simple pass-through elements.
const stubComponents = {
  Button: { template: '<button class="btn"><slot /></button>' },
  Card: { template: '<div class="card"><slot /></div>' },
  CardHeader: { template: '<div class="card-header"><slot /></div>' },
  CardTitle: { template: '<div class="card-title"><slot /></div>' },
  CardContent: { template: '<div class="card-content"><slot /></div>' },
  Badge: { template: '<span class="badge"><slot /></span>' },
  Table: { template: '<table><slot /></table>' },
  TableHeader: { template: '<thead><slot /></thead>' },
  TableBody: { template: '<tbody><slot /></tbody>' },
  TableRow: { template: '<tr class="table-row"><slot /></tr>' },
  TableHead: { template: '<th><slot /></th>' },
  TableCell: { template: '<td class="table-cell"><slot /></td>' },
  Input: { template: '<input class="input" />' },
  Checkbox: { template: '<input type="checkbox" class="checkbox" />' },
  DataTable: {
    props: ['columns', 'data', 'rowSelection'],
    template: `<div class="data-table">
      <slot name="toolbar" :table="mockTable" />
      <template v-if="data && data.length">
        <div v-for="(row, i) in data" :key="i" class="table-row">
          <span class="cell-identifier">{{ row.identifier }}</span>
          <span class="cell-display">{{ row.display_name }}</span>
          <span class="badge">{{ row.state }}</span>
        </div>
      </template>
      <div v-else class="empty-state">No results found.</div>
      <slot name="pagination" :table="mockTable" />
    </div>`,
    setup() {
      const mockTable = {
        getAllColumns: () => [],
        getState: () => ({ columnVisibility: {} }),
        setColumnFilters: () => {},
        getIsAllPageRowsSelected: () => false,
        getIsSomePageRowsSelected: () => false,
        toggleAllPageRowsSelected: () => {},
      }
      return { mockTable }
    },
  },
  DataTablePagination: { template: '<div class="pagination" />' },
  DropdownMenu: { template: '<div class="dropdown"><slot /></div>' },
  DropdownMenuTrigger: { template: '<div class="dropdown-trigger"><slot /></div>' },
  DropdownMenuContent: { template: '<div class="dropdown-content"><slot /></div>' },
  DropdownMenuCheckboxItem: { template: '<div class="dropdown-item"><slot /></div>' },
}

// Stub lucide icons — must include every export used by the view.
vi.mock('lucide-vue-next', () => ({
  Plus: { template: '<span class="icon-plus" />' },
  Search: { template: '<span class="icon-search" />' },
  ChevronDown: { template: '<span class="icon-chevron-down" />' },
  ArrowUpDown: { template: '<span class="icon-arrow-updown" />' },
  ArrowUp: { template: '<span class="icon-arrow-up" />' },
  ArrowDown: { template: '<span class="icon-arrow-down" />' },
  CheckCircle2: { template: '<span class="icon-check" />' },
  XCircle: { template: '<span class="icon-xcircle" />' },
  Ban: { template: '<span class="icon-ban" />' },
}))

/** Create a mock Response compatible with the api client. */
function mockResponse(body: unknown): Response {
  return new Response(JSON.stringify(body), { status: 200, headers: { 'Content-Type': 'application/json' } })
}

// Minimal router for tests.
function makeRouter() {
  return createRouter({
    history: createWebHistory(),
    routes: [
      { path: '/', component: { template: '<div />' } },
      { path: '/identities/:id', component: { template: '<div />' } },
      { path: '/s/:type/new', component: { template: '<div />' } },
    ],
  })
}

describe('IdentityListView', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    // Provide localStorage mock for happy-dom.
    if (!globalThis.localStorage || typeof globalThis.localStorage.getItem !== 'function') {
      const store: Record<string, string> = {}
      Object.defineProperty(globalThis, 'localStorage', {
        value: {
          getItem: (key: string) => store[key] ?? null,
          setItem: (key: string, value: string) => { store[key] = value },
          removeItem: (key: string) => { delete store[key] },
          clear: () => { Object.keys(store).forEach((k) => delete store[k]) },
        },
        writable: true,
        configurable: true,
      })
    }
  })

  async function mountView(fetchImpl: typeof fetch) {
    vi.spyOn(globalThis, 'fetch').mockImplementation(fetchImpl)

    const router = makeRouter()
    await router.push('/')
    await router.isReady()

    const wrapper = mount(IdentityListView, {
      props: { schemaType: 'human_user' },
      global: { plugins: [router], stubs: stubComponents },
    })
    // Multiple flushes to handle chained async operations in onMounted.
    for (let i = 0; i < 5; i++) await flushPromises()
    return wrapper
  }

  it('renders "No human users found" when empty', async () => {
    const wrapper = await mountView(() =>
      Promise.resolve(mockResponse({ items: [] })),
    )
    expect(wrapper.text()).toContain('No results found.')
    expect(wrapper.text()).toContain('0 human users total')
  })

  it('renders identity rows from API', async () => {
    const wrapper = await mountView((url: any) => {
      const urlStr = typeof url === 'string' ? url : url.toString()
      if (urlStr.includes('$meta'))
        return Promise.resolve(mockResponse({}))
      return Promise.resolve(
        mockResponse({
          items: [
            { id: '1', identifier: 'admin@test.com', display_name: 'Admin', state: 'active', created_at: '2026-01-01T00:00:00Z' },
            { id: '2', identifier: 'user@test.com', display_name: 'User', state: 'deactivated', created_at: '2026-01-02T00:00:00Z' },
          ],
        }),
      )
    })

    expect(wrapper.text()).toContain('2 human users total')

    // Should show both identifiers.
    expect(wrapper.text()).toContain('admin@test.com')
    expect(wrapper.text()).toContain('user@test.com')

    // Should show state badges.
    const badges = wrapper.findAll('.badge')
    expect(badges.length).toBeGreaterThanOrEqual(2)
    expect(badges.some((b) => b.text() === 'active')).toBe(true)
    expect(badges.some((b) => b.text() === 'deactivated')).toBe(true)
  })

  it('has a "New" button', async () => {
    const wrapper = await mountView(() =>
      Promise.resolve(mockResponse({ items: [] })),
    )
    const btn = wrapper.find('.btn')
    expect(btn.exists()).toBe(true)
    expect(btn.text()).toContain('New')
  })
})
