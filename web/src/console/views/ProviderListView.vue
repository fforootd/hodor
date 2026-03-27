<template>
  <div>
    <div class="toolbar">
      <h3>{{ providers.length }} provider{{ providers.length !== 1 ? 's' : '' }}</h3>
    </div>
    <div class="table-wrap">
      <table>
        <thead>
          <tr>
            <th>Name</th>
            <th>Protocol</th>
            <th>Template</th>
            <th>Enabled</th>
            <th>Created</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="p in providers" :key="p.id" class="clickable">
            <td class="name">{{ p.name }}</td>
            <td><span class="protocol-badge">{{ p.protocol }}</span></td>
            <td>{{ p.template }}</td>
            <td><span class="badge" :class="p.enabled ? 'active' : 'deactivated'">{{ p.enabled ? 'enabled' : 'disabled' }}</span></td>
            <td class="time">{{ formatTime(p.created_at) }}</td>
          </tr>
        </tbody>
      </table>
      <div v-if="!providers.length" class="empty">No providers configured</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'

interface Provider {
  id: string; name: string; protocol: string; template: string;
  enabled: boolean; created_at: string;
}

const providers = ref<Provider[]>([])

onMounted(async () => {
  try {
    const res = await fetch('/v1/providers')
    const data = await res.json()
    providers.value = data.items || []
  } catch { /* ignore */ }
})

function formatTime(ts: string) { return new Date(ts).toLocaleDateString() }
</script>

<style scoped>
.toolbar { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem; }
.toolbar h3 { font-size: 0.875rem; font-weight: 500; color: #6b7280; }
.table-wrap { background: #fff; border: 1px solid #e5e7eb; border-radius: 10px; overflow: hidden; }
table { width: 100%; border-collapse: collapse; }
th { text-align: left; padding: 0.75rem 1.25rem; font-size: 0.75rem; font-weight: 600; color: #6b7280; text-transform: uppercase; letter-spacing: 0.05em; border-bottom: 1px solid #e5e7eb; }
td { padding: 0.75rem 1.25rem; font-size: 0.875rem; color: #1a1a2e; border-bottom: 1px solid #f3f4f6; }
.clickable { cursor: pointer; }
.clickable:hover { background: #f9fafb; }
.name { font-weight: 500; }
.protocol-badge {
  display: inline-block; padding: 0.125rem 0.5rem; border-radius: 4px;
  font-size: 0.75rem; font-weight: 500; background: #eff6ff; color: #2563eb;
  text-transform: uppercase;
}
.time { color: #9ca3af; font-size: 0.8125rem; }
.badge { display: inline-block; padding: 0.125rem 0.5rem; border-radius: 4px; font-size: 0.75rem; font-weight: 500; }
.badge.active { background: #ecfdf5; color: #059669; }
.badge.deactivated { background: #fef2f2; color: #dc2626; }
.empty { padding: 3rem; text-align: center; color: #9ca3af; }
</style>
