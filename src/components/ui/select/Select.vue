<script setup lang="ts">
import { Check, ChevronDown } from "lucide-vue-next";
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

type Option = { label: string; value: string };
defineProps<{ modelValue: string; options: Option[]; placeholder?: string }>();
const emit = defineEmits<{ (e: "update:modelValue", value: string): void }>();
</script>

<template>
  <SelectRoot :model-value="modelValue" @update:model-value="emit('update:modelValue', $event)">
    <SelectTrigger class="cn-select-trigger"><SelectValue :placeholder="placeholder" /><ChevronDown :size="15" /></SelectTrigger>
    <SelectPortal><SelectContent class="cn-select-content" :side-offset="5"><SelectViewport>
      <SelectItem v-for="option in options" :key="option.value" :value="option.value" class="cn-select-item"><SelectItemText>{{ option.label }}</SelectItemText><SelectItemIndicator><Check :size="14" /></SelectItemIndicator></SelectItem>
    </SelectViewport></SelectContent></SelectPortal>
  </SelectRoot>
</template>
