<script setup lang="ts">
import { computed } from "vue";
import { Check, ChevronDown } from "lucide-vue-next";
import ToolLogo from "@/components/ToolLogo.vue";
import {
  SelectContent,
  SelectItem,
  SelectItemIndicator,
  SelectItemText,
  SelectPortal,
  SelectRoot,
  SelectTrigger,
  SelectValue,
  SelectViewport,
} from "reka-ui";

type Option = { label: string; value: string; logo?: string };
const props = defineProps<{ modelValue: string; options: Option[]; placeholder?: string }>();
const emit = defineEmits<{ (e: "update:modelValue", value: string): void }>();
const selected = computed(() => props.options.find((option) => option.value === props.modelValue));
</script>

<template>
  <SelectRoot :model-value="modelValue" @update:model-value="emit('update:modelValue', $event)">
    <SelectTrigger class="cn-select-trigger">
      <span v-if="selected" class="cn-select-value">
        <ToolLogo v-if="selected.logo" :platform="selected.logo" :size="16" />
        <span>{{ selected.label }}</span>
      </span>
      <SelectValue v-else :placeholder="placeholder" />
      <ChevronDown :size="15" />
    </SelectTrigger>
    <SelectPortal><SelectContent class="cn-select-content" position="popper" side="bottom" align="start" :side-offset="6" :collision-padding="12"><SelectViewport>
      <SelectItem v-for="option in options" :key="option.value" :value="option.value" class="cn-select-item">
        <SelectItemText><span class="cn-select-option"><ToolLogo v-if="option.logo" :platform="option.logo" :size="17" /><span>{{ option.label }}</span></span></SelectItemText>
        <SelectItemIndicator><Check :size="14" /></SelectItemIndicator>
      </SelectItem>
    </SelectViewport></SelectContent></SelectPortal>
  </SelectRoot>
</template>

<style scoped>
.cn-select-value,
.cn-select-option { display: flex; align-items: center; gap: 8px; min-width: 0; }
.cn-select-value > span:last-child,
.cn-select-option > span:last-child { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
