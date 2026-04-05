<template>
  <div class="space-y-3">
    <div v-for="m in methodsList" :key="m.key" class="flex items-center justify-between">
      <Label :for="'method-' + m.key" class="text-sm font-normal cursor-pointer">{{ m.label }}</Label>
      <Switch :id="'method-' + m.key" :checked="m.enabled" @update:checked="val => { m.enabled = val; emit('change') }" />
    </div>
    <p v-if="!methodsList.length" class="text-xs text-muted-foreground">No non-interactive auth methods configured</p>
  </div>
</template>

<script setup>
import { ref } from 'vue'
import { Switch } from '@/components/ui/switch'
import { Label } from '@/components/ui/label'

const props = defineProps({
  config: { type: Object, required: true },
})
const emit = defineEmits(['change'])

const labelMap = {
  pat: 'Personal access tokens',
  api_key: 'API keys',
  client_cert: 'Client certificates',
  client_secret: 'Client secret',
  password: 'Password',
  magic_link: 'Magic link',
  passkey: 'Passkey',
  sso: 'SSO',
}

const methodsList = ref(
  Object.entries(props.config || {})
    .filter(([, v]) => !v.interactive)
    .map(([key, val]) => ({
      key,
      label: labelMap[key] || key,
      enabled: val.enabled ?? false,
    }))
)
</script>
