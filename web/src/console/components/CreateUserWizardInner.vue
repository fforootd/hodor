<template>
  <div class="flex flex-col h-[660px] max-h-[85vh]">
    <!-- Header -->
    <div class="px-6 py-4 border-b flex items-center justify-between flex-none">
      <h2 class="text-xl font-semibold tracking-tight">Create User</h2>
      <button
        class="rounded-sm opacity-70 ring-offset-background transition-opacity hover:opacity-100 focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2"
        @click="$emit('close')"
      >
        <X class="h-4 w-4" />
        <span class="sr-only">Close</span>
      </button>
    </div>

    <div class="flex flex-1 overflow-hidden">
      <!-- Left Sidebar: Vertical Stepper -->
      <div class="w-56 bg-muted/30 border-r p-6 flex flex-col shrink-0 overflow-y-auto">
        <div class="relative">
          <!-- Connecting line -->
          <div
            class="absolute left-[11px] top-[14px] w-0.5 bg-border pointer-events-none"
            :style="{ height: `${(steps.length - 1) * 56}px` }"
          />

          <div
            v-for="(step, index) in steps"
            :key="index"
            class="relative flex items-start mb-6 last:mb-0"
          >
            <!-- Step circle -->
            <div
              class="relative z-10 w-6 h-6 rounded-full flex items-center justify-center text-xs font-medium shrink-0 transition-all duration-200"
              :class="stepCircleClass(index)"
            >
              <Check v-if="currentStep > index" class="w-3.5 h-3.5" />
              <ChevronRight v-else-if="currentStep === index" class="w-3.5 h-3.5" />
              <span v-else>{{ index + 1 }}</span>
            </div>

            <!-- Step label -->
            <div class="ml-3 min-w-0">
              <div
                class="text-sm font-medium leading-tight"
                :class="currentStep >= index ? 'text-foreground' : 'text-muted-foreground'"
              >
                {{ step.title }}
              </div>
              <div
                v-if="currentStep === index"
                class="text-xs text-muted-foreground mt-0.5 truncate"
              >
                {{ step.description }}
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Right Content Area -->
      <div class="flex-1 flex flex-col overflow-hidden">
        <div class="flex-1 overflow-y-auto p-6">
          <!-- ═══ STEP: Organization ═══ -->
          <div v-if="currentStep === 0" class="space-y-6 animate-in fade-in slide-in-from-right-4 duration-200">
            <div>
              <h3 class="text-base font-medium mb-1">Select Organization</h3>
              <p class="text-sm text-muted-foreground mb-4">Choose which organization this user will belong to</p>

              <div v-if="loadingOrgs" class="flex items-center text-sm text-muted-foreground py-8">
                <Loader2 class="w-4 h-4 mr-2 animate-spin" />
                Loading organizations...
              </div>

              <div v-else class="space-y-2 max-h-64 overflow-y-auto pr-1">
                <div
                  v-for="org in availableOrgs"
                  :key="org.id"
                  class="rounded-lg border p-3.5 flex items-start gap-3 cursor-pointer transition-colors hover:bg-muted/50"
                  :class="selectedOrgs.includes(String(org.id))
                    ? 'border-primary bg-primary/5'
                    : ''"
                  @click="$emit('toggle-org', String(org.id))"
                >
                  <Checkbox
                    :id="'org-' + org.id"
                    :checked="selectedOrgs.includes(String(org.id))"
                    @update:checked="$emit('toggle-org', String(org.id))"
                    class="mt-0.5"
                  />
                  <div class="min-w-0 flex-1">
                    <label
                      :for="'org-' + org.id"
                      class="text-sm font-medium leading-none cursor-pointer block"
                    >{{ org.name || org.display_name || org.id }}</label>
                    <p class="text-xs text-muted-foreground mt-1">{{ org.id }}</p>
                  </div>
                </div>

                <div
                  v-if="availableOrgs.length === 0"
                  class="text-sm text-muted-foreground italic py-4 text-center"
                >
                  No organizations available.
                </div>
              </div>
            </div>
          </div>

          <!-- ═══ STEP: User Profile (Schema-driven) ═══ -->
          <div v-if="currentStep === 1" class="space-y-5 animate-in fade-in slide-in-from-right-4 duration-200">
            <div>
              <h3 class="text-base font-medium mb-1">User Information</h3>
              <p class="text-sm text-muted-foreground mb-4">Fields are defined by the
                <span class="font-medium text-foreground">{{ selectedSchemaLabel }}</span> schema</p>
            </div>

            <!-- Schema version selector (only if multiple versions) -->
            <div v-if="schemaVersions.length > 1" class="flex flex-wrap gap-2 mb-2">
              <button
                v-for="sv in schemaVersions"
                :key="sv.id"
                type="button"
                class="rounded-md border px-2.5 py-1 text-xs font-medium transition-colors"
                :class="selectedSchemaId === sv.id
                  ? 'border-primary bg-primary/5 text-primary'
                  : 'border-border text-muted-foreground hover:border-primary/40'"
                @click="$emit('select-schema', sv.id)"
              >
                v{{ sv.version }}
                <span v-if="sv.is_default" class="ml-1 text-[10px] opacity-70">(default)</span>
              </button>
            </div>

            <div v-if="loadingSchema" class="flex items-center text-sm text-muted-foreground py-8">
              <Loader2 class="w-4 h-4 mr-2 animate-spin" />
              Loading schema fields...
            </div>

            <div v-else class="space-y-4 max-h-[400px] overflow-y-auto pr-1">
              <div
                v-for="field in schemaFields"
                :key="field.name"
                class="space-y-1.5"
              >
                <Label :for="`field-${field.name}`" class="text-sm font-medium flex items-center gap-1.5">
                  {{ field.label }}
                  <span v-if="field.required" class="text-destructive text-xs">*</span>
                  <span v-if="field.xIdentifier" class="text-[10px] font-normal text-muted-foreground bg-muted px-1.5 py-0.5 rounded">identifier</span>
                  <span v-if="field.xUnique" class="text-[10px] font-normal text-muted-foreground bg-muted px-1.5 py-0.5 rounded">unique/{{ field.xUnique }}</span>
                </Label>

                <!-- Boolean field -->
                <Select
                  v-if="field.type === 'boolean'"
                  :model-value="profileData[field.name] || ''"
                  @update:model-value="$emit('update:profile-field', field.name, String($event ?? ''))"
                >
                  <SelectTrigger><SelectValue placeholder="—" /></SelectTrigger>
                  <SelectContent>
                    <SelectItem value="">—</SelectItem>
                    <SelectItem value="true">true</SelectItem>
                    <SelectItem value="false">false</SelectItem>
                  </SelectContent>
                </Select>

                <!-- Enum field -->
                <Select
                  v-else-if="field.enum"
                  :model-value="profileData[field.name] || ''"
                  @update:model-value="$emit('update:profile-field', field.name, String($event ?? ''))"
                >
                  <SelectTrigger><SelectValue placeholder="—" /></SelectTrigger>
                  <SelectContent>
                    <SelectItem value="">—</SelectItem>
                    <SelectItem v-for="opt in field.enum" :key="opt" :value="opt">{{ opt }}</SelectItem>
                  </SelectContent>
                </Select>

                <!-- Text/number/email/url input -->
                <Input
                  v-else
                  :id="`field-${field.name}`"
                  :model-value="profileData[field.name] || ''"
                  :type="field.inputType"
                  :placeholder="field.description || ''"
                  @update:model-value="$emit('update:profile-field', field.name, String($event ?? ''))"
                />

                <!-- Description text -->
                <p v-if="field.description" class="text-xs text-muted-foreground">{{ field.description }}</p>

                <!-- Validation error -->
                <p
                  v-if="validationErrors[field.name] && profileData[field.name]"
                  class="text-xs text-destructive flex items-center gap-1"
                >
                  <AlertCircle class="w-3 h-3 shrink-0" />
                  {{ validationErrors[field.name] }}
                </p>
              </div>

              <div v-if="schemaFields.length === 0" class="text-sm text-muted-foreground italic py-4 text-center">
                No fields defined in schema. The user will be created with default properties.
              </div>
            </div>
          </div>

          <!-- ═══ STEP: Authentication ═══ -->
          <div
            v-if="currentStep === authStepIndex"
            class="space-y-6 animate-in fade-in slide-in-from-right-4 duration-200"
          >
            <div>
              <h3 class="text-base font-medium mb-1">Authentication Method</h3>
              <p class="text-sm text-muted-foreground mb-4">
                Choose how the user will sign in
                <span class="text-xs">(based on <code class="bg-muted px-1 rounded">x-auth-methods</code>)</span>
              </p>
            </div>

            <RadioGroup :model-value="authMethod" @update:model-value="$emit('update:auth-method', String($event ?? ''))" class="space-y-3">
              <!-- Invite Email (only if magic_link is enabled) -->
              <div
                v-if="authMethodsConfig.magic_link?.enabled"
                class="flex items-center gap-3 rounded-lg border p-4 hover:bg-muted/50 transition-colors cursor-pointer"
                :class="authMethod === 'invite' ? 'border-primary bg-primary/5' : ''"
                @click="$emit('update:auth-method', 'invite')"
              >
                <RadioGroupItem value="invite" id="auth-invite" />
                <div class="flex flex-col flex-1 min-w-0">
                  <div class="flex items-center text-sm font-medium">
                    <Mail class="w-4 h-4 mr-2 text-muted-foreground shrink-0" />
                    Send Invite Email
                  </div>
                  <span class="text-xs text-muted-foreground mt-0.5">User will receive an email to set up their own password</span>
                </div>
              </div>

              <!-- Password (only if password is enabled) -->
              <div
                v-if="authMethodsConfig.password?.enabled"
                class="rounded-lg border transition-colors"
                :class="authMethod === 'password' ? 'border-primary bg-primary/5' : ''"
              >
                <div
                  class="flex items-center gap-3 p-4 hover:bg-muted/50 cursor-pointer"
                  @click="$emit('update:auth-method', 'password')"
                >
                  <RadioGroupItem value="password" id="auth-password" />
                  <div class="flex flex-col flex-1 min-w-0">
                    <div class="flex items-center text-sm font-medium">
                      <Lock class="w-4 h-4 mr-2 text-muted-foreground shrink-0" />
                      Set Password
                    </div>
                    <span class="text-xs text-muted-foreground mt-0.5">Create a password for the user now</span>
                  </div>
                </div>

                <!-- Inline password input -->
                <div v-if="authMethod === 'password'" class="px-4 pb-4 animate-in fade-in slide-in-from-top-2 duration-150">
                  <div class="space-y-1.5 pl-7">
                    <Label for="new-pw" class="text-sm">Initial Password</Label>
                    <Input
                      id="new-pw"
                      type="password"
                      :model-value="initialPassword"
                      placeholder="••••••••"
                      @update:model-value="$emit('update:initial-password', String($event ?? ''))"
                    />
                  </div>
                </div>
              </div>

              <!-- Passwordless (only if passkey is enabled) -->
              <div
                v-if="authMethodsConfig.passkey?.enabled"
                class="flex items-center gap-3 rounded-lg border p-4 hover:bg-muted/50 transition-colors cursor-pointer"
                :class="authMethod === 'passwordless' ? 'border-primary bg-primary/5' : ''"
                @click="$emit('update:auth-method', 'passwordless')"
              >
                <RadioGroupItem value="passwordless" id="auth-passwordless" />
                <div class="flex flex-col flex-1 min-w-0">
                  <div class="flex items-center text-sm font-medium">
                    <Shield class="w-4 h-4 mr-2 text-muted-foreground shrink-0" />
                    Passwordless Only
                  </div>
                  <span class="text-xs text-muted-foreground mt-0.5">User will use passkeys or magic links to sign in</span>
                </div>
              </div>
            </RadioGroup>

            <!-- Welcome message checkbox -->
            <div v-if="authMethod === 'invite'" class="flex items-center gap-2 pt-2 border-t">
              <Checkbox
                id="welcome-msg"
                :checked="sendWelcomeMessage"
                @update:checked="$emit('update:send-welcome-message', $event)"
              />
              <Label for="welcome-msg" class="text-sm cursor-pointer">
                Include welcome message in invitation email
              </Label>
            </div>
          </div>

          <!-- ═══ STEP: Confirmation ═══ -->
          <div
            v-if="currentStep === confirmStepIndex"
            class="space-y-6 animate-in fade-in slide-in-from-right-4 duration-200"
          >
            <div>
              <h3 class="text-base font-medium mb-4">Review User Details</h3>
            </div>

            <div class="rounded-lg border overflow-hidden">
              <!-- Organization row -->
              <div class="grid grid-cols-[1fr_auto] p-3 border-b bg-muted/20 text-sm">
                <span class="text-muted-foreground">Organization</span>
                <span class="font-medium text-right truncate max-w-[200px]" :title="selectedOrgNames">
                  {{ selectedOrgNames }}
                </span>
              </div>

              <!-- Profile fields -->
              <div
                v-for="(val, key) in profileDataOverview"
                :key="key"
                class="grid grid-cols-[1fr_auto] p-3 border-b text-sm"
              >
                <span class="text-muted-foreground capitalize">{{ String(key).replace(/_/g, ' ') }}</span>
                <span class="font-medium text-right max-w-[200px] truncate">{{ val || '—' }}</span>
              </div>

              <!-- Auth method -->
              <div v-if="hasAuthMethods" class="grid grid-cols-[1fr_auto] p-3 text-sm">
                <span class="text-muted-foreground">Authentication</span>
                <span class="font-medium text-right">{{ authMethodLabel }}</span>
              </div>
            </div>

            <!-- Invite notice -->
            <div
              v-if="authMethod === 'invite' && primaryIdentifier"
              class="rounded-lg border border-emerald-200 bg-emerald-50 p-4 text-sm"
            >
              <h4 class="font-semibold mb-1 text-emerald-900">Invitation Email</h4>
              <p class="text-emerald-800">
                An invitation will be sent to <strong>{{ primaryIdentifier }}</strong> with instructions to set up their account.
              </p>
            </div>

            <!-- Error -->
            <div
              v-if="errorMsg"
              class="p-3 bg-destructive/10 text-destructive text-sm rounded-md border border-destructive/20"
            >
              {{ errorMsg }}
            </div>
          </div>
        </div>

        <!-- Bottom Actions -->
        <div class="p-4 border-t bg-muted/20 flex items-center justify-between flex-none">
          <Button
            variant="outline"
            size="sm"
            @click="$emit('prev')"
            :disabled="currentStep === 0 || submitting"
          >
            Back
          </Button>
          <Button
            size="sm"
            @click="$emit('next')"
            :disabled="!canProceed || submitting"
          >
            <Loader2 v-if="submitting" class="w-4 h-4 mr-2 animate-spin" />
            {{ isLastStep ? (submitting ? 'Creating...' : 'Create User') : 'Continue' }}
          </Button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import {
  Check, ChevronRight, X, Mail, Lock, Shield,
  Loader2, AlertCircle
} from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Checkbox } from '@/components/ui/checkbox'
import { RadioGroup, RadioGroupItem } from '@/components/ui/radio-group'

// ---------- Props ----------
const props = defineProps<{
  steps: { title: string; description: string }[]
  currentStep: number
  loadingOrgs: boolean
  loadingSchema: boolean
  submitting: boolean
  errorMsg: string
  availableOrgs: any[]
  selectedOrgs: string[]
  schemaFields: any[]
  schemaVersions: any[]
  selectedSchemaId: string
  profileData: Record<string, string>
  authMethodsConfig: Record<string, any>
  authMethod: string
  initialPassword: string
  sendWelcomeMessage: boolean
  canProceed: boolean
  validationErrors: Record<string, string>
  selectedOrgNames: string
  profileDataOverview: Record<string, string>
  primaryIdentifier: string
  authMethodLabel: string
}>()

defineEmits<{
  (e: 'toggle-org', id: string): void
  (e: 'select-schema', id: string): void
  (e: 'update:profile-field', name: string, value: string): void
  (e: 'update:auth-method', method: string): void
  (e: 'update:initial-password', value: string): void
  (e: 'update:send-welcome-message', value: boolean): void
  (e: 'prev'): void
  (e: 'next'): void
  (e: 'close'): void
}>()

// ---------- Computed ----------
const hasAuthMethods = computed(() => Object.keys(props.authMethodsConfig).length > 0)
const authStepIndex = computed(() => hasAuthMethods.value ? 2 : -1)
const confirmStepIndex = computed(() => hasAuthMethods.value ? 3 : 2)
const isLastStep = computed(() => props.currentStep === props.steps.length - 1)

const selectedSchemaLabel = computed(() => {
  const sv = props.schemaVersions.find((s: any) => s.id === props.selectedSchemaId)
  return sv ? `${sv.type} v${sv.version}` : 'schema'
})

function stepCircleClass(index: number): string {
  if (props.currentStep > index) {
    return 'bg-primary text-primary-foreground border-2 border-primary'
  }
  if (props.currentStep === index) {
    return 'bg-background border-2 border-foreground text-foreground'
  }
  return 'bg-background border-2 border-muted-foreground/30 text-muted-foreground'
}
</script>
