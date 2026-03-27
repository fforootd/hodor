<template>
  <div class="sidebar-section">
    <h4 class="sidebar-heading">Fields</h4>
    <div v-for="f in fields" :key="f.name" class="field-item">
      <span class="field-name">{{ f.name }}</span>
      <span v-if="f.isIdentifier" class="mini-badge id">ID</span>
      <span v-if="f.isSensitive" class="mini-badge pii">PII</span>
      <span v-if="f.hasMfa" class="mini-badge mfa">MFA</span>
      <span v-if="f.hasClaimMapping" class="mini-badge claim">⇄</span>
    </div>
    <div v-if="!fields.length" class="field-empty">No fields defined</div>
  </div>
</template>

<script setup>
import { computed } from 'vue'

const props = defineProps({
  schema: { type: Object, required: true },
})

const fields = computed(() => {
  const properties = props.schema?.properties || {}
  return Object.entries(properties)
    .filter(([, v]) => !v['x-hidden'])
    .map(([name, v]) => ({
      name,
      isIdentifier: !!v['x-auth']?.identifier,
      isSensitive: !!v['x-sensitive'],
      hasMfa: !!v['x-auth']?.mfa,
      hasClaimMapping: !!v['x-claim-mapping'],
    }))
})
</script>
