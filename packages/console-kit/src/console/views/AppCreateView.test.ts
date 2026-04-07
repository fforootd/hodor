import { defineComponent, h } from 'vue'
import { mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'

vi.mock('vue-router', () => ({
  useRouter: () => ({ push: vi.fn() }),
  useRoute: () => ({ params: {}, name: 'applications' }),
}))

vi.mock('@/api/resources', () => ({
  appApi: { create: vi.fn() },
}))

vi.mock('@/lib/notify', () => ({
  notifySuccess: vi.fn(),
  notifyError: vi.fn(),
}))

vi.mock('lucide-vue-next', () => ({
  Globe: { template: '<span class="icon" />' },
  Smartphone: { template: '<span class="icon" />' },
  Server: { template: '<span class="icon" />' },
  Chrome: { template: '<span class="icon" />' },
  Check: { template: '<span class="icon" />' },
  ChevronRight: { template: '<span class="icon" />' },
  X: { template: '<span class="icon" />' },
  Loader2: { template: '<span class="icon" />' },
}))

import AppCreateView from './AppCreateView.vue'

describe('AppCreateView', () => {
  it('renders the wizard with application type step when open', () => {
    const wrapper = mount(AppCreateView, {
      props: { open: true },
      global: {
        stubs: {
          Sheet: defineComponent({ setup: (_, { slots }) => () => h('div', slots.default?.()) }),
          SheetContent: defineComponent({ setup: (_, { slots }) => () => h('div', slots.default?.()) }),
          RadioGroup: defineComponent({
            props: ['modelValue'],
            setup: (_, { slots }) => () => h('div', slots.default?.()),
          }),
          RadioGroupItem: defineComponent({
            props: ['value'],
            setup: () => () => h('span'),
          }),
          Badge: defineComponent({ setup: (_, { slots }) => () => h('span', slots.default?.()) }),
          Input: defineComponent({
            props: ['modelValue', 'id', 'placeholder'],
            setup: (props) => () => h('input', { id: props.id }),
          }),
          Label: defineComponent({
            props: ['for'],
            setup: (props, { slots }) => () => h('label', { for: props.for }, slots.default?.()),
          }),
          Button: defineComponent({
            props: ['disabled', 'variant', 'size'],
            setup: (props, { attrs, slots }) => () => h('button', { ...attrs, disabled: props.disabled }, slots.default?.()),
          }),
        },
      },
    })

    expect(wrapper.text()).toContain('Create Application')
    expect(wrapper.text()).toContain('Application Type')
    expect(wrapper.text()).toContain('Web Application')
    expect(wrapper.text()).toContain('Native / Mobile')
    expect(wrapper.text()).toContain('API / Machine-to-Machine')
    expect(wrapper.text()).toContain('Browser Extension')
  })
})
