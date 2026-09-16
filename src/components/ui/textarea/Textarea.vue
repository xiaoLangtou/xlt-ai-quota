<script setup lang="ts">
import { computed } from "vue";
import type { TextareaHTMLAttributes } from "vue";
import { cn } from "@/lib/utils";

defineOptions({ inheritAttrs: false });
const props = defineProps<{
  modelValue?: string;
  class?: TextareaHTMLAttributes["class"];
}>();
const emit = defineEmits<{ (e: "update:modelValue", value: string): void }>();
const value = computed(() => props.modelValue ?? "");
</script>

<template>
  <textarea
    v-bind="$attrs"
    :value="value"
    :class="cn('cn-textarea', props.class)"
    @input="emit('update:modelValue', ($event.target as HTMLTextAreaElement).value)"
  />
</template>
