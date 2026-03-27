<template>
  <div class="sidebar-section">
    <h4 class="sidebar-heading">Login Flow</h4>
    <div class="field-row">
      <span class="field-label">Preset</span>
      <select v-model="preset" class="select-input" @change="emit('change')">
        <option value="identifier_first">Identifier first</option>
        <option value="passkey_first">Passkey first</option>
        <option value="sso_only">SSO only</option>
        <option value="custom">Custom</option>
      </select>
    </div>
    <div class="toggle-group">
      <label class="toggle-row" v-for="m in methods" :key="m.key">
        <input type="checkbox" v-model="m.enabled" @change="emit('change')" />
        <span>{{ m.label }}</span>
      </label>
    </div>
    <label class="toggle-row mfa-row">
      <input type="checkbox" v-model="mfaRequired" @change="emit('change')" />
      <span>Require MFA</span>
    </label>
    <label class="toggle-row">
      <input type="checkbox" v-model="registrationAllowed" @change="emit('change')" />
      <span>Allow registration</span>
    </label>
  </div>
</template>

<script setup>
import { ref, watch } from 'vue'

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
