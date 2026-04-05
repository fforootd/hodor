import { computed, defineComponent, h, ref } from 'vue'
import { mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'

vi.mock('@/console/composables/useOrgContext', () => ({
  useOrgContext: () => ({
    currentOrgId: ref('org-demo'),
  }),
}))

vi.mock('@/console/composables/useResourceCreate', () => ({
  useResourceCreate: () => ({
    schemaContext: ref({
      schema: {
        type: 'object',
        properties: {
          client_name: { type: 'string' },
          app_type: { type: 'string' },
          redirect_uris: { type: 'array', items: { type: 'string' } },
        },
      },
      schemaType: 'app',
      display: {},
    }),
    formData: ref({
      client_name: 'Console',
      app_type: 'web',
      redirect_uris: ['https://example.com/callback'],
      grant_types: ['authorization_code'],
      response_types: ['code'],
    }),
    jsonValid: ref(true),
    jsonContent: ref('{}'),
    jsonError: ref(''),
    submitting: ref(false),
    error: ref(''),
    payload: computed(() => ({
      data: {
        client_name: 'Console',
        app_type: 'web',
      },
    })),
    curlSnippets: computed(() => []),
    reviewFacts: computed(() => [{ label: 'Client Name', value: 'Console' }]),
    submit: vi.fn(),
    onJsonValid: vi.fn(),
    onJsonError: vi.fn(),
  }),
}))

import AppCreateView from './AppCreateView.vue'

const ResourceCreateCockpitStub = defineComponent({
  props: {
    extraTabs: { type: Array, default: () => [] },
    reviewSummaryCards: { type: Array, default: () => [] },
    summaryCards: { type: Array, default: () => [] },
  },
  setup(props, { slots }) {
    return () => h('div', { class: 'resource-create-cockpit' }, [
      h('div', { class: 'tabs' }, [
        h('span', 'Details'),
        ...(props.extraTabs as Array<{ label: string }>).map((tab) => h('span', tab.label)),
        h('span', 'Review'),
        h('span', 'API'),
      ]),
      h('div', { class: 'summary' }, JSON.stringify(props.summaryCards)),
      h('div', { class: 'review-summary' }, JSON.stringify(props.reviewSummaryCards)),
      slots.details?.(),
      slots['tab-protocol']?.(),
    ])
  },
})

describe('AppCreateView', () => {
  it('renders the cockpit tabs and protocol summary wiring', () => {
    const wrapper = mount(AppCreateView, {
      global: {
        stubs: {
          ResourceCreateCockpit: ResourceCreateCockpitStub,
          SchemaFieldEditor: defineComponent({ setup: () => () => h('div', 'schema-editor') }),
          Card: defineComponent({ setup: (_, { slots }) => () => h('section', slots.default?.()) }),
          CardHeader: defineComponent({ setup: (_, { slots }) => () => h('div', slots.default?.()) }),
          CardTitle: defineComponent({ setup: (_, { slots }) => () => h('div', slots.default?.()) }),
          CardContent: defineComponent({ setup: (_, { slots }) => () => h('div', slots.default?.()) }),
        },
      },
    })

    expect(wrapper.text()).toContain('Details')
    expect(wrapper.text()).toContain('Protocol')
    expect(wrapper.text()).toContain('Review')
    expect(wrapper.text()).toContain('API')
    expect(wrapper.text()).toContain('Protocol posture')
    expect(wrapper.text()).toContain('Grant types')
    expect(wrapper.text()).toContain('authorization_code')
  })
})
