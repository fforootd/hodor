<template>
  <div v-if="schema" class="space-y-3">
    <div class="space-y-2">
      <div class="flex items-center justify-between">
        <span class="text-xs text-muted-foreground">Type</span>
        <span class="text-sm font-mono font-medium">{{ schema.type }}</span>
      </div>
      <div class="flex items-center justify-between">
        <span class="text-xs text-muted-foreground">Version</span>
        <div class="flex items-center gap-1.5">
          <Badge variant="secondary" class="font-mono text-xs">v{{ schema.version }}</Badge>
          <Badge v-if="schema.is_default" class="text-[10px]">default</Badge>
          <Badge v-else variant="outline" class="text-[10px] border-yellow-300 bg-yellow-50 text-yellow-700">draft</Badge>
        </div>
      </div>
      <div v-if="schema.message" class="flex items-center justify-between">
        <span class="text-xs text-muted-foreground">Message</span>
        <span class="text-sm truncate max-w-[160px]" :title="schema.message">{{ schema.message }}</span>
      </div>
      <div v-if="entityCount >= 0" class="flex items-center justify-between">
        <span class="text-xs text-muted-foreground">Entities</span>
        <Badge :variant="entityCount > 0 ? 'secondary' : 'outline'" :class="entityCount > 0 ? 'bg-yellow-50 text-yellow-700 border-yellow-200' : ''">
          {{ entityCount.toLocaleString() }} {{ entityCount === 1 ? 'entity' : 'entities' }}
        </Badge>
      </div>
    </div>

    <!-- Version history -->
    <div v-if="versions.length > 1">
      <Separator class="my-2" />
      <Collapsible v-model:open="historyOpen">
        <CollapsibleTrigger class="flex w-full items-center justify-between py-1 text-xs font-semibold text-muted-foreground uppercase tracking-wider cursor-pointer hover:text-foreground transition-colors">
          Version History
          <ChevronDown class="size-3.5 transition-transform" :class="historyOpen ? 'rotate-180' : ''" />
        </CollapsibleTrigger>
        <CollapsibleContent>
          <div class="mt-2 space-y-0.5">
            <router-link
              v-for="v in versions" :key="v.id"
              :to="'/schemas/' + v.id"
              class="flex items-center gap-2 rounded-md px-2 py-1.5 text-sm no-underline transition-colors"
              :class="v.id === schema.id ? 'bg-primary/5 text-primary' : 'hover:bg-muted text-foreground'"
            >
              <Badge variant="secondary" class="font-mono text-[10px] px-1.5">v{{ v.version }}</Badge>
              <span v-if="v.is_default" class="text-emerald-600 text-xs">★</span>
              <span class="text-xs text-muted-foreground truncate">{{ v.message || 'No message' }}</span>
            </router-link>
          </div>
        </CollapsibleContent>
      </Collapsible>
    </div>

    <!-- Promote button -->
    <Button
      v-if="!schema.is_default"
      variant="outline"
      size="sm"
      class="w-full"
      @click="$emit('promote')"
      :disabled="promoteLoading"
    >
      {{ promoteLoading ? 'Promoting…' : '★ Promote to Default' }}
    </Button>
  </div>
</template>

<script setup>
import { ref } from 'vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible'
import { ChevronDown } from 'lucide-vue-next'

defineProps({
  schema: { type: Object, required: true },
  versions: { type: Array, default: () => [] },
  entityCount: { type: Number, default: -1 },
  promoteLoading: { type: Boolean, default: false },
})
defineEmits(['promote'])

const historyOpen = ref(true)
</script>
