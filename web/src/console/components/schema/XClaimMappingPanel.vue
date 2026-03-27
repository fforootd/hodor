<template>
  <div class="sidebar-section">
    <h4 class="sidebar-heading">Claim Mapping</h4>
    <div class="claim-table">
      <div class="claim-header">
        <span>Field</span>
        <span>{{ direction === 'inbound' ? 'IDP Attribute' : 'OIDC Claim' }}</span>
      </div>
      <div v-for="m in mappings" :key="m.field" class="claim-row">
        <span class="claim-field">{{ m.field }}</span>
        <code class="claim-expr">{{ m.expr }}</code>
      </div>
      <div v-if="!mappings.length" class="claim-empty">No claim mappings defined</div>
    </div>
  </div>
</template>

<script setup>
import { computed } from 'vue'

const props = defineProps({
  schema: { type: Object, required: true },
  direction: { type: String, default: 'outbound' }, // outbound (schema→claims), inbound (idp→schema)
})

const mappings = computed(() => {
  const props_ = props.schema?.properties || {}
  return Object.entries(props_)
    .filter(([, v]) => v['x-claim-mapping'])
    .map(([field, v]) => ({
      field,
      expr: v['x-claim-mapping'],
    }))
})
</script>

<style scoped>
.claim-table { border: 1px solid #e5e7eb; border-radius: 6px; overflow: hidden; }
.claim-header {
  display: grid; grid-template-columns: 1fr 2fr; gap: 0.5rem;
  padding: 0.375rem 0.5rem; background: #f9fafb;
  font-size: 0.6875rem; font-weight: 600; color: #9ca3af;
  text-transform: uppercase; letter-spacing: 0.04em;
}
.claim-row {
  display: grid; grid-template-columns: 1fr 2fr; gap: 0.5rem;
  padding: 0.375rem 0.5rem; border-top: 1px solid #f3f4f6;
  font-size: 0.8125rem; align-items: center;
}
.claim-row:hover { background: #f9fafb; }
.claim-field { font-weight: 500; color: #1a1a2e; }
.claim-expr {
  font-size: 0.75rem; color: #6366f1; background: #f0f2ff;
  padding: 0.125rem 0.375rem; border-radius: 3px;
  font-family: 'SF Mono', 'Fira Code', monospace;
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.claim-empty { padding: 0.5rem; text-align: center; color: #9ca3af; font-size: 0.8125rem; }
</style>
