<template>
  <div>
    <div class="table-wrap">
      <table>
        <thead>
          <tr><th>Identifier</th><th>User Agent</th><th>IP</th><th>Created</th><th>Expires</th><th></th></tr>
        </thead>
        <tbody>
          <tr v-for="s in sessions" :key="s.id">
            <td class="identifier">{{ s.identifier || `identity-${s.identity_id}` }}</td>
            <td class="meta">{{ truncate(s.user_agent, 30) }}</td>
            <td class="meta">{{ s.ip_address || '—' }}</td>
            <td class="time">{{ formatTime(s.created_at) }}</td>
            <td class="time">{{ formatTime(s.expires_at) }}</td>
            <td><button class="btn-danger" @click="revoke(s.id)">Revoke</button></td>
          </tr>
        </tbody>
      </table>
      <div v-if="!sessions.length" class="empty">No active sessions</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { sessionApi, type Session } from '@/api/resources'

const sessions = ref<Session[]>([])

onMounted(async () => {
  try { sessions.value = await sessionApi.list() } catch {}
})

async function revoke(id: string) {
  await sessionApi.revoke(id)
  sessions.value = sessions.value.filter(s => s.id !== id)
}

function truncate(s: string, n: number) { return s?.length > n ? s.slice(0, n) + '…' : s || '—' }
function formatTime(ts: string) { return new Date(ts).toLocaleString() }
</script>

<style scoped>
.table-wrap { background: #fff; border: 1px solid #e5e7eb; border-radius: 10px; overflow: hidden; }
table { width: 100%; border-collapse: collapse; }
th { text-align: left; padding: 0.75rem 1.25rem; font-size: 0.75rem; font-weight: 600; color: #6b7280; text-transform: uppercase; border-bottom: 1px solid #e5e7eb; }
td { padding: 0.75rem 1.25rem; font-size: 0.875rem; color: #1a1a2e; border-bottom: 1px solid #f3f4f6; }
.identifier { font-weight: 500; }
.meta { color: #6b7280; font-size: 0.8125rem; }
.time { color: #9ca3af; font-size: 0.8125rem; }
.btn-danger { padding: 0.25rem 0.5rem; background: #fef2f2; color: #dc2626; border: 1px solid #fecaca; border-radius: 4px; font-size: 0.75rem; cursor: pointer; }
.btn-danger:hover { background: #fee2e2; }
.empty { padding: 3rem; text-align: center; color: #9ca3af; }
</style>
