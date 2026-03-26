<template>
  <div v-if="loading" class="loading">Loading schema…</div>
  <div v-else-if="!schema" class="loading">Schema not found</div>
  <div v-else class="editor-layout">
    <!-- Quick Settings Sidebar -->
    <aside class="sidebar">
      <div class="sidebar-section">
        <h4 class="sidebar-heading">Schema</h4>
        <div class="field-row">
          <span class="field-label">Type</span>
          <span class="field-value mono">{{ schema.type }}</span>
        </div>
        <div class="field-row">
          <span class="field-label">Version</span>
          <span class="version-badge">v{{ schema.version }}</span>
        </div>
        <div v-if="identityCount >= 0" class="field-row">
          <span class="field-label">Identities</span>
          <span class="impact-badge" :class="{ warn: identityCount > 0 }">
            {{ identityCount.toLocaleString() }} {{ identityCount === 1 ? 'user' : 'users' }}
          </span>
        </div>
      </div>

      <div class="sidebar-section">
        <h4 class="sidebar-heading">Login Flow</h4>
        <div class="field-row">
          <span class="field-label">Preset</span>
          <select v-model="loginPreset" class="select-input" @change="onQuickSettingChange">
            <option value="identifier_first">Identifier first</option>
            <option value="passkey_first">Passkey first</option>
            <option value="sso_only">SSO only</option>
            <option value="custom">Custom</option>
          </select>
        </div>
        <div class="toggle-group">
          <label class="toggle-row">
            <input type="checkbox" v-model="authPassword" @change="onQuickSettingChange" />
            <span>Password</span>
          </label>
          <label class="toggle-row">
            <input type="checkbox" v-model="authMagicLink" @change="onQuickSettingChange" />
            <span>Magic link</span>
          </label>
          <label class="toggle-row">
            <input type="checkbox" v-model="authPasskey" @change="onQuickSettingChange" />
            <span>Passkey</span>
          </label>
          <label class="toggle-row">
            <input type="checkbox" v-model="authSSO" @change="onQuickSettingChange" />
            <span>SSO</span>
          </label>
        </div>
        <label class="toggle-row mfa-row">
          <input type="checkbox" v-model="mfaRequired" @change="onQuickSettingChange" />
          <span>Require MFA</span>
        </label>
        <label class="toggle-row">
          <input type="checkbox" v-model="registrationAllowed" @change="onQuickSettingChange" />
          <span>Allow registration</span>
        </label>
      </div>

      <div class="sidebar-section">
        <h4 class="sidebar-heading">Branding</h4>
        <div class="field-row">
          <span class="field-label">Heading</span>
          <input type="text" v-model="brandHeading" class="text-input" @input="onQuickSettingChange" />
        </div>
        <div class="field-row">
          <span class="field-label">Primary</span>
          <div class="color-row">
            <input type="color" v-model="brandPrimary" class="color-input" @input="onQuickSettingChange" />
            <span class="mono">{{ brandPrimary }}</span>
          </div>
        </div>
      </div>

      <div class="sidebar-section">
        <h4 class="sidebar-heading">Fields</h4>
        <div v-for="f in schemaFields" :key="f.name" class="field-chip">
          <span class="field-name">{{ f.name }}</span>
          <span v-if="f.identifier" class="chip-tag id">ID</span>
          <span v-if="f.sensitive" class="chip-tag sens">PII</span>
          <span v-if="f.mfa" class="chip-tag mfa">MFA</span>
        </div>
        <div v-if="!schemaFields.length" class="empty-fields">No properties defined</div>
      </div>

      <div class="sidebar-actions">
        <button class="btn-save" :disabled="!dirty || saving" @click="saveSchema">
          {{ saving ? 'Saving…' : 'Save changes' }}
        </button>
        <button v-if="dirty" class="btn-diff" @click="showDiff = !showDiff">
          {{ showDiff ? '← Editor' : 'Review changes' }}
        </button>
        <span v-if="saveSuccess" class="save-msg success">✓ Saved</span>
        <span v-if="saveError" class="save-msg error">{{ saveError }}</span>
      </div>
    </aside>

    <!-- Editor Panel -->
    <div class="editor-main">
      <div class="editor-toolbar">
        <span class="editor-title">{{ schema.id }}</span>
        <span v-if="dirty" class="dirty-dot" title="Unsaved changes">●</span>
        <div class="toolbar-right">
          <button v-if="dirty" class="btn-diff-toolbar" :class="{ active: showDiff }" @click="showDiff = !showDiff">
            {{ showDiff ? 'Editor' : 'Diff' }}
          </button>
          <button class="btn-copy" @click="copyToClipboard">Copy JSON</button>
          <button class="btn-format" @click="formatJson">Format</button>
        </div>
      </div>

      <!-- Diff View -->
      <div v-if="showDiff && dirty" class="diff-container">
        <div class="diff-header">
          <span class="diff-stat">
            <span class="diff-add">+{{ diffStats.added }}</span>
            <span class="diff-del">−{{ diffStats.removed }}</span>
            lines changed
          </span>
        </div>
        <div class="diff-content">
          <div v-for="(line, i) in diffLines" :key="i"
               class="diff-line" :class="line.type">
            <span class="diff-gutter">{{ line.num || '' }}</span>
            <span class="diff-marker">{{ line.marker }}</span>
            <span class="diff-text" v-html="line.html"></span>
          </div>
        </div>
      </div>

      <!-- Code Editor with Syntax Highlighting -->
      <div v-else class="editor-container">
        <div class="editor-scroll" ref="scrollEl" @scroll="syncScroll">
          <!-- Line numbers gutter -->
          <div class="line-gutter" ref="gutterEl">
            <div v-for="(line, idx) in editorLines" :key="idx"
                 class="line-num" :class="{ hidden: isLineHidden(idx) }">
              <span v-if="foldableLines.has(idx)" class="fold-toggle"
                    @click.stop="toggleFold(idx)">
                {{ collapsedLines.has(idx) ? '▸' : '▾' }}
              </span>
              <span class="num-text">{{ idx + 1 }}</span>
            </div>
          </div>
          <!-- Highlighted overlay (line-by-line for fold support) -->
          <div class="highlight-layer" ref="highlightEl" aria-hidden="true">
            <div v-for="(line, idx) in highlightedLines" :key="idx"
                 class="hl-line" :class="{ hidden: isLineHidden(idx), 'fold-start': collapsedLines.has(idx) }">
              <span v-html="line"></span>
              <span v-if="collapsedLines.has(idx)" class="fold-placeholder">
                ⋯ {{ foldRanges.get(idx) ? foldRanges.get(idx)![1] - foldRanges.get(idx)![0] : 0 }} lines
              </span>
            </div>
          </div>
          <!-- Transparent textarea (captures input) -->
          <textarea
            ref="editorEl"
            v-model="editorContent"
            class="code-editor"
            spellcheck="false"
            @input="onEditorChange"
            @scroll="syncScroll"
          ></textarea>
        </div>
        <div v-if="jsonError" class="json-error">⚠ {{ jsonError }}</div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch, nextTick } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { schemaApi, type Schema } from '@/api/resources'

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
const identityCount = ref(-1)
const showDiff = ref(false)

// Refs for scroll sync
const scrollEl = ref<HTMLElement | null>(null)
const gutterEl = ref<HTMLElement | null>(null)
const highlightEl = ref<HTMLElement | null>(null)
const editorEl = ref<HTMLTextAreaElement | null>(null)

// Quick settings state
const loginPreset = ref('identifier_first')
const authPassword = ref(true)
const authMagicLink = ref(true)
const authPasskey = ref(false)
const authSSO = ref(true)
const mfaRequired = ref(false)
const registrationAllowed = ref(true)
const brandHeading = ref('Welcome back')
const brandPrimary = ref('#6366f1')

const dirty = computed(() => editorContent.value !== originalContent.value)
const editorLines = computed(() => editorContent.value.split('\n'))
const lineCount = computed(() => editorLines.value.length)

// --- Fold State ---
const collapsedLines = ref(new Set<number>())

// Compute foldable line ranges: lines ending with { or [
const foldRanges = computed(() => {
  const ranges = new Map<number, [number, number]>()
  const lines = editorLines.value
  const stack: Array<{ line: number; char: string }> = []

  for (let i = 0; i < lines.length; i++) {
    const trimmed = lines[i].trimEnd()
    // Check for opening braces/brackets (possibly followed by comma)
    if (trimmed.endsWith('{') || trimmed.endsWith('[')) {
      stack.push({ line: i, char: trimmed.endsWith('{') ? '}' : ']' })
    }
    // Check for closing braces/brackets
    const lastChar = trimmed.replace(/,\s*$/, '').slice(-1)
    if ((lastChar === '}' || lastChar === ']') && stack.length > 0) {
      const top = stack[stack.length - 1]
      if (lastChar === top.char && i > top.line) {
        ranges.set(top.line, [top.line + 1, i])
        stack.pop()
      }
    }
  }
  return ranges
})

const foldableLines = computed(() => new Set(foldRanges.value.keys()))

function isLineHidden(idx: number): boolean {
  for (const [start, [from, to]] of foldRanges.value) {
    if (collapsedLines.value.has(start) && idx >= from && idx <= to) return true
  }
  return false
}

function toggleFold(line: number) {
  const next = new Set(collapsedLines.value)
  if (next.has(line)) next.delete(line)
  else next.add(line)
  collapsedLines.value = next
}

interface FieldInfo {
  name: string
  identifier: boolean
  sensitive: boolean
  mfa: string
}

const schemaFields = computed<FieldInfo[]>(() => {
  try {
    const parsed = JSON.parse(editorContent.value)
    const props = parsed?.properties || {}
    return Object.entries(props).map(([name, def]: [string, any]) => ({
      name,
      identifier: def?.['x-auth']?.identifier || false,
      sensitive: def?.['x-sensitive'] || false,
      mfa: def?.['x-auth']?.mfa || '',
    }))
  } catch { return [] }
})

// --- Syntax Highlighting ---

function highlightJSON(json: string): string {
  return json.replace(
    /("(?:x-[a-z][a-z0-9_-]*)")\s*:/g, // x-* annotation keys
    '<span class="tok-annotation">$1</span>:'
  ).replace(
    /("(?!x-)[^"]*")\s*:/g, // regular keys (not already matched)
    '<span class="tok-key">$1</span>:'
  ).replace(
    /:\s*("(?:\\.|[^"\\])*")/g, // string values
    ': <span class="tok-string">$1</span>'
  ).replace(
    /:\s*(\d+(?:\.\d+)?)/g, // numbers
    ': <span class="tok-number">$1</span>'
  ).replace(
    /:\s*(true|false)/g, // booleans
    ': <span class="tok-bool">$1</span>'
  ).replace(
    /:\s*(null)/g, // null
    ': <span class="tok-null">$1</span>'
  )
}

function escapeHtml(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
}

const highlightedLines = computed(() => {
  return editorLines.value.map(line => highlightJSON(escapeHtml(line)))
})

// --- Diff Engine ---

interface DiffLine {
  type: 'add' | 'del' | 'ctx'
  num: number | null
  marker: string
  html: string
}

function computeDiff(oldText: string, newText: string): DiffLine[] {
  const oldLines = oldText.split('\n')
  const newLines = newText.split('\n')
  const result: DiffLine[] = []

  // Simple LCS-based diff
  const m = oldLines.length, n = newLines.length
  // For perf, use a simple O(mn) approach — schemas are small
  const dp: number[][] = Array.from({ length: m + 1 }, () => new Array(n + 1).fill(0))
  for (let i = 1; i <= m; i++) {
    for (let j = 1; j <= n; j++) {
      dp[i][j] = oldLines[i - 1] === newLines[j - 1]
        ? dp[i - 1][j - 1] + 1
        : Math.max(dp[i - 1][j], dp[i][j - 1])
    }
  }

  // Backtrack to build diff
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

  // Convert to display format with context windowing (show 3 lines around changes)
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
      result.push({ type: 'ctx', num: null, marker: ' ', html: '<span class="diff-ellipsis">···</span>' })
    }
    lastShown = idx

    const markers = { ctx: ' ', add: '+', del: '-' }
    result.push({
      type: d.type,
      num: d.lineNum,
      marker: markers[d.type],
      html: highlightJSON(escapeHtml(d.text)),
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

// --- Scroll Sync ---

function syncScroll() {
  if (!editorEl.value || !highlightEl.value || !gutterEl.value) return
  const st = editorEl.value.scrollTop
  const sl = editorEl.value.scrollLeft
  highlightEl.value.scrollTop = st
  highlightEl.value.scrollLeft = sl
  gutterEl.value.scrollTop = st
}

// --- Business Logic (unchanged) ---

onMounted(async () => {
  const id = route.params.id as string
  try {
    const s = await schemaApi.get(id)
    schema.value = s
    const json = JSON.stringify(s.schema, null, 2)
    editorContent.value = json
    originalContent.value = json
    syncSidebarFromJson(json)

    schemaApi.identityCount(id).then(c => { identityCount.value = c }).catch(() => {})
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
    const methods = login.auth_methods || {}

    loginPreset.value = login.preset || 'identifier_first'
    authPassword.value = methods.password?.enabled ?? true
    authMagicLink.value = methods.magic_link?.enabled ?? true
    authPasskey.value = methods.passkey?.enabled ?? false
    authSSO.value = methods.sso?.enabled ?? true
    mfaRequired.value = login.mfa_required ?? false
    registrationAllowed.value = login.registration_allowed ?? true
    brandHeading.value = branding.heading || 'Welcome back'
    brandPrimary.value = branding.colors?.primary || '#6366f1'
  } catch {}
}

function onEditorChange() {
  jsonError.value = ''
  saveSuccess.value = false
  saveError.value = ''
  try {
    JSON.parse(editorContent.value)
    syncSidebarFromJson(editorContent.value)
  } catch (e: any) {
    jsonError.value = e.message?.replace('JSON.parse: ', '') || 'Invalid JSON'
  }
  nextTick(syncScroll)
}

function onQuickSettingChange() {
  try {
    const parsed = JSON.parse(editorContent.value)

    if (!parsed['x-login']) parsed['x-login'] = {}
    parsed['x-login'].preset = loginPreset.value
    if (!parsed['x-login'].auth_methods) parsed['x-login'].auth_methods = {}
    const m = parsed['x-login'].auth_methods
    m.password = { ...(m.password || {}), enabled: authPassword.value }
    m.magic_link = { ...(m.magic_link || {}), enabled: authMagicLink.value }
    m.passkey = { ...(m.passkey || {}), enabled: authPasskey.value }
    m.sso = { ...(m.sso || {}), enabled: authSSO.value }
    parsed['x-login'].mfa_required = mfaRequired.value
    parsed['x-login'].registration_allowed = registrationAllowed.value

    if (!parsed['x-branding']) parsed['x-branding'] = {}
    parsed['x-branding'].heading = brandHeading.value
    if (!parsed['x-branding'].colors) parsed['x-branding'].colors = {}
    parsed['x-branding'].colors.primary = brandPrimary.value

    editorContent.value = JSON.stringify(parsed, null, 2)
    jsonError.value = ''
  } catch {}
}

async function saveSchema() {
  if (!schema.value || jsonError.value) return
  saving.value = true
  saveSuccess.value = false
  saveError.value = ''
  try {
    const parsed = JSON.parse(editorContent.value)
    const updated = await schemaApi.update(schema.value.id, parsed)
    schema.value = updated
    originalContent.value = editorContent.value
    showDiff.value = false
    saveSuccess.value = true
    setTimeout(() => { saveSuccess.value = false }, 3000)
  } catch (e: any) {
    saveError.value = e.message || 'Save failed'
  } finally {
    saving.value = false
  }
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

<style scoped>
.loading { padding: 3rem; text-align: center; color: #9ca3af; }

.editor-layout {
  display: flex; gap: 0; min-height: calc(100vh - 140px);
  background: #fff; border: 1px solid #e5e7eb; border-radius: 12px; overflow: hidden;
}

/* Sidebar */
.sidebar {
  width: 280px; border-right: 1px solid #e5e7eb; padding: 1.25rem;
  overflow-y: auto; display: flex; flex-direction: column; gap: 0; background: #fafbfc;
}
.sidebar-section { padding: 0.75rem 0; border-bottom: 1px solid #f0f1f3; }
.sidebar-section:first-child { padding-top: 0; }
.sidebar-section:last-of-type { border-bottom: none; }
.sidebar-heading {
  font-size: 0.6875rem; font-weight: 700; text-transform: uppercase; letter-spacing: 0.06em;
  color: #9ca3af; margin-bottom: 0.625rem;
}

.field-row {
  display: flex; align-items: center; justify-content: space-between; gap: 0.5rem;
  margin-bottom: 0.5rem;
}
.field-label { font-size: 0.8125rem; color: #4b5563; }
.field-value { font-size: 0.8125rem; color: #1a1a2e; font-weight: 500; }

.version-badge {
  font-size: 0.6875rem; font-weight: 600; padding: 0.125rem 0.5rem;
  background: #f0f2ff; color: #6366f1; border-radius: 4px;
}
.impact-badge {
  font-size: 0.75rem; font-weight: 600; padding: 0.125rem 0.5rem;
  background: #f3f4f6; color: #6b7280; border-radius: 4px;
}
.impact-badge.warn { background: #fef3c7; color: #92400e; }

.select-input {
  flex: 1; max-width: 160px; padding: 0.25rem 0.5rem; border: 1px solid #d1d5db;
  border-radius: 6px; font-size: 0.8125rem; font-family: inherit; background: #fff;
}
.select-input:focus { outline: none; border-color: #6366f1; box-shadow: 0 0 0 2px rgba(99,102,241,.1); }

.text-input {
  flex: 1; max-width: 160px; padding: 0.25rem 0.5rem; border: 1px solid #d1d5db;
  border-radius: 6px; font-size: 0.8125rem; font-family: inherit; background: #fff;
}
.text-input:focus { outline: none; border-color: #6366f1; box-shadow: 0 0 0 2px rgba(99,102,241,.1); }

.toggle-group { display: flex; flex-direction: column; gap: 0.25rem; margin-bottom: 0.5rem; }
.toggle-row {
  display: flex; align-items: center; gap: 0.5rem; font-size: 0.8125rem; color: #374151;
  cursor: pointer;
}
.toggle-row input[type="checkbox"] {
  width: 16px; height: 16px; accent-color: #6366f1; cursor: pointer;
}
.mfa-row { margin-top: 0.25rem; }

.color-row { display: flex; align-items: center; gap: 0.5rem; }
.color-input {
  width: 28px; height: 28px; border: 1px solid #d1d5db; border-radius: 6px;
  cursor: pointer; padding: 0;
}
.mono { font-family: 'SF Mono', 'Fira Code', monospace; font-size: 0.75rem; color: #6b7280; }

/* Fields list */
.field-chip {
  display: flex; align-items: center; gap: 0.375rem; padding: 0.25rem 0;
}
.field-name { font-size: 0.8125rem; color: #1a1a2e; font-weight: 500; }
.chip-tag {
  font-size: 0.5625rem; font-weight: 700; padding: 0.0625rem 0.375rem; border-radius: 3px;
  text-transform: uppercase; letter-spacing: 0.04em;
}
.chip-tag.id { background: #dbeafe; color: #1d4ed8; }
.chip-tag.sens { background: #fee2e2; color: #991b1b; }
.chip-tag.mfa { background: #d1fae5; color: #065f46; }
.empty-fields { font-size: 0.8125rem; color: #9ca3af; }

.sidebar-actions { padding-top: 0.75rem; margin-top: auto; }
.btn-save {
  width: 100%; padding: 0.5rem; border: none; border-radius: 8px;
  background: #6366f1; color: #fff; font-size: 0.875rem; font-weight: 600;
  font-family: inherit; cursor: pointer; transition: background 0.15s;
}
.btn-save:hover:not(:disabled) { background: #4f46e5; }
.btn-save:disabled { opacity: 0.5; cursor: not-allowed; }
.btn-diff {
  width: 100%; padding: 0.5rem; border: 1px solid #d1d5db; border-radius: 8px;
  background: #fff; color: #4b5563; font-size: 0.8125rem; font-weight: 500;
  font-family: inherit; cursor: pointer; transition: all 0.15s; margin-top: 0.5rem;
}
.btn-diff:hover { border-color: #6366f1; color: #6366f1; }
.save-msg { display: block; margin-top: 0.5rem; font-size: 0.75rem; text-align: center; }
.save-msg.success { color: #16a34a; }
.save-msg.error { color: #ef4444; }

/* Editor */
.editor-main { flex: 1; display: flex; flex-direction: column; min-width: 0; }
.editor-toolbar {
  display: flex; align-items: center; gap: 0.75rem; padding: 0.75rem 1.25rem;
  border-bottom: 1px solid #e5e7eb; background: #fafbfc;
}
.editor-title { font-size: 0.8125rem; font-weight: 600; color: #1a1a2e; font-family: 'SF Mono', monospace; }
.dirty-dot { color: #f59e0b; font-size: 1rem; }
.toolbar-right { margin-left: auto; display: flex; gap: 0.5rem; }
.btn-copy, .btn-format, .btn-diff-toolbar {
  padding: 0.25rem 0.75rem; border: 1px solid #d1d5db; border-radius: 6px;
  background: #fff; font-size: 0.75rem; font-family: inherit; color: #4b5563;
  cursor: pointer; transition: all 0.15s;
}
.btn-copy:hover, .btn-format:hover, .btn-diff-toolbar:hover { background: #f3f4f6; border-color: #9ca3af; }
.btn-diff-toolbar.active { background: #f0f2ff; border-color: #6366f1; color: #6366f1; }

/* Code Editor with Highlighting */
.editor-container { flex: 1; position: relative; overflow: hidden; }
.editor-scroll {
  position: absolute; inset: 0; display: flex; overflow: auto;
}

/* Line number gutter */
.line-gutter {
  position: sticky; left: 0; z-index: 2;
  min-width: 56px; padding: 1rem 0; background: #fafbfc;
  border-right: 1px solid #e5e7eb; user-select: none;
  overflow: hidden;
}
.line-num {
  display: flex; align-items: center; justify-content: flex-end; gap: 0;
  font-family: 'SF Mono', 'Fira Code', monospace;
  font-size: 0.75rem; line-height: 1.65; height: calc(0.8125rem * 1.65);
  padding-right: 0.5rem; color: #c4c7cc;
}
.line-num.hidden { display: none; }
.num-text { min-width: 20px; text-align: right; }
.fold-toggle {
  cursor: pointer; font-size: 0.625rem; color: #9ca3af; width: 14px;
  display: inline-flex; align-items: center; justify-content: center;
  transition: color 0.1s;
}
.fold-toggle:hover { color: #6366f1; }

/* Highlight layer (line-by-line for fold support) */
.highlight-layer {
  position: absolute; top: 0; left: 56px; right: 0; bottom: 0;
  padding: 1rem 1.25rem; margin: 0;
  font-family: 'SF Mono', 'Fira Code', 'Cascadia Code', monospace;
  font-size: 0.8125rem; line-height: 1.65;
  white-space: pre; overflow: hidden; pointer-events: none;
  background: transparent; border: none;
}
.hl-line { min-height: calc(0.8125rem * 1.65); }
.hl-line.hidden { display: none; }
.fold-placeholder {
  color: #9ca3af; font-style: italic; font-size: 0.75rem;
  background: #f3f4f6; border-radius: 3px; padding: 0.0625rem 0.375rem;
  margin-left: 0.25rem; pointer-events: auto; cursor: pointer;
}
/* Transparent textarea on top */
.code-editor {
  position: absolute; top: 0; left: 56px; right: 0; bottom: 0;
  padding: 1rem 1.25rem; border: none; resize: none;
  font-family: 'SF Mono', 'Fira Code', 'Cascadia Code', monospace;
  font-size: 0.8125rem; line-height: 1.65;
  color: transparent; caret-color: #1a1a2e;
  background: transparent; z-index: 1;
  tab-size: 2; white-space: pre; overflow: auto;
}
.code-editor:focus { outline: none; }
.code-editor::selection { background: rgba(99, 102, 241, 0.15); }

/* Syntax tokens */
.hl-line { color: #4b5563; }
.hl-line :deep(.tok-key) { color: #1a1a2e; font-weight: 500; }
.hl-line :deep(.tok-annotation) { color: #d97706; font-weight: 600; }
.hl-line :deep(.tok-string) { color: #059669; }
.hl-line :deep(.tok-number) { color: #7c3aed; }
.hl-line :deep(.tok-bool) { color: #2563eb; font-weight: 500; }
.hl-line :deep(.tok-null) { color: #9ca3af; font-style: italic; }

/* JSON error bar */
.json-error {
  position: absolute; bottom: 0; left: 0; right: 0;
  padding: 0.5rem 1.25rem; background: #fef2f2; color: #991b1b;
  font-size: 0.75rem; border-top: 1px solid #fecaca; z-index: 3;
}

/* Diff View */
.diff-container {
  flex: 1; display: flex; flex-direction: column; overflow: hidden;
}
.diff-header {
  padding: 0.5rem 1.25rem; border-bottom: 1px solid #e5e7eb; background: #fafbfc;
}
.diff-stat { font-size: 0.75rem; color: #6b7280; }
.diff-add { color: #16a34a; font-weight: 600; margin-right: 0.375rem; }
.diff-del { color: #dc2626; font-weight: 600; margin-right: 0.375rem; }

.diff-content {
  flex: 1; overflow: auto; padding: 0.5rem 0;
  font-family: 'SF Mono', 'Fira Code', monospace; font-size: 0.8125rem;
}
.diff-line {
  display: flex; line-height: 1.65; padding: 0 1rem;
}
.diff-line.add { background: #dcfce7; }
.diff-line.del { background: #fee2e2; }
.diff-line.ctx { background: transparent; }

.diff-gutter {
  min-width: 40px; text-align: right; padding-right: 0.75rem;
  color: #c4c7cc; user-select: none;
}
.diff-marker {
  min-width: 16px; text-align: center; user-select: none;
  font-weight: 600;
}
.diff-line.add .diff-marker { color: #16a34a; }
.diff-line.del .diff-marker { color: #dc2626; }
.diff-line.ctx .diff-marker { color: #d1d5db; }

.diff-text { flex: 1; white-space: pre; }
.diff-line :deep(.tok-annotation) { color: #d97706; font-weight: 600; }
.diff-line :deep(.tok-key) { color: #1a1a2e; }
.diff-line :deep(.tok-string) { color: #059669; }
.diff-line :deep(.tok-number) { color: #7c3aed; }
.diff-line :deep(.tok-bool) { color: #2563eb; }

:deep(.diff-ellipsis) {
  color: #9ca3af; font-style: italic; user-select: none;
}
</style>
