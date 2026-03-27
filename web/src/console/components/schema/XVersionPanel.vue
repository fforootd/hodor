<template>
  <div class="sidebar-section" v-if="schema">
    <h4 class="sidebar-heading">Schema</h4>
    <div class="field-row">
      <span class="field-label">Type</span>
      <span class="field-value mono">{{ schema.type }}</span>
    </div>
    <div class="field-row">
      <span class="field-label">Version</span>
      <span class="version-badge">v{{ schema.version }}</span>
      <span v-if="schema.is_default" class="default-tag">default</span>
      <span v-else class="draft-tag">draft</span>
    </div>
    <div v-if="schema.message" class="field-row">
      <span class="field-label">Message</span>
      <span class="field-value">{{ schema.message }}</span>
    </div>
    <div v-if="entityCount >= 0" class="field-row">
      <span class="field-label">Entities</span>
      <span class="impact-badge" :class="{ warn: entityCount > 0 }">
        {{ entityCount.toLocaleString() }} {{ entityCount === 1 ? 'entity' : 'entities' }}
      </span>
    </div>
    <!-- Version history -->
    <div v-if="versions.length > 1" class="version-list">
      <h5 class="version-list-title">Version History</h5>
      <div
        v-for="v in versions" :key="v.id"
        class="version-item"
        :class="{ active: v.id === schema.id }"
      >
        <router-link :to="'/schemas/' + v.id" class="version-item-link">
          <span class="version-badge-sm">v{{ v.version }}</span>
          <span v-if="v.is_default" class="default-dot">★</span>
          <span class="version-item-msg">{{ v.message || 'No message' }}</span>
        </router-link>
      </div>
    </div>
    <!-- Promote button -->
    <button
      v-if="!schema.is_default"
      class="btn-promote"
      @click="$emit('promote')"
      :disabled="promoteLoading"
    >
      {{ promoteLoading ? 'Promoting…' : '★ Promote to Default' }}
    </button>
  </div>
</template>

<script setup>
defineProps({
  schema: { type: Object, required: true },
  versions: { type: Array, default: () => [] },
  entityCount: { type: Number, default: -1 },
  promoteLoading: { type: Boolean, default: false },
})
defineEmits(['promote'])
</script>
