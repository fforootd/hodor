import { beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import ZitadelIdentityDetail from './zitadel-identity-detail.ce.vue'

const mocks = vi.hoisted(() => ({
  mockGet: vi.fn(),
  mockDelete: vi.fn(),
  dispatchWCEvent: vi.fn(),
  notifyMutationError: vi.fn(),
  notifyMutationSuccess: vi.fn(),
}))

vi.mock('@/wc/wc-api-client', () => ({
  createWCApiClient: () => ({
    get: mocks.mockGet,
    delete: mocks.mockDelete,
    patch: vi.fn(),
  }),
}))

vi.mock('@/wc/host-utils', () => ({
  dispatchWCEvent: mocks.dispatchWCEvent,
  resolveApiBase: (value: string) => value,
  isDarkMode: () => false,
}))

vi.mock('@/lib/notify', () => ({
  notifyMutationError: mocks.notifyMutationError,
  notifyMutationSuccess: mocks.notifyMutationSuccess,
}))

describe('zitadel-identity-detail', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mocks.mockGet.mockResolvedValue({
      id: 'user_1',
      identifier: 'ada@example.com',
      display_name: 'Ada Lovelace',
      state: 'active',
      profile: { first_name: 'Ada' },
      schema_name: 'human_user',
      created_at: '2026-01-01T00:00:00Z',
      updated_at: '2026-01-02T00:00:00Z',
    })
    mocks.mockDelete.mockResolvedValue(undefined)
  })

  it('uses a styled delete dialog instead of browser confirm and keeps delete side effects', async () => {
    const confirmSpy = vi.fn()
    ;(window as any).confirm = confirmSpy

    const wrapper = mount(ZitadelIdentityDetail, {
      props: {
        identityId: 'user_1',
        editable: true,
      },
      global: {
        stubs: {
          WCToaster: { template: '<div class="wc-toaster" />' },
        },
      },
    })

    await flushPromises()

    const deleteButtons = wrapper.findAll('button').filter((button) => button.text() === 'Delete')
    expect(deleteButtons).toHaveLength(1)

    await deleteButtons[0].trigger('click')
    expect(confirmSpy).not.toHaveBeenCalled()
    expect(wrapper.text()).toContain('Delete Identity')

    const dialog = wrapper.find('[role="dialog"]')
    const confirmButton = dialog.findAll('button').find((button) => button.text() === 'Delete')
    expect(confirmButton).toBeDefined()

    await confirmButton!.trigger('click')
    await flushPromises()

    expect(mocks.mockDelete).toHaveBeenCalledWith('/v1/users/user_1')
    expect(mocks.dispatchWCEvent).toHaveBeenCalledWith('zitadel-identity-detail', 'identity-deleted', {
      id: 'user_1',
    })
    expect(mocks.notifyMutationSuccess).toHaveBeenCalledWith('Identity', 'delete')
  })

  it('shows an error toast when delete fails', async () => {
    mocks.mockDelete.mockRejectedValueOnce(new Error('Delete failed'))

    const wrapper = mount(ZitadelIdentityDetail, {
      props: {
        identityId: 'user_1',
        editable: true,
      },
      global: {
        stubs: {
          WCToaster: { template: '<div class="wc-toaster" />' },
        },
      },
    })

    await flushPromises()

    await wrapper.findAll('button').find((button) => button.text() === 'Delete')!.trigger('click')
    await wrapper.find('[role="dialog"]').findAll('button').find((button) => button.text() === 'Delete')!.trigger('click')
    await flushPromises()

    expect(mocks.notifyMutationError).toHaveBeenCalledWith('Identity', 'delete', expect.any(Error))
  })
})
