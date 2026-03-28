<template>
  <div class="space-y-3">
    <div class="space-y-1.5">
      <Label for="brand-heading" class="text-xs font-medium text-muted-foreground">Heading</Label>
      <Input id="brand-heading" v-model="heading" class="h-8 text-sm" @input="emit('change')" />
    </div>
    <div class="space-y-1.5">
      <Label class="text-xs font-medium text-muted-foreground">Primary Color</Label>
      <Popover>
        <PopoverTrigger asChild>
          <Button variant="outline" class="h-8 w-full justify-start gap-2 text-xs font-normal">
            <span class="size-4 rounded border" :style="{ background: primary }" />
            <span class="font-mono text-muted-foreground">{{ primary }}</span>
          </Button>
        </PopoverTrigger>
        <PopoverContent class="w-auto p-3" align="start">
          <input type="color" v-model="primary" class="size-32 cursor-pointer border-0 p-0" @input="emit('change')" />
        </PopoverContent>
      </Popover>
    </div>
  </div>
</template>

<script setup>
import { ref } from 'vue'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Button } from '@/components/ui/button'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'

const props = defineProps({
  config: { type: Object, required: true },
})
const emit = defineEmits(['change'])

const heading = ref(props.config?.heading || 'Welcome back')
const primary = ref(props.config?.colors?.primary || '#6366f1')
</script>
