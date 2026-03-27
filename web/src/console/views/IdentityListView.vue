<template>
  <div>
    <div class="toolbar">
      <h3>{{ identities.length }} {{ label.toLowerCase() }}</h3>
      <router-link :to="`/s/${schemaType}/new`" class="btn-primary">+ New {{ singularLabel }}</router-link>
    </div>

    <!-- Use ApplicationListView-style for 'app' type -->
    <div v-if="schemaType === 'app'" class="app-discovery">
      <h4>OIDC Discovery</h4>
      <div class="discovery-row">
        <span>Issuer</span>
        <code @click="copy(issuer)">{{ issuer }}</code>
      </div>
      <div class="discovery-row">
        <span>Discovery</span>
        <code @click="copy(issuer + '/.well-known/openid-configuration')">{{ issuer }}/.well-known/openid-configuration</code>
      </div>
    </div>

    <div class="table-wrap">
      <table>
        <thead>
          <tr>
            <th>Identifier</th>
            <th>Display Name</th>
            <th v-if="schemaType === 'app'">Type</th>
            <th v-if="schemaType === 'app'">Redirect URIs</th>
            <th>State</th>
            <th>Created</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="i in identities" :key="i.id" @click="$router.push(`/identities/${i.id}`)" class="clickable">
            <td :class="schemaType === 'app' ? 'client-id' : 'identifier'">{{ i.identifier }}</td>
            <td>{{ getField(i, 'client_name') || getField(i, 'display_name') || i.display_name || '—' }}</td>
            <td v-if="schemaType === 'app'"><span class="app-type-badge">{{ getField(i, 'app_type') || '—' }}</span></td>
            <td v-if="schemaType === 'app'" class="uris">{{ formatUris(i) }}</td>
            <td><span class="badge" :class="i.state">{{ i.state }}</span></td>
            <td class="time">{{ formatTime(i.created_at) }}</td>
          </tr>
        </tbody>
      </table>
      <div v-if="!identities.length" class="empty">No {{ label.toLowerCase() }} found</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { type Identity } from '@/api/resources'

const props = defineProps<{ schemaType: string }>()

const schemaDisplay = ref<any>({})
const label = computed(() => schemaDisplay.value.alias || props.schemaType.replace(/_/g, ' ').replace(/\b\w/g, (c: string) => c.toUpperCase()) + 's')
const singularLabel = computed(() => schemaDisplay.value.singular || label.value.replace(/s$/, '').replace(/ie$/, 'y'))
const issuer = window.location.origin

const identities = ref<Identity[]>([])

onMounted(async () => {
  // Resolve the API path from the schema's x-display metadata
  let apiPath = props.schemaType
  try {
    const schemaRes = await fetch('/v1/schemas')
    const schemaData = await schemaRes.json()
    const match = (schemaData.items || []).find((s: any) => s.type === props.schemaType)
    if (match?.schema?.['x-display']) {
      schemaDisplay.value = match.schema['x-display']
      apiPath = match.schema['x-display'].path || props.schemaType
    }
  } catch { /* ignore */ }

  // Use the alias route: /v1/users, /v1/apps, etc.
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

<style scoped>
.toolbar { display: flex; justify-content: space-between; align-items: center; margin-bottom: 1rem; }
.toolbar h3 { font-size: 0.875rem; font-weight: 500; color: #6b7280; }
.btn-primary {
  padding: 0.5rem 1rem; background: #1a1a2e; color: #fff; border-radius: 8px;
  font-size: 0.8125rem; font-weight: 600; text-decoration: none;
}
.table-wrap { background: #fff; border: 1px solid #e5e7eb; border-radius: 10px; overflow: hidden; }
table { width: 100%; border-collapse: collapse; }
th { text-align: left; padding: 0.75rem 1.25rem; font-size: 0.75rem; font-weight: 600; color: #6b7280; text-transform: uppercase; letter-spacing: 0.05em; border-bottom: 1px solid #e5e7eb; }
td { padding: 0.75rem 1.25rem; font-size: 0.875rem; color: #1a1a2e; border-bottom: 1px solid #f3f4f6; }
.clickable { cursor: pointer; }
.clickable:hover { background: #f9fafb; }
.identifier { font-weight: 500; }
.client-id { font-family: 'SF Mono', 'Fira Code', monospace; font-weight: 500; font-size: 0.8125rem; color: #4f46e5; }
.uris { font-size: 0.8125rem; color: #6b7280; max-width: 300px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.app-type-badge {
  display: inline-block; padding: 0.125rem 0.5rem; border-radius: 4px;
  font-size: 0.75rem; font-weight: 500; background: #eff6ff; color: #2563eb;
  text-transform: uppercase; letter-spacing: 0.03em;
}
.time { color: #9ca3af; font-size: 0.8125rem; }
.badge { display: inline-block; padding: 0.125rem 0.5rem; border-radius: 4px; font-size: 0.75rem; font-weight: 500; }
.badge.active { background: #ecfdf5; color: #059669; }
.badge.deactivated { background: #fef2f2; color: #dc2626; }
.empty { padding: 3rem; text-align: center; color: #9ca3af; }

/* OIDC discovery panel (shown for app type) */
.app-discovery {
  margin-bottom: 1rem; background: #f8fafc; border: 1px solid #e2e8f0; border-radius: 10px; padding: 1rem 1.25rem;
}
.app-discovery h4 { margin: 0 0 0.5rem; font-size: 0.875rem; font-weight: 600; color: #1a1a2e; }
.discovery-row { display: flex; align-items: center; gap: 0.75rem; margin-bottom: 0.25rem; }
.discovery-row span { font-size: 0.8125rem; color: #6b7280; min-width: 80px; }
.discovery-row code { font-size: 0.8125rem; color: #4f46e5; cursor: pointer; padding: 0.125rem 0.375rem; border-radius: 4px; background: #eef2ff; }
.discovery-row code:hover { background: #e0e7ff; }
</style>
