<template>
  <div class="space-y-4">
    <Card v-for="snippet in snippets" :key="snippet.title">
      <CardHeader class="pb-3">
        <div class="flex items-center justify-between gap-3">
          <CardTitle class="text-sm">{{ snippet.title }}</CardTitle>
          <Button variant="outline" size="sm" @click="copy(snippet.command)">Copy cURL</Button>
        </div>
      </CardHeader>
      <CardContent>
        <pre class="overflow-x-auto rounded-lg border bg-muted/30 p-4 text-xs leading-5"><code>{{ snippet.command }}</code></pre>
      </CardContent>
    </Card>
  </div>
</template>

<script setup lang="ts">
import type { CurlSnippet } from '@/console/utils/schema-resource'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'

defineProps<{
  snippets: CurlSnippet[]
}>()

function copy(value: string) {
  navigator.clipboard.writeText(value)
}
</script>
