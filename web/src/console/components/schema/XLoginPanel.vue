<template>
  <div class="space-y-4">
    <div class="space-y-2">
      <Label class="text-xs font-medium text-muted-foreground">Preset</Label>
      <Select v-model="preset" @update:model-value="emit('change')">
        <SelectTrigger class="h-8 text-xs">
          <SelectValue placeholder="Select preset" />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="identifier_first">Identifier first</SelectItem>
          <SelectItem value="passkey_first">Passkey first</SelectItem>
          <SelectItem value="sso_only">SSO only</SelectItem>
          <SelectItem value="custom">Custom</SelectItem>
        </SelectContent>
      </Select>
    </div>

    <Separator />

    <div class="space-y-3">
      <Label class="text-xs font-medium text-muted-foreground">Auth Methods</Label>
      <div v-for="m in methods" :key="m.key" class="flex items-center justify-between">
        <Label :for="'auth-' + m.key" class="text-sm font-normal cursor-pointer">{{ m.label }}</Label>
        <Switch :id="'auth-' + m.key" :checked="m.enabled" @update:checked="val => { m.enabled = val; emit('change') }" />
      </div>
    </div>

    <Separator />

    <div class="space-y-3">
      <div class="flex items-center justify-between">
        <Label for="mfa-required" class="text-sm font-normal cursor-pointer">Require MFA</Label>
        <Switch id="mfa-required" :checked="mfaRequired" @update:checked="val => { mfaRequired = val; emit('change') }" />
      </div>
      <div class="flex items-center justify-between">
        <Label for="registration" class="text-sm font-normal cursor-pointer">Allow registration</Label>
        <Switch id="registration" :checked="registrationAllowed" @update:checked="val => { registrationAllowed = val; emit('change') }" />
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, watch } from 'vue'
import { Switch } from '@/components/ui/switch'
import { Label } from '@/components/ui/label'
import { Separator } from '@/components/ui/separator'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'

const props = defineProps({
  config: { type: Object, required: true },
  authMethods: { type: Object, default: () => ({}) },
})
const emit = defineEmits(['change', 'update:config', 'update:authMethods'])

const preset = ref(props.config?.preset || 'identifier_first')
const mfaRequired = ref(props.config?.mfa_required || false)
const registrationAllowed = ref(props.config?.registration_allowed ?? true)

const methods = ref([
  { key: 'password', label: 'Password', enabled: !!props.authMethods?.password?.enabled },
  { key: 'magic_link', label: 'Magic link', enabled: !!props.authMethods?.magic_link?.enabled },
  { key: 'passkey', label: 'Passkey', enabled: !!props.authMethods?.passkey?.enabled },
  { key: 'sso', label: 'SSO', enabled: !!props.authMethods?.sso?.enabled },
])

watch([preset, mfaRequired, registrationAllowed, methods], () => {
  emit('update:config', {
    preset: preset.value,
    mfa_required: mfaRequired.value,
    registration_allowed: registrationAllowed.value,
  })
}, { deep: true })
</script>
