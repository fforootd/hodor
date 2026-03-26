<template>
  <div class="grid">
    <router-link
      v-for="schema in schemas" :key="schema.id"
      :to="'/schemas/' + schema.id"
      class="schema-card"
    >
      <div class="schema-header">
        <h3>{{ schema.type }}</h3>
        <span class="version-badge">v{{ schema.version }}</span>
      </div>
      <p class="desc">{{ schema.id }}</p>
      <div class="schema-fields">
        <span v-for="field in schemaFields(schema)" :key="field" class="field-tag">{{ field }}</span>
      </div>
      <div class="schema-annotations">
        <span v-if="hasAnnotation(schema, 'x-login')" class="anno-tag login">login flow</span>
        <span v-if="hasAnnotation(schema, 'x-branding')" class="anno-tag branding">branding</span>
        <span v-if="hasAuthFields(schema)" class="anno-tag auth">x-auth</span>
      </div>
      <div class="schema-meta">Created {{ formatTime(schema.created_at) }}</div>
    </router-link>
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

function hasAnnotation(s: Schema, key: string): boolean {
  return !!(s.schema as any)?.[key]
}

function hasAuthFields(s: Schema): boolean {
  const props = (s.schema as any)?.properties
  if (!props) return false
  return Object.values(props).some((p: any) => p?.['x-auth'])
}

function formatTime(ts: string) { return new Date(ts).toLocaleDateString() }
</script>

<style scoped>
.grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 1rem; }
.schema-card {
  background: #fff; border: 1px solid #e5e7eb; border-radius: 10px; padding: 1.25rem;
  transition: box-shadow 0.2s, border-color 0.2s; text-decoration: none; color: inherit; display: block;
  cursor: pointer;
}
.schema-card:hover { box-shadow: 0 4px 16px rgba(99,102,241,.1); border-color: #c7d2fe; }
.schema-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 0.25rem; }
h3 { font-size: 1rem; font-weight: 600; color: #1a1a2e; }
.version-badge {
  font-size: 0.6875rem; font-weight: 600; padding: 0.125rem 0.5rem;
  background: #f0f2ff; color: #6366f1; border-radius: 4px;
}
.desc { font-size: 0.8125rem; color: #6b7280; margin-bottom: 0.75rem; font-family: monospace; }
.schema-fields { display: flex; flex-wrap: wrap; gap: 0.375rem; margin-bottom: 0.5rem; }
.field-tag { font-size: 0.75rem; padding: 0.125rem 0.5rem; background: #f3f4f6; color: #374151; border-radius: 4px; }
.schema-annotations { display: flex; flex-wrap: wrap; gap: 0.375rem; margin-bottom: 0.75rem; }
.anno-tag {
  font-size: 0.6875rem; font-weight: 600; padding: 0.125rem 0.5rem; border-radius: 4px;
  text-transform: uppercase; letter-spacing: 0.03em;
}
.anno-tag.login { background: #fef3c7; color: #92400e; }
.anno-tag.branding { background: #ede9fe; color: #6d28d9; }
.anno-tag.auth { background: #d1fae5; color: #065f46; }
.schema-meta { font-size: 0.75rem; color: #9ca3af; }
.empty { grid-column: 1/-1; padding: 3rem; text-align: center; color: #9ca3af; }
</style>
