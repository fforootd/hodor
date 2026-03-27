<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-bold tracking-tight">Events</h1>
        <p class="text-muted-foreground">Audit log of all system events.</p>
      </div>
      <Select v-model="typeFilter" @update:modelValue="load">
        <SelectTrigger class="w-[200px]">
          <SelectValue placeholder="All events" />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="__all__">All events</SelectItem>
          <SelectItem v-for="t in eventTypes" :key="t" :value="t">{{ t }}</SelectItem>
        </SelectContent>
      </Select>
    </div>

    <Card>
      <CardContent class="p-0">
        <div class="divide-y">
          <div
            v-for="event in events" :key="event.id"
            class="flex items-center justify-between px-6 py-3"
          >
            <div class="flex items-center gap-3">
              <Badge :variant="eventVariant(event.event_type)" class="text-xs">
                {{ event.event_type }}
              </Badge>
              <span class="text-sm text-muted-foreground font-mono">
                {{ event.aggregate_type }}:{{ event.aggregate_id }}
              </span>
            </div>
            <span class="text-xs text-muted-foreground">{{ formatTime(event.created_at) }}</span>
          </div>
          <div v-if="!events.length" class="flex h-24 items-center justify-center text-muted-foreground">
            No events
          </div>
        </div>
      </CardContent>
    </Card>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { eventApi, type Event } from '@/api/resources'
import { Card, CardContent } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'

const events = ref<Event[]>([])
const typeFilter = ref('__all__')
const eventTypes = ref<string[]>([])

async function load() {
  try {
    events.value = await eventApi.list({ type: typeFilter.value === '__all__' ? undefined : typeFilter.value, limit: 50 })
  } catch {}
}

onMounted(async () => {
  await load()
  eventTypes.value = [...new Set(events.value.map(e => e.event_type))]
})

function eventVariant(type: string): 'default' | 'secondary' | 'destructive' | 'outline' {
  if (type.includes('created')) return 'default'
  if (type.includes('deleted') || type.includes('revoked')) return 'destructive'
  return 'secondary'
}
function formatTime(ts: string) { return new Date(ts).toLocaleString() }
</script>
