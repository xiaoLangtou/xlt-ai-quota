<script setup lang="ts">
import { computed } from "vue";
import type { HTMLAttributes } from "vue";
import { cn } from "@/lib/utils";

defineOptions({ inheritAttrs: false });
const props = withDefaults(defineProps<{
  modelValue?: string | number | null;
  type?: string;
  class?: HTMLAttributes["class"];
  modelModifiers?: {
    trim?: boolean;
    number?: boolean;
  };
}>(), { type: "text" });
const emit = defineEmits<{ (e: "update:modelValue", value: string | number): void }>();
const value = computed(() => props.modelValue ?? "");

function update(event: Event): void {
  let next = (event.target as HTMLInputElement).value;
  if (props.modelModifiers?.trim) next = next.trim();
  const isNumber = props.type === "number" || props.modelModifiers?.number;
  emit("update:modelValue", isNumber && next !== "" ? Number(next) : next);
}
</script>

<template><input v-bind="$attrs" :type="type" :value="value" :class="cn('cn-input', props.class)" @input="update"></template>
