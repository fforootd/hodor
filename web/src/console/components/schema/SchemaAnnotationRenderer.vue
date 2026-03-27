<template>
  <div class="annotation-panels">
    <!-- Always show version info -->
    <XVersionPanel
      :schema="schemaMeta"
      :versions="versions"
      :entity-count="entityCount"
      :promote-loading="promoteLoading"
      @promote="$emit('promote')"
    />

    <!-- Login flow: only if x-login exists in parsed schema -->
    <XLoginPanel
      v-if="has('x-login')"
      :config="annotation('x-login')"
      :auth-methods="annotation('x-auth-methods')"
      @change="$emit('change')"
    />

    <!-- Non-interactive auth methods: only if x-auth-methods exists AND no x-login -->
    <XAuthMethodsPanel
      v-if="has('x-auth-methods') && !has('x-login')"
      :config="annotation('x-auth-methods')"
      @change="$emit('change')"
    />

    <!-- Branding: only if x-branding exists -->
    <XBrandingPanel
      v-if="has('x-branding')"
      :config="annotation('x-branding')"
      @change="$emit('change')"
    />

    <!-- Claim mapping: only if any field has x-claim-mapping -->
    <XClaimMappingPanel
      v-if="hasClaimMappings"
      :schema="parsedSchema"
    />

    <!-- Fields: always show -->
    <XFieldsPanel :schema="parsedSchema" />

    <!-- Commit message + save -->
    <div class="sidebar-section">
      <div class="commit-msg-row">
        <input
          type="text"
          v-model="commitMsg"
          class="commit-input"
          placeholder="What changed? (optional)"
        />
      </div>
      <button class="btn-save" @click="$emit('save', commitMsg)">
        Save as new version
      </button>
      <div v-if="saveStatus" class="save-status">{{ saveStatus }}</div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed } from 'vue'
import XVersionPanel from './XVersionPanel.vue'
import XLoginPanel from './XLoginPanel.vue'
import XAuthMethodsPanel from './XAuthMethodsPanel.vue'
import XBrandingPanel from './XBrandingPanel.vue'
import XClaimMappingPanel from './XClaimMappingPanel.vue'
import XFieldsPanel from './XFieldsPanel.vue'

const props = defineProps({
  parsedSchema: { type: Object, required: true },
  schemaMeta: { type: Object, required: true },
  versions: { type: Array, default: () => [] },
  entityCount: { type: Number, default: -1 },
  promoteLoading: { type: Boolean, default: false },
  saveStatus: { type: String, default: '' },
})

defineEmits(['promote', 'change', 'save'])

const commitMsg = ref('')

function has(key) {
  return props.parsedSchema && key in props.parsedSchema
}

function annotation(key) {
  return props.parsedSchema?.[key] || {}
}

const hasClaimMappings = computed(() => {
  const properties = props.parsedSchema?.properties || {}
  return Object.values(properties).some(v => v['x-claim-mapping'])
})
</script>
