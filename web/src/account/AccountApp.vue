<template>
  <AppBootstrapScreen
    v-if="bootstrapState !== 'ready'"
    app-name="account"
    :state="bootstrapState"
    :error="bootstrapError"
    :retry-delay-ms="bootstrapRetryDelayMs"
    @retry="retryAccountBootstrap"
  />

  <div v-else class="min-h-screen bg-background">
    <!-- Header -->
    <header class="border-b bg-card">
      <div class="mx-auto flex h-14 max-w-3xl items-center justify-between px-4">
        <div class="flex items-center gap-3">
          <Avatar class="size-8">
            <AvatarFallback>{{ initial }}</AvatarFallback>
          </Avatar>
          <div>
            <p class="text-sm font-semibold">{{ profile?.display_name || profile?.identifier || 'My Account' }}</p>
            <p class="text-xs text-muted-foreground">{{ branding.org_name || 'Zitadel' }}</p>
          </div>
        </div>
        <Button variant="outline" size="sm" @click="signOut">
          <LogOut class="mr-1.5 size-3.5" />
          Sign out
        </Button>
      </div>
    </header>

    <main class="mx-auto max-w-3xl px-4 py-6">
      <Tabs default-value="profile" class="space-y-4">
        <TabsList>
          <TabsTrigger value="profile">Profile</TabsTrigger>
          <TabsTrigger value="sessions">Sessions</TabsTrigger>
          <TabsTrigger value="activity">Activity</TabsTrigger>
        </TabsList>

        <!-- PROFILE TAB -->
        <TabsContent value="profile">
          <Card>
            <CardHeader>
              <div class="flex items-center gap-4">
                <Avatar class="size-12">
                  <AvatarFallback class="text-lg">{{ initial }}</AvatarFallback>
                </Avatar>
                <div>
                  <CardTitle>{{ profile.display_name || profile.identifier }}</CardTitle>
                  <p class="text-sm text-muted-foreground mt-0.5">
                    {{ profile.identifier }}
                    <Badge variant="outline" class="ml-2" :class="profile.state === 'active' ? 'border-emerald-300 text-emerald-700' : ''">
                      {{ profile.state }}
                    </Badge>
                  </p>
                </div>
              </div>
            </CardHeader>
            <CardContent class="space-y-4">
              <div v-for="(perm, field) in visibleFields" :key="field" class="space-y-1.5">
                <Label class="flex items-center gap-2">
                  {{ formatLabel(field as string) }}
                  <span v-if="!perm.editable" class="text-xs" :title="'Set by ' + perm.source">🔒</span>
                  <Badge v-if="perm.sensitive" variant="secondary" class="text-[10px] h-4 px-1">sensitive</Badge>
                </Label>
                <div class="flex items-center gap-2">
                  <template v-if="perm.editable">
                    <Input
                      v-model="editFields[field as string]"
                      :type="perm.sensitive && !showSensitive[field as string] ? 'password' : 'text'"
                      :placeholder="field as string"
                    />
                    <Button v-if="perm.sensitive" variant="outline" size="sm" @click="showSensitive[field as string] = !showSensitive[field as string]">
                      {{ showSensitive[field as string] ? 'Hide' : 'Show' }}
                    </Button>
                  </template>
                  <template v-else>
                    <div class="flex-1 rounded-md border bg-muted/50 px-3 py-2 text-sm text-muted-foreground">
                      <template v-if="perm.sensitive && !showSensitive[field as string]">•••••</template>
                      <template v-else>{{ profileData[field as string] || '—' }}</template>
                    </div>
                    <Button v-if="perm.sensitive" variant="outline" size="sm" @click="showSensitive[field as string] = !showSensitive[field as string]">
                      {{ showSensitive[field as string] ? 'Hide' : 'Show' }}
                    </Button>
                  </template>
                </div>
                <p v-if="!perm.editable && perm.source !== 'user'" class="text-xs text-muted-foreground italic">
                  Set by {{ perm.source }}
                </p>
              </div>

              <Button @click="saveProfile" :disabled="saving" class="mt-2">
                {{ saving ? 'Saving…' : 'Save changes' }}
              </Button>
            </CardContent>
          </Card>
        </TabsContent>

        <!-- SESSIONS TAB -->
        <TabsContent value="sessions">
          <Card>
            <CardHeader>
              <CardTitle>Active Sessions</CardTitle>
            </CardHeader>
            <CardContent>
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Device</TableHead>
                    <TableHead>IP</TableHead>
                    <TableHead class="text-right">Action</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  <TableRow v-for="s in sessions" :key="s.id">
                    <TableCell>
                      <div class="flex items-center gap-2">
                        <span class="text-sm font-medium">{{ parseUserAgent(s.user_agent) }}</span>
                        <Badge v-if="s.current" class="text-[10px] h-4 px-1">This device</Badge>
                      </div>
                    </TableCell>
                    <TableCell class="text-sm text-muted-foreground font-mono">{{ s.ip_address || '—' }}</TableCell>
                    <TableCell class="text-right">
                      <Button v-if="!s.current" variant="destructive" size="sm" @click="revokeSession(s.id)">
                        Revoke
                      </Button>
                    </TableCell>
                  </TableRow>
                  <TableRow v-if="!sessions.length">
                    <TableCell colspan="3" class="text-center text-muted-foreground py-8">
                      No active sessions
                    </TableCell>
                  </TableRow>
                </TableBody>
              </Table>
              <Button v-if="sessions.length > 1" variant="outline" class="mt-4 text-destructive border-destructive/30" @click="revokeOthers">
                Revoke all other sessions
              </Button>
            </CardContent>
          </Card>
        </TabsContent>

        <!-- ACTIVITY TAB -->
        <TabsContent value="activity">
          <Card>
            <CardHeader>
              <CardTitle>My Activity</CardTitle>
            </CardHeader>
            <CardContent>
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Event</TableHead>
                    <TableHead class="text-right">Time</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  <TableRow v-for="e in events" :key="e.id">
                    <TableCell>
                      <Badge variant="outline" class="font-mono text-xs">{{ e.event_type }}</Badge>
                    </TableCell>
                    <TableCell class="text-right text-sm text-muted-foreground">{{ e.time_ago }}</TableCell>
                  </TableRow>
                  <TableRow v-if="!events.length">
                    <TableCell colspan="2" class="text-center text-muted-foreground py-8">
                      No activity yet
                    </TableCell>
                  </TableRow>
                </TableBody>
              </Table>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </main>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed, onMounted, onUnmounted } from 'vue'
import { api } from '@/api/client'
import { brandingApi, type Branding } from '@/api/branding'
import {
  createReadyzWaiter,
  useAppBootstrap,
} from '@/bootstrap/app-bootstrap'
import AppBootstrapScreen from '@/components/AppBootstrapScreen.vue'

// shadcn components
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Badge } from '@/components/ui/badge'
import { Avatar, AvatarFallback } from '@/components/ui/avatar'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'

import { LogOut } from 'lucide-vue-next'

const DEFAULT_BRANDING: Branding = {
  org_id: '', org_name: 'Zitadel', logo_url: '',
  heading: '', description: '',
  colors: { primary: '#6366f1', background: '#f0f2ff', surface: '#fff', text: '#1a1a2e', error: '#ef4444' },
  font_family: 'Inter, system-ui, sans-serif', hide_zitadel_branding: false,
}

const branding = ref<Branding>(DEFAULT_BRANDING)
const profile = ref<any>(null)
const profileData = ref<Record<string, any>>({})
const fieldPermissions = ref<Record<string, any>>({})
const editFields = reactive<Record<string, string>>({})
const showSensitive = reactive<Record<string, boolean>>({})
const sessions = ref<any[]>([])
const events = ref<any[]>([])
const saving = ref(false)
const {
  state: bootstrapState,
  error: bootstrapError,
  retryDelayMs: bootstrapRetryDelayMs,
  run: runBootstrap,
  retry: retryBootstrap,
  dispose: disposeBootstrap,
} = useAppBootstrap(
  async () => {
    try {
      branding.value = await brandingApi.get()
    } catch {}
    await loadProfile()
  },
  {
    waitForReady: createReadyzWaiter(),
  },
)

const initial = computed(() =>
  ((profile.value?.display_name || profile.value?.identifier || '?')[0] || '?').toUpperCase()
)

const visibleFields = computed(() => {
  const result: Record<string, any> = {}
  for (const [field, perm] of Object.entries(fieldPermissions.value)) {
    if (!(perm as any).hidden) result[field] = perm
  }
  return result
})

function formatLabel(field: string) {
  return field.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase())
}

function parseUserAgent(ua: string): string {
  if (!ua) return 'Unknown device'
  let browser = 'Unknown browser'
  if (ua.includes('Firefox/')) browser = 'Firefox'
  else if (ua.includes('Edg/')) browser = 'Edge'
  else if (ua.includes('Chrome/')) browser = 'Chrome'
  else if (ua.includes('Safari/') && !ua.includes('Chrome')) browser = 'Safari'
  else if (ua.includes('curl')) browser = 'curl'
  let os = ''
  if (ua.includes('Mac OS X') || ua.includes('Macintosh')) os = 'macOS'
  else if (ua.includes('Windows')) os = 'Windows'
  else if (ua.includes('Linux')) os = 'Linux'
  else if (ua.includes('Android')) os = 'Android'
  else if (ua.includes('iPhone') || ua.includes('iPad')) os = 'iOS'
  return os ? `${browser} on ${os}` : browser
}

async function loadProfile() {
  const data = await api.get<any>('/v1/account/profile')
  profile.value = data.identity
  profileData.value = data.identity.profile || {}
  fieldPermissions.value = data.field_permissions || {}
  for (const [field, perm] of Object.entries(fieldPermissions.value) as any[]) {
    if (perm.editable) editFields[field] = profileData.value[field] || ''
  }
}

async function loadSessions() {
  try {
    const data = await api.get<any>('/v1/account/sessions')
    sessions.value = data.sessions || []
  } catch {}
}

async function loadActivity() {
  try {
    const data = await api.get<any>('/v1/account/activity?limit=10')
    events.value = data.events || []
  } catch {}
}

async function saveProfile() {
  saving.value = true
  try {
    const profileUpdates: Record<string, any> = {}
    for (const [field, perm] of Object.entries(fieldPermissions.value) as any[]) {
      if (perm.editable && editFields[field] !== (profileData.value[field] || '')) {
        profileUpdates[field] = editFields[field]
      }
    }
    const body: any = {}
    if (Object.keys(profileUpdates).length) body.profile = profileUpdates
    if (editFields['display_name'] !== profile.value.display_name) {
      body.display_name = editFields['display_name'] || profile.value.display_name
    }
    await api.patch<any>('/v1/account/profile', body)
    await loadProfile()
    await loadActivity()
  } catch {}
  saving.value = false
}

async function revokeSession(id: string) {
  try {
    await api.post<any>(`/v1/account/sessions/${id}/revoke`, {})
    sessions.value = sessions.value.filter(s => s.id !== id)
    await loadActivity()
  } catch {}
}

async function revokeOthers() {
  try {
    await api.post<any>('/v1/account/sessions/revoke-others', {})
    await loadSessions()
    await loadActivity()
  } catch {}
}

function signOut() {
  document.cookie = '__zitadel_session=; Path=/; Max-Age=0'
  window.location.href = '/login'
}

async function startAccountBootstrap() {
  const ready = await runBootstrap()
  if (ready) {
    void Promise.all([loadSessions(), loadActivity()])
  }
}

async function retryAccountBootstrap() {
  const ready = await retryBootstrap()
  if (ready) {
    void Promise.all([loadSessions(), loadActivity()])
  }
}

onMounted(async () => {
  await startAccountBootstrap()
})

onUnmounted(() => {
  disposeBootstrap()
})
</script>
