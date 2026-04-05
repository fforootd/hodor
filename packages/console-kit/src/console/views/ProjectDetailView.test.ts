import { computed, defineComponent, h, ref } from 'vue'
import { mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'

vi.mock('@/console/composables/useOrgContext', () => ({
  useOrgContext: () => ({
    currentOrgId: ref('org-demo'),
  }),
}))

vi.mock('@/console/composables/useResourceDetail', () => ({
  useResourceDetail: () => ({
    item: ref({
      id: 'project_1',
      name: 'Console Project',
      state: 'active',
      schema_type: 'project',
      created_at: '2026-01-01T00:00:00Z',
      updated_at: '2026-01-02T00:00:00Z',
    }),
    members: ref([
      { user_id: 'user_1', display_name: 'Ada', role: 'member' },
    ]),
    formData: ref({ name: 'Console Project', description: 'Admin workspace' }),
    schemaContext: ref({
      schema: {
        type: 'object',
        properties: {
          name: { type: 'string' },
        },
      },
      schemaType: 'project',
      display: {},
    }),
    loading: ref(false),
    saving: ref(false),
    deleting: ref(false),
    jsonValid: ref(true),
    jsonContent: ref('{}'),
    jsonError: ref(''),
    loadError: ref(''),
    curlSnippets: computed(() => []),
    overviewFacts: computed(() => [{ label: 'Name', value: 'Console Project' }]),
    save: vi.fn(),
    deleteResource: vi.fn(),
    addMember: vi.fn(),
    removeMember: vi.fn(),
    onJsonValid: vi.fn(),
    onJsonError: vi.fn(),
  }),
}))

import ProjectDetailView from './ProjectDetailView.vue'

const ResourceDetailCockpitStub = defineComponent({
  props: {
    extraTabs: { type: Array, default: () => [] },
  },
  setup(props, { slots }) {
    return () => h('div', { class: 'resource-detail-cockpit' }, [
      h('div', { class: 'tabs' }, [
        h('span', 'Overview'),
        ...(props.extraTabs as Array<{ label: string }>).map((tab) => h('span', tab.label)),
        h('span', 'Edit & API'),
      ]),
      slots['tab-members']?.(),
      slots['edit-form']?.(),
    ])
  },
})

describe('ProjectDetailView', () => {
  it('renders the aligned members tab and dedicated members section', () => {
    const wrapper = mount(ProjectDetailView, {
      global: {
        stubs: {
          ResourceDetailCockpit: ResourceDetailCockpitStub,
          ResourceMembersSection: defineComponent({
            props: { members: { type: Array, default: () => [] } },
            setup(props) {
              return () => h('div', `members:${props.members.length}`)
            },
          }),
          SchemaFieldEditor: defineComponent({ setup: () => () => h('div', 'schema-editor') }),
        },
      },
    })

    expect(wrapper.text()).toContain('Overview')
    expect(wrapper.text()).toContain('Members')
    expect(wrapper.text()).toContain('Edit & API')
    expect(wrapper.text()).toContain('members:1')
  })
})
