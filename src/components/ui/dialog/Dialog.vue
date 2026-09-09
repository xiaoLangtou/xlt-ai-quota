<script setup lang="ts">
import { DialogContent, DialogOverlay, DialogPortal, DialogRoot } from "reka-ui";

defineProps<{ open: boolean; contentClass?: string; static?: boolean }>();
const emit = defineEmits<{ (e: "update:open", value: boolean): void }>();
</script>

<template>
  <template v-if="static"><slot /></template>
  <DialogRoot v-else :open="open" @update:open="emit('update:open', $event)">
    <DialogPortal>
      <DialogOverlay class="cn-dialog-overlay" />
      <DialogContent :class="['cn-dialog-content', contentClass]">
        <slot />
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>
