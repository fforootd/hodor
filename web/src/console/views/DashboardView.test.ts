import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import DashboardView from './DashboardView.vue'

// Mock the API layer.
vi.mock('@/api/resources', () => ({
  entityApi: { list: vi.fn() },
  schemaApi: { list: vi.fn() },
  sessionApi: { list: vi.fn() },
  eventApi: { list: vi.fn() },
}))

import { entityApi, schemaApi, sessionApi, eventApi } from '@/api/resources'

describe('DashboardView', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('renders stat cards with zeros initially', () => {
    // APIs never resolve — test initial state.
    ;(entityApi.list as any).mockReturnValue(new Promise(() => {}))
    ;(schemaApi.list as any).mockReturnValue(new Promise(() => {}))
    ;(sessionApi.list as any).mockReturnValue(new Promise(() => {}))
    ;(eventApi.list as any).mockReturnValue(new Promise(() => {}))

    const wrapper = mount(DashboardView)
    const statValues = wrapper.findAll('.stat-value')
    expect(statValues).toHaveLength(4)
    statValues.forEach((el) => {
      expect(el.text()).toBe('0')
    })
  })

  it('loads and displays stats from API', async () => {
    ;(entityApi.list as any).mockResolvedValue([{ id: 1 }, { id: 2 }])
    ;(schemaApi.list as any).mockResolvedValue([{ id: 's1' }])
    ;(sessionApi.list as any).mockResolvedValue([{ id: 'ses1' }, { id: 'ses2' }, { id: 'ses3' }])
    ;(eventApi.list as any).mockResolvedValue([
      { id: 'e1', event_type: 'identity.created', created_at: '2026-01-01T00:00:00Z' },
    ])

    const wrapper = mount(DashboardView)
    await flushPromises()

    const statValues = wrapper.findAll('.stat-value')
    expect(statValues[0].text()).toBe('2')  // identities
    expect(statValues[1].text()).toBe('3')  // sessions
    expect(statValues[2].text()).toBe('1')  // events
    expect(statValues[3].text()).toBe('1')  // schemas
  })

  it('displays "No events yet" when empty', async () => {
    ;(entityApi.list as any).mockResolvedValue([])
    ;(schemaApi.list as any).mockResolvedValue([])
    ;(sessionApi.list as any).mockResolvedValue([])
    ;(eventApi.list as any).mockResolvedValue([])

    const wrapper = mount(DashboardView)
    await flushPromises()

    expect(wrapper.find('.empty').text()).toBe('No events yet')
  })

  it('renders recent events with type badges', async () => {
    ;(entityApi.list as any).mockResolvedValue([])
    ;(schemaApi.list as any).mockResolvedValue([])
    ;(sessionApi.list as any).mockResolvedValue([])
    ;(eventApi.list as any).mockResolvedValue([
      { id: '1', event_type: 'identity.created', created_at: '2026-01-01T10:00:00Z' },
      { id: '2', event_type: 'session.deleted', created_at: '2026-01-01T11:00:00Z' },
    ])

    const wrapper = mount(DashboardView)
    await flushPromises()

    const rows = wrapper.findAll('.event-row')
    expect(rows).toHaveLength(2)
    expect(rows[0].find('.event-type').classes()).toContain('created')
    expect(rows[1].find('.event-type').classes()).toContain('deleted')
  })
})
