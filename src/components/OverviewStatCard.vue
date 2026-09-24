<script setup lang="ts">
import { computed } from "vue";
import Sparkline from "@/components/Sparkline.vue";

/**
 * 概览统计卡：参考改版原型 statCard，落回当前系统的配色令牌。
 * 大数字等宽 tnum，delta 徽标，右下角迷你走势。
 */
const props = withDefaults(
  defineProps<{
    label: string;
    value: string;
    unit?: string;
    hint?: string;
    /** 与上一周期对比；正=上升，负=下降 */
    deltaPct?: number;
    spark?: number[];
    sparkColor?: string;
    /** 数值语气色：默认使用正文色 */
    tone?: "default" | "ok" | "warn" | "crit";
    clickable?: boolean;
  }>(),
  { tone: "default", sparkColor: "var(--u-ok)" },
);

const emit = defineEmits<{ (e: "click"): void }>();

const hasDelta = computed(() => props.deltaPct != null);
const deltaUp = computed(() => (props.deltaPct ?? 0) >= 0);
const deltaText = computed(() => {
  if (props.deltaPct == null) return "";
  const arrow = deltaUp.value ? "↑" : "↓";
  return `${arrow} ${Math.abs(props.deltaPct)}%`;
});
const hasSpark = computed(() => (props.spark?.length ?? 0) > 1);
</script>

<template>
  <article
    class="stat-card"
    :class="{ clickable }"
    @click="clickable && emit('click')"
  >
    <div class="stat-head">
      <span class="stat-label">{{ label }}</span>
      <span v-if="hasDelta" class="delta" :class="deltaUp ? 'up' : 'down'">{{ deltaText }}</span>
    </div>
    <div class="stat-value" :class="`tone-${tone}`">
      {{ value }}<span v-if="unit" class="unit">{{ unit }}</span>
    </div>
    <div v-if="hint" class="stat-sub">{{ hint }}</div>
    <div v-if="hasSpark" class="stat-spark">
      <Sparkline :values="spark ?? []" :color="sparkColor" />
    </div>
  </article>
</template>

<style scoped>
.stat-card {
  position: relative;
  display: flex;
  min-width: 0;
  min-height: 118px;
  flex-direction: column;
  gap: 6px;
  padding: 17px 19px;
  overflow: hidden;
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  background: var(--surface);
  box-shadow: var(--shadow-card);
  transition: transform 0.2s ease, border-color 0.2s ease, box-shadow 0.2s ease;
}

.stat-card.clickable {
  cursor: pointer;
}

.stat-card.clickable:hover {
  transform: translateY(-2px);
  border-color: var(--border-strong);
  box-shadow: var(--shadow-pop);
}

.stat-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.stat-label {
  overflow: hidden;
  color: var(--text-muted);
  font-size: 12.5px;
  font-weight: 500;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.delta {
  flex: 0 0 auto;
  padding: 2px 8px;
  border-radius: 999px;
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 600;
  white-space: nowrap;
}

.delta.down {
  background: color-mix(in srgb, var(--u-ok) 13%, transparent);
  color: var(--u-ok);
}

.delta.up {
  background: color-mix(in srgb, var(--u-warn) 13%, transparent);
  color: var(--u-warn);
}

.stat-value {
  display: flex;
  align-items: baseline;
  margin-top: 2px;
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 30px;
  font-weight: 600;
  line-height: 1.1;
  letter-spacing: -0.02em;
  font-variant-numeric: tabular-nums;
}

.stat-value .unit {
  margin-left: 3px;
  color: var(--text-subtle);
  font-size: 14px;
  font-weight: 500;
}

.stat-value.tone-ok {
  color: var(--u-ok);
}

.stat-value.tone-warn {
  color: var(--u-warn);
}

.stat-value.tone-crit {
  color: var(--u-crit);
}

.stat-sub {
  overflow: hidden;
  color: var(--text-subtle);
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.stat-spark {
  position: absolute;
  right: 14px;
  bottom: 14px;
  width: 84px;
  height: 30px;
  opacity: 0.9;
}
</style>
