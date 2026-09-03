<script setup lang="ts">
import { computed } from "vue";
import { formatTokens } from "@/utils/format";

const props = defineProps<{
  label: string;
  value: number;
  /** 如 "占总用量 78%" */
  hint?: string;
  /** 与上一周期对比的百分比；正=上升，负=下降 */
  deltaPct?: number;
}>();

const formatted = computed(() => formatTokens(props.value));
const deltaText = computed(() => {
  if (props.deltaPct == null) return null;
  const arrow = props.deltaPct >= 0 ? "↑" : "↓";
  return `${arrow} ${Math.abs(props.deltaPct)}%`;
});
</script>

<template>
  <article class="stat">
    <label>{{ label }}</label>
    <strong>{{ formatted }}</strong>
    <small v-if="deltaText"><b>{{ deltaText }}</b> 相比上一周期</small>
    <small v-else-if="hint">{{ hint }}</small>
  </article>
</template>

<style scoped>
.stat {
  padding: 18px 20px;
  background: var(--card);
  border: 1px solid var(--line);
  border-radius: 16px;
  box-shadow: 0 7px 18px rgba(35, 44, 70, 0.025);
}
.stat label {
  color: var(--muted);
  font-size: 13px;
}
.stat strong {
  display: block;
  margin-top: 11px;
  font-size: 27px;
  letter-spacing: -1px;
}
.stat small {
  display: block;
  margin-top: 7px;
  color: #8a909d;
  font-size: 12px;
}
.stat small b {
  color: var(--green);
}
</style>
