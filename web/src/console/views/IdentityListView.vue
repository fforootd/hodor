<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-bold tracking-tight">{{ label }}</h1>
        <p class="text-muted-foreground">{{ identities.length }} {{ label.toLowerCase() }} total</p>
      </div>
      <Button as-child>
        <router-link :to="`/s/${schemaType}/new`">
          <Plus class="mr-2 size-4" />
          New {{ singularLabel }}
        </router-link>
      </Button>
    </div>

    <!-- OIDC Discovery panel (shown for app type) -->
    <Card v-if="schemaType === 'app'" class="bg-muted/50">
      <CardHeader class="pb-3">
        <CardTitle class="text-sm font-medium">OIDC Discovery</CardTitle>
      </CardHeader>
      <CardContent class="space-y-2">
        <div class="flex items-center gap-3">
          <span class="text-sm text-muted-foreground w-20">Issuer</span>
          <code
            class="cursor-pointer rounded bg-primary/10 px-2 py-0.5 text-sm font-mono text-primary hover:bg-primary/20 transition-colors"
            @click="copy(issuer)"
          >{{ issuer }}</code>
        </div>
        <div class="flex items-center gap-3">
          <span class="text-sm text-muted-foreground w-20">Discovery</span>
          <code
            class="cursor-pointer rounded bg-primary/10 px-2 py-0.5 text-sm font-mono text-primary hover:bg-primary/20 transition-colors"
            @click="copy(issuer + '/.well-known/openid-configuration')"
          >{{ issuer }}/.well-known/openid-configuration</code>
        </div>
      </CardContent>
    </Card>

    <Card>
      <CardContent class="p-0">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Identifier</TableHead>
              <TableHead>Display Name</TableHead>
              <TableHead v-if="schemaType === 'app'">Type</TableHead>
              <TableHead v-if="schemaType === 'app'">Redirect URIs</TableHead>
              <TableHead>State</TableHead>
              <TableHead>Created</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow
              v-for="i in identities" :key="i.id"
              class="cursor-pointer"
              @click="$router.push(`/identities/${i.id}`)"
            >
              <TableCell :class="schemaType === 'app' ? 'font-mono text-sm text-primary' : 'font-medium'">
                {{ i.identifier }}
              </TableCell>
              <TableCell>{{ getField(i, 'client_name') || getField(i, 'display_name') || i.display_name || '—' }}</TableCell>
              <TableCell v-if="schemaType === 'app'">
                <Badge variant="outline" class="text-xs uppercase">{{ getField(i, 'app_type') || '—' }}</Badge>
              </TableCell>
              <TableCell v-if="schemaType === 'app'" class="max-w-[300px] truncate text-sm text-muted-foreground">
                {{ formatUris(i) }}
              </TableCell>
              <TableCell>
                <Badge
                  :variant="i.state === 'active' ? 'default' : 'destructive'"
                  class="text-xs"
                >{{ i.state }}</Badge>
              </TableCell>
              <TableCell class="text-muted-foreground text-sm">{{ formatTime(i.created_at) }}</TableCell>
            </TableRow>
            <TableRow v-if="!identities.length">
              <TableCell :colspan="schemaType === 'app' ? 6 : 4" class="h-24 text-center text-muted-foreground">
                No {{ label.toLowerCase() }} found
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </CardContent>
    </Card>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { type Identity } from '@/api/resources'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { Badge } from '@/components/ui/badge'
import { Plus } from 'lucide-vue-next'

const props = defineProps<{ schemaType: string }>()

const schemaDisplay = ref<any>({})
const label = computed(() => schemaDisplay.value.alias || props.schemaType.replace(/_/g, ' ').replace(/\b\w/g, (c: string) => c.toUpperCase()) + 's')
const singularLabel = computed(() => schemaDisplay.value.singular || label.value.replace(/s$/, '').replace(/ie$/, 'y'))
const issuer = window.location.origin

const identities = ref<Identity[]>([])

onMounted(async () => {
  let apiPath = props.schemaType
  try {
    const metaRes = await fetch('/v1/schemas/$meta')
    const metaData = await metaRes.json()
    const catalog = metaData['x-catalog'] || {}
    const entry = catalog[props.schemaType]
    if (entry) {
      schemaDisplay.value = { alias: entry.alias, singular: entry.singular, path: entry.path, icon: entry.icon }
      apiPath = entry.path || props.schemaType
    }
  } catch { /* ignore */ }

  try {
    let url = `/v1/${apiPath}`
    const orgId = localStorage.getItem('zitadel_org')
    if (orgId) url += `?org_id=${orgId}`
    const res = await fetch(url)
    const data = await res.json()
    identities.value = data.items || []
  } catch { /* ignore */ }
})

function getField(item: Identity, field: string): string {
  try {
    const d = typeof item.data === 'string' ? JSON.parse(item.data) : (item.data || {})
    return d[field] || ''
  } catch { return '' }
}

function formatUris(item: Identity): string {
  try {
    const d = typeof item.data === 'string' ? JSON.parse(item.data) : (item.data || {})
    const uris = d.redirect_uris || []
    if (uris.length === 0) return '—'
    if (uris.length === 1) return uris[0]
    return `${uris[0]} +${uris.length - 1} more`
  } catch { return '—' }
}

function copy(text: string) { navigator.clipboard.writeText(text) }
function formatTime(ts: string) { return new Date(ts).toLocaleDateString() }
</script>
