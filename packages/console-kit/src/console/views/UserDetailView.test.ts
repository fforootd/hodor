import { beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import { createMemoryHistory, createRouter } from 'vue-router'
import { computed, defineComponent, h, inject, provide, ref, toRef, watch, type Ref } from 'vue'

const mocked = vi.hoisted(() => ({
  analyticsPost: vi.fn(),
  eventApiList: vi.fn(),
  loadResourceSchemaContext: vi.fn(),
  magicLinkSend: vi.fn(),
  orgApiList: vi.fn(),
  orgMembersAdd: vi.fn(),
  orgMembersRemove: vi.fn(),
  sessionApiList: vi.fn(),
  sessionApiRevoke: vi.fn(),
  userApiDelete: vi.fn(),
  userApiGet: vi.fn(),
  userApiSetPassword: vi.fn(),
  userApiUpdate: vi.fn(),
}))

vi.mock('@/api/resources', () => ({
  userApi: {
    get: mocked.userApiGet,
    update: mocked.userApiUpdate,
    setPassword: mocked.userApiSetPassword,
    delete: mocked.userApiDelete,
  },
  sessionApi: {
    list: mocked.sessionApiList,
    revoke: mocked.sessionApiRevoke,
  },
  eventApi: {
    list: mocked.eventApiList,
  },
  orgApi: {
    list: mocked.orgApiList,
  },
  orgMembersApi: {
    add: mocked.orgMembersAdd,
    remove: mocked.orgMembersRemove,
  },
  magicLinkApi: {
    send: mocked.magicLinkSend,
  },
}))

vi.mock('@/api/client', () => ({
  api: {
    post: mocked.analyticsPost,
  },
}))

vi.mock('@/console/composables/useOrgContext', () => ({
  useOrgContext: () => ({
    currentOrgId: ref('org-home'),
  }),
}))

vi.mock('@/lib/notify', () => ({
  notifyError: vi.fn(),
  notifyMutationError: vi.fn(),
  notifyMutationSuccess: vi.fn(),
  notifySuccess: vi.fn(),
}))

vi.mock('@/console/utils/schema-resource', async () => {
  const actual = await vi.importActual<typeof import('@/console/utils/schema-resource')>('@/console/utils/schema-resource')
  return {
    ...actual,
    loadResourceSchemaContext: mocked.loadResourceSchemaContext,
  }
})

vi.mock('lucide-vue-next', () => {
  const Icon = { template: '<span class="icon" />' }
  return {
    Activity: Icon,
    ArrowLeft: Icon,
    Ban: Icon,
    CheckCircle2: Icon,
    Clock: Icon,
    Code2: Icon,
    KeyRound: Icon,
    Mail: Icon,
    Monitor: Icon,
    Pencil: Icon,
    Plus: Icon,
    Route: Icon,
    Trash2: Icon,
    X: Icon,
  }
})

import UserDetailView from './UserDetailView.vue'

const dialogState = Symbol('dialog-state')
const tabsState = Symbol('tabs-state')

const ButtonStub = defineComponent({
  props: {
    asChild: { type: Boolean, default: false },
    disabled: { type: Boolean, default: false },
  },
  emits: ['click'],
  setup(props, { attrs, emit, slots }) {
    return () => (
      props.asChild
        ? h('div', attrs, slots.default?.())
        : h('button', {
          ...attrs,
          disabled: props.disabled,
          onClick: (event: MouseEvent) => emit('click', event),
        }, slots.default?.())
    )
  },
})

const SimpleWrapper = (tag: string, className: string) => defineComponent({
  setup(_, { attrs, slots }) {
    return () => h(tag, { ...attrs, class: [className, attrs.class] }, slots.default?.())
  },
})

const InputStub = defineComponent({
  props: {
    autocomplete: { type: String, default: '' },
    id: { type: String, default: '' },
    modelValue: { type: String, default: '' },
    placeholder: { type: String, default: '' },
    type: { type: String, default: 'text' },
  },
  emits: ['update:modelValue'],
  setup(props, { emit }) {
    return () => h('input', {
      autocomplete: props.autocomplete,
      id: props.id,
      placeholder: props.placeholder,
      type: props.type,
      value: props.modelValue,
      onInput: (event: Event) => emit('update:modelValue', (event.target as HTMLInputElement).value),
    })
  },
})

const DialogStub = defineComponent({
  props: { open: { type: Boolean, default: false } },
  setup(props, { slots }) {
    provide(dialogState, toRef(props, 'open'))
    return () => h('div', { class: 'dialog-root' }, slots.default?.())
  },
})

const DialogContentStub = defineComponent({
  setup(_, { slots }) {
    const open = inject<Ref<boolean>>(dialogState, ref(true))
    return () => open.value ? h('div', { class: 'dialog-content' }, slots.default?.()) : null
  },
})

const DialogScaffold = SimpleWrapper('div', 'dialog-scaffold')

const SchemaTabsEditorStub = defineComponent({
  props: {
    modelValue: {
      type: Object,
      required: true,
    },
  },
  emits: ['update:modelValue', 'update:jsonValid'],
  setup(props, { emit }) {
    return () => h('div', { class: 'schema-tabs-stub' }, [
      h('pre', { class: 'schema-json' }, JSON.stringify(props.modelValue)),
      h('button', {
        class: 'schema-tabs-update',
        onClick: () => {
          emit('update:modelValue', {
            ...props.modelValue,
            locale: 'de-CH',
          })
          emit('update:jsonValid', true)
        },
      }, 'Update form'),
    ])
  },
})

const DropdownItemStub = defineComponent({
  emits: ['click'],
  setup(_, { attrs, emit, slots }) {
    return () => h('button', {
      ...attrs,
      onClick: (event: MouseEvent) => emit('click', event),
    }, slots.default?.())
  },
})

const TabsStub = defineComponent({
  props: {
    modelValue: { type: String, default: '' },
  },
  emits: ['update:modelValue'],
  setup(props, { emit, slots }) {
    const current = ref(props.modelValue)
    watch(() => props.modelValue, (value) => {
      current.value = value
    })
    provide(tabsState, {
      current,
      setCurrent: (value: string) => emit('update:modelValue', value),
    })
    return () => h('div', { class: 'tabs-root' }, slots.default?.())
  },
})

const TabsListStub = SimpleWrapper('div', 'tabs-list')

const TabsTriggerStub = defineComponent({
  props: {
    value: { type: String, required: true },
  },
  setup(props, { attrs, slots }) {
    const tabs = inject<{ current: Ref<string>; setCurrent: (value: string) => void }>(tabsState)
    return () => h('button', {
      ...attrs,
      'data-active': tabs?.current.value === props.value ? 'true' : 'false',
      onClick: () => tabs?.setCurrent(props.value),
    }, slots.default?.())
  },
})

const TabsContentStub = defineComponent({
  props: {
    value: { type: String, required: true },
  },
  setup(props, { attrs, slots }) {
    const tabs = inject<{ current: Ref<string> }>(tabsState)
    const visible = computed(() => tabs?.current.value === props.value)
    return () => visible.value ? h('div', attrs, slots.default?.()) : null
  },
})

const stubs = {
  Avatar: SimpleWrapper('div', 'avatar'),
  AvatarFallback: SimpleWrapper('div', 'avatar-fallback'),
  AvatarImage: defineComponent({
    props: {
      alt: { type: String, default: '' },
      src: { type: String, default: '' },
    },
    setup(props) {
      return () => h('img', { alt: props.alt, src: props.src })
    },
  }),
  Badge: SimpleWrapper('span', 'badge'),
  Button: ButtonStub,
  Card: SimpleWrapper('section', 'card'),
  CardContent: SimpleWrapper('div', 'card-content'),
  CardHeader: SimpleWrapper('div', 'card-header'),
  CardTitle: SimpleWrapper('div', 'card-title'),
  Dialog: DialogStub,
  DialogContent: DialogContentStub,
  DialogDescription: DialogScaffold,
  DialogFooter: DialogScaffold,
  DialogHeader: DialogScaffold,
  DialogTitle: DialogScaffold,
  DropdownMenu: SimpleWrapper('div', 'dropdown'),
  DropdownMenuContent: SimpleWrapper('div', 'dropdown-content'),
  DropdownMenuItem: DropdownItemStub,
  DropdownMenuTrigger: SimpleWrapper('div', 'dropdown-trigger'),
  Input: InputStub,
  SchemaTabsEditor: SchemaTabsEditorStub,
  Separator: { template: '<hr class="separator" />' },
  StateBadge: defineComponent({
    props: { state: { type: String, default: '' } },
    setup(props) {
      return () => h('span', { class: 'state-badge' }, props.state)
    },
  }),
  Tabs: TabsStub,
  TabsContent: TabsContentStub,
  TabsList: TabsListStub,
  TabsTrigger: TabsTriggerStub,
}

function makeRouter(initialPath = '/users/user-1') {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/users', component: { template: '<div>Users</div>' } },
      { path: '/users/:id', component: UserDetailView },
      { path: '/sessions', component: { template: '<div>Sessions</div>' } },
      { path: '/events', component: { template: '<div>Events</div>' } },
      { path: '/traces', component: { template: '<div>Traces</div>' } },
      { path: '/s/:schemaType', component: { template: '<div>Schema list</div>' } },
      { path: '/s/:schemaType/:id', component: UserDetailView },
    ],
  })

  return router.push(initialPath).then(() => router.isReady()).then(() => router)
}

function makeIdentity(overrides: Record<string, any> = {}) {
  return {
    id: 'user-1',
    identifier: 'james@example.com',
    display_name: 'James Smith',
    state: 'active',
    schema_id: 'schema-human',
    schema_type: 'human_user',
    created_at: '2026-03-29T10:00:00Z',
    updated_at: '2026-03-30T08:00:00Z',
    data: {
      email: 'james@example.com',
      locale: 'en-US',
      timezone: 'Europe/Zurich',
    },
    orgs: [
      { org_id: 'org-1', org_name: 'Acme Corp', role: 'member', added_at: '2026-03-01T00:00:00Z' },
    ],
    capabilities: ['reset_password'],
    ...overrides,
  }
}

function makeSchemaContext(overrides: Record<string, any> = {}) {
  return {
    display: { singular: 'Human User' },
    schemaId: 'schema-human',
    schemaType: 'human_user',
    versions: [],
    schema: {
      properties: {
        email: { type: 'string' },
        locale: { type: 'string' },
        timezone: { type: 'string' },
      },
      'x-auth-methods': {
        password: { enabled: true, interactive: true, position: 1 },
        magic_link: { enabled: true, interactive: true, position: 2 },
        pat: { enabled: false, interactive: false },
      },
    },
    ...overrides,
  }
}

function makeTracePreviewRows() {
  return {
    columns: ['trace_group', 'request_id', 'session_id', 'started_at', 'span_count', 'client_id', 'fingerprint', 'sample_payload'],
    rows: [
      [
        'trace-1',
        'req-1',
        'sess-1',
        '2026-03-30T09:10:00Z',
        3,
        'app-1',
        'fp-1',
        JSON.stringify({ method: 'POST', path: '/v1/login', status: 200, duration_ms: 42 }),
      ],
    ],
  }
}

async function mountView(options: {
  identity?: Record<string, any>
  path?: string
  schemaContext?: Record<string, any>
  sessions?: Array<Record<string, any>>
  events?: Array<Record<string, any>>
} = {}) {
  const identity = makeIdentity(options.identity)

  mocked.userApiGet.mockResolvedValue(identity)
  mocked.userApiUpdate.mockImplementation(async (_id: string, body: any) => ({
    ...identity,
    data: body.data,
  }))
  mocked.userApiSetPassword.mockResolvedValue(undefined)
  mocked.userApiDelete.mockResolvedValue(undefined)
  mocked.sessionApiList.mockResolvedValue(options.sessions || [
    {
      id: 'sess-1',
      user_id: identity.id,
      state: 'active',
      created_at: '2026-03-30T07:00:00Z',
      expires_at: '2027-04-06T07:00:00Z',
      user_agent: 'Chrome on macOS',
      ip_address: '203.0.113.1',
    },
  ])
  mocked.sessionApiRevoke.mockResolvedValue(undefined)
  mocked.eventApiList.mockResolvedValue(options.events || [
    {
      id: 'evt-1',
      event_type: 'user.updated',
      actor_id: identity.id,
      aggregate_id: identity.id,
      aggregate_type: 'user',
      payload: {},
      created_at: '2026-03-30T09:30:00Z',
    },
  ])
  mocked.orgApiList.mockResolvedValue([
    { id: 'org-1', name: 'Acme Corp' },
    { id: 'org-2', name: 'Secondary Org' },
  ])
  mocked.orgMembersAdd.mockResolvedValue(undefined)
  mocked.orgMembersRemove.mockResolvedValue(undefined)
  mocked.magicLinkSend.mockResolvedValue(undefined)
  mocked.analyticsPost.mockResolvedValue(makeTracePreviewRows())
  mocked.loadResourceSchemaContext.mockResolvedValue(makeSchemaContext(options.schemaContext))

  const router = await makeRouter(options.path)
  const wrapper = mount(UserDetailView, {
    global: {
      plugins: [router],
      stubs,
    },
  })

  for (let index = 0; index < 5; index += 1) {
    await flushPromises()
  }

  return { router, wrapper }
}

describe('UserDetailView', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('shows password and invite actions for a human user, opens edit mode, saves, and keeps canonical drilldowns', async () => {
    const { wrapper } = await mountView()

    expect(wrapper.find('[data-testid="set-password"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="send-invite"]').exists()).toBe(true)

    await wrapper.find('[data-testid="tab-security"]').trigger('click')
    await flushPromises()
    expect(wrapper.find('a[href="/sessions?user_id=user-1"]').exists()).toBe(true)

    await wrapper.find('[data-testid="tab-activity"]').trigger('click')
    await flushPromises()
    expect(wrapper.find('a[href="/events?aggregate_id=user-1"]').exists()).toBe(true)
    expect(wrapper.find('a[href="/traces?actor_id=user-1"]').exists()).toBe(true)

    expect(wrapper.find('.schema-tabs-stub').exists()).toBe(false)
    await wrapper.find('[data-testid="edit-user"]').trigger('click')
    await flushPromises()

    expect(wrapper.find('.schema-tabs-stub').exists()).toBe(true)

    await wrapper.find('.schema-tabs-update').trigger('click')
    await flushPromises()
    await wrapper.find('[data-testid="save-user"]').trigger('click')
    await flushPromises()

    expect(mocked.userApiUpdate).toHaveBeenCalledWith('user-1', expect.objectContaining({
      data: expect.objectContaining({
        email: 'james@example.com',
        locale: 'de-CH',
      }),
      identifier: 'james@example.com',
    }))
  })

  it('renders the identity shell before secondary lookups settle', async () => {
    const identity = makeIdentity()
    mocked.userApiGet.mockResolvedValue(identity)
    mocked.userApiUpdate.mockResolvedValue(identity)
    mocked.userApiSetPassword.mockResolvedValue(undefined)
    mocked.userApiDelete.mockResolvedValue(undefined)
    mocked.sessionApiList.mockReturnValue(new Promise(() => {}))
    mocked.eventApiList.mockReturnValue(new Promise(() => {}))
    mocked.orgApiList.mockReturnValue(new Promise(() => {}))
    mocked.analyticsPost.mockReturnValue(new Promise(() => {}))
    mocked.loadResourceSchemaContext.mockReturnValue(new Promise(() => {}))
    mocked.orgMembersAdd.mockResolvedValue(undefined)
    mocked.orgMembersRemove.mockResolvedValue(undefined)
    mocked.magicLinkSend.mockResolvedValue(undefined)

    const router = await makeRouter('/users/user-1')
    const wrapper = mount(UserDetailView, {
      global: {
        plugins: [router],
        stubs,
      },
    })

    for (let index = 0; index < 5; index += 1) {
      await flushPromises()
    }

    expect(wrapper.text()).toContain('James Smith')
    expect(wrapper.text()).not.toContain('Loading identity…')
    expect(wrapper.text()).toContain('Loading…')
  })

  it('keeps facts and drilldowns visible for service users and AI agents while hiding unsupported actions', async () => {
    const service = await mountView({
      identity: {
        id: 'svc-1',
        identifier: 'service-runner',
        display_name: 'Sync Service',
        schema_type: 'service_user',
        schema_id: 'schema-service',
        data: { description: 'Rotates customer tokens' },
        capabilities: ['issue_pat'],
      },
      path: '/users/svc-1',
      schemaContext: {
        display: { singular: 'Service User' },
        schemaId: 'schema-service',
        schemaType: 'service_user',
        schema: {
          properties: {
            description: { type: 'string' },
          },
          'x-auth-methods': {
            pat: { enabled: true, interactive: false },
            password: { enabled: true, interactive: true, position: 1 },
          },
        },
      },
      sessions: [],
      events: [],
    })

    expect(service.wrapper.text()).toContain('Rotates customer tokens')
    expect(service.wrapper.find('[data-testid="send-invite"]').exists()).toBe(false)
    expect(service.wrapper.find('[data-testid="set-password"]').exists()).toBe(true)
    await service.wrapper.find('[data-testid="tab-activity"]').trigger('click')
    await flushPromises()
    expect(service.wrapper.find('a[href="/traces?actor_id=svc-1"]').exists()).toBe(true)

    service.wrapper.unmount()

    const agent = await mountView({
      identity: {
        id: 'agent-1',
        identifier: 'agent://fraud-check',
        display_name: 'Fraud Agent',
        schema_type: 'ai_agent',
        schema_id: 'schema-agent',
        data: { model: 'gpt-5.4' },
        capabilities: ['issue_pat'],
      },
      path: '/users/agent-1',
      schemaContext: {
        display: { singular: 'AI Agent' },
        schemaId: 'schema-agent',
        schemaType: 'ai_agent',
        schema: {
          properties: {
            model: { type: 'string' },
          },
          'x-auth-methods': {
            pat: { enabled: true, interactive: false },
          },
        },
      },
      sessions: [],
      events: [],
    })

    expect(agent.wrapper.text()).toContain('gpt-5.4')
    expect(agent.wrapper.find('[data-testid="send-invite"]').exists()).toBe(false)
    expect(agent.wrapper.find('[data-testid="set-password"]').exists()).toBe(false)
    await agent.wrapper.find('[data-testid="tab-security"]').trigger('click')
    await flushPromises()
    expect(agent.wrapper.find('a[href="/sessions?user_id=agent-1"]').exists()).toBe(true)
  })

  it('revokes preview sessions and preserves the password + delete flows', async () => {
    const { router, wrapper } = await mountView()

    await wrapper.find('[data-testid="tab-security"]').trigger('click')
    await flushPromises()
    await wrapper.find('[data-testid="revoke-session-sess-1"]').trigger('click')
    await flushPromises()

    expect(mocked.sessionApiRevoke).toHaveBeenCalledWith('sess-1')
    expect(wrapper.find('[data-testid="revoke-session-sess-1"]').exists()).toBe(false)
    expect(wrapper.text()).toContain('revoked')

    await wrapper.find('[data-testid="set-password"]').trigger('click')
    await flushPromises()
    await wrapper.find('input[placeholder="New password"]').setValue('StrongerPass!42')
    await wrapper.find('[data-testid="confirm-set-password"]').trigger('click')
    await flushPromises()

    expect(mocked.userApiSetPassword).toHaveBeenCalledWith('user-1', 'StrongerPass!42')

    await wrapper.find('[data-testid="delete-user"]').trigger('click')
    await flushPromises()
    await wrapper.find('[data-testid="confirm-delete-user"]').trigger('click')
    await flushPromises()

    expect(mocked.userApiDelete).toHaveBeenCalledWith('user-1')
    expect(router.currentRoute.value.fullPath).toBe('/users')
  })
})
