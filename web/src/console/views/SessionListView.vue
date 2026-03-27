<template>
  <div class="space-y-6">
    <div>
      <h1 class="text-2xl font-bold tracking-tight">Sessions</h1>
      <p class="text-muted-foreground">Active user sessions across your instance.</p>
    </div>

    <Card>
      <CardContent class="p-0">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Identifier</TableHead>
              <TableHead>User Agent</TableHead>
              <TableHead>IP</TableHead>
              <TableHead>Created</TableHead>
              <TableHead>Expires</TableHead>
              <TableHead class="w-24"></TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="s in sessions" :key="s.id">
              <TableCell class="font-medium">{{ s.identifier || `identity-${s.identity_id}` }}</TableCell>
              <TableCell class="text-muted-foreground text-sm">{{ truncate(s.user_agent, 30) }}</TableCell>
              <TableCell class="text-muted-foreground text-sm font-mono">{{ s.ip_address || '—' }}</TableCell>
              <TableCell class="text-muted-foreground text-sm">{{ formatTime(s.created_at) }}</TableCell>
              <TableCell class="text-muted-foreground text-sm">{{ formatTime(s.expires_at) }}</TableCell>
              <TableCell>
                <Button variant="destructive" size="sm" @click="revoke(s.id)">
                  Revoke
                </Button>
              </TableCell>
            </TableRow>
            <TableRow v-if="!sessions.length">
              <TableCell :colspan="6" class="h-24 text-center text-muted-foreground">
                No active sessions
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </CardContent>
    </Card>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { sessionApi, type Session } from '@/api/resources'
import { Card, CardContent } from '@/components/ui/card'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { Button } from '@/components/ui/button'

const sessions = ref<Session[]>([])

onMounted(async () => {
  try { sessions.value = await sessionApi.list() } catch {}
})

async function revoke(id: string) {
  await sessionApi.revoke(id)
  sessions.value = sessions.value.filter(s => s.id !== id)
}

function truncate(s: string, n: number) { return s?.length > n ? s.slice(0, n) + '…' : s || '—' }
function formatTime(ts: string) { return new Date(ts).toLocaleString() }
</script>
