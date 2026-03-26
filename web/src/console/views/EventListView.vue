<template>
  <div>
    <div class="filters">
      <select v-model="typeFilter" @change="load">
        <option value="">All events</option>
        <option v-for="t in eventTypes" :key="t" :value="t">{{ t }}</option>
      </select>
    </div>
    <div class="event-list">
      <div v-for="event in events" :key="event.id" class="event-row">
        <div class="event-left">
          <span class="event-type" :class="eventClass(event.event_type)">{{ event.event_type }}</span>
          <span class="event-detail">{{ event.aggregate_type }}:{{ event.aggregate_id }}</span>
        </div>
        <span class="event-time">{{ formatTime(event.created_at) }}</span>
      </div>
      <div v-if="!events.length" class="empty">No events</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { eventApi, type Event } from '@/api/resources'

const events = ref<Event[]>([])
const typeFilter = ref('')
const eventTypes = ref<string[]>([])

async function load() {
  try {
    events.value = await eventApi.list({ type: typeFilter.value || undefined, limit: 50 })
  } catch {}
}

onMounted(async () => {
  await load()
  eventTypes.value = [...new Set(events.value.map(e => e.event_type))]
})

function eventClass(type: string) {
  if (type.includes('created')) return 'created'
  if (type.includes('deleted') || type.includes('revoked')) return 'deleted'
  if (type.includes('login')) return 'auth'
  return ''
}
function formatTime(ts: string) { return new Date(ts).toLocaleString() }
</script>

<style scoped>
.filters { margin-bottom: 1rem; }
select {
  padding: 0.5rem 0.75rem; border: 1px solid #e5e7eb; border-radius: 8px;
  font-size: 0.875rem; font-family: inherit; background: #fff;
}
.event-list { background: #fff; border: 1px solid #e5e7eb; border-radius: 10px; overflow: hidden; }
.event-row {
  display: flex; justify-content: space-between; align-items: center;
  padding: 0.75rem 1.25rem; border-bottom: 1px solid #f3f4f6;
}
.event-row:last-child { border-bottom: none; }
.event-left { display: flex; align-items: center; gap: 0.75rem; }
.event-type {
  font-size: 0.8125rem; font-weight: 500; padding: 0.125rem 0.5rem;
  border-radius: 4px; background: #f3f4f6; color: #4b5563;
}
.event-type.created { background: #ecfdf5; color: #059669; }
.event-type.deleted { background: #fef2f2; color: #dc2626; }
.event-type.auth { background: #eff6ff; color: #2563eb; }
.event-detail { font-size: 0.8125rem; color: #9ca3af; }
.event-time { font-size: 0.75rem; color: #9ca3af; }
.empty { padding: 3rem; text-align: center; color: #9ca3af; }
</style>
