<script setup lang="ts">
import { computed } from "vue";
import { useNow } from "@vueuse/core";
import { HOUR_HEIGHT } from "@/utils/productivity";

// 参考 Nuxt UI calendar 模板的“当前时间”指示线。
const now = useNow({ interval: 60_000 });
const top = computed(() => ((now.value.getHours() * 60 + now.value.getMinutes()) / 60) * HOUR_HEIGHT);
</script>

<template>
  <div class="now-indicator" :style="{ top: `${top}px` }">
    <span class="now-dot" />
  </div>
</template>

<style scoped>
.now-indicator {
  position: absolute;
  inset-inline: 0;
  z-index: 6;
  border-top: 2px solid var(--u-crit);
  pointer-events: none;
}
.now-dot {
  position: absolute;
  top: -5px;
  inset-inline-start: -4px;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--u-crit);
}
</style>
