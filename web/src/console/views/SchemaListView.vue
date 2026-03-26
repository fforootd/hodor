<template>
  <div class="grid">
    <div v-for="schema in schemas" :key="schema.id" class="schema-card">
      <h3>{{ schema.name }}</h3>
      <p class="desc">{{ schema.description || 'No description' }}</p>
      <div class="schema-fields">
        <span v-for="field in schemaFields(schema)" :key="field" class="field-tag">{{ field }}</span>
      </div>
      <div class="schema-meta">Created {{ formatTime(schema.created_at) }}</div>
    </div>
    <div v-if="!schemas.length" class="empty">No schemas found</div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { schemaApi, type Schema } from '@/api/resources'

const schemas = ref<Schema[]>([])

onMounted(async () => {
  try { schemas.value = await schemaApi.list() } catch {}
})

function schemaFields(s: Schema): string[] {
  const props = (s.schema as any)?.properties
  return props ? Object.keys(props) : []
}
function formatTime(ts: string) { return new Date(ts).toLocaleDateString() }
</script>

<style scoped>
.grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 1rem; }
.schema-card {
  background: #fff; border: 1px solid #e5e7eb; border-radius: 10px; padding: 1.25rem;
  transition: box-shadow 0.15s;
}
.schema-card:hover { box-shadow: 0 4px 12px rgba(0,0,0,.06); }
h3 { font-size: 1rem; font-weight: 600; color: #1a1a2e; margin-bottom: 0.25rem; }
.desc { font-size: 0.8125rem; color: #6b7280; margin-bottom: 0.75rem; }
.schema-fields { display: flex; flex-wrap: wrap; gap: 0.375rem; margin-bottom: 0.75rem; }
.field-tag { font-size: 0.75rem; padding: 0.125rem 0.5rem; background: #f0f2ff; color: #4f46e5; border-radius: 4px; }
.schema-meta { font-size: 0.75rem; color: #9ca3af; }
.empty { grid-column: 1/-1; padding: 3rem; text-align: center; color: #9ca3af; }
</style>
