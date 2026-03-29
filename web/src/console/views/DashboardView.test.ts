import { describe, it, expect, vi, beforeEach } from 'vitest'
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

// Stub lucide icons as simple spans.
vi.mock('lucide-vue-next', () => ({
  Users: { template: '<span class="icon-users" />' },
  FileJson: { template: '<span class="icon-filejson" />' },
  Globe: { template: '<span class="icon-globe" />' },
  Activity: { template: '<span class="icon-activity" />' },
}))

// Mock the resources module — this is the actual import used by DashboardView.
vi.mock('@/api/resources', () => ({
  countsApi: { get: vi.fn() },
  schemaApi: { list: vi.fn() },
  providerApi: { list: vi.fn() },
  eventApi: { list: vi.fn() },
}))

import { countsApi, schemaApi, providerApi, eventApi } from '@/api/resources'

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
    vi.mocked(schemaApi.list).mockReturnValue(new Promise(() => {}))
    vi.mocked(providerApi.list).mockReturnValue(new Promise(() => {}))
    vi.mocked(eventApi.list).mockReturnValue(new Promise(() => {}))

    const wrapper = mountView()
    const cards = wrapper.findAll('.card')
    // 4 stat cards + 1 events card = 5 total cards
    expect(cards.length).toBeGreaterThanOrEqual(4)

    // Initial stat values should be '—' (em dash placeholder)
    const cardContents = wrapper.findAll('.card-content')
    const statValues = cardContents.slice(0, 4).map((c) => c.find('.text-2xl').text())
    statValues.forEach((val) => {
      expect(val).toBe('—')
    })
  })

  it('loads and displays stats from API', async () => {
    vi.mocked(countsApi.get).mockResolvedValue({ human_user: 2, org: 1 })
    vi.mocked(schemaApi.list).mockResolvedValue([{ id: 's1' }])
    vi.mocked(providerApi.list).mockResolvedValue([{ id: 'p1' }, { id: 'p2' }])
    vi.mocked(eventApi.list).mockResolvedValue([
      { id: 'e1', event_type: 'identity.created', created_at: '2026-01-01T00:00:00Z' },
    ])

    const wrapper = mountView()
    await flushPromises()

    const cardContents = wrapper.findAll('.card-content')
    const statValues = cardContents.slice(0, 4).map((c) => c.find('.text-2xl').text())
    expect(statValues[0]).toBe('3') // identities (2 human_user + 1 org)
    expect(statValues[1]).toBe('1') // schemas
    expect(statValues[2]).toBe('2') // providers
    expect(statValues[3]).toBe('1') // events
  })

  it('displays "No recent events" when empty', async () => {
    vi.mocked(countsApi.get).mockResolvedValue({})
    vi.mocked(schemaApi.list).mockResolvedValue([])
    vi.mocked(providerApi.list).mockResolvedValue([])
    vi.mocked(eventApi.list).mockResolvedValue([])

    const wrapper = mountView()
    await flushPromises()

    expect(wrapper.text()).toContain('No recent events')
  })

  it('renders recent events with badges', async () => {
    vi.mocked(countsApi.get).mockResolvedValue({})
    vi.mocked(schemaApi.list).mockResolvedValue([])
    vi.mocked(providerApi.list).mockResolvedValue([])
    vi.mocked(eventApi.list).mockResolvedValue([
      { id: '1', event_type: 'identity.created', created_at: '2026-01-01T10:00:00Z' },
      { id: '2', event_type: 'session.deleted', created_at: '2026-01-01T11:00:00Z' },
    ])

    const wrapper = mountView()
    await flushPromises()

    const badges = wrapper.findAll('tbody .badge')
    expect(badges).toHaveLength(2)
    expect(badges[0].text()).toBe('identity.created')
    expect(badges[1].text()).toBe('session.deleted')
  })
})
