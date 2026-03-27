<template>
  <div class="dashboard">
    <div class="stats">
      <div class="stat-card">
        <div class="stat-label">IDENTITIES</div>
        <div class="stat-value">{{ stats.identities }}</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">ACTIVE SESSIONS</div>
        <div class="stat-value">{{ stats.sessions }}</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">EVENTS</div>
        <div class="stat-value">{{ stats.events }}</div>
      </div>
      <div class="stat-card">
        <div class="stat-label">SCHEMAS</div>
        <div class="stat-value">{{ stats.schemas }}</div>
      </div>
    </div>

    <div class="section">
      <h3>Recent Events</h3>
      <div class="event-list">
        <div v-for="event in recentEvents" :key="event.id" class="event-row">
          <span class="event-type" :class="eventClass(event.event_type)">{{ event.event_type }}</span>
          <span class="event-time">{{ formatTime(event.created_at) }}</span>
        </div>
        <div v-if="!recentEvents.length" class="empty">No events yet</div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { entityApi, schemaApi, sessionApi, eventApi, type Event } from '@/api/resources'

const stats = ref({ identities: 0, sessions: 0, events: 0, schemas: 0 })
const recentEvents = ref<Event[]>([])

onMounted(async () => {
  try {
    const [identities, schemas, sessions, events] = await Promise.all([
      entityApi.list(), schemaApi.list(), sessionApi.list(), eventApi.list({ limit: 10 }),
    ])
    stats.value = {
      identities: identities.length, schemas: schemas.length,
      sessions: sessions.length, events: events.length,
    }
    recentEvents.value = events.slice(0, 5)
  } catch { /* fallback to zeros */ }
})

function eventClass(type: string) {
  if (type.includes('created')) return 'created'
  if (type.includes('deleted')) return 'deleted'
  return 'default'
}

function formatTime(ts: string) {
  return new Date(ts).toLocaleString()
}
</script>

<style scoped>
.stats { display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: 1rem; margin-bottom: 2rem; }
.stat-card {
  background: #fff; border: 1px solid #e5e7eb; border-radius: 10px; padding: 1.25rem 1.5rem;
}
.stat-label { font-size: 0.75rem; font-weight: 600; color: #6b7280; text-transform: uppercase; letter-spacing: 0.05em; }
.stat-value { font-size: 2rem; font-weight: 700; color: #1a1a2e; margin-top: 0.25rem; }

.section h3 { font-size: 1rem; font-weight: 600; color: #1a1a2e; margin-bottom: 1rem; }
.event-list { background: #fff; border: 1px solid #e5e7eb; border-radius: 10px; overflow: hidden; }
.event-row {
  display: flex; justify-content: space-between; align-items: center;
  padding: 0.75rem 1.25rem; border-bottom: 1px solid #f3f4f6;
}
.event-row:last-child { border-bottom: none; }
.event-type {
  font-size: 0.8125rem; font-weight: 500; padding: 0.125rem 0.5rem;
  border-radius: 4px; background: #f3f4f6; color: #4b5563;
}
.event-type.created { background: #ecfdf5; color: #059669; }
.event-type.deleted { background: #fef2f2; color: #dc2626; }
.event-time { font-size: 0.75rem; color: #9ca3af; }
.empty { padding: 2rem; text-align: center; color: #9ca3af; font-size: 0.875rem; }
</style>
