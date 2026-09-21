<script setup lang="ts">
import { computed } from "vue";
import { agentInitial, agentStyle } from "@/utils/skills-agents";

const props = withDefaults(defineProps<{ agent?: string; size?: number }>(), { size: 22 });

const style = computed(() => agentStyle(props.agent));
const initial = computed(() => agentInitial(props.agent));
</script>

<template>
  <span
    class="inline-flex shrink-0 items-center justify-center rounded-full ring-1 ring-inset ring-black/10 dark:ring-white/15 overflow-hidden"
    :style="{
      width: `${size}px`,
      height: `${size}px`,
      background: style.icon ? '#ffffff' : style.color,
    }"
    :title="style.label"
  >
    <img
      v-if="style.icon"
      :src="style.icon"
      :alt="style.label"
      class="object-contain"
      :style="{ width: `${size * 0.72}px`, height: `${size * 0.72}px` }"
    />
    <span
      v-else
      class="font-semibold leading-none text-white"
      :style="{ fontSize: `${size * 0.46}px` }"
      >{{ initial }}</span
    >
  </span>
</template>
