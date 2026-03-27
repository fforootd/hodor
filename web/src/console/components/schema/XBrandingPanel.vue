<template>
  <div class="sidebar-section">
    <h4 class="sidebar-heading">Branding</h4>
    <div class="field-row">
      <span class="field-label">Heading</span>
      <input type="text" v-model="heading" class="text-input" @input="emit('change')" />
    </div>
    <div class="field-row">
      <span class="field-label">Primary</span>
      <div class="color-row">
        <input type="color" v-model="primary" class="color-input" @input="emit('change')" />
        <span class="mono" style="font-size:0.75rem">{{ primary }}</span>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref } from 'vue'

const props = defineProps({
  config: { type: Object, required: true },
})
const emit = defineEmits(['change'])

const heading = ref(props.config?.heading || 'Welcome back')
const primary = ref(props.config?.colors?.primary || '#6366f1')
</script>
