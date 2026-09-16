<script setup lang="ts">
import { computed } from "vue";
import type { PlatformDailyPoint } from "@/types/usage";
import { formatTokens, shortDate } from "@/utils/format";
import { useTheme } from "@/composables/useTheme";
import { VChart } from "@/echarts";

const props = defineProps<{ data: PlatformDailyPoint[]; labels?: string[] }>();
const { resolved } = useTheme();

function cssVar(name: string, fallback: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
}

const PLATFORM_VAR: Record<string, [string, string]> = {
  ark: ["--brand-ark", "#ef8c4a"],
  codex: ["--brand-codex", "#4b8ef0"],
  claude: ["--brand-claude", "#cf7350"],
  kiro: ["--brand-kiro", "#8b7ff0"],
  qoder: ["--brand-qoder", "#e5697a"],
  "opencode-go": ["--brand-open", "#17b8a6"],
  gemini: ["--brand-gemini", "#4989f5"],
  copilot: ["--brand-copilot", "#6e7681"],
};
const PLATFORM_LABEL: Record<string, string> = {
  ark: "火山方舟",
  codex: "Codex",
  claude: "Claude Code",
  kiro: "Kiro",
  qoder: "Qoder",
  "opencode-go": "OpenCode Go",
  gemini: "Gemini CLI",
  copilot: "GitHub Copilot",
};

const option = computed(() => {
  // 依赖 resolved，主题切换时重算配色
  void resolved.value;
  const axis = cssVar("--border", "#e5e7ec");
  const label = cssVar("--text-subtle", "#8b93a2");
  const tooltipBg = cssVar("--tooltip-bg", "#ffffff");
  const text = cssVar("--text", "#171b23");

  // 累计每个平台在整个周期内的用量，只渲染有实际用量的平台，避免图例与柱子不一致。
  const totalByPlatform = new Map<string, number>();
  for (const point of props.data) {
    for (const [key, value] of Object.entries(point.byPlatform)) {
      totalByPlatform.set(key, (totalByPlatform.get(key) ?? 0) + value);
    }
  }
  const order = ["ark", "codex", "claude", "kiro", "qoder", "opencode-go", "gemini", "copilot"];
  const series = order
    .filter((p) => (totalByPlatform.get(p) ?? 0) > 0)
    .map((p) => {
      const [varName, fallback] = PLATFORM_VAR[p] ?? ["--accent", "#5457e6"];
      return {
        name: PLATFORM_LABEL[p] ?? p,
        type: "bar" as const,
        stack: "total",
        barWidth: "55%",
        itemStyle: { color: cssVar(varName, fallback), borderRadius: [2, 2, 0, 0] as [number, number, number, number] },
        data: props.data.map((d) => d.byPlatform[p] ?? 0),
      };
    });

  return {
    grid: { left: 0, right: 0, top: 24, bottom: 36, containLabel: false },
    tooltip: {
      trigger: "axis",
      axisPointer: { type: "shadow" },
      backgroundColor: tooltipBg,
      borderColor: axis,
      borderWidth: 1,
      textStyle: { color: text, fontSize: 12 },
      formatter: (params: { name: string; seriesName: string; value: number; marker: string }[]) => {
        const items = params
          .filter((p) => Number(p.value) > 0)
          .map((p) => `${p.marker}${p.seriesName}: <b>${formatTokens(Number(p.value))}</b>`);
        if (items.length === 0) return `${params[0]?.name ?? ""}<br/>无用量`;
        return `${params[0].name}<br/>${items.join("<br/>")}`;
      },
    },
    legend: {
      data: series.map((s) => s.name),
      bottom: 0,
      itemWidth: 10,
      itemHeight: 10,
      textStyle: { color: label, fontSize: 11 },
    },
    xAxis: {
      type: "category",
      data: props.labels ?? props.data.map((d) => shortDate(d.date)),
      axisLine: { lineStyle: { color: axis } },
      axisTick: { show: false },
      axisLabel: { color: label, fontSize: 11 },
    },
    yAxis: { type: "value", show: false },
    series,
  };
});
</script>

<template>
  <article class="chart">
    <div class="chart-heading">
      <div>
        <h3>平台分布</h3>
        <p>按平台汇总的每日 Token 用量</p>
      </div>
      <span class="chart-chip">按平台</span>
    </div>
    <VChart class="graph" :option="option" autoresize />
  </article>
</template>

<style scoped>
.chart {
  min-width: 0;
  overflow: hidden;
  min-height: 278px;
  padding: 19px 20px 14px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  box-shadow: var(--shadow-card);
}
.chart-heading {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}
.chart h3 {
  margin: 0;
  color: var(--text);
  font-size: 15px;
  font-weight: 650;
}
.chart p {
  margin: 6px 0 0;
  color: var(--text-muted);
  font-size: 12px;
}
.chart-chip {
  padding: 4px 8px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--surface-2);
  color: var(--text-muted);
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 600;
}
.graph {
  position: relative;
  width: 100%;
  min-width: 0;
  height: 188px;
  margin-top: 12px;
  border-top: 1px solid var(--border);
}
@media (max-width: 600px) {
  .chart {
    min-height: 260px;
    padding: 18px;
  }
  .graph {
    height: 168px;
  }
}
.chart { border-color: var(--glass-border); border-radius: 20px; background: var(--glass-fill); box-shadow: var(--glass-shadow); backdrop-filter: blur(18px) saturate(180%); }
.chart h3 { font-family: "Manrope", "PingFang SC", sans-serif; font-size: 16.5px; font-weight: 700; }
.chart p { margin-top: 3px; color: var(--text-subtle); font-size: 12.5px; }
.graph { border-color: var(--border); }
</style>
