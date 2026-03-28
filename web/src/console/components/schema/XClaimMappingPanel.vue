<template>
  <div>
    <Table v-if="mappings.length">
      <TableHeader>
        <TableRow>
          <TableHead class="h-8 text-xs">Field</TableHead>
          <TableHead class="h-8 text-xs">{{ direction === 'inbound' ? 'IDP Attribute' : 'OIDC Claim' }}</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow v-for="m in mappings" :key="m.field">
          <TableCell class="py-1.5 text-sm font-medium">{{ m.field }}</TableCell>
          <TableCell class="py-1.5">
            <code class="rounded bg-primary/5 px-1.5 py-0.5 text-xs font-mono text-primary">{{ m.expr }}</code>
          </TableCell>
        </TableRow>
      </TableBody>
    </Table>
    <p v-else class="text-xs text-muted-foreground">No claim mappings defined</p>
  </div>
</template>

<script setup>
import { computed } from 'vue'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'

const props = defineProps({
  schema: { type: Object, required: true },
  direction: { type: String, default: 'outbound' },
})

const mappings = computed(() => {
  const props_ = props.schema?.properties || {}
  return Object.entries(props_)
    .filter(([, v]) => v['x-claim-mapping'])
    .map(([field, v]) => ({
      field,
      expr: v['x-claim-mapping'],
    }))
})
</script>
