<script setup lang="ts">
import type { TabsContentProps } from "reka-ui";
import type { HTMLAttributes } from "vue";
import { reactiveOmit } from "@vueuse/core";
import { TabsContent } from "reka-ui";
import { cn } from "@/lib/utils";
const props = defineProps<TabsContentProps & { class?: HTMLAttributes["class"] }>();
</script>
<template><TabsContent v-bind="reactiveOmit(props, 'class')" :class="cn('cn-tabs-content', props.class)"><slot /></TabsContent></template>
