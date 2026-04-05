import { flushPromises, shallowMount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'

const mocked = vi.hoisted(() => ({
  entityCount: vi.fn(),
  get: vi.fn(),
  listByType: vi.fn(),
  update: vi.fn(),
}))

vi.mock('vue-router', () => ({
  useRoute: () => ({
    params: {
      id: 'schema-human-user-v1',
    },
  }),
  useRouter: () => ({
    push: vi.fn(),
  }),
}))

vi.mock('@/api/resources', () => ({
  schemaApi: {
    entityCount: mocked.entityCount,
    get: mocked.get,
    listByType: mocked.listByType,
    update: mocked.update,
  },
}))

import SchemaDetailView from './SchemaDetailView.vue'

describe('SchemaDetailView', () => {
  beforeEach(() => {
    mocked.entityCount.mockReset()
    mocked.get.mockReset()
    mocked.listByType.mockReset()
    mocked.update.mockReset()
  })

  it('loads the schema, its entity count, and its version history', async () => {
    mocked.get.mockResolvedValue({
      id: 'schema-human-user-v1',
      type: 'human_user',
      schema: {
        title: 'Human User',
        type: 'object',
      },
    })
    mocked.entityCount.mockResolvedValue(3)
    mocked.listByType.mockResolvedValue([
      { id: 'schema-human-user-v1', type: 'human_user', schema: { title: 'Human User' } },
    ])

    const wrapper = shallowMount(SchemaDetailView, {
      global: {
        stubs: {
          SchemaAnnotationRenderer: {
            props: ['schemaMeta'],
            template: '<div class="schema-meta">{{ schemaMeta?.id }}</div>',
          },
        },
      },
    })
    await flushPromises()

    const vm = wrapper.vm as unknown as {
      loading: boolean
      schema: { id: string } | null
      entityCount: number
      versionHistory: Array<{ id: string }>
    }

    expect(mocked.get).toHaveBeenCalledWith('schema-human-user-v1')
    expect(mocked.entityCount).toHaveBeenCalledWith('schema-human-user-v1')
    expect(mocked.listByType).toHaveBeenCalledWith('human_user')
    expect(vm.loading).toBe(false)
    expect(vm.schema?.id).toBe('schema-human-user-v1')
    expect(vm.entityCount).toBe(3)
    expect(vm.versionHistory).toEqual([
      { id: 'schema-human-user-v1', type: 'human_user', schema: { title: 'Human User' } },
    ])
  })

  it('renders the empty state when the schema cannot be loaded', async () => {
    mocked.get.mockRejectedValue(new Error('no schema'))
    mocked.entityCount.mockResolvedValue(0)
    mocked.listByType.mockResolvedValue([])

    const wrapper = shallowMount(SchemaDetailView)
    await flushPromises()

    expect(wrapper.text()).toContain('Schema not found')
  })
})
