<script setup lang="ts">
import type { TabsListProps } from "reka-ui";
import type { HTMLAttributes } from "vue";
import { reactiveOmit } from "@vueuse/core";
import { TabsList } from "reka-ui";
import { cn } from "@/lib/utils";
const props = defineProps<TabsListProps & { class?: HTMLAttributes["class"] }>();
</script>
<template><TabsList v-bind="reactiveOmit(props, 'class')" :class="cn('cn-tabs-list', props.class)"><slot /></TabsList></template>
