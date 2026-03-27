<template>
  <div>
    <!-- Schema types grouped with version timeline -->
    <div v-for="group in schemaGroups" :key="group.type" class="schema-group">
      <div class="group-header">
        <h3>{{ group.type }}</h3>
        <span class="version-count">{{ group.versions.length }} version{{ group.versions.length !== 1 ? 's' : '' }}</span>
      </div>

      <!-- Version timeline -->
      <div class="version-timeline">
        <div
          v-for="s in group.versions" :key="s.id"
          class="version-row"
          :class="{ 'is-default': s.is_default, 'is-draft': !s.is_default }"
        >
          <div class="timeline-dot" :class="{ active: s.is_default }"></div>
          <div class="version-content">
            <router-link :to="'/schemas/' + s.id" class="version-link">
              <span class="version-badge">v{{ s.version }}</span>
              <span v-if="s.is_default" class="default-badge">default</span>
              <span v-else class="draft-badge">draft</span>
            </router-link>
            <span v-if="s.message" class="commit-msg">{{ s.message }}</span>
            <div class="version-meta">
              <span class="field-tags">
                <span v-for="field in schemaFields(s)" :key="field" class="field-tag">{{ field }}</span>
              </span>
            </div>
            <div class="version-footer">
              <span v-if="s.created_by" class="author">by {{ s.created_by }}</span>
              <span class="time">{{ formatTime(s.created_at) }}</span>
              <div class="version-actions" @click.stop>
                <button
                  v-if="!s.is_default"
                  class="btn-promote"
                  @click="promoteVersion(s)"
                  :disabled="promoting === s.id"
                >
                  {{ promoting === s.id ? 'Promoting…' : '★ Promote' }}
                </button>
                <button
                  v-if="group.versions.length > 1 && !s.is_default"
                  class="btn-diff"
                  @click="showDiff(group.defaultVersion, s)"
                >
                  Diff
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Diff Modal -->
    <div v-if="diffResult" class="diff-modal-backdrop" @click="diffResult = null">
      <div class="diff-modal" @click.stop>
        <div class="diff-modal-header">
          <h4>
            {{ diffResult.left.id }} <span class="diff-arrow">→</span> {{ diffResult.right.id }}
          </h4>
          <button class="btn-close" @click="diffResult = null">✕</button>
        </div>
        <div class="diff-modal-body">
          <div v-if="!diffResult.changes?.length" class="no-changes">No field-level changes detected</div>
          <div v-for="c in diffResult.changes" :key="c.field" class="diff-change">
            <span class="diff-field">{{ c.field }}</span>
            <span class="diff-action" :class="c.action">{{ c.action }}</span>
            <div v-if="c.action === 'modified'" class="diff-values">
              <code class="old">{{ JSON.stringify(c.old?.['x-claim-mapping'] || c.old?.type || c.old, null, 0) }}</code>
              <span class="diff-arrow">→</span>
              <code class="new">{{ JSON.stringify(c.new?.['x-claim-mapping'] || c.new?.type || c.new, null, 0) }}</code>
            </div>
            <div v-else-if="c.action === 'added'" class="diff-values">
              <code class="new">+ {{ JSON.stringify(c.new?.type || c.new, null, 0) }}</code>
            </div>
            <div v-else-if="c.action === 'removed'" class="diff-values">
              <code class="old">- {{ JSON.stringify(c.old?.type || c.old, null, 0) }}</code>
            </div>
          </div>
        </div>
      </div>
    </div>

    <div v-if="!schemaGroups.length" class="empty">No schemas found</div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { schemaApi, type Schema } from '@/api/resources'

const allSchemas = ref<Schema[]>([])
const promoting = ref<string | null>(null)
const diffResult = ref<any>(null)

interface SchemaGroup {
  type: string
  versions: Schema[]
  defaultVersion: Schema
}

const schemaGroups = computed<SchemaGroup[]>(() => {
  const groups = new Map<string, Schema[]>()
  for (const s of allSchemas.value) {
    if (!groups.has(s.type)) groups.set(s.type, [])
    groups.get(s.type)!.push(s)
  }
  return Array.from(groups.entries()).map(([type, versions]) => ({
    type,
    versions: versions.sort((a, b) => b.version - a.version), // newest first
    defaultVersion: versions.find(v => v.is_default) || versions[0],
  }))
})

onMounted(async () => {
  try { allSchemas.value = await schemaApi.list() } catch {}
})

function schemaFields(s: Schema): string[] {
  const props = (s.schema as any)?.properties
  return props ? Object.keys(props) : []
}

async function promoteVersion(s: Schema) {
  promoting.value = s.id
  try {
    await schemaApi.promote(s.id)
    allSchemas.value = await schemaApi.list()
  } catch {}
  promoting.value = null
}

async function showDiff(current: Schema, draft: Schema) {
  try {
    diffResult.value = await schemaApi.diff(current.id, draft.id)
  } catch {}
}

function formatTime(ts: string) { return new Date(ts).toLocaleDateString() }
</script>

<style scoped>
.schema-group {
  background: #fff; border: 1px solid #e5e7eb; border-radius: 10px;
  padding: 1.25rem; margin-bottom: 1rem;
}
.group-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 1rem; }
.group-header h3 { font-size: 1rem; font-weight: 600; color: #1a1a2e; }
.version-count { font-size: 0.75rem; color: #9ca3af; }

/* Timeline */
.version-timeline { position: relative; padding-left: 1.5rem; }
.version-row {
  position: relative; padding: 0.625rem 0; padding-left: 1rem;
  border-left: 2px solid #e5e7eb;
}
.version-row:last-child { border-left-color: transparent; }
.version-row.is-default { border-left-color: #6366f1; }

.timeline-dot {
  position: absolute; left: -0.375rem; top: 0.875rem;
  width: 0.625rem; height: 0.625rem; border-radius: 50%;
  background: #d1d5db; border: 2px solid #fff;
}
.timeline-dot.active { background: #6366f1; box-shadow: 0 0 0 3px rgba(99,102,241,.2); }

.version-content { padding-left: 0.75rem; }
.version-link {
  display: inline-flex; align-items: center; gap: 0.5rem;
  text-decoration: none; color: inherit;
}
.version-link:hover .version-badge { background: #e0e2ff; }

.version-badge {
  font-size: 0.75rem; font-weight: 700; padding: 0.125rem 0.5rem;
  background: #f0f2ff; color: #6366f1; border-radius: 4px; transition: background 0.15s;
  font-family: 'SF Mono', monospace;
}
.default-badge {
  font-size: 0.625rem; font-weight: 600; padding: 0.0625rem 0.375rem;
  background: #ecfdf5; color: #059669; border-radius: 3px; text-transform: uppercase;
  letter-spacing: 0.04em;
}
.draft-badge {
  font-size: 0.625rem; font-weight: 600; padding: 0.0625rem 0.375rem;
  background: #fef3c7; color: #92400e; border-radius: 3px; text-transform: uppercase;
  letter-spacing: 0.04em;
}

.commit-msg {
  display: block; font-size: 0.8125rem; color: #4b5563; margin-top: 0.25rem;
  font-style: italic;
}

.version-meta { margin-top: 0.375rem; }
.field-tags { display: flex; flex-wrap: wrap; gap: 0.25rem; }
.field-tag { font-size: 0.6875rem; padding: 0.0625rem 0.375rem; background: #f3f4f6; color: #6b7280; border-radius: 3px; }

.version-footer {
  display: flex; align-items: center; gap: 0.75rem; margin-top: 0.375rem; flex-wrap: wrap;
}
.author { font-size: 0.75rem; color: #6b7280; }
.time { font-size: 0.75rem; color: #9ca3af; }

.version-actions { display: flex; gap: 0.375rem; margin-left: auto; }
.btn-promote {
  padding: 0.25rem 0.625rem; border: 1px solid #c7d2fe; border-radius: 6px;
  background: #f0f2ff; color: #6366f1; font-size: 0.75rem; font-weight: 600;
  font-family: inherit; cursor: pointer; transition: all 0.15s;
}
.btn-promote:hover { background: #e0e2ff; border-color: #6366f1; }
.btn-promote:disabled { opacity: 0.5; cursor: not-allowed; }
.btn-diff {
  padding: 0.25rem 0.625rem; border: 1px solid #d1d5db; border-radius: 6px;
  background: #fff; color: #4b5563; font-size: 0.75rem; font-weight: 500;
  font-family: inherit; cursor: pointer; transition: all 0.15s;
}
.btn-diff:hover { border-color: #9ca3af; background: #f9fafb; }

/* Diff Modal */
.diff-modal-backdrop {
  position: fixed; inset: 0; background: rgba(0,0,0,0.3); z-index: 100;
  display: flex; align-items: center; justify-content: center;
}
.diff-modal {
  background: #fff; border-radius: 12px; width: 600px; max-height: 80vh;
  overflow: hidden; box-shadow: 0 20px 60px rgba(0,0,0,0.2);
}
.diff-modal-header {
  display: flex; align-items: center; justify-content: space-between;
  padding: 1rem 1.25rem; border-bottom: 1px solid #e5e7eb;
}
.diff-modal-header h4 { font-size: 0.9375rem; font-weight: 600; color: #1a1a2e; }
.diff-arrow { color: #9ca3af; margin: 0 0.25rem; }
.btn-close {
  width: 28px; height: 28px; border: 1px solid #e5e7eb; border-radius: 6px;
  background: #fff; cursor: pointer; font-size: 0.75rem; display: flex;
  align-items: center; justify-content: center; transition: all 0.15s;
}
.btn-close:hover { background: #f9fafb; border-color: #9ca3af; }
.diff-modal-body { padding: 1.25rem; overflow-y: auto; max-height: 60vh; }
.no-changes { text-align: center; color: #9ca3af; padding: 2rem; }
.diff-change {
  padding: 0.625rem 0; border-bottom: 1px solid #f3f4f6;
}
.diff-change:last-child { border-bottom: none; }
.diff-field { font-weight: 600; font-size: 0.875rem; color: #1a1a2e; margin-right: 0.5rem; }
.diff-action { font-size: 0.6875rem; font-weight: 600; padding: 0.0625rem 0.375rem; border-radius: 3px; text-transform: uppercase; }
.diff-action.added { background: #ecfdf5; color: #059669; }
.diff-action.removed { background: #fef2f2; color: #dc2626; }
.diff-action.modified { background: #fef3c7; color: #92400e; }
.diff-values { margin-top: 0.375rem; font-size: 0.8125rem; }
.diff-values code {
  padding: 0.125rem 0.375rem; border-radius: 3px; font-size: 0.75rem;
  font-family: 'SF Mono', monospace;
}
.diff-values code.old { background: #fee2e2; color: #991b1b; }
.diff-values code.new { background: #dcfce7; color: #166534; }

.empty { padding: 3rem; text-align: center; color: #9ca3af; }
</style>
