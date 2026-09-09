<script setup lang="ts">
import { computed } from "vue";
import { formatTokens } from "@/utils/format";
import Sparkline from "@/components/Sparkline.vue";

const props = defineProps<{
  label: string;
  value: number;
  /** 如 "占总用量 78%" */
  hint?: string;
  /** 与上一周期对比的百分比；正=上升，负=下降 */
  deltaPct?: number;
  /** 迷你趋势数据 */
  spark?: number[];
}>();

const formatted = computed(() => formatTokens(props.value));
const deltaText = computed(() => {
  if (props.deltaPct == null) return null;
  const arrow = props.deltaPct >= 0 ? "↑" : "↓";
  return `${arrow} ${Math.abs(props.deltaPct)}%`;
});
const deltaUp = computed(() => (props.deltaPct ?? 0) >= 0);
</script>

<template>
  <article class="stat">
    <div class="stat-head">
      <label>{{ label }}</label>
      <span v-if="deltaText" class="delta-pill" :class="deltaUp ? 'up' : 'down'">{{ deltaText }}</span>
    </div>
    <div class="stat-main">
      <strong>{{ formatted }}</strong>
      <div v-if="spark && spark.length > 1" class="stat-spark">
        <Sparkline :values="spark" :color="deltaUp ? 'var(--u-ok)' : 'var(--text-subtle)'" />
      </div>
    </div>
    <small v-if="hint" class="stat-hint">{{ hint }}</small>
  </article>
</template>

<style scoped>
.stat {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 16px 18px;
  background: var(--surface);
  border: 0;
}
.stat-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.stat label {
  color: var(--text-subtle);
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.8px;
}
.delta-pill {
  padding: 2px 7px;
  border-radius: 999px;
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 600;
  white-space: nowrap;
}
.delta-pill.up {
  background: color-mix(in srgb, var(--u-ok) 16%, transparent);
  color: var(--u-ok);
}
.delta-pill.down {
  background: var(--surface-2);
  color: var(--text-muted);
}
.stat-main {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 10px;
}
.stat strong {
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 22px;
  font-weight: 650;
  line-height: 1;
  letter-spacing: -0.6px;
  font-variant-numeric: tabular-nums;
}
.stat-spark {
  flex: 0 0 84px;
  width: 84px;
  height: 30px;
}
.stat-hint {
  color: var(--text-subtle);
  font-size: 12px;
}
</style>
