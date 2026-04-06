import { describe, it, expect, vi, beforeEach } from 'vitest'
import { ref } from 'vue'
import { mount, flushPromises } from '@vue/test-utils'
import DashboardView from './DashboardView.vue'

// Stub all shadcn-vue components as simple pass-through elements.
const stubComponents = {
  Card: { template: '<div class="card"><slot /></div>' },
  CardHeader: { template: '<div class="card-header"><slot /></div>' },
  CardTitle: { template: '<div class="card-title"><slot /></div>' },
  CardContent: { template: '<div class="card-content"><slot /></div>' },
  Badge: { template: '<span class="badge"><slot /></span>' },
  Table: { template: '<table><slot /></table>' },
  TableHeader: { template: '<thead><slot /></thead>' },
  TableBody: { template: '<tbody><slot /></tbody>' },
  TableRow: { template: '<tr><slot /></tr>' },
  TableHead: { template: '<th><slot /></th>' },
  TableCell: { template: '<td><slot /></td>' },
}

// Mock vue-router.
vi.mock('vue-router', () => ({
  useRouter: () => ({ push: vi.fn() }),
}))

// Mock the API client to prevent real HTTP calls.
vi.mock('@/api/client', () => ({
  api: { get: vi.fn().mockRejectedValue(new Error('not available')) },
  getInstanceContext: vi.fn().mockReturnValue(null),
}))

// Mock instance context composable.
vi.mock('@/console/composables/useInstanceContext', () => ({
  useInstanceContext: () => ({
    currentInstanceId: ref(null),
    setInstance: vi.fn(),
  }),
}))

// Stub lucide icons as simple spans.
vi.mock('lucide-vue-next', () => {
  const Icon = { template: '<span class="icon" />' }
  return {
    Users: Icon,
    Building2: Icon,
    AppWindow: Icon,
    FileJson: Icon,
    Globe: Icon,
    Activity: Icon,
    Server: Icon,
    Plus: Icon,
    Search: Icon,
    LayoutGrid: Icon,
  }
})

// Mock the resources module — this is the actual import used by DashboardView.
vi.mock('@/api/resources', () => ({
  countsApi: { get: vi.fn() },
  orgApi: { list: vi.fn() },
  schemaApi: { list: vi.fn() },
  providerApi: { list: vi.fn() },
  eventApi: { list: vi.fn() },
}))

import { countsApi, orgApi, schemaApi, providerApi, eventApi } from '@/api/resources'

describe('DashboardView', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  function mountView() {
    return mount(DashboardView, {
      global: {
        stubs: stubComponents,
      },
    })
  }

  it('renders stat cards with placeholder values initially', () => {
    // APIs never resolve — test initial state.
    vi.mocked(countsApi.get).mockReturnValue(new Promise(() => {}))
    vi.mocked(orgApi.list).mockReturnValue(new Promise(() => {}))
    vi.mocked(schemaApi.list).mockReturnValue(new Promise(() => {}))
    vi.mocked(providerApi.list).mockReturnValue(new Promise(() => {}))
    vi.mocked(eventApi.list).mockReturnValue(new Promise(() => {}))

    const wrapper = mountView()
    const cards = wrapper.findAll('.card')
    // 6 stat cards + 1 events card = 7 total cards
    expect(cards.length).toBeGreaterThanOrEqual(6)

    // Initial stat values should be '—' (em dash placeholder)
    const cardContents = wrapper.findAll('.card-content')
    const statValues = cardContents.slice(0, 6).map((c) => c.find('.text-2xl').text())
    statValues.forEach((val) => {
      expect(val).toBe('—')
    })
  })

  it('loads and displays stats from API', async () => {
    vi.mocked(countsApi.get).mockResolvedValue({ human_user: 5, service_user: 0, ai_agent: 0, app: 2 })
    vi.mocked(orgApi.list).mockResolvedValue([{ id: 'o1', name: 'Default' }] as any)
    vi.mocked(schemaApi.list).mockResolvedValue([{ id: 's1', type: 'user', schema: {}, message: '', org_id: 'o1', created_at: '2026-01-01T00:00:00Z', updated_at: '2026-01-01T00:00:00Z', is_default: false, version: 1 }] as any)
    vi.mocked(providerApi.list).mockResolvedValue([{ id: 'p1', name: 'Google', type: 'oidc', enabled: true, config: {}, created_at: '2026-01-01T00:00:00Z' }, { id: 'p2', name: 'GitHub', type: 'oidc', enabled: true, config: {}, created_at: '2026-01-01T00:00:00Z' }])
    vi.mocked(eventApi.list).mockResolvedValue([
      { id: 'e1', event_type: 'identity.created', created_at: new Date().toISOString(), actor_id: 'a1', aggregate_id: 'agg1', aggregate_type: 'identity', payload: {} },
    ])

    const wrapper = mountView()
    await flushPromises()

    const cardContents = wrapper.findAll('.card-content')
    const statValues = cardContents.slice(0, 6).map((c) => c.find('.text-2xl').text())
    expect(statValues[0]).toBe('5') // users
    expect(statValues[1]).toBe('1') // orgs
    expect(statValues[2]).toBe('2') // apps
    expect(statValues[3]).toBe('1') // schemas
    expect(statValues[4]).toBe('2') // providers
    expect(statValues[5]).toBe('1') // events last 1h
  })

  it('displays "No recent events" when empty', async () => {
    vi.mocked(countsApi.get).mockResolvedValue({})
    vi.mocked(orgApi.list).mockResolvedValue([])
    vi.mocked(schemaApi.list).mockResolvedValue([])
    vi.mocked(providerApi.list).mockResolvedValue([])
    vi.mocked(eventApi.list).mockResolvedValue([])

    const wrapper = mountView()
    await flushPromises()

    expect(wrapper.text()).toContain('No recent events')
  })

  it('renders recent events with badges', async () => {
    vi.mocked(countsApi.get).mockResolvedValue({})
    vi.mocked(orgApi.list).mockResolvedValue([])
    vi.mocked(schemaApi.list).mockResolvedValue([])
    vi.mocked(providerApi.list).mockResolvedValue([])
    vi.mocked(eventApi.list).mockResolvedValue([
      { id: '1', event_type: 'identity.created', created_at: '2026-01-01T10:00:00Z', actor_id: 'a1', aggregate_id: 'agg1', aggregate_type: 'identity', payload: {} },
      { id: '2', event_type: 'session.deleted', created_at: '2026-01-01T11:00:00Z', actor_id: 'a2', aggregate_id: 'agg2', aggregate_type: 'session', payload: {} },
    ])

    const wrapper = mountView()
    await flushPromises()

    const badges = wrapper.findAll('tbody .badge')
    expect(badges).toHaveLength(2)
    expect(badges[0].text()).toBe('identity.created')
    expect(badges[1].text()).toBe('session.deleted')
  })
})
