<template>
  <div class="flex h-full flex-col">
    <Accordion type="multiple" :default-value="defaultPanels" class="flex-1 overflow-y-auto">
      <!-- Version info — always first -->
      <AccordionItem value="version" class="border-b">
        <AccordionTrigger class="px-4 py-2.5 text-xs font-semibold uppercase tracking-wider text-muted-foreground hover:no-underline">
          Schema
        </AccordionTrigger>
        <AccordionContent class="px-4 pb-3">
          <XVersionPanel
            :schema="schemaMeta"
            :versions="versions"
            :entity-count="entityCount"
            :promote-loading="promoteLoading"
            @promote="$emit('promote')"
          />
        </AccordionContent>
      </AccordionItem>

      <!-- Login flow: only if x-login exists -->
      <AccordionItem v-if="has('x-login')" value="login" class="border-b">
        <AccordionTrigger class="px-4 py-2.5 text-xs font-semibold uppercase tracking-wider text-muted-foreground hover:no-underline">
          Login Flow
        </AccordionTrigger>
        <AccordionContent class="px-4 pb-3">
          <XLoginPanel
            :config="annotation('x-login')"
            :auth-methods="annotation('x-auth-methods')"
            @change="$emit('change')"
          />
        </AccordionContent>
      </AccordionItem>

      <!-- Non-interactive auth methods: only if x-auth-methods exists AND no x-login -->
      <AccordionItem v-if="has('x-auth-methods') && !has('x-login')" value="auth-methods" class="border-b">
        <AccordionTrigger class="px-4 py-2.5 text-xs font-semibold uppercase tracking-wider text-muted-foreground hover:no-underline">
          Auth Methods
        </AccordionTrigger>
        <AccordionContent class="px-4 pb-3">
          <XAuthMethodsPanel
            :config="annotation('x-auth-methods')"
            @change="$emit('change')"
          />
        </AccordionContent>
      </AccordionItem>

      <!-- Branding: only if x-branding exists -->
      <AccordionItem v-if="has('x-branding')" value="branding" class="border-b">
        <AccordionTrigger class="px-4 py-2.5 text-xs font-semibold uppercase tracking-wider text-muted-foreground hover:no-underline">
          Branding
        </AccordionTrigger>
        <AccordionContent class="px-4 pb-3">
          <XBrandingPanel
            :config="annotation('x-branding')"
            @change="$emit('change')"
          />
        </AccordionContent>
      </AccordionItem>

      <!-- Claim mapping: only if any field has x-claim-mapping -->
      <AccordionItem v-if="hasClaimMappings" value="claims" class="border-b">
        <AccordionTrigger class="px-4 py-2.5 text-xs font-semibold uppercase tracking-wider text-muted-foreground hover:no-underline">
          Claim Mapping
        </AccordionTrigger>
        <AccordionContent class="px-4 pb-3">
          <XClaimMappingPanel :schema="parsedSchema" />
        </AccordionContent>
      </AccordionItem>

      <!-- Fields: always show -->
      <AccordionItem value="fields" class="border-b">
        <AccordionTrigger class="px-4 py-2.5 text-xs font-semibold uppercase tracking-wider text-muted-foreground hover:no-underline">
          Fields
        </AccordionTrigger>
        <AccordionContent class="px-4 pb-3">
          <XFieldsPanel :schema="parsedSchema" />
        </AccordionContent>
      </AccordionItem>
    </Accordion>

    <!-- Sticky footer: commit message + save -->
    <div class="border-t bg-background p-4 space-y-2">
      <Input
        v-model="commitMsg"
        placeholder="What changed? (optional)"
        class="h-8 text-sm"
      />
      <Button class="w-full" size="sm" @click="$emit('save', commitMsg)">
        Save as new version
      </Button>
      <p v-if="saveStatus" class="text-center text-xs" :class="saveStatus.startsWith('✓') ? 'text-emerald-600' : 'text-destructive'">
        {{ saveStatus }}
      </p>
    </div>
  </div>
</template>

<script setup>
import { ref, computed } from 'vue'
import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from '@/components/ui/accordion'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
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

// Open version + fields by default, plus any annotation sections that exist
const defaultPanels = computed(() => {
  const panels = ['version', 'fields']
  if (has('x-login')) panels.push('login')
  if (has('x-branding')) panels.push('branding')
  return panels
})
</script>
