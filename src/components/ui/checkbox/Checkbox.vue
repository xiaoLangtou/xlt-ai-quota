<script setup lang="ts">
import type { CheckboxRootEmits, CheckboxRootProps } from "reka-ui";
import type { HTMLAttributes } from "vue";
import { reactiveOmit } from "@vueuse/core";
import { Check } from "lucide-vue-next";
import { CheckboxIndicator, CheckboxRoot, useForwardPropsEmits } from "reka-ui";
import { cn } from "@/lib/utils";

const props = defineProps<CheckboxRootProps & { class?: HTMLAttributes["class"] }>();
const emits = defineEmits<CheckboxRootEmits>();
const forwarded = useForwardPropsEmits(reactiveOmit(props, "class"), emits);
</script>

<template>
  <CheckboxRoot v-bind="forwarded" :class="cn('cn-checkbox', props.class)">
    <CheckboxIndicator class="cn-checkbox-indicator">
      <Check :size="11" />
    </CheckboxIndicator>
  </CheckboxRoot>
</template>
