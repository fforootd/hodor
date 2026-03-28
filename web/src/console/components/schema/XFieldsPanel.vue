<template>
  <div class="space-y-2">
    <div v-for="f in fields" :key="f.name" class="flex items-center gap-1.5 py-0.5">
      <span class="text-sm font-medium truncate">{{ f.name }}</span>
      <Badge v-if="f.isIdentifier" variant="secondary" class="text-[10px] px-1.5 bg-blue-100 text-blue-700 border-blue-200">ID</Badge>
      <Badge v-if="f.isSensitive" variant="secondary" class="text-[10px] px-1.5 bg-red-100 text-red-700 border-red-200">PII</Badge>
      <Badge v-if="f.hasMfa" variant="secondary" class="text-[10px] px-1.5 bg-emerald-100 text-emerald-700 border-emerald-200">MFA</Badge>
      <Badge v-if="f.hasClaimMapping" variant="outline" class="text-[10px] px-1.5">⇄</Badge>
    </div>
    <p v-if="!fields.length" class="text-xs text-muted-foreground">No fields defined</p>
  </div>
</template>

<script setup>
import { computed } from 'vue'
import { Badge } from '@/components/ui/badge'

const props = defineProps({
  schema: { type: Object, required: true },
})

const fields = computed(() => {
  const properties = props.schema?.properties || {}
  return Object.entries(properties)
    .filter(([, v]) => !v['x-hidden'])
    .map(([name, v]) => ({
      name,
      isIdentifier: !!v['x-auth']?.identifier,
      isSensitive: !!v['x-sensitive'],
      hasMfa: !!v['x-auth']?.mfa,
      hasClaimMapping: !!v['x-claim-mapping'],
    }))
})
</script>
