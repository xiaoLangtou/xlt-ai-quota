<script setup lang="ts">
import { computed } from "vue";
import { formatTokens, formatUsd } from "@/utils/format";
import Sparkline from "@/components/Sparkline.vue";

const props = defineProps<{
  total: number;
  input: number;
  output: number;
  cached?: number;
  cacheWrite?: number;
  reasoning?: number;
  cacheHitRate?: number;
  requests: number;
  costUsd: number;
  deltaPct?: number;
  inputDeltaPct?: number;
  outputDeltaPct?: number;
  requestsDeltaPct?: number;
  costDeltaPct?: number;
  /** 总 Token 的迷你趋势 */
  spark?: number[];
}>();

/** 环比徽标：正=上升，负=下降；无对比数据返回 null。 */
function deltaInfo(value?: number): { text: string; up: boolean } | null {
  if (value == null) return null;
  return { text: `${value >= 0 ? "↑" : "↓"} ${Math.abs(value)}%`, up: value >= 0 };
}

const totalDelta = computed(() => deltaInfo(props.deltaPct));
const costDelta = computed(() => deltaInfo(props.costDeltaPct));
const inputDelta = computed(() => deltaInfo(props.inputDeltaPct));
const outputDelta = computed(() => deltaInfo(props.outputDeltaPct));
const requestsDelta = computed(() => deltaInfo(props.requestsDeltaPct));
const inputPct = computed(() =>
  props.total > 0 ? Math.round((props.input / props.total) * 100) : 0,
);
const outputPct = computed(() =>
  props.total > 0 ? Math.round((props.output / props.total) * 100) : 0,
);
/** 与 juejin 对齐：缓存命中率 = 缓存读 ÷（净输入 + 缓存读 + 缓存写）。 */
const cacheHitPct = computed(() => {
  const cached = props.cached ?? 0;
  const denom = props.input + cached + (props.cacheWrite ?? 0);
  return denom > 0 ? Math.round((cached / denom) * 1000) / 10 : 0;
});
</script>

<template>
  <article class="summary-panel">
    <div class="sp-metric hero">
      <div class="sp-labelrow">
        <span class="sp-label">总 TOKEN</span>
        <span v-if="totalDelta" class="sp-delta" :class="totalDelta.up ? 'up' : 'down'">
          {{ totalDelta.text }}
        </span>
      </div>
      <strong class="sp-value">{{ formatTokens(total) }}</strong>
      <span class="sp-hint">输入 + 输出 + 缓存读/写 + 推理</span>
      <div v-if="spark && spark.length > 1" class="sp-spark">
        <Sparkline
          :values="spark"
          :color="(deltaPct ?? 0) >= 0 ? 'var(--u-warn)' : 'var(--u-ok)'"
        />
      </div>
    </div>

    <div class="sp-metric hero">
      <div class="sp-labelrow">
        <span class="sp-label">预估费用</span>
        <span v-if="costDelta" class="sp-delta" :class="costDelta.up ? 'up' : 'down'">
          {{ costDelta.text }}
        </span>
      </div>
      <strong class="sp-value">{{ formatUsd(costUsd) }}</strong>
      <span class="sp-hint">按模型单价估算</span>
    </div>

    <div class="sp-metric">
      <div class="sp-labelrow">
        <span class="sp-label">输入 TOKEN</span>
        <span v-if="inputDelta" class="sp-delta small" :class="inputDelta.up ? 'up' : 'down'">
          {{ inputDelta.text }}
        </span>
      </div>
      <strong class="sp-value small">{{ formatTokens(input) }}</strong>
      <span class="sp-hint">
        占总量 {{ inputPct }}%<template v-if="(cached ?? 0) > 0"> · 缓存命中 {{ cacheHitPct }}%</template>
      </span>
    </div>

    <div class="sp-metric">
      <div class="sp-labelrow">
        <span class="sp-label">输出 TOKEN</span>
        <span v-if="outputDelta" class="sp-delta small" :class="outputDelta.up ? 'up' : 'down'">
          {{ outputDelta.text }}
        </span>
      </div>
      <strong class="sp-value small">{{ formatTokens(output) }}</strong>
      <span class="sp-hint">占总量 {{ outputPct }}%</span>
    </div>

    <div class="sp-metric">
      <div class="sp-labelrow">
        <span class="sp-label">缓存</span>
      </div>
      <strong class="sp-value small">{{ formatTokens((cached ?? 0) + (cacheWrite ?? 0)) }}</strong>
      <span class="sp-hint">
        读 {{ formatTokens(cached ?? 0) }} · 写 {{ formatTokens(cacheWrite ?? 0) }}
        <template v-if="cacheHitRate != null"> · 命中 {{ cacheHitRate }}%</template>
      </span>
    </div>

    <div class="sp-metric">
      <div class="sp-labelrow">
        <span class="sp-label">请求数</span>
        <span v-if="requestsDelta" class="sp-delta small" :class="requestsDelta.up ? 'up' : 'down'">
          {{ requestsDelta.text }}
        </span>
      </div>
      <strong class="sp-value small">{{ requests.toLocaleString() }}</strong>
    </div>
  </article>
</template>

<style scoped>
.summary-panel {
  display: grid;
  grid-template-columns: repeat(6, minmax(0, 1fr));
  gap: 14px;
}
.sp-metric {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 7px;
  padding: 16px 18px;
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  background: var(--surface);
  box-shadow: var(--shadow-card);
}
.sp-labelrow {
  display: flex;
  min-width: 0;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.sp-label {
  overflow: hidden;
  color: var(--text-subtle);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.7px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.sp-value {
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 30px;
  font-weight: 700;
  line-height: 1.05;
  letter-spacing: -0.8px;
  font-variant-numeric: tabular-nums;
}
.sp-value.small {
  font-size: 18px;
  letter-spacing: -0.3px;
}
.sp-spark {
  width: 100%;
  height: 34px;
  margin-top: 2px;
}
.sp-hint {
  color: var(--text-subtle);
  font-size: 11.5px;
}
.sp-delta {
  flex: 0 0 auto;
  padding: 2px 8px;
  border-radius: 999px;
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 600;
  white-space: nowrap;
}
.sp-delta.small {
  padding: 1px 6px;
  font-size: 10px;
}
.sp-delta.up {
  background: color-mix(in srgb, var(--u-warn) 13%, transparent);
  color: var(--u-warn);
}
.sp-delta.down {
  background: color-mix(in srgb, var(--u-ok) 13%, transparent);
  color: var(--u-ok);
}
/* 小屏：合并为一张卡的两行布局——两个强调指标在上、三个次要指标在下。 */
@media (max-width: 1100px) {
  .summary-panel {
    grid-template-columns: repeat(6, minmax(0, 1fr));
    gap: 0;
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: var(--r-lg);
    background: var(--surface);
    box-shadow: var(--shadow-card);
  }
  .sp-metric {
    border: 0;
    border-radius: 0;
    box-shadow: none;
    border-top: 1px solid var(--border);
    border-left: 1px solid var(--border);
  }
  .sp-metric:nth-child(1) {
    grid-column: span 3;
    border-top: 0;
    border-left: 0;
  }
  .sp-metric:nth-child(2) {
    grid-column: span 3;
    border-top: 0;
  }
  .sp-metric:nth-child(3) {
    grid-column: span 2;
    border-left: 0;
  }
  .sp-metric:nth-child(4) {
    grid-column: span 2;
  }
  .sp-metric:nth-child(5) {
    grid-column: span 2;
  }
}
@media (max-width: 620px) {
  .summary-panel {
    grid-template-columns: 1fr;
  }
  .summary-panel .sp-metric {
    grid-column: auto;
    border-left: 0;
    border-top: 1px solid var(--border);
  }
  .summary-panel .sp-metric:first-child {
    border-top: 0;
  }
  .summary-panel .sp-value {
    font-size: 26px;
  }
}
</style>
