<template>
  <div>
    <div class="toolbar">
      <h3>{{ identities.length }} identities</h3>
      <router-link to="/identities/new" class="btn-primary">+ New Identity</router-link>
    </div>
    <div class="table-wrap">
      <table>
        <thead>
          <tr>
            <th>Identifier</th>
            <th>Display Name</th>
            <th>State</th>
            <th>Schema</th>
            <th>Created</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="i in identities" :key="i.id" @click="$router.push(`/identities/${i.id}`)" class="clickable">
            <td class="identifier">{{ i.identifier }}</td>
            <td>{{ i.display_name || '—' }}</td>
            <td><span class="badge" :class="i.state">{{ i.state }}</span></td>
            <td class="schema">{{ i.schema_name || '—' }}</td>
            <td class="time">{{ formatTime(i.created_at) }}</td>
          </tr>
        </tbody>
      </table>
      <div v-if="!identities.length" class="empty">No identities found</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { identityApi, type Identity } from '@/api/resources'

const identities = ref<Identity[]>([])

onMounted(async () => {
  try { identities.value = await identityApi.list() } catch {}
})

function formatTime(ts: string) { return new Date(ts).toLocaleDateString() }
</script>

<style scoped>
.toolbar { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem; }
.toolbar h3 { font-size: 0.875rem; font-weight: 500; color: #6b7280; }
.btn-primary {
  padding: 0.5rem 1rem; background: #1a1a2e; color: #fff; border-radius: 8px;
  font-size: 0.8125rem; font-weight: 600; text-decoration: none;
}
.table-wrap { background: #fff; border: 1px solid #e5e7eb; border-radius: 10px; overflow: hidden; }
table { width: 100%; border-collapse: collapse; }
th { text-align: left; padding: 0.75rem 1.25rem; font-size: 0.75rem; font-weight: 600; color: #6b7280; text-transform: uppercase; letter-spacing: 0.05em; border-bottom: 1px solid #e5e7eb; }
td { padding: 0.75rem 1.25rem; font-size: 0.875rem; color: #1a1a2e; border-bottom: 1px solid #f3f4f6; }
.clickable { cursor: pointer; }
.clickable:hover { background: #f9fafb; }
.identifier { font-weight: 500; }
.schema { color: #6b7280; }
.time { color: #9ca3af; font-size: 0.8125rem; }
.badge {
  display: inline-block; padding: 0.125rem 0.5rem; border-radius: 4px;
  font-size: 0.75rem; font-weight: 500;
}
.badge.active { background: #ecfdf5; color: #059669; }
.badge.deactivated { background: #fef2f2; color: #dc2626; }
.empty { padding: 3rem; text-align: center; color: #9ca3af; }
</style>
