import { defineComponent, h, ref } from 'vue'
import { mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'

vi.mock('vue-router', () => ({
  useRouter: () => ({ push: vi.fn() }),
}))

vi.mock('@/api/resources', () => ({
  appApi: { create: vi.fn() },
}))

vi.mock('@/lib/notify', () => ({
  notifySuccess: vi.fn(),
  notifyError: vi.fn(),
}))

vi.mock('lucide-vue-next', () => ({
  ArrowLeft: { template: '<span class="icon" />' },
}))

import AppCreateView from './AppCreateView.vue'

describe('AppCreateView', () => {
  it('renders the create form with name input and app type buttons', () => {
    const wrapper = mount(AppCreateView, {
      global: {
        stubs: {
          'router-link': defineComponent({
            props: ['to'],
            setup: (props, { slots }) => () => h('a', { href: props.to }, slots.default?.()),
          }),
          Card: defineComponent({ setup: (_, { slots }) => () => h('section', slots.default?.()) }),
          CardContent: defineComponent({ setup: (_, { slots }) => () => h('div', slots.default?.()) }),
          Input: defineComponent({
            props: ['modelValue', 'id', 'placeholder'],
            setup: (props) => () => h('input', { id: props.id, placeholder: props.placeholder }),
          }),
          Label: defineComponent({
            props: ['for'],
            setup: (props, { slots }) => () => h('label', { for: props.for }, slots.default?.()),
          }),
          Button: defineComponent({
            props: ['disabled', 'variant', 'asChild'],
            setup: (props, { attrs, slots }) => () => h('button', { ...attrs, disabled: props.disabled }, slots.default?.()),
          }),
          Spinner: { template: '<span class="spinner" />' },
        },
      },
    })

    expect(wrapper.text()).toContain('Create Application')
    expect(wrapper.text()).toContain('Name')
    expect(wrapper.text()).toContain('Type')
    expect(wrapper.text()).toContain('Web')
    expect(wrapper.text()).toContain('Native')
    expect(wrapper.text()).toContain('API')
    expect(wrapper.text()).toContain('Machine')
  })
})
