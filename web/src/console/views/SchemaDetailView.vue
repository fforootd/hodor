<template>
  <div v-if="loading" class="flex h-64 items-center justify-center text-muted-foreground">
    <Spinner class="mr-2" /> Loading schema…
  </div>
  <div v-else-if="!schema" class="flex h-64 items-center justify-center text-muted-foreground">
    Schema not found
  </div>
  <div v-else class="flex flex-col" style="height: calc(100vh - 100px)">
    <ResizablePanelGroup direction="horizontal" class="rounded-lg border bg-background">
      <!-- Sidebar Panel -->
      <ResizablePanel :default-size="25" :min-size="18" :max-size="40">
        <SchemaAnnotationRenderer
          :parsed-schema="parsedSchemaJSON"
          :schema-meta="schema"
          :versions="versionHistory"
          :entity-count="entityCount"
          :promote-loading="promoteLoading"
          :save-status="saveSuccess ? '✓ Created v' + newVersionNum : saveError || ''"
          @promote="promoteThis"
          @change="onQuickSettingChange"
          @save="saveSchemaWithMsg"
        />
      </ResizablePanel>

      <ResizableHandle withHandle />

      <!-- Editor Panel -->
      <ResizablePanel :default-size="75">
        <div class="flex h-full flex-col">
          <!-- Toolbar -->
          <div class="flex items-center gap-2 border-b bg-muted/30 px-4 py-2">
            <span class="text-sm font-mono font-medium truncate">{{ schema.id }}</span>
            <Badge v-if="dirty" variant="outline" class="text-[10px] border-amber-300 bg-amber-50 text-amber-700 animate-pulse">
              unsaved
            </Badge>
            <div class="ml-auto flex items-center gap-1.5">
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button variant="ghost" size="sm" class="h-7 w-7 p-0" @click="copyToClipboard">
                    <Copy class="size-3.5" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent>Copy JSON</TooltipContent>
              </Tooltip>
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button variant="ghost" size="sm" class="h-7 w-7 p-0" @click="formatJson">
                    <AlignLeft class="size-3.5" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent>Format</TooltipContent>
              </Tooltip>
            </div>
          </div>

          <!-- Tabs: Editor / Diff -->
          <Tabs v-model="activeTab" class="flex flex-1 flex-col overflow-hidden">
            <div class="border-b bg-muted/20 px-4">
              <TabsList class="h-9 bg-transparent p-0">
                <TabsTrigger value="editor" class="rounded-none border-b-2 border-transparent px-3 pb-2 pt-1.5 text-xs data-[state=active]:border-primary data-[state=active]:bg-transparent data-[state=active]:shadow-none">
                  Editor
                </TabsTrigger>
                <TabsTrigger value="diff" class="rounded-none border-b-2 border-transparent px-3 pb-2 pt-1.5 text-xs data-[state=active]:border-primary data-[state=active]:bg-transparent data-[state=active]:shadow-none" :disabled="!dirty">
                  Diff
                  <Badge v-if="dirty" variant="secondary" class="ml-1.5 text-[10px] px-1">
                    +{{ diffStats.added }}/-{{ diffStats.removed }}
                  </Badge>
                </TabsTrigger>
              </TabsList>
            </div>

            <TabsContent value="editor" class="flex-1 m-0 overflow-hidden">
              <vue-monaco-editor
                v-model:value="editorContent"
                language="json"
                theme="vs"
                :options="monacoOptions"
                @mount="onEditorMount"
                @change="onEditorChange"
              />
            </TabsContent>

            <TabsContent value="diff" class="flex-1 m-0 overflow-hidden">
              <div v-if="!dirty" class="flex h-full items-center justify-center text-muted-foreground text-sm">
                No changes to show
              </div>
              <div v-else class="flex h-full flex-col overflow-hidden">
                <div class="flex items-center gap-3 border-b px-4 py-2 bg-muted/20">
                  <span class="text-xs text-muted-foreground">
                    <span class="font-semibold text-emerald-600">+{{ diffStats.added }}</span>
                    <span class="mx-1 text-muted-foreground/50">·</span>
                    <span class="font-semibold text-red-600">−{{ diffStats.removed }}</span>
                    <span class="ml-1">lines changed</span>
                  </span>
                </div>
                <div class="flex-1 overflow-auto font-mono text-xs leading-relaxed">
                  <div v-for="(line, i) in diffLines" :key="i"
                    class="flex px-4 py-px"
                    :class="{
                      'bg-emerald-50 text-emerald-900': line.type === 'add',
                      'bg-red-50 text-red-900': line.type === 'del',
                    }"
                  >
                    <span class="w-10 shrink-0 text-right pr-3 select-none text-muted-foreground/50">{{ line.num || '' }}</span>
                    <span class="w-4 shrink-0 text-center select-none font-semibold"
                      :class="{
                        'text-emerald-600': line.type === 'add',
                        'text-red-600': line.type === 'del',
                        'text-muted-foreground/30': line.type === 'ctx',
                      }"
                    >{{ line.marker }}</span>
                    <span class="flex-1 whitespace-pre">{{ line.text }}</span>
                  </div>
                </div>
              </div>
            </TabsContent>
          </Tabs>

          <!-- JSON error bar -->
          <div v-if="jsonError" class="border-t border-red-200 bg-red-50 px-4 py-2 text-xs text-red-700 font-mono">
            ⚠ {{ jsonError }}
          </div>
        </div>
      </ResizablePanel>
    </ResizablePanelGroup>
  </div>

  <!-- Schema Upgrade Preview Sheet -->
  <SchemaUpgradePreview
    v-model:open="showUpgradePreview"
    :schema-type="schema?.type || ''"
    :proposed-schema="proposedSchema"
    @confirm="confirmSave"
  />
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { VueMonacoEditor } from '@guolao/vue-monaco-editor'
import { schemaApi, type Schema } from '@/api/resources'
import SchemaAnnotationRenderer from '@/console/components/schema/SchemaAnnotationRenderer.vue'
import SchemaUpgradePreview from '@/console/components/schema/SchemaUpgradePreview.vue'

import { ResizablePanelGroup, ResizablePanel, ResizableHandle } from '@/components/ui/resizable'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { Spinner } from '@/components/ui/spinner'
import { Copy, AlignLeft } from 'lucide-vue-next'

const route = useRoute()
const router = useRouter()

const schema = ref<Schema | null>(null)
const loading = ref(true)
const editorContent = ref('')
const originalContent = ref('')
const jsonError = ref('')
const saving = ref(false)
const saveSuccess = ref(false)
const saveError = ref('')
const entityCount = ref(-1)
const commitMessage = ref('')
const newVersionNum = ref(0)
const versionHistory = ref<Schema[]>([])
const promoteLoading = ref(false)
const activeTab = ref('editor')

// Monaco editor options
const monacoOptions = computed(() => ({
  minimap: { enabled: false },
  fontSize: 13,
  lineNumbers: 'on' as const,
  scrollBeyondLastLine: false,
  wordWrap: 'off' as const,
  tabSize: 2,
  automaticLayout: true,
  renderLineHighlight: 'all' as const,
  bracketPairColorization: { enabled: true },
  padding: { top: 8, bottom: 8 },
  folding: true,
  foldingStrategy: 'indentation' as const,
  scrollbar: {
    verticalScrollbarSize: 8,
    horizontalScrollbarSize: 8,
  },
}))

// Quick settings state
const loginPreset = ref('identifier_first')
const authPassword = ref(true)
const authMagicLink = ref(true)
const authPasskey = ref(false)
const authSSO = ref(true)
const mfaRequired = ref(false)
const registrationAllowed = ref(true)
const authPAT = ref(false)
const authAPIKey = ref(false)
const authClientCert = ref(false)
const brandHeading = ref('Welcome back')
const brandPrimary = ref('#6366f1')

const dirty = computed(() => editorContent.value !== originalContent.value)

// Parsed schema JSON for annotation renderer
const parsedSchemaJSON = computed(() => {
  try {
    return JSON.parse(editorContent.value || '{}')
  } catch {
    return {}
  }
})

// Save with commit message from annotation renderer
function saveSchemaWithMsg(msg: string) {
  commitMessage.value = msg || ''
  saveSchema()
}

// --- Monaco Mount ---
let monacoInstance: any = null

function onEditorMount(editor: any, monaco: any) {
  monacoInstance = monaco
  // Configure JSON schema validation
  monaco.languages.json.jsonDefaults.setDiagnosticsOptions({
    validate: true,
    allowComments: false,
    trailingCommas: 'error',
  })
}

function onEditorChange(value: string | undefined) {
  if (value == null) return
  jsonError.value = ''
  saveSuccess.value = false
  saveError.value = ''
  try {
    JSON.parse(value)
    syncSidebarFromJson(value)
  } catch (e: any) {
    jsonError.value = e.message?.replace('JSON.parse: ', '') || 'Invalid JSON'
  }
}

// --- Diff Engine ---

interface DiffLine {
  type: 'add' | 'del' | 'ctx'
  num: number | null
  marker: string
  text: string
}

function computeDiff(oldText: string, newText: string): DiffLine[] {
  const oldLines = oldText.split('\n')
  const newLines = newText.split('\n')
  const result: DiffLine[] = []

  const m = oldLines.length, n = newLines.length
  const dp: number[][] = Array.from({ length: m + 1 }, () => new Array(n + 1).fill(0))
  for (let i = 1; i <= m; i++) {
    for (let j = 1; j <= n; j++) {
      dp[i][j] = oldLines[i - 1] === newLines[j - 1]
        ? dp[i - 1][j - 1] + 1
        : Math.max(dp[i - 1][j], dp[i][j - 1])
    }
  }

  const rawDiff: Array<{ type: 'ctx' | 'del' | 'add'; text: string; lineNum: number }> = []
  let i = m, j = n
  while (i > 0 || j > 0) {
    if (i > 0 && j > 0 && oldLines[i - 1] === newLines[j - 1]) {
      rawDiff.unshift({ type: 'ctx', text: oldLines[i - 1], lineNum: j })
      i--; j--
    } else if (j > 0 && (i === 0 || dp[i][j - 1] >= dp[i - 1][j])) {
      rawDiff.unshift({ type: 'add', text: newLines[j - 1], lineNum: j })
      j--
    } else {
      rawDiff.unshift({ type: 'del', text: oldLines[i - 1], lineNum: i })
      i--
    }
  }

  // Context windowing: show 3 lines around changes
  const changeIndices = new Set<number>()
  rawDiff.forEach((d, idx) => {
    if (d.type !== 'ctx') {
      for (let k = Math.max(0, idx - 3); k <= Math.min(rawDiff.length - 1, idx + 3); k++) {
        changeIndices.add(k)
      }
    }
  })

  let lastShown = -1
  rawDiff.forEach((d, idx) => {
    if (!changeIndices.has(idx)) return
    if (lastShown >= 0 && idx - lastShown > 1) {
      result.push({ type: 'ctx', num: null, marker: ' ', text: '···' })
    }
    lastShown = idx

    const markers = { ctx: ' ', add: '+', del: '-' }
    result.push({
      type: d.type,
      num: d.lineNum,
      marker: markers[d.type],
      text: d.text,
    })
  })

  return result
}

const diffLines = computed(() => computeDiff(originalContent.value, editorContent.value))
const diffStats = computed(() => {
  const lines = diffLines.value
  return {
    added: lines.filter(l => l.type === 'add').length,
    removed: lines.filter(l => l.type === 'del').length,
  }
})

// --- Business Logic ---

onMounted(async () => {
  const id = route.params.id as string
  try {
    const s = await schemaApi.get(id)
    schema.value = s
    const json = JSON.stringify(s.schema, null, 2)
    editorContent.value = json
    originalContent.value = json
    syncSidebarFromJson(json)

    schemaApi.entityCount(id).then(c => { entityCount.value = c }).catch(() => {})
    schemaApi.listByType(s.type).then(versions => { versionHistory.value = versions }).catch(() => {})
  } catch {
    schema.value = null
  } finally {
    loading.value = false
  }
})

function syncSidebarFromJson(json: string) {
  try {
    const parsed = JSON.parse(json)
    const login = parsed?.['x-login'] || {}
    const branding = parsed?.['x-branding'] || {}
    const methods = parsed?.['x-auth-methods'] || login.auth_methods || {}

    loginPreset.value = login.preset || 'identifier_first'
    authPassword.value = methods.password?.enabled ?? true
    authMagicLink.value = methods.magic_link?.enabled ?? true
    authPasskey.value = methods.passkey?.enabled ?? false
    authSSO.value = methods.sso?.enabled ?? true
    mfaRequired.value = login.mfa_required ?? false
    registrationAllowed.value = login.registration_allowed ?? true
    authPAT.value = methods.pat?.enabled ?? false
    authAPIKey.value = methods.api_key?.enabled ?? false
    authClientCert.value = methods.client_cert?.enabled ?? false
    brandHeading.value = branding.heading || 'Welcome back'
    brandPrimary.value = branding.colors?.primary || '#6366f1'
  } catch {}
}

function onQuickSettingChange() {
  try {
    const parsed = JSON.parse(editorContent.value)

    if (!parsed['x-auth-methods']) parsed['x-auth-methods'] = {}
    const am = parsed['x-auth-methods']
    am.password = { ...(am.password || {}), enabled: authPassword.value, interactive: true }
    am.magic_link = { ...(am.magic_link || {}), enabled: authMagicLink.value, interactive: true }
    am.passkey = { ...(am.passkey || {}), enabled: authPasskey.value, interactive: true }
    am.sso = { ...(am.sso || {}), enabled: authSSO.value, interactive: true }
    am.pat = { ...(am.pat || {}), enabled: authPAT.value, interactive: false }
    am.api_key = { ...(am.api_key || {}), enabled: authAPIKey.value, interactive: false }
    am.client_cert = { ...(am.client_cert || {}), enabled: authClientCert.value, interactive: false }

    if (!parsed['x-login']) parsed['x-login'] = {}
    parsed['x-login'].preset = loginPreset.value
    parsed['x-login'].mfa_required = mfaRequired.value
    parsed['x-login'].registration_allowed = registrationAllowed.value
    delete parsed['x-login'].auth_methods

    if (!parsed['x-branding']) parsed['x-branding'] = {}
    parsed['x-branding'].heading = brandHeading.value
    if (!parsed['x-branding'].colors) parsed['x-branding'].colors = {}
    parsed['x-branding'].colors.primary = brandPrimary.value

    editorContent.value = JSON.stringify(parsed, null, 2)
    jsonError.value = ''
  } catch {}
}

// Upgrade preview state
const showUpgradePreview = ref(false)
const proposedSchema = ref<Record<string, any> | null>(null)

async function saveSchema() {
  if (!schema.value || jsonError.value) return

  // If entities exist, show upgrade preview first
  if (entityCount.value > 0 && dirty.value) {
    try {
      proposedSchema.value = JSON.parse(editorContent.value)
      showUpgradePreview.value = true
      return // Wait for user confirmation
    } catch (e: any) {
      saveError.value = e.message || 'Invalid JSON'
      return
    }
  }

  await commitSave()
}

async function confirmSave() {
  showUpgradePreview.value = false
  await commitSave()
}

async function commitSave() {
  if (!schema.value || jsonError.value) return
  saving.value = true
  saveSuccess.value = false
  saveError.value = ''
  try {
    const parsed = JSON.parse(editorContent.value)
    const updated = await schemaApi.update(schema.value.id, parsed, commitMessage.value)
    newVersionNum.value = updated.version
    saveSuccess.value = true
    commitMessage.value = ''
    activeTab.value = 'editor'
    setTimeout(() => {
      router.push('/schemas/' + updated.id)
    }, 1500)
  } catch (e: any) {
    saveError.value = e.message || 'Save failed'
  } finally {
    saving.value = false
  }
}

async function promoteThis() {
  if (!schema.value) return
  promoteLoading.value = true
  try {
    await schemaApi.promote(schema.value.id)
    const s = await schemaApi.get(schema.value.id)
    schema.value = s
    versionHistory.value = await schemaApi.listByType(s.type)
  } catch {}
  promoteLoading.value = false
}

function formatJson() {
  try {
    const parsed = JSON.parse(editorContent.value)
    editorContent.value = JSON.stringify(parsed, null, 2)
    jsonError.value = ''
  } catch {}
}

function copyToClipboard() {
  navigator.clipboard.writeText(editorContent.value)
}
</script>
