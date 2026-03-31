<template>
  <div class="space-y-6">
    <div class="grid gap-4 lg:grid-cols-[minmax(0,1fr)_320px]">
      <Card>
        <CardHeader>
          <CardTitle>Notification Channels</CardTitle>
        </CardHeader>
        <CardContent class="space-y-4">
          <p class="text-sm text-muted-foreground">
            Local development defaults to stdout delivery for both email and SMS. Use this page to
            override channels, templates, preview rendering, and send test deliveries without leaving
            the Console.
          </p>

          <div class="grid gap-4 md:grid-cols-2">
            <div class="space-y-2">
              <Label for="notification-scope">Scope</Label>
              <Select v-model="selectedScope">
                <SelectTrigger id="notification-scope">
                  <SelectValue placeholder="Select scope" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="instance">Instance</SelectItem>
                  <SelectItem v-if="currentOrgId" value="org">Current org</SelectItem>
                </SelectContent>
              </Select>
            </div>

            <div class="space-y-2">
              <Label>Resolved target</Label>
              <div class="rounded-md border bg-muted/30 px-3 py-2 text-sm">
                <span v-if="selectedScope === 'org' && currentOrgId">
                  Org override for <code>{{ currentOrgId }}</code>
                </span>
                <span v-else>Instance-wide default</span>
              </div>
            </div>
          </div>

          <div class="grid gap-3 md:grid-cols-2">
            <div class="rounded-lg border p-3">
              <div class="mb-2 flex items-center justify-between">
                <span class="text-sm font-medium">Effective Email Channel</span>
                <Badge variant="secondary">{{ effectiveEmailChannel }}</Badge>
              </div>
              <p class="text-xs text-muted-foreground">
                Zero-config fallback stays on. If you save nothing here, local email still resolves to
                stdout.
              </p>
            </div>
            <div class="rounded-lg border p-3">
              <div class="mb-2 flex items-center justify-between">
                <span class="text-sm font-medium">Effective SMS Channel</span>
                <Badge variant="secondary">{{ effectiveSMSChannel }}</Badge>
              </div>
              <p class="text-xs text-muted-foreground">
                SMS uses the same queue and test tools, with stdout as the default local sink.
              </p>
            </div>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Built-in Presets</CardTitle>
        </CardHeader>
        <CardContent class="space-y-3">
          <div v-if="loadingPresets" class="text-sm text-muted-foreground">Loading presets…</div>
          <div v-else-if="!presets.length" class="text-sm text-muted-foreground">
            No presets available.
          </div>
          <div v-for="preset in presets" :key="preset.id" class="rounded-lg border p-3">
            <div class="mb-2 flex items-center gap-2">
              <span class="font-medium">{{ preset.label }}</span>
              <Badge variant="outline">{{ preset.medium }}</Badge>
              <Badge variant="outline">{{ preset.driver }}</Badge>
            </div>
            <p class="mb-2 text-xs text-muted-foreground">{{ preset.description }}</p>
            <pre class="overflow-auto rounded bg-muted/50 p-2 text-xs">{{ formatJSON(preset.config) }}</pre>
          </div>
        </CardContent>
      </Card>
    </div>

    <Tabs default-value="channels" class="space-y-4">
      <TabsList>
        <TabsTrigger value="channels">Channels</TabsTrigger>
        <TabsTrigger value="templates">Templates</TabsTrigger>
        <TabsTrigger value="preview">Preview & Test</TabsTrigger>
      </TabsList>

      <TabsContent value="channels">
        <Card>
          <CardHeader class="flex flex-row items-center justify-between space-y-0">
            <CardTitle>Channel Configuration</CardTitle>
            <Button :disabled="savingSettings || !!settingsError" @click="saveSettings">
              {{ savingSettings ? 'Saving…' : 'Save channels' }}
            </Button>
          </CardHeader>
          <CardContent class="space-y-4">
            <p class="text-sm text-muted-foreground">
              Save only the override for the selected scope. Instance and org settings merge
              hierarchically.
            </p>
            <JsonEditor
              v-model="settingsJSON"
              label="notification"
              height="420px"
              @valid="onSettingsValid"
              @error="settingsError = $event"
            />
            <p v-if="settingsError" class="text-sm text-destructive">{{ settingsError }}</p>
          </CardContent>
        </Card>
      </TabsContent>

      <TabsContent value="templates">
        <Card>
          <CardHeader class="flex flex-row items-center justify-between space-y-0">
            <CardTitle>Template Overrides</CardTitle>
            <Button :disabled="savingTemplates || !!templatesError" @click="saveTemplates">
              {{ savingTemplates ? 'Saving…' : 'Save templates' }}
            </Button>
          </CardHeader>
          <CardContent class="space-y-4">
            <p class="text-sm text-muted-foreground">
              Override only the locales or template keys you need. Built-in English content remains the
              fallback.
            </p>
            <JsonEditor
              v-model="templatesJSON"
              label="notification_templates"
              height="420px"
              @valid="onTemplatesValid"
              @error="templatesError = $event"
            />
            <p v-if="templatesError" class="text-sm text-destructive">{{ templatesError }}</p>
          </CardContent>
        </Card>
      </TabsContent>

      <TabsContent value="preview">
        <div class="grid gap-4 lg:grid-cols-[360px_minmax(0,1fr)]">
          <Card>
            <CardHeader>
              <CardTitle>Preview & Test Send</CardTitle>
            </CardHeader>
            <CardContent class="space-y-4">
              <div class="space-y-2">
                <Label for="medium">Medium</Label>
                <Select v-model="previewForm.medium">
                  <SelectTrigger id="medium">
                    <SelectValue placeholder="Select medium" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="email">Email</SelectItem>
                    <SelectItem value="sms">SMS</SelectItem>
                  </SelectContent>
                </Select>
              </div>

              <div class="space-y-2">
                <Label for="template-key">Template Key</Label>
                <Input id="template-key" v-model="previewForm.template_key" />
              </div>

              <div class="space-y-2">
                <Label for="locale">Locale</Label>
                <Input id="locale" v-model="previewForm.locale" placeholder="en" />
              </div>

              <div class="space-y-2">
                <Label for="channel-id">Channel ID</Label>
                <Input id="channel-id" v-model="previewForm.channel_id" placeholder="optional" />
              </div>

              <div class="space-y-2">
                <Label for="recipient">Recipient</Label>
                <Input
                  id="recipient"
                  v-model="previewForm.recipient"
                  placeholder="alice@example.com or +15551234567"
                />
              </div>

              <div class="space-y-2">
                <Label>Payload</Label>
                <JsonEditor
                  v-model="previewPayloadJSON"
                  label="payload"
                  height="220px"
                  @valid="onPreviewPayloadValid"
                  @error="previewPayloadError = $event"
                />
              </div>

              <div class="flex flex-wrap gap-2">
                <Button :disabled="previewing || !!previewPayloadError" @click="runPreview">
                  {{ previewing ? 'Rendering…' : 'Preview render' }}
                </Button>
                <Button
                  variant="outline"
                  :disabled="testing || !!previewPayloadError || !previewForm.recipient"
                  @click="runTestSend"
                >
                  {{ testing ? 'Sending…' : 'Send test' }}
                </Button>
              </div>
              <p v-if="previewPayloadError" class="text-sm text-destructive">{{ previewPayloadError }}</p>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Rendered Output</CardTitle>
            </CardHeader>
            <CardContent class="space-y-4">
              <div v-if="!rendered" class="text-sm text-muted-foreground">
                Use Preview render or Send test to inspect the resolved subject, body, locale fallback,
                and chosen channel.
              </div>
              <template v-else>
                <div class="grid gap-3 md:grid-cols-2">
                  <div class="rounded-lg border p-3">
                    <div class="text-xs uppercase tracking-wide text-muted-foreground">Template</div>
                    <div class="mt-1 font-medium">{{ rendered.template_key }}</div>
                  </div>
                  <div class="rounded-lg border p-3">
                    <div class="text-xs uppercase tracking-wide text-muted-foreground">Channel</div>
                    <div class="mt-1 font-medium">{{ rendered.channel_id || 'resolved at delivery time' }}</div>
                  </div>
                </div>

                <div class="space-y-2">
                  <Label>Subject</Label>
                  <div class="rounded-md border bg-muted/30 px-3 py-2 text-sm">
                    {{ rendered.subject || '(no subject)' }}
                  </div>
                </div>

                <div class="space-y-2">
                  <Label>Text Body</Label>
                  <pre class="overflow-auto rounded-md border bg-muted/30 p-3 text-sm whitespace-pre-wrap">{{ rendered.text_body }}</pre>
                </div>

                <div v-if="rendered.html_body" class="space-y-2">
                  <Label>HTML Body</Label>
                  <pre class="overflow-auto rounded-md border bg-muted/30 p-3 text-sm whitespace-pre-wrap">{{ rendered.html_body }}</pre>
                </div>
              </template>
            </CardContent>
          </Card>
        </div>
      </TabsContent>
    </Tabs>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { toast } from 'vue-sonner'
import JsonEditor from '@/console/components/JsonEditor.vue'
import { useOrgContext } from '@/console/composables/useOrgContext'
import {
  notificationApi,
  type NotificationPreset,
  type NotificationRender,
} from '@/api/resources'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'

const { currentOrgId } = useOrgContext()

const selectedScope = ref<'instance' | 'org'>(currentOrgId.value ? 'org' : 'instance')
const presets = ref<NotificationPreset[]>([])
const loadingPresets = ref(false)

const settingsJSON = ref('{}')
const templatesJSON = ref('{}')
const settingsValue = ref<Record<string, unknown>>({})
const templatesValue = ref<Record<string, unknown>>({})
const settingsError = ref('')
const templatesError = ref('')
const savingSettings = ref(false)
const savingTemplates = ref(false)
const effectiveSettings = ref<Record<string, any>>({})

const previewForm = ref({
  medium: 'email',
  template_key: 'magic_link_login',
  locale: 'en',
  channel_id: '',
  recipient: 'dev@example.com',
})
const previewPayloadJSON = ref(
  JSON.stringify(
    {
      link: 'http://localhost:8080/v1/auth/magic-link/verify?token=example',
      expires_at: '2026-03-30T12:00:00Z',
      email: 'dev@example.com',
    },
    null,
    2,
  ),
)
const previewPayload = ref<Record<string, unknown>>({})
const previewPayloadError = ref('')
const rendered = ref<NotificationRender | null>(null)
const previewing = ref(false)
const testing = ref(false)

const scopeId = computed(() => (selectedScope.value === 'org' ? currentOrgId.value || '' : ''))
const effectiveEmailChannel = computed(
  () => effectiveSettings.value?.email?.default_channel || 'dev_stdout',
)
const effectiveSMSChannel = computed(
  () => effectiveSettings.value?.sms?.default_channel || 'dev_stdout',
)

function formatJSON(value: unknown) {
  return JSON.stringify(value || {}, null, 2)
}

function onSettingsValid(value: Record<string, unknown>) {
  settingsValue.value = value
  settingsError.value = ''
}

function onTemplatesValid(value: Record<string, unknown>) {
  templatesValue.value = value
  templatesError.value = ''
}

function onPreviewPayloadValid(value: Record<string, unknown>) {
  previewPayload.value = value
  previewPayloadError.value = ''
}

async function loadSettings() {
  const [rawSettings, rawTemplates, resolvedSettings] = await Promise.all([
    notificationApi.getSettings(selectedScope.value, scopeId.value),
    notificationApi.getTemplates(selectedScope.value, scopeId.value),
    notificationApi.getEffectiveSettings(selectedScope.value, scopeId.value),
  ])
  settingsValue.value = (rawSettings.data || {}) as Record<string, unknown>
  templatesValue.value = (rawTemplates.data || {}) as Record<string, unknown>
  effectiveSettings.value = (resolvedSettings.effective || {}) as Record<string, unknown>
  settingsJSON.value = formatJSON(settingsValue.value)
  templatesJSON.value = formatJSON(templatesValue.value)
}

async function loadPresets() {
  loadingPresets.value = true
  try {
    const resp = await notificationApi.listPresets()
    presets.value = resp.presets || []
  } finally {
    loadingPresets.value = false
  }
}

async function saveSettings() {
  savingSettings.value = true
  try {
    await notificationApi.saveSettings(settingsValue.value, selectedScope.value, scopeId.value)
    await loadSettings()
    toast.success('Notification channels saved')
  } catch (err: any) {
    toast.error('Failed to save channels', { description: err.message })
  } finally {
    savingSettings.value = false
  }
}

async function saveTemplates() {
  savingTemplates.value = true
  try {
    await notificationApi.saveTemplates(templatesValue.value, selectedScope.value, scopeId.value)
    toast.success('Notification templates saved')
  } catch (err: any) {
    toast.error('Failed to save templates', { description: err.message })
  } finally {
    savingTemplates.value = false
  }
}

async function runPreview() {
  previewing.value = true
  try {
    rendered.value = await notificationApi.preview({
      org_id: scopeId.value || undefined,
      medium: previewForm.value.medium,
      template_key: previewForm.value.template_key,
      locale: previewForm.value.locale || undefined,
      payload: previewPayload.value,
    })
    toast.success('Notification rendered')
  } catch (err: any) {
    toast.error('Preview failed', { description: err.message })
  } finally {
    previewing.value = false
  }
}

async function runTestSend() {
  testing.value = true
  try {
    rendered.value = await notificationApi.sendTest({
      org_id: scopeId.value || undefined,
      medium: previewForm.value.medium,
      template_key: previewForm.value.template_key,
      locale: previewForm.value.locale || undefined,
      channel_id: previewForm.value.channel_id || undefined,
      recipient: previewForm.value.recipient,
      payload: previewPayload.value,
    })
    toast.success('Test notification sent')
  } catch (err: any) {
    toast.error('Test send failed', { description: err.message })
  } finally {
    testing.value = false
  }
}

watch(
  () => currentOrgId.value,
  (orgId) => {
    if (selectedScope.value === 'org' && !orgId) {
      selectedScope.value = 'instance'
    }
  },
)

watch([selectedScope, scopeId], async () => {
  try {
    await loadSettings()
  } catch (err: any) {
    toast.error('Failed to load notification settings', { description: err.message })
  }
})

onMounted(async () => {
  try {
    await Promise.all([loadSettings(), loadPresets()])
  } catch (err: any) {
    toast.error('Failed to load notifications view', { description: err.message })
  }
})
</script>
