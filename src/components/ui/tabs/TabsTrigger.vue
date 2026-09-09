<script setup lang="ts">
import type { TabsTriggerProps } from "reka-ui";
import type { HTMLAttributes } from "vue";
import { reactiveOmit } from "@vueuse/core";
import { TabsTrigger, useForwardProps } from "reka-ui";
import { cn } from "@/lib/utils";
const props = defineProps<TabsTriggerProps & { class?: HTMLAttributes["class"] }>();
const forwarded = useForwardProps(reactiveOmit(props, "class"));
</script>
<template><TabsTrigger v-bind="forwarded" :class="cn('cn-tabs-trigger', props.class)"><slot /></TabsTrigger></template>
