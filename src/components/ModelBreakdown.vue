<script setup lang="ts">
import { computed, ref } from "vue";
import type { ModelUsage } from "@/types/usage";
import { formatTokens, formatUsd } from "@/utils/format";
import { useTheme } from "@/composables/useTheme";
import { VChart } from "@/echarts";
import ToolLogo from "@/components/ToolLogo.vue";

const props = defineProps<{ models: ModelUsage[]; rangeLabel: string }>();
const { resolved } = useTheme();

// 分布指标：Token 用量 / 预估费用。
type Metric = "tokens" | "cost";
const metric = ref<Metric>("tokens");
function metricValue(m: ModelUsage): number {
  return metric.value === "cost" ? m.costUsd : m.total;
}

// 平台配色（与工具分布一致）；同平台下多模型用不同亮度区分。
const PLATFORM_BRAND: Record<string, [string, string]> = {
  ark: ["--brand-ark", "#ef8c4a"],
  codex: ["--brand-codex", "#4b8ef0"],
  claude: ["--brand-claude", "#cf7350"],
  kiro: ["--brand-kiro", "#8b7ff0"],
  qoder: ["--brand-qoder", "#e5697a"],
  "opencode-go": ["--brand-open", "#17b8a6"],
  gemini: ["--brand-gemini", "#4989f5"],
  copilot: ["--brand-copilot", "#6e7681"],
};

function cssVar(name: string, fallback: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
}
function baseColor(platform: string): string {
  const [varName, fallback] = PLATFORM_BRAND[platform] ?? ["--accent", "#5457e6"];
  return cssVar(varName, fallback);
}

/** 十六进制颜色按比例混白，用于同平台多模型的层次区分。 */
function lighten(hex: string, ratio: number): string {
  const m = /^#?([\da-f]{6})$/i.exec(hex.trim());
  if (!m) return hex;
  const n = parseInt(m[1], 16);
  const r = (n >> 16) & 255;
  const g = (n >> 8) & 255;
  const b = n & 255;
  const mix = (c: number) => Math.round(c + (255 - c) * ratio);
  return `#${((1 << 24) + (mix(r) << 16) + (mix(g) << 8) + mix(b)).toString(16).slice(1)}`;
}

/** 为每个模型分配颜色 + 按当前指标算占比；同平台内按序递增亮度。 */
const colored = computed(() => {
  const seenPerPlatform = new Map<string, number>();
  const grand = props.models.reduce((s, m) => s + metricValue(m), 0);
  return props.models
    .map((m) => {
      const idx = seenPerPlatform.get(m.platform) ?? 0;
      seenPerPlatform.set(m.platform, idx + 1);
      const value = metricValue(m);
      return {
        ...m,
        color: lighten(baseColor(m.platform), Math.min(idx * 0.16, 0.6)),
        metricValue: value,
        metricPct: grand > 0 ? Math.round((value / grand) * 1000) / 10 : 0,
      };
    })
    .sort((a, b) => b.metricValue - a.metricValue);
});

const metricTotal = computed(() => props.models.reduce((s, m) => s + metricValue(m), 0));

const option = computed(() => {
  void resolved.value;
  const isCost = metric.value === "cost";
  const surface = cssVar("--surface", "#fff");
  const tooltipBg = cssVar("--tooltip-bg", "#fff");
  const border = cssVar("--border", "#e5e7ec");
  const text = cssVar("--text", "#171b23");
  const data = colored.value
    .filter((m) => m.metricValue > 0)
    .map((m) => ({ name: m.model, value: m.metricValue, itemStyle: { color: m.color } }));
  return {
    tooltip: {
      trigger: "item",
      backgroundColor: tooltipBg,
      borderColor: border,
      borderWidth: 1,
      textStyle: { color: text, fontSize: 12 },
      formatter: (p: { name: string; value: number; percent: number }) =>
        isCost
          ? `${p.name}<br/><b>${formatUsd(p.value)}</b> · ${p.percent}%`
          : `${p.name}<br/><b>${p.value.toLocaleString()}</b> tokens · ${p.percent}%`,
    },
    series: [
      {
        type: "pie",
        radius: ["62%", "88%"],
        center: ["50%", "50%"],
        avoidLabelOverlap: false,
        itemStyle: { borderColor: surface, borderWidth: 2 },
        label: { show: false },
        labelLine: { show: false },
        data,
      },
    ],
  };
});
</script>

<template>
  <article class="modelbreak">
    <div class="mb-head">
      <div>
        <h3>按模型用量</h3>
        <p>{{ rangeLabel }} · 各模型的 {{ metric === "cost" ? "预估费用" : "Token" }} 分布</p>
      </div>
      <div class="mb-toggle" role="group" aria-label="分布指标">
        <button type="button" :class="{ on: metric === 'tokens' }" @click="metric = 'tokens'">Token</button>
        <button type="button" :class="{ on: metric === 'cost' }" @click="metric = 'cost'">费用</button>
      </div>
    </div>

    <div v-if="colored.length" class="mb-body">
      <div class="mb-wrap">
        <VChart v-if="metricTotal > 0" class="mb-donut" :option="option" autoresize />
        <div v-else class="mb-donut-empty">暂无数据</div>
        <div v-if="metricTotal > 0" class="mb-center">
          <span>{{ metric === "cost" ? "预估费用" : "模型数" }}</span>
          <strong>{{ metric === "cost" ? formatUsd(metricTotal) : colored.length }}</strong>
        </div>
      </div>

      <div class="mb-list" role="table">
        <div class="mb-row mb-header" role="row">
          <span>模型</span>
          <span>占比</span>
          <span class="num">Token</span>
          <span class="num">费用</span>
        </div>
        <div v-for="m in colored" :key="m.model" class="mb-row" role="row">
          <span class="mb-name">
            <ToolLogo :platform="m.platform" :size="18" />
            <b class="mb-model" :title="m.model">{{ m.model }}</b>
          </span>
          <span class="mb-share">
            <span class="mb-bar">
              <span :style="{ width: `${m.metricPct}%`, background: m.color }" />
            </span>
            <em>{{ m.metricPct }}%</em>
          </span>
          <span class="num mb-total">{{ formatTokens(m.total) }}</span>
          <span class="num mb-cost" :class="{ est: !m.priced }" :title="m.priced ? '' : '未内置定价，按默认价预估'">
            {{ formatUsd(m.costUsd) }}<i v-if="!m.priced">*</i>
          </span>
        </div>
      </div>
    </div>
    <div v-else class="mb-empty">该周期暂无模型记录</div>
  </article>
</template>

<style scoped>
.modelbreak {
  min-width: 0;
  border: 1px solid var(--glass-border);
  border-radius: 20px;
  background: var(--glass-fill);
  box-shadow: var(--glass-shadow);
  backdrop-filter: blur(18px) saturate(180%);
  -webkit-backdrop-filter: blur(18px) saturate(180%);
}
.mb-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  padding: 21px 26px 5px;
}
.mb-toggle {
  display: inline-flex;
  flex: 0 0 auto;
  padding: 3px;
  border-radius: 999px;
  background: var(--surface-3);
}
.mb-toggle button {
  padding: 5px 14px;
  border: 0;
  border-radius: 999px;
  background: transparent;
  color: var(--text-muted);
  font-family: var(--font-mono);
  font-size: 11.5px;
  font-weight: 600;
  cursor: pointer;
}
.mb-toggle button.on {
  background: var(--surface);
  color: var(--text);
  box-shadow: var(--shadow-card);
}
.mb-cost {
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 13px;
  font-weight: 650;
  font-variant-numeric: tabular-nums;
}
.mb-cost.est {
  color: var(--text-muted);
}
.mb-cost i {
  color: var(--text-subtle);
  font-style: normal;
}
.mb-head h3 {
  margin: 0;
  color: var(--text);
  font-family: "Manrope", "PingFang SC", sans-serif;
  font-size: 16.5px;
  font-weight: 700;
}
.mb-head p {
  margin: 3px 0 0;
  color: var(--text-subtle);
  font-size: 12.5px;
}
.mb-body {
  display: flex;
  align-items: center;
  gap: 22px;
  padding: 12px 26px 20px;
}
.mb-wrap {
  position: relative;
  flex: 0 0 150px;
  width: 150px;
  height: 150px;
}
.mb-donut {
  width: 150px;
  height: 150px;
}
.mb-donut-empty {
  display: grid;
  place-items: center;
  width: 150px;
  height: 150px;
  border-radius: 50%;
  border: 2px dashed var(--border-strong);
  color: var(--text-subtle);
  font-size: 12.5px;
}
.mb-center {
  position: absolute;
  inset: 0;
  display: grid;
  place-content: center;
  text-align: center;
  pointer-events: none;
}
.mb-center span {
  color: var(--text-subtle);
  font-family: var(--font-mono);
  font-size: 10px;
  letter-spacing: 0.6px;
}
.mb-center strong {
  display: block;
  margin-top: 3px;
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 22px;
  font-weight: 650;
  letter-spacing: -0.6px;
}
.mb-list {
  flex: 1;
  min-width: 0;
}
.mb-row {
  display: grid;
  grid-template-columns: minmax(120px, 1.3fr) minmax(120px, 1.2fr) 84px minmax(130px, 1fr);
  align-items: center;
  gap: 14px;
  padding: 9px 0;
  border-top: 1px solid var(--border);
}
.mb-header {
  padding: 0 0 7px;
  border-top: 0;
  color: var(--text-subtle);
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.6px;
}
.mb-row .num {
  text-align: right;
}
.mb-name {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}
.mb-model {
  overflow: hidden;
  color: var(--text);
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12.5px;
  font-weight: 600;
}
.mb-share {
  display: flex;
  align-items: center;
  gap: 10px;
}
.mb-bar {
  flex: 1;
  height: 7px;
  border-radius: 999px;
  background: var(--surface-3);
  overflow: hidden;
}
.mb-bar span {
  display: block;
  height: 100%;
  min-width: 2px;
  border-radius: 999px;
}
.mb-share em {
  flex: 0 0 auto;
  width: 40px;
  color: var(--text-muted);
  font-family: var(--font-mono);
  font-size: 12px;
  font-weight: 600;
  font-style: normal;
  text-align: right;
}
.mb-total {
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 13.5px;
  font-weight: 650;
  font-variant-numeric: tabular-nums;
}
.mb-io {
  color: var(--text-muted);
  font-family: var(--font-mono);
  font-size: 11.5px;
  font-variant-numeric: tabular-nums;
}
.mb-empty {
  display: grid;
  place-items: center;
  padding: 40px 0;
  color: var(--text-subtle);
  font-size: 13px;
}
@media (max-width: 900px) {
  .mb-body {
    flex-direction: column;
    align-items: stretch;
    gap: 14px;
  }
  .mb-wrap {
    align-self: center;
  }
}
@media (max-width: 820px) {
  .mb-row {
    grid-template-columns: 1fr auto;
    gap: 6px 12px;
  }
  .mb-header {
    display: none;
  }
  .mb-share {
    grid-column: 1 / -1;
    order: 3;
  }
  .mb-io {
    grid-column: 1 / -1;
    order: 4;
    text-align: left;
  }
}
</style>
