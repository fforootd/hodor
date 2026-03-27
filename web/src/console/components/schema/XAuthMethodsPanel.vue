<template>
  <div class="sidebar-section">
    <h4 class="sidebar-heading">Auth Methods</h4>
    <div class="toggle-group">
      <label class="toggle-row" v-for="m in methodsList" :key="m.key">
        <input type="checkbox" v-model="m.enabled" @change="emit('change')" />
        <span>{{ m.label }}</span>
      </label>
    </div>
  </div>
</template>

<script setup>
import { ref } from 'vue'

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

// Only show non-interactive methods (interactive ones go in XLoginPanel)
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
