<template>
  <div class="space-y-6 pb-10">
    <div
      v-if="loadError && !identity"
      class="rounded-2xl border border-destructive/50 bg-destructive/10 px-4 py-3 text-sm text-destructive"
    >
      {{ loadError }}
    </div>

    <div
      v-else-if="loading && !identity"
      class="flex h-64 items-center justify-center rounded-3xl border bg-card text-sm text-muted-foreground"
    >
      Loading identity…
    </div>

    <template v-else-if="identity">
      <section class="sticky top-0 z-10 rounded-3xl border bg-background/95 p-6 shadow-sm backdrop-blur">
        <div class="flex flex-col gap-5 xl:flex-row xl:items-start xl:justify-between">
          <div class="flex items-start gap-4">
            <Button variant="ghost" size="icon" as-child class="mt-1 shrink-0">
              <RouterLink :to="backRoute" aria-label="Back to users">
                <ArrowLeft class="size-4" />
              </RouterLink>
            </Button>

            <Avatar class="size-16 rounded-2xl border bg-card shadow-sm">
              <AvatarImage v-if="avatarUrl" :src="avatarUrl" :alt="identityTitle" />
              <AvatarFallback class="rounded-2xl bg-primary text-lg font-semibold text-primary-foreground">
                {{ identityInitials }}
              </AvatarFallback>
            </Avatar>

            <div class="min-w-0 space-y-3">
              <div class="space-y-1">
                <p class="text-xs font-semibold uppercase tracking-[0.24em] text-muted-foreground">
                  Identity cockpit
                </p>
                <h1 class="truncate text-3xl font-semibold tracking-tight">{{ identityTitle }}</h1>
                <p class="truncate text-sm text-muted-foreground">{{ primaryIdentifier || identity.id }}</p>
              </div>

              <div class="flex flex-wrap items-center gap-2">
                <StateBadge :state="identity.state" />
                <Badge variant="outline" class="text-xs">{{ schemaLabel }}</Badge>
                <Badge v-for="(membership, membershipIndex) in headerOrgMemberships" :key="membershipIndex" variant="secondary" class="text-xs">
                  {{ membership.org_name || membership.org_id }}
                </Badge>
                <Badge v-if="remainingOrgCount > 0" variant="secondary" class="text-xs">
                  +{{ remainingOrgCount }} more
                </Badge>
              </div>
            </div>
          </div>

          <div class="flex flex-wrap gap-2 xl:justify-end">
            <Button variant="outline" data-testid="edit-user" @click="openEditSection">
              <Pencil class="mr-2 size-4" />
              Edit
            </Button>
            <Button
              v-if="canSetPassword"
              variant="outline"
              data-testid="set-password"
              @click="showPasswordDialog = true"
            >
              <KeyRound class="mr-2 size-4" />
              Set Password
            </Button>
            <Button
              v-if="canSendInvite"
              variant="outline"
              :disabled="inviting"
              data-testid="send-invite"
              @click="sendInvite"
            >
              <Mail class="mr-2 size-4" />
              {{ inviting ? 'Sending…' : 'Send Invite' }}
            </Button>
            <Button
              variant="destructive"
              :disabled="deleting"
              data-testid="delete-user"
              @click="showDeleteDialog = true"
            >
              <Trash2 class="mr-2 size-4" />
              {{ deleting ? 'Deleting…' : 'Delete' }}
            </Button>
          </div>
        </div>
      </section>

      <Tabs v-model="activeTab" class="space-y-6">
        <TabsList class="grid w-full grid-cols-2 gap-2 rounded-2xl bg-muted/50 p-1 md:grid-cols-4">
          <TabsTrigger value="overview" data-testid="tab-overview">Overview</TabsTrigger>
          <TabsTrigger value="security" data-testid="tab-security">Security & Sessions</TabsTrigger>
          <TabsTrigger value="activity" data-testid="tab-activity">Activity</TabsTrigger>
          <TabsTrigger value="edit" data-testid="tab-edit">Edit & API</TabsTrigger>
        </TabsList>

        <TabsContent value="overview" class="space-y-6">
          <div class="grid gap-6 xl:grid-cols-[1.35fr_minmax(0,0.95fr)]">
            <Card class="rounded-3xl shadow-sm">
              <CardHeader class="pb-3">
                <CardTitle class="text-sm">Overview</CardTitle>
              </CardHeader>
              <CardContent class="space-y-5">
                <section class="space-y-3">
                  <div class="flex items-center justify-between gap-3">
                    <h2 class="text-sm font-semibold">Key facts</h2>
                    <Badge variant="outline" class="text-xs">{{ summaryFacts.length }} facts</Badge>
                  </div>
                  <dl v-if="summaryFacts.length" class="grid gap-4 sm:grid-cols-2">
                    <div v-for="(fact, factIndex) in summaryFacts" :key="factIndex" class="space-y-1 rounded-2xl border bg-muted/20 px-4 py-3">
                      <dt class="text-[11px] font-medium uppercase tracking-wider text-muted-foreground">
                        {{ fact.label }}
                      </dt>
                      <dd class="text-sm font-medium text-foreground">{{ fact.value }}</dd>
                    </div>
                  </dl>
                  <p v-else class="text-sm text-muted-foreground">
                    This identity does not expose schema-specific facts yet.
                  </p>
                </section>

                <Separator />

                <section class="space-y-3">
                  <div class="flex items-center justify-between gap-3">
                    <h2 class="text-sm font-semibold">Access & Security</h2>
                    <Badge variant="secondary" class="text-xs">{{ enabledAuthMethodItems.length }} enabled</Badge>
                  </div>
                  <div v-if="authMethodItems.length" class="flex flex-wrap gap-2">
                    <Badge v-for="(method, methodIndex) in authMethodItems" :key="methodIndex" variant="outline" class="gap-1.5 border-dashed text-xs" :class="authMethodBadgeClass(method)">
                      <span>{{ method.label }}</span>
                      <span class="text-[10px] uppercase tracking-wider opacity-75">
                        {{ method.enabled ? (method.interactive ? 'interactive' : 'service') : 'disabled' }}
                      </span>
                    </Badge>
                  </div>
                  <div v-if="capabilityItems.length" class="flex flex-wrap gap-2">
                    <Badge v-for="(capability, capabilityIndex) in capabilityItems" :key="capabilityIndex" variant="secondary" class="text-xs">
                      {{ formatFieldLabel(capability) }}
                    </Badge>
                  </div>
                  <div class="grid gap-3 sm:grid-cols-2">
                    <div class="rounded-2xl border bg-background/80 px-4 py-3">
                      <p class="text-[11px] uppercase tracking-wider text-muted-foreground">Security posture</p>
                      <p class="mt-1 text-sm font-medium">{{ securitySummary }}</p>
                    </div>
                    <div class="rounded-2xl border bg-background/80 px-4 py-3">
                      <p class="text-[11px] uppercase tracking-wider text-muted-foreground">Organizations</p>
                      <p class="mt-1 text-sm font-medium">{{ organizationSummary }}</p>
                    </div>
                  </div>
                </section>
              </CardContent>
            </Card>

            <div class="space-y-6">
              <Card class="rounded-3xl shadow-sm">
                <CardHeader class="pb-3">
                  <CardTitle class="text-sm">Current State</CardTitle>
                </CardHeader>
                <CardContent>
                  <dl class="space-y-3 text-sm">
                    <div class="flex items-center justify-between gap-4">
                      <dt class="text-muted-foreground">Created</dt>
                      <dd class="text-right font-medium">{{ formatDateTime(identity.created_at) }}</dd>
                    </div>
                    <div class="flex items-center justify-between gap-4">
                      <dt class="text-muted-foreground">Updated</dt>
                      <dd class="text-right font-medium">{{ formatDateTime(identity.updated_at) }}</dd>
                    </div>
                    <div class="flex items-center justify-between gap-4">
                      <dt class="text-muted-foreground">Sessions</dt>
                      <dd class="text-right font-medium">{{ activeSessionCount }} active / {{ sessions.length }} total</dd>
                    </div>
                    <div class="flex items-center justify-between gap-4">
                      <dt class="text-muted-foreground">Last event</dt>
                      <dd class="text-right font-medium">{{ formatDateTime(lastEventAt) }}</dd>
                    </div>
                    <div class="flex items-center justify-between gap-4">
                      <dt class="text-muted-foreground">Last trace</dt>
                      <dd class="text-right font-medium">{{ formatDateTime(lastTraceAt) }}</dd>
                    </div>
                  </dl>
                </CardContent>
              </Card>

              <Card class="rounded-3xl shadow-sm">
                <CardHeader class="pb-3">
                  <div class="flex items-center justify-between gap-3">
                    <CardTitle class="text-sm">Recent activity summary</CardTitle>
                    <Button variant="outline" size="sm" as-child data-testid="view-all-activity">
                      <RouterLink :to="eventsRoute">Open activity</RouterLink>
                    </Button>
                  </div>
                </CardHeader>
                <CardContent class="space-y-3">
                  <div class="rounded-2xl border bg-muted/20 px-4 py-3">
                    <p class="text-[11px] uppercase tracking-wider text-muted-foreground">Last event</p>
                    <p class="mt-1 text-sm font-medium">{{ lastEventSummary }}</p>
                  </div>
                  <div class="rounded-2xl border bg-muted/20 px-4 py-3">
                    <p class="text-[11px] uppercase tracking-wider text-muted-foreground">Trace signal</p>
                    <p class="mt-1 text-sm font-medium">{{ lastTraceSummary }}</p>
                  </div>
                </CardContent>
              </Card>
            </div>
          </div>
        </TabsContent>

        <TabsContent value="security" class="space-y-6">
          <div class="grid gap-6 xl:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)]">
            <Card class="rounded-3xl shadow-sm">
              <CardHeader class="pb-3">
                <div class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                  <div>
                    <CardTitle class="text-lg">Access & Membership</CardTitle>
                    <p class="mt-1 text-sm text-muted-foreground">
                      Login methods, access posture, and organization relationships.
                    </p>
                  </div>
                </div>
              </CardHeader>
              <CardContent class="space-y-5">
                <section class="space-y-3">
                  <div class="flex items-center justify-between gap-3">
                    <h2 class="text-sm font-semibold">Login methods</h2>
                    <Badge variant="outline" class="text-xs">{{ enabledAuthMethodItems.length }} enabled</Badge>
                  </div>
                  <div v-if="authMethodItems.length" class="flex flex-wrap gap-2">
                    <Badge v-for="(method, methodIndex) in authMethodItems" :key="methodIndex" variant="outline" class="text-xs" :class="authMethodBadgeClass(method)">
                      {{ method.label }}
                    </Badge>
                  </div>
                </section>

                <Separator />

                <section class="space-y-3">
                  <div class="flex items-center justify-between gap-3">
                    <h2 class="text-sm font-semibold">Organization memberships</h2>
                    <DropdownMenu v-if="availableOrgs.length">
                      <DropdownMenuTrigger as-child>
                        <Button variant="outline" size="sm">
                          <Plus class="mr-1.5 size-3.5" />
                          Add membership
                        </Button>
                      </DropdownMenuTrigger>
                      <DropdownMenuContent align="end">
                        <DropdownMenuItem v-for="(org, orgIndex) in availableOrgs" :key="orgIndex" @click="addToOrg(org.id)">
                          {{ org.name }}
                        </DropdownMenuItem>
                      </DropdownMenuContent>
                    </DropdownMenu>
                  </div>

                  <div v-if="userOrgs.length" class="space-y-2">
                    <div v-for="(membership, membershipIndex) in userOrgs" :key="membershipIndex" class="flex items-center justify-between gap-4 rounded-2xl border bg-muted/20 px-4 py-3">
                      <div class="min-w-0">
                        <p class="truncate text-sm font-medium">{{ membership.org_name || membership.org_id }}</p>
                        <p class="text-xs text-muted-foreground">
                          {{ formatFieldLabel(membership.role) }} · added {{ formatDate(membership.added_at) }}
                        </p>
                      </div>
                      <Button
                        variant="ghost"
                        size="icon"
                        class="size-8 shrink-0 text-muted-foreground hover:text-destructive"
                        :disabled="removingOrgId === membership.org_id"
                        @click="removeFromOrg(membership.org_id)"
                      >
                        <X class="size-3.5" />
                      </Button>
                    </div>
                  </div>
                  <p v-else class="text-sm text-muted-foreground">This identity is not a member of any organizations.</p>
                </section>
              </CardContent>
            </Card>

            <Card class="rounded-3xl shadow-sm">
              <CardHeader class="pb-3">
                <div class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                  <div>
                    <CardTitle class="text-lg">Sessions</CardTitle>
                    <p class="mt-1 text-sm text-muted-foreground">
                      Current devices first, with deeper details available in the dedicated sessions view.
                    </p>
                  </div>
                  <div class="flex items-center gap-2">
                    <Badge variant="secondary" class="text-xs">{{ activeSessionCount }} active</Badge>
                    <Button variant="outline" size="sm" as-child data-testid="view-all-sessions">
                      <RouterLink :to="sessionsRoute">
                        <Monitor class="mr-1.5 size-3.5" />
                        View all sessions
                      </RouterLink>
                    </Button>
                  </div>
                </div>
              </CardHeader>
              <CardContent class="space-y-3">
                <div v-if="sessionPreview.length" class="space-y-3">
                  <div v-for="(session, sessionIndex) in sessionPreview" :key="sessionIndex" class="rounded-2xl border bg-background/80 px-4 py-3">
                    <div class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                      <div class="min-w-0 space-y-2">
                        <div class="flex flex-wrap items-center gap-2">
                          <Badge variant="outline" class="text-xs">{{ sessionDeviceLabel(session.user_agent) }}</Badge>
                          <Badge variant="outline" class="text-xs">{{ sessionOsLabel(session.user_agent) }}</Badge>
                          <Badge variant="outline" class="text-xs" :class="sessionBadgeClass(session.state)">
                            {{ session.state }}
                          </Badge>
                        </div>
                        <div class="flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground">
                          <span v-if="session.ip_address">IP {{ session.ip_address }}</span>
                          <span>{{ sessionLocationSummary(session) }}</span>
                          <span>Seen {{ formatDateTime(session.created_at) }}</span>
                        </div>
                        <details class="text-xs text-muted-foreground">
                          <summary class="cursor-pointer select-none">Device details</summary>
                          <div class="mt-2 space-y-1">
                            <p>{{ session.user_agent || 'Unknown user agent' }}</p>
                            <p>Expires {{ formatDateTime(session.expires_at) }}</p>
                            <p v-if="session.revoked_at">Revoked {{ formatDateTime(session.revoked_at) }}</p>
                          </div>
                        </details>
                      </div>

                      <div class="flex items-center gap-2 sm:justify-end">
                        <Button variant="ghost" size="sm" as-child>
                          <RouterLink :to="sessionsRoute">Open</RouterLink>
                        </Button>
                        <Button
                          v-if="session.state === 'active'"
                          variant="outline"
                          size="sm"
                          :disabled="revokingSessionId === session.id"
                          :data-testid="`revoke-session-${session.id}`"
                          @click="revokeSession(session.id)"
                        >
                          {{ revokingSessionId === session.id ? 'Revoking…' : 'Revoke' }}
                        </Button>
                      </div>
                    </div>
                  </div>
                </div>
                <p v-else class="text-sm text-muted-foreground">No sessions found for this identity.</p>
              </CardContent>
            </Card>
          </div>
        </TabsContent>

        <TabsContent value="activity" class="space-y-6">
          <div class="grid gap-6 xl:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
            <Card class="rounded-3xl shadow-sm">
              <CardHeader class="pb-3">
                <div class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                  <div>
                    <CardTitle class="text-lg">Events</CardTitle>
                    <p class="mt-1 text-sm text-muted-foreground">
                      Audit trail preview. Use the full events view for forensic work.
                    </p>
                  </div>
                  <Button variant="outline" size="sm" as-child data-testid="view-all-events">
                    <RouterLink :to="eventsRoute">
                      <Activity class="mr-1.5 size-3.5" />
                      Open events
                    </RouterLink>
                  </Button>
                </div>
              </CardHeader>
              <CardContent class="space-y-3">
                <div v-if="recentEvents.length" class="space-y-2">
                  <div v-for="(event, eventIndex) in recentEvents" :key="eventIndex" class="flex items-start justify-between gap-4 rounded-2xl border bg-background/80 px-4 py-3">
                    <div class="min-w-0 space-y-1">
                      <div class="flex flex-wrap items-center gap-2">
                        <Badge variant="outline" class="font-mono text-[11px]">{{ event.event_type }}</Badge>
                        <span class="text-xs text-muted-foreground">{{ formatDateTime(event.created_at) }}</span>
                      </div>
                      <p class="truncate text-sm font-medium">{{ describeEvent(event) }}</p>
                      <p class="truncate text-xs text-muted-foreground">
                        Aggregate {{ event.aggregate_type }} · {{ truncateId(event.aggregate_id) }}
                      </p>
                    </div>
                    <Button variant="ghost" size="sm" as-child :data-testid="`event-link-${event.id}`">
                      <RouterLink :to="eventRoute(event.id)">Open</RouterLink>
                    </Button>
                  </div>
                </div>
                <p v-else class="text-sm text-muted-foreground">No recent events for this identity.</p>
              </CardContent>
            </Card>

            <Card class="rounded-3xl shadow-sm">
              <CardHeader class="pb-3">
                <div class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                  <div>
                    <CardTitle class="text-lg">Traces</CardTitle>
                    <p class="mt-1 text-sm text-muted-foreground">
                      Actor trace preview. Deep diagnostics live in the dedicated traces view.
                    </p>
                  </div>
                  <Button variant="outline" size="sm" as-child data-testid="view-all-traces">
                    <RouterLink :to="tracesRoute">
                      <Route class="mr-1.5 size-3.5" />
                      Open traces
                    </RouterLink>
                  </Button>
                </div>
              </CardHeader>
              <CardContent class="space-y-3">
                <div v-if="recentTraces.length" class="space-y-2">
                  <div v-for="(trace, traceIndex) in recentTraces" :key="traceIndex" class="flex items-start justify-between gap-4 rounded-2xl border bg-background/80 px-4 py-3">
                    <div class="min-w-0 space-y-1">
                      <div class="flex flex-wrap items-center gap-2">
                        <Badge variant="outline" class="text-[11px]">
                          {{ trace.method || 'trace' }}
                        </Badge>
                        <Badge
                          v-if="typeof trace.status === 'number'"
                          variant="outline"
                          class="text-[11px]"
                          :class="trace.status >= 400 ? 'border-red-200 text-red-700' : 'border-emerald-200 text-emerald-700'"
                        >
                          {{ trace.status }}
                        </Badge>
                        <span class="text-xs text-muted-foreground">{{ formatDateTime(trace.started_at) }}</span>
                      </div>
                      <p class="truncate text-sm font-medium">
                        {{ trace.path || trace.request_id || trace.session_id || trace.trace_group }}
                      </p>
                      <div class="flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground">
                        <span>{{ trace.span_count }} events</span>
                        <span v-if="trace.duration">{{ trace.duration }}ms</span>
                        <span v-if="trace.client_id">client {{ truncateId(trace.client_id) }}</span>
                        <span v-if="trace.fingerprint">fingerprint {{ truncateId(trace.fingerprint, 10) }}</span>
                      </div>
                    </div>
                    <Button variant="ghost" size="sm" as-child :data-testid="`trace-link-${trace.trace_group}`">
                      <RouterLink :to="tracesRoute">Open</RouterLink>
                    </Button>
                  </div>
                </div>
                <p v-else class="text-sm text-muted-foreground">No trace groups found for this identity.</p>
              </CardContent>
            </Card>
          </div>
        </TabsContent>

        <TabsContent value="edit" class="space-y-6" data-testid="edit-api-section">
          <Card class="rounded-3xl shadow-sm">
            <CardHeader class="pb-3">
              <CardTitle class="text-lg">Edit & API</CardTitle>
              <p class="text-sm text-muted-foreground">
                Developer tooling is intentionally separated from the operator views. Form editing stays first, raw JSON and cURL remain available when needed.
              </p>
            </CardHeader>
            <CardContent class="space-y-4">
              <SchemaTabsEditor
                v-model="formData"
                :schema="schemaContext.schema"
                :curl-snippets="curlSnippets"
                :form-title="`${schemaLabel} fields`"
                @update:json-valid="(value) => jsonValid = value"
              />

              <div class="flex flex-col gap-3 border-t pt-4 sm:flex-row sm:items-center sm:justify-between">
                <p class="text-sm text-muted-foreground">
                  Save persists the schema-backed user payload. Delete stays in the header because it changes the whole resource state.
                </p>
                <Button
                  :disabled="saving || !jsonValid"
                  data-testid="save-user"
                  @click="save"
                >
                  {{ saving ? 'Saving…' : 'Save changes' }}
                </Button>
              </div>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </template>
  </div>

  <Dialog :open="showPasswordDialog" @update:open="showPasswordDialog = $event">
    <DialogContent class="sm:max-w-md">
      <DialogHeader>
        <DialogTitle>Set Password</DialogTitle>
        <DialogDescription>
          Set a new password for <strong>{{ identityTitle }}</strong>.
        </DialogDescription>
      </DialogHeader>
      <div class="space-y-2 py-2">
        <Input
          v-model="newPassword"
          type="password"
          placeholder="New password"
          autocomplete="new-password"
        />
      </div>
      <DialogFooter class="gap-2">
        <Button variant="outline" @click="showPasswordDialog = false">Cancel</Button>
        <Button
          :disabled="!newPassword.trim() || settingPassword"
          data-testid="confirm-set-password"
          @click="setPassword"
        >
          {{ settingPassword ? 'Setting…' : 'Set Password' }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>

  <Dialog :open="showDeleteDialog" @update:open="showDeleteDialog = $event">
    <DialogContent class="sm:max-w-md">
      <DialogHeader>
        <DialogTitle>Delete {{ schemaLabel }}</DialogTitle>
        <DialogDescription>
          Delete <strong>{{ identityTitle }}</strong>? This removes the identity and cannot be undone.
        </DialogDescription>
      </DialogHeader>
      <DialogFooter class="gap-2">
        <Button variant="outline" @click="showDeleteDialog = false">Cancel</Button>
        <Button
          variant="destructive"
          :disabled="deleting"
          data-testid="confirm-delete-user"
          @click="deleteIdentity"
        >
          {{ deleting ? 'Deleting…' : 'Delete identity' }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import { api } from '@/api/client'
import {
  eventApi,
  magicLinkApi,
  orgApi,
  orgMembersApi,
  sessionApi,
  userApi,
  type Event,
  type Identity,
  type Org,
  type Session,
} from '@/api/resources'
import SchemaTabsEditor from '@/console/components/SchemaTabsEditor.vue'
import { useOrgContext } from '@/console/composables/useOrgContext'
import { formatDate, formatDateTime } from '@/console/utils/format'
import { escapeSqlLiteral } from '@/console/utils/route-filters'
import {
  buildCurlSnippets,
  buildResourceWriteBody,
  extractSchemaFields,
  formatFieldLabel,
  loadResourceSchemaContext,
  normalizeResourceData,
  type ResourceSchemaContext,
} from '@/console/utils/schema-resource'
import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'
import { Input } from '@/components/ui/input'
import { Separator } from '@/components/ui/separator'
import { StateBadge } from '@/components/ui/state-badge'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { notifyError, notifyMutationError, notifyMutationSuccess, notifySuccess } from '@/lib/notify'
import {
  Activity,
  ArrowLeft,
  Code2,
  KeyRound,
  Mail,
  Monitor,
  Pencil,
  Plus,
  Route,
  Trash2,
  X,
} from 'lucide-vue-next'

interface OrgMembership {
  org_id: string
  org_name: string
  role: string
  added_at: string
}

interface DisplayFact {
  label: string
  value: string
}

interface AuthMethodDisplay {
  enabled: boolean
  interactive: boolean
  label: string
  meta?: string
  name: string
  position: number
}

type SessionPreview = Session & { state: string } & Record<string, any>

interface TracePreview {
  trace_group: string
  request_id: string
  session_id: string
  started_at: string
  span_count: number
  method?: string
  path?: string
  status?: number
  duration?: number
  client_id?: string
  fingerprint?: string
}

const route = useRoute()
const router = useRouter()
const { currentOrgId } = useOrgContext()

const identity = ref<Identity | null>(null)
const formData = ref<Record<string, any>>({})
const schemaContext = ref<ResourceSchemaContext>({
  display: {},
  schema: null,
  schemaId: '',
  schemaType: '',
  versions: [],
})
const allOrgs = ref<Org[]>([])
const userOrgs = ref<OrgMembership[]>([])
const sessions = ref<SessionPreview[]>([])
const recentEvents = ref<Event[]>([])
const recentTraces = ref<TracePreview[]>([])
const activeTab = ref('overview')
const removingOrgId = ref('')
const revokingSessionId = ref('')
const jsonValid = ref(true)
const saving = ref(false)
const deleting = ref(false)
const loading = ref(false)
const inviting = ref(false)
const loadError = ref('')
const showPasswordDialog = ref(false)
const showDeleteDialog = ref(false)
const newPassword = ref('')
const settingPassword = ref(false)

const identityId = computed(() => String(route.params.id || ''))
const routeSchemaType = computed(() => String(route.params.schemaType || ''))
const schemaLabel = computed(() =>
  String(
    schemaContext.value.display.singular
    || formatFieldLabel((identity.value?.schema_type || routeSchemaType.value || 'user').replace(/_/g, ' ')),
  ),
)
const identityTitle = computed(() =>
  String(identity.value?.display_name || formData.value.display_name || identity.value?.identifier || schemaLabel.value),
)
const primaryIdentifier = computed(() =>
  String(
    identity.value?.identifier
    || formData.value.email
    || formData.value.username
    || formData.value.phone
    || '',
  ),
)
const avatarUrl = computed(() =>
  String(
    formData.value.avatar_url
    || (identity.value as any)?.profile?.avatar_url
    || (identity.value as any)?.profile?.picture
    || '',
  ),
)
const backRoute = computed(() => routeSchemaType.value ? `/s/${routeSchemaType.value}` : '/users')
const payload = computed(() =>
  buildResourceWriteBody('user', schemaContext.value.schemaId, normalizeResourceData(formData.value)),
)
const curlSnippets = computed(() => buildCurlSnippets({
  path: `/v1/users/${encodeURIComponent(identityId.value)}`,
  body: payload.value,
  includeOrgHeader: true,
  orgId: currentOrgId.value,
  methods: ['GET', 'PATCH'],
}))
const sessionsRoute = computed(() => ({ path: '/sessions', query: { user_id: identityId.value } }))
const eventsRoute = computed(() => ({ path: '/events', query: { aggregate_id: identityId.value } }))
const tracesRoute = computed(() => ({ path: '/traces', query: { actor_id: identityId.value } }))
const authMethods = computed<Record<string, { enabled?: boolean; interactive?: boolean; position?: number; max_tokens?: number }>>(
  () => (schemaContext.value.schema?.['x-auth-methods'] as any) || {},
)
const authMethodItems = computed<AuthMethodDisplay[]>(() =>
  Object.entries(authMethods.value)
    .map(([name, config]) => ({
      enabled: config?.enabled !== false,
      interactive: config?.interactive !== false,
      label: authMethodLabel(name),
      meta: typeof config?.max_tokens === 'number' ? `${config.max_tokens} max` : undefined,
      name,
      position: typeof config?.position === 'number' ? config.position : Number.MAX_SAFE_INTEGER,
    }))
    .sort((left, right) => left.position - right.position || left.label.localeCompare(right.label)),
)
const enabledAuthMethodItems = computed(() => authMethodItems.value.filter((method) => method.enabled))
const capabilityItems = computed(() =>
  Array.isArray(identity.value?.capabilities)
    ? (identity.value?.capabilities || []).map((capability) => String(capability))
    : [],
)
const canSetPassword = computed(() => Boolean(authMethods.value.password?.enabled))
const canSendInvite = computed(() =>
  isEmailLike(primaryIdentifier.value)
  && enabledAuthMethodItems.value.some((method) => method.interactive),
)
const availableOrgs = computed(() => {
  const memberOrgIds = new Set(userOrgs.value.map((membership) => membership.org_id))
  return allOrgs.value.filter((org) => !memberOrgIds.has(org.id))
})
const headerOrgMemberships = computed(() => userOrgs.value.slice(0, 3))
const remainingOrgCount = computed(() => Math.max(userOrgs.value.length - headerOrgMemberships.value.length, 0))
const identityInitials = computed(() => initials(identityTitle.value))
const summaryFacts = computed(() => collectSummaryFacts(formData.value, schemaContext.value.schema).slice(0, 6))
const sessionPreview = computed(() => sessions.value.slice(0, 5))
const activeSessionCount = computed(() => sessions.value.filter((session) => session.state === 'active').length)
const lastEventAt = computed(() => recentEvents.value[0]?.created_at || '')
const lastTraceAt = computed(() => recentTraces.value[0]?.started_at || '')
const lastEventSummary = computed(() =>
  recentEvents.value[0]
    ? `${recentEvents.value[0].event_type} · ${formatDateTime(recentEvents.value[0].created_at)}`
    : 'No recent events'
)
const lastTraceSummary = computed(() =>
  recentTraces.value[0]
    ? `${recentTraces.value[0].method || 'trace'} ${recentTraces.value[0].path || truncateId(recentTraces.value[0].trace_group)}`
    : 'No recent traces'
)
const securitySummary = computed(() => {
  if (!enabledAuthMethodItems.value.length) return 'No enabled authentication methods'
  return enabledAuthMethodItems.value.map((method) => method.label).join(', ')
})
const organizationSummary = computed(() => {
  if (!userOrgs.value.length) return 'No organization memberships'
  if (userOrgs.value.length === 1) return userOrgs.value[0].org_name || userOrgs.value[0].org_id
  return `${userOrgs.value[0].org_name || userOrgs.value[0].org_id} +${userOrgs.value.length - 1} more`
})

async function loadIdentity() {
  if (!identityId.value) return
  loading.value = true
  loadError.value = ''

  try {
    const loaded = await userApi.get(identityId.value)
    identity.value = loaded
    formData.value = normalizeResourceData(loaded.data || {})
    userOrgs.value = ((loaded.orgs as OrgMembership[] | undefined) || []).map((membership) => ({ ...membership }))
    sessions.value = []
    recentEvents.value = []
    recentTraces.value = []

    const [schemaResult, orgsResult, sessionsResult, eventsResult, tracesResult] = await Promise.allSettled([
      loadResourceSchemaContext(loaded.schema_type || routeSchemaType.value || 'human_user', loaded.schema_id || ''),
      orgApi.list(),
      sessionApi.list({ user_id: loaded.id }),
      eventApi.list({ aggregate_id: loaded.id, limit: 8 }),
      loadTracePreview(loaded.id),
    ])

    if (schemaResult.status === 'fulfilled') {
      schemaContext.value = schemaResult.value
    }
    if (orgsResult.status === 'fulfilled') {
      allOrgs.value = orgsResult.value
    }
    if (sessionsResult.status === 'fulfilled') {
      sessions.value = normalizeSessions(sessionsResult.value)
    }
    if (eventsResult.status === 'fulfilled') {
      recentEvents.value = [...eventsResult.value].sort(sortByNewest).slice(0, 8)
    }
    if (tracesResult.status === 'fulfilled') {
      recentTraces.value = tracesResult.value
    }
  } catch (err: any) {
    loadError.value = err?.message || 'Failed to load identity'
  } finally {
    loading.value = false
  }
}

async function refreshIdentity() {
  if (!identity.value) return
  const loaded = await userApi.get(identity.value.id)
  identity.value = loaded
  formData.value = normalizeResourceData(loaded.data || {})
  userOrgs.value = ((loaded.orgs as OrgMembership[] | undefined) || []).map((membership) => ({ ...membership }))
}

async function loadTracePreview(userId: string): Promise<TracePreview[]> {
  const cutoff = new Date(Date.now() - 14 * 24 * 60 * 60 * 1000).toISOString()
  const sql = `
    SELECT
      COALESCE(NULLIF(request_id, ''), NULLIF(session_id, ''), id) AS trace_group,
      MAX(request_id) AS request_id,
      MAX(session_id) AS session_id,
      MIN(created_at) AS started_at,
      COUNT(*) AS span_count,
      MAX(NULLIF(client_id, '')) AS client_id,
      MAX(NULLIF(fingerprint, '')) AS fingerprint,
      MAX(payload) AS sample_payload
    FROM events
    WHERE created_at >= '${cutoff}'
      AND actor_id = '${escapeSqlLiteral(userId)}'
    GROUP BY COALESCE(NULLIF(request_id, ''), NULLIF(session_id, ''), id)
    ORDER BY started_at DESC
    LIMIT 5
  `

  const data = await api.post<any>('/v1/analytics/query', { sql, limit: 5 })
  if (data?.error || !Array.isArray(data?.rows) || !Array.isArray(data?.columns)) {
    return []
  }

  const columns = data.columns.map((column: unknown) => String(column).toLowerCase())
  return data.rows.map((row: any[]) => {
    const record: Record<string, any> = {}
    columns.forEach((column: string, index: number) => {
      record[column] = row[index]
    })

    let payload: Record<string, any> = {}
    try {
      payload = JSON.parse(record.sample_payload || '{}')
    } catch {
      payload = {}
    }

    return {
      trace_group: record.trace_group || '',
      request_id: record.request_id || '',
      session_id: record.session_id || '',
      started_at: record.started_at || '',
      span_count: Number(record.span_count || 0),
      method: payload.method,
      path: payload.path,
      status: typeof payload.status === 'number' ? payload.status : undefined,
      duration: typeof payload.duration_ms === 'number' ? payload.duration_ms : undefined,
      client_id: record.client_id || '',
      fingerprint: record.fingerprint || '',
    } satisfies TracePreview
  })
}

async function save() {
  if (!identity.value) return
  saving.value = true
  try {
    identity.value = await userApi.update(identity.value.id, payload.value)
    formData.value = normalizeResourceData(identity.value.data || {})
    userOrgs.value = ((identity.value.orgs as OrgMembership[] | undefined) || []).map((membership) => ({ ...membership }))
    notifyMutationSuccess(schemaLabel.value, 'update')
  } catch (err: any) {
    notifyMutationError(schemaLabel.value, 'update', err)
  } finally {
    saving.value = false
  }
}

async function setPassword() {
  if (!identity.value || !newPassword.value.trim()) return
  settingPassword.value = true
  try {
    await userApi.setPassword(identity.value.id, newPassword.value)
    notifySuccess('Password updated')
    showPasswordDialog.value = false
    newPassword.value = ''
  } catch (err: any) {
    notifyError('Failed to set password', err)
  } finally {
    settingPassword.value = false
  }
}

async function sendInvite() {
  if (!identity.value) return
  inviting.value = true
  try {
    await magicLinkApi.send(primaryIdentifier.value)
    notifySuccess('Invite sent')
  } catch (err: any) {
    notifyError('Failed to send invite', err)
  } finally {
    inviting.value = false
  }
}

async function addToOrg(orgId: string) {
  if (!identity.value) return
  try {
    await orgMembersApi.add(orgId, identity.value.id)
    await refreshIdentity()
    notifyMutationSuccess('Organization membership', 'add')
  } catch (err: any) {
    notifyMutationError('Organization membership', 'add', err)
  }
}

async function removeFromOrg(orgId: string) {
  if (!identity.value) return
  removingOrgId.value = orgId
  try {
    await orgMembersApi.remove(orgId, identity.value.id)
    await refreshIdentity()
    notifyMutationSuccess('Organization membership', 'remove')
  } catch (err: any) {
    notifyMutationError('Organization membership', 'remove', err)
  } finally {
    removingOrgId.value = ''
  }
}

async function revokeSession(sessionId: string) {
  revokingSessionId.value = sessionId
  try {
    await sessionApi.revoke(sessionId)
    sessions.value = sessions.value.map((session) => (
      session.id === sessionId
        ? { ...session, state: 'revoked', revoked_at: new Date().toISOString() }
        : session
    ))
    notifySuccess('Session revoked')
  } catch (err: any) {
    notifyError('Failed to revoke session', err)
  } finally {
    revokingSessionId.value = ''
  }
}

async function deleteIdentity() {
  if (!identity.value) return
  deleting.value = true
  try {
    await userApi.delete(identity.value.id)
    notifyMutationSuccess(schemaLabel.value, 'delete')
    showDeleteDialog.value = false
    router.push(backRoute.value)
  } catch (err: any) {
    notifyMutationError(schemaLabel.value, 'delete', err)
  } finally {
    deleting.value = false
  }
}

async function openEditSection() {
  activeTab.value = 'edit'
  await nextTick()
}

function eventRoute(eventId: string) {
  return {
    path: '/events',
    query: { aggregate_id: identityId.value, id: eventId },
  }
}

function describeEvent(event: Event): string {
  if (event.request_id) {
    return `Request ${truncateId(event.request_id)}`
  }
  if (event.session_id) {
    return `Session ${truncateId(event.session_id)}`
  }
  if (event.actor_id && event.actor_id !== identityId.value) {
    return `Actor ${truncateId(event.actor_id)}`
  }
  return 'Activity on this identity'
}

function normalizeSessions(items: Session[]): SessionPreview[] {
  return [...items]
    .map((session) => ({
      ...session,
      state: sessionState(session),
    }))
    .sort((left, right) => (
      new Date((right as any).created_at || 0).getTime() - new Date((left as any).created_at || 0).getTime()
    ))
}

function sessionState(session: Session): string {
  if ((session as any).revoked_at) return 'revoked'
  if ((session as any).expires_at && new Date((session as any).expires_at) < new Date()) return 'expired'
  return 'active'
}

function sessionBadgeClass(state: string): string {
  if (state === 'active') return 'border-emerald-200 text-emerald-700'
  if (state === 'revoked') return 'border-red-200 text-red-700'
  return 'border-amber-200 text-amber-700'
}

function sessionDeviceLabel(userAgent?: string): string {
  const value = String(userAgent || '').toLowerCase()
  if (!value) return 'Unknown'
  if (value.includes('chrome')) return 'Chrome'
  if (value.includes('safari')) return 'Safari'
  if (value.includes('firefox')) return 'Firefox'
  if (value.includes('edge')) return 'Edge'
  return 'Browser'
}

function sessionOsLabel(userAgent?: string): string {
  const value = String(userAgent || '').toLowerCase()
  if (!value) return 'Unknown OS'
  if (value.includes('mac os') || value.includes('macos')) return 'macOS'
  if (value.includes('windows')) return 'Windows'
  if (value.includes('linux')) return 'Linux'
  if (value.includes('ios')) return 'iOS'
  if (value.includes('android')) return 'Android'
  return 'Unknown OS'
}

function sessionLocationSummary(session: SessionPreview): string {
  const geo = (session as any).geo
  if (geo?.city || geo?.country) {
    return [geo.city, geo.country].filter(Boolean).join(', ')
  }
  return 'Location unavailable'
}

function authMethodBadgeClass(method: AuthMethodDisplay): string {
  if (!method.enabled) return 'border-muted-foreground/20 text-muted-foreground'
  return method.interactive
    ? 'border-emerald-200 text-emerald-700'
    : 'border-blue-200 text-blue-700'
}

function authMethodLabel(name: string): string {
  const labels: Record<string, string> = {
    api_key: 'API Key',
    magic_link: 'Magic Link',
    pat: 'PAT',
    sso: 'SSO',
  }

  return labels[name] || formatFieldLabel(name)
}

function collectSummaryFacts(
  data: Record<string, any>,
  schema: Record<string, any> | null,
): DisplayFact[] {
  const fields = extractSchemaFields(schema)
  const facts: DisplayFact[] = []

  const visitField = (field: ReturnType<typeof extractSchemaFields>[number]) => {
    if (facts.length >= 6 || field.hidden || field.sensitive) return
    if (['avatar_url', 'display_name', 'metadata'].includes(field.name)) return

    const value = getValueAtPath(data, field.path)
    if (value == null || value === '') {
      if (field.properties?.length) field.properties.forEach(visitField)
      return
    }

    if (field.type === 'object') {
      field.properties?.forEach(visitField)
      return
    }

    const displayValue = formatFactValue(value)
    if (!displayValue) return
    facts.push({ label: field.label, value: displayValue })
  }

  fields.forEach(visitField)
  return facts
}

function formatFactValue(value: unknown): string {
  if (value == null || value === '') return ''
  if (typeof value === 'boolean') return value ? 'Yes' : 'No'
  if (Array.isArray(value)) {
    const items = value
      .map((item) => (typeof item === 'object' ? '' : String(item)))
      .filter(Boolean)
    return items.join(', ')
  }
  if (typeof value === 'object') return ''
  return String(value)
}

function getValueAtPath(source: Record<string, any>, path: string): unknown {
  return path.split('.').reduce<unknown>((current, segment) => {
    if (!current || typeof current !== 'object') return undefined
    return (current as Record<string, any>)[segment]
  }, source)
}

function initials(value: string): string {
  return value
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part.charAt(0).toUpperCase())
    .join('') || 'ID'
}

function truncateId(value?: string, max = 14): string {
  if (!value) return '—'
  if (value.length <= max) return value
  return `${value.slice(0, max)}…`
}

function isEmailLike(value: string): boolean {
  return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value)
}

function sortByNewest<T extends { created_at?: string }>(left: T, right: T): number {
  return new Date(right.created_at || 0).getTime() - new Date(left.created_at || 0).getTime()
}

onMounted(loadIdentity)
watch(identityId, loadIdentity)
</script>
