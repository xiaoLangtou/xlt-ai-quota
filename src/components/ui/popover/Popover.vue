<script setup lang="ts">
import { PopoverContent, PopoverPortal, PopoverRoot, PopoverTrigger } from "reka-ui";

withDefaults(defineProps<{
  open: boolean;
  align?: "start" | "center" | "end";
  sideOffset?: number;
  contentClass?: string;
  triggerClass?: string;
  triggerIconOnly?: boolean;
  triggerTitle?: string;
  triggerAriaLabel?: string;
}>(), {
  align: "center",
  sideOffset: 8,
});

const emit = defineEmits<{ (event: "update:open", value: boolean): void }>();
</script>

<template>
  <PopoverRoot :open="open" @update:open="emit('update:open', $event)">
    <PopoverTrigger
      :class="['cn-popover-trigger', { 'cn-popover-trigger-icon': triggerIconOnly }, triggerClass]"
      :title="triggerTitle"
      :aria-label="triggerAriaLabel"
    >
      <slot name="trigger" />
    </PopoverTrigger>
    <PopoverPortal>
      <PopoverContent
        :class="['cn-popover-content', contentClass]"
        :align="align"
        :side-offset="sideOffset"
        :collision-padding="12"
      >
        <slot />
      </PopoverContent>
    </PopoverPortal>
  </PopoverRoot>
</template>

<style scoped>
.cn-popover-trigger {
  display: inline-flex;
  height: 36px;
  align-items: center;
  justify-content: flex-start;
  gap: 7px;
  padding: 0 10px;
  border: 1px solid transparent;
  border-radius: var(--r-sm);
  background: transparent;
  color: var(--text-muted);
  font: inherit;
  font-size: 13px;
  line-height: 1;
  white-space: nowrap;
  cursor: pointer;
  transition: border-color .16s ease, background-color .16s ease, color .16s ease;
}

.cn-popover-trigger:hover,
.cn-popover-trigger[data-state="open"] {
  border-color: color-mix(in srgb, var(--accent) 28%, transparent);
  background: var(--accent-weak);
  color: var(--accent);
}

.cn-popover-trigger :deep(code) {
  color: var(--text-subtle);
  font-family: var(--font-mono);
  font-size: 11px;
}

.cn-popover-trigger :deep(svg) { flex: none; }
.cn-popover-trigger:not(.cn-popover-trigger-icon) :deep(svg:last-child) { transition: transform .16s ease; }
.cn-popover-trigger:not(.cn-popover-trigger-icon)[data-state="open"] :deep(svg:last-child) { transform: rotate(180deg); }

.cn-popover-trigger-icon {
  width: 36px;
  justify-content: center;
  padding: 0;
}

@media (max-width: 760px) {
  .cn-popover-trigger:not(.cn-popover-trigger-icon) { width: 100%; }
}

@media (prefers-reduced-motion: reduce) {
  .cn-popover-trigger :deep(svg:last-child) { transition: none; }
}
</style>
