<script setup lang="ts">
import { computed } from 'vue'
import { Badge } from '@/components/ui/badge'
import { CheckCircle2, Ban, Clock, AlertTriangle } from 'lucide-vue-next'

const props = defineProps<{
  state?: string
}>()

const config = computed(() => {
  const s = (props.state || 'active').toLowerCase()
  switch (s) {
    case 'active':
      return { icon: CheckCircle2, class: 'text-green-700 bg-green-100 border-green-200' }
    case 'deactivated':
    case 'locked':
    case 'disabled':
      return { icon: Ban, class: 'text-red-700 bg-red-100 border-red-200' }
    case 'pending':
    case 'initializing':
      return { icon: Clock, class: 'text-amber-700 bg-amber-100 border-amber-200' }
    case 'warning':
      return { icon: AlertTriangle, class: 'text-amber-700 bg-amber-100 border-amber-200' }
    default:
      return { icon: CheckCircle2, class: 'text-gray-700 bg-gray-100 border-gray-200' }
  }
})

const label = computed(() => props.state || 'active')
</script>

<template>
  <Badge
    variant="outline"
    :class="`font-normal flex items-center gap-1 capitalize whitespace-nowrap ${config.class}`"
  >
    <component :is="config.icon" class="w-3 h-3 shrink-0" />
    <span>{{ label }}</span>
  </Badge>
</template>
