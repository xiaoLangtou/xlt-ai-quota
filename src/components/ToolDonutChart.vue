<script setup lang="ts">
import { computed } from "vue";
import type { ToolUsage } from "@/types/usage";
import { formatTokens } from "@/utils/format";
import { useTheme } from "@/composables/useTheme";
import { VChart } from "@/echarts";
import ToolLogo from "@/components/ToolLogo.vue";

const props = defineProps<{ tools: ToolUsage[]; title?: string }>();
const { resolved } = useTheme();

const TOOL_LABEL: Record<string, string> = {
  ark: "火山方舟",
  codex: "Codex",
  claude: "Claude Code",
  kiro: "Kiro",
  qoder: "Qoder",
  "opencode-go": "OpenCode Go",
};
const TOOL_BRAND: Record<string, [string, string]> = {
  ark: ["--brand-ark", "#ef8c4a"],
  codex: ["--brand-codex", "#4b8ef0"],
  claude: ["--brand-claude", "#cf7350"],
  kiro: ["--brand-kiro", "#8b7ff0"],
  qoder: ["--brand-qoder", "#e5697a"],
  "opencode-go": ["--brand-open", "#17b8a6"],
};

function cssVar(name: string, fallback: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
}
function label(platform: string): string {
  return TOOL_LABEL[platform] ?? platform;
}
function color(platform: string): string {
  const [varName, fallback] = TOOL_BRAND[platform] ?? ["--accent", "#5457e6"];
  return cssVar(varName, fallback);
}

const totalTokens = computed(() => props.tools.reduce((s, t) => s + t.total, 0));

const legend = computed(() =>
  props.tools.map((t) => ({
    platform: t.platform,
    name: label(t.platform),
    color: color(t.platform),
    total: t.total,
    pct: t.pct,
  })),
);

const option = computed(() => {
  void resolved.value; // 主题切换重算
  const surface = cssVar("--surface", "#fff");
  const tooltipBg = cssVar("--tooltip-bg", "#fff");
  const border = cssVar("--border", "#e5e7ec");
  const text = cssVar("--text", "#171b23");
  const data = props.tools
    .filter((t) => t.total > 0)
    .map((t) => ({ name: label(t.platform), value: t.total, itemStyle: { color: color(t.platform) } }));
  return {
    tooltip: {
      trigger: "item",
      backgroundColor: tooltipBg,
      borderColor: border,
      borderWidth: 1,
      textStyle: { color: text, fontSize: 12 },
      formatter: (p: { name: string; value: number; percent: number }) =>
        `${p.name}<br/><b>${p.value.toLocaleString()}</b> tokens · ${p.percent}%`,
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
  <article class="donut-card">
    <div class="donut-head">
      <h3>{{ title ?? "工具分布" }}</h3>
      <p>按 AI 工具查看 Token 占比</p>
    </div>
    <div class="donut-body">
      <div class="donut-wrap">
        <VChart v-if="totalTokens > 0" class="donut" :option="option" autoresize />
        <div v-else class="donut-empty">暂无数据</div>
        <div v-if="totalTokens > 0" class="donut-center">
          <span>Tokens</span>
          <strong>{{ formatTokens(totalTokens) }}</strong>
        </div>
      </div>
      <ul class="donut-legend">
        <li v-for="item in legend" :key="item.platform">
          <ToolLogo :platform="item.platform" :size="18" />
          <span class="lg-name">{{ item.name }}</span>
          <span class="lg-val">{{ formatTokens(item.total) }}</span>
          <span class="lg-pct">{{ item.pct }}%</span>
        </li>
      </ul>
    </div>
  </article>
</template>

<style scoped>
.donut-card {
  min-width: 0;
  padding: 19px 20px 16px;
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  background: var(--surface);
  box-shadow: var(--shadow-card);
}
.donut-head h3 {
  margin: 0;
  color: var(--text);
  font-size: 15px;
  font-weight: 650;
}
.donut-head p {
  margin: 6px 0 0;
  color: var(--text-muted);
  font-size: 12px;
}
.donut-body {
  display: flex;
  align-items: center;
  gap: 18px;
  margin-top: 14px;
}
.donut-wrap {
  position: relative;
  flex: 0 0 168px;
  width: 168px;
  height: 168px;
}
.donut {
  width: 168px;
  height: 168px;
}
.donut-empty {
  display: grid;
  place-items: center;
  width: 168px;
  height: 168px;
  border-radius: 50%;
  border: 1px dashed var(--border-strong);
  color: var(--text-subtle);
  font-size: 12px;
}
.donut-center {
  position: absolute;
  inset: 0;
  display: grid;
  place-content: center;
  text-align: center;
  pointer-events: none;
}
.donut-center span {
  color: var(--text-subtle);
  font-family: var(--font-mono);
  font-size: 10px;
  letter-spacing: 0.6px;
}
.donut-center strong {
  display: block;
  margin-top: 3px;
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 20px;
  font-weight: 650;
  letter-spacing: -0.6px;
}
.donut-legend {
  flex: 1;
  min-width: 0;
  margin: 0;
  padding: 0;
  list-style: none;
}
.donut-legend li {
  display: grid;
  grid-template-columns: 18px 1fr auto auto;
  align-items: center;
  gap: 8px;
  padding: 6px 0;
  font-size: 12.5px;
}
.lg-name {
  overflow: hidden;
  color: var(--text);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.lg-val {
  color: var(--text-muted);
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
}
.lg-pct {
  min-width: 44px;
  color: var(--text);
  font-family: var(--font-mono);
  font-weight: 600;
  text-align: right;
  font-variant-numeric: tabular-nums;
}
@media (max-width: 520px) {
  .donut-body {
    flex-direction: column;
    align-items: stretch;
  }
  .donut-wrap {
    align-self: center;
  }
}

/* Glass prototype */
.donut-card {
  padding: 0;
  border: 1px solid var(--glass-border);
  border-radius: 20px;
  background: var(--glass-fill);
  box-shadow: var(--glass-shadow);
  backdrop-filter: blur(18px) saturate(180%);
  -webkit-backdrop-filter: blur(18px) saturate(180%);
}
.donut-head { padding: 21px 26px 0; }
.donut-head h3 { font-family: "Manrope", "PingFang SC", sans-serif; font-size: 16.5px; font-weight: 700; }
.donut-head p { margin-top: 3px; color: var(--text-subtle); font-size: 12.5px; }
.donut-body { flex-direction: column; justify-content: center; gap: 8px; margin: 0; padding: 16px 24px 24px; }
.donut-wrap,
.donut,
.donut-empty { width: 132px; height: 132px; flex-basis: 132px; }
.donut-empty { border: 2px dashed var(--border-strong); color: var(--text-subtle); font-size: 12.5px; }
.donut-legend { width: 100%; }
.donut-center strong { font-size: 17px; }
</style>
