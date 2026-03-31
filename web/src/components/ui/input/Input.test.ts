import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import Input from './Input.vue'

describe('Input', () => {
  it('emits update:modelValue when the value changes', async () => {
    const wrapper = mount(Input, {
      props: {
        modelValue: '',
      },
    })

    await wrapper.find('input').setValue('james@example.com')

    expect(wrapper.emitted('update:modelValue')).toEqual([
      ['james@example.com'],
    ])
  })
})
