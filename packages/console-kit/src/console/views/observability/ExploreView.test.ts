import { beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import ExploreView from './ExploreView.vue'

const mocks = vi.hoisted(() => ({
  routerReplace: vi.fn(() => Promise.resolve()),
  apiGet: vi.fn(),
  apiPost: vi.fn(),
  apiDelete: vi.fn(),
  notifyError: vi.fn(),
  notifyMutationError: vi.fn(),
  notifyMutationSuccess: vi.fn(),
  notifySuccess: vi.fn(),
}))

vi.mock('vue-router', () => ({
  useRoute: () => ({ query: {} }),
  useRouter: () => ({ replace: mocks.routerReplace }),
  RouterLink: { template: '<a><slot /></a>' },
}))

vi.mock('@/api/client', () => ({
  api: {
    get: mocks.apiGet,
    post: mocks.apiPost,
    delete: mocks.apiDelete,
  },
}))

vi.mock('@/lib/notify', () => ({
  notifyError: mocks.notifyError,
  notifyMutationError: mocks.notifyMutationError,
  notifyMutationSuccess: mocks.notifyMutationSuccess,
  notifySuccess: mocks.notifySuccess,
}))

vi.mock('@guolao/vue-monaco-editor', () => ({
  VueMonacoEditor: {
    props: ['value'],
    emits: ['update:value'],
    template: '<textarea class="monaco-stub" :value="value" @input="$emit(\'update:value\', $event.target.value)" />',
  },
}))

vi.mock('@unovis/vue', () => ({
  VisXYContainer: { template: '<div><slot /></div>' },
  VisLine: { template: '<div />' },
  VisArea: { template: '<div />' },
  VisAxis: { template: '<div />' },
  VisStackedBar: { template: '<div />' },
}))

vi.mock('lucide-vue-next', () => {
  const icon = { template: '<span class="icon" />' }
  return {
    Play: icon,
    FileJson: icon,
    BarChart3: icon,
    TrendingUp: icon,
    Search: icon,
    Database: icon,
    ExternalLink: icon,
    Save: icon,
    Trash2: icon,
    Bookmark: icon,
  }
})

const stubs = {
  Tabs: { template: '<div><slot /></div>' },
  TabsContent: { template: '<div><slot /></div>' },
  TabsList: { template: '<div><slot /></div>' },
  TabsTrigger: { template: '<button><slot /></button>' },
  Label: { template: '<label><slot /></label>' },
  Card: { template: '<div><slot /></div>' },
  CardHeader: { template: '<div><slot /></div>' },
  CardTitle: { template: '<div><slot /></div>' },
  CardContent: { template: '<div><slot /></div>' },
  Button: {
    props: ['disabled'],
    template: '<button :disabled="disabled" @click="$emit(\'click\', $event)"><slot /></button>',
  },
  Input: {
    props: ['modelValue', 'placeholder'],
    emits: ['update:modelValue'],
    template: '<input :value="modelValue" :placeholder="placeholder" @input="$emit(\'update:modelValue\', $event.target.value)" />',
  },
  Select: { template: '<div><slot /></div>' },
  SelectContent: { template: '<div><slot /></div>' },
  SelectItem: { template: '<div><slot /></div>' },
  SelectTrigger: { template: '<div><slot /></div>' },
  SelectValue: { template: '<div><slot /></div>' },
  Table: { template: '<table><slot /></table>' },
  TableHeader: { template: '<thead><slot /></thead>' },
  TableBody: { template: '<tbody><slot /></tbody>' },
  TableRow: { template: '<tr><slot /></tr>' },
  TableHead: { template: '<th><slot /></th>' },
  TableCell: { template: '<td><slot /></td>' },
  ChartContainer: { template: '<div><slot /></div>' },
  ChartCrosshair: { template: '<div />' },
  Badge: { template: '<span><slot /></span>' },
  DropdownMenu: { template: '<div><slot /></div>' },
  DropdownMenuContent: { template: '<div><slot /></div>' },
  DropdownMenuTrigger: { template: '<div><slot /></div>' },
  Dialog: {
    props: ['open'],
    template: '<div v-if="open" class="dialog"><slot /></div>',
  },
  DialogContent: { template: '<div class="dialog-content"><slot /></div>' },
  DialogDescription: { template: '<p><slot /></p>' },
  DialogFooter: { template: '<div><slot /></div>' },
  DialogHeader: { template: '<div><slot /></div>' },
  DialogTitle: { template: '<h2><slot /></h2>' },
}

describe('ExploreView delete confirmation', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    mocks.apiGet.mockImplementation(async (path: string) => {
      if (path === '/v1/analytics/schema') return { events: { columns: [] } }
      if (path === '/v1/analytics/queries') {
        return {
          items: [
            {
              id: 'sq_1',
              name: 'Recent logins',
              description: 'Latest auth events',
              sql: 'select * from events',
              created_at: '2026-01-01T00:00:00Z',
            },
          ],
        }
      }
      return {}
    })
    mocks.apiPost.mockResolvedValue({ rows: [], columns: [] })
    mocks.apiDelete.mockResolvedValue(undefined)
  })

  it('replaces browser confirm with a dialog before deleting a saved query', async () => {
    const confirmSpy = vi.fn()
    ;(window as any).confirm = confirmSpy

    const wrapper = mount(ExploreView, {
      global: {
        stubs,
      },
    })

    await flushPromises()

    const deleteButton = wrapper.findAll('button').find((button) => {
      const className = String(button.attributes('class') || '')
      return className.includes('hover:text-destructive')
    })
    expect(deleteButton).toBeDefined()

    await deleteButton!.trigger('click')
    expect(confirmSpy).not.toHaveBeenCalled()
    expect(wrapper.text()).toContain('Delete Saved Query')

    const confirmDelete = wrapper.findAll('button').find((button) => button.text() === 'Delete')
    expect(confirmDelete).toBeDefined()

    await confirmDelete!.trigger('click')
    await flushPromises()

    expect(mocks.apiDelete).toHaveBeenCalledWith('/v1/analytics/queries/sq_1')
    expect(mocks.notifyMutationSuccess).toHaveBeenCalledWith('Saved query', 'delete')
  })
})
