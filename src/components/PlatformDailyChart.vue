<script setup lang="ts">
import { computed } from "vue";
import type { PlatformDailyPoint } from "@/types/usage";
import { shortDate } from "@/utils/format";
import { VChart } from "@/echarts";

const props = defineProps<{ data: PlatformDailyPoint[] }>();

const PLATFORM_COLORS: Record<string, string> = {
  ark: "#1683f5",
  codex: "#f3a12a",
  claude: "#c96f49",
  kiro: "#4bc192",
  "opencode-go": "#1ca781",
  openai: "#6e55c3",
};
const PLATFORM_LABEL: Record<string, string> = {
  ark: "火山方舟",
  codex: "Codex",
  claude: "Claude Code",
  kiro: "Kiro",
  "opencode-go": "OpenCode Go",
  openai: "OpenAI",
};

const option = computed(() => {
  const platforms = new Set<string>();
  for (const p of props.data) {
    for (const k of Object.keys(p.byPlatform)) platforms.add(k);
  }
  // 固定展示顺序
  const order = ["ark", "codex", "claude", "kiro", "opencode-go", "openai"];
  const series = order
    .filter((p) => platforms.has(p))
    .map((p) => ({
      name: PLATFORM_LABEL[p] ?? p,
      type: "bar" as const,
      stack: "total",
      barWidth: "55%",
      itemStyle: { color: PLATFORM_COLORS[p] },
      data: props.data.map((d) => d.byPlatform[p] ?? 0),
    }));

  return {
    grid: { left: 0, right: 0, top: 24, bottom: 36, containLabel: false },
    tooltip: {
      trigger: "axis",
      axisPointer: { type: "shadow" },
    },
    legend: {
      data: series.map((s) => s.name),
      bottom: 0,
      itemWidth: 10,
      itemHeight: 10,
      textStyle: { color: "#9197a4", fontSize: 11 },
    },
    xAxis: {
      type: "category",
      data: props.data.map((d) => shortDate(d.date)),
      axisLine: { lineStyle: { color: "#e8ebf0" } },
      axisTick: { show: false },
      axisLabel: { color: "#9197a4", fontSize: 11 },
    },
    yAxis: { type: "value", show: false },
    series,
  };
});
</script>

<template>
  <article class="chart">
    <h3>每日使用量</h3>
    <p>按平台汇总的 Token 用量</p>
    <VChart class="graph" :option="option" autoresize />
  </article>
</template>

<style scoped>
.chart {
  min-height: 282px;
  padding: 20px 22px;
  background: var(--card);
  border: 1px solid var(--line);
  border-radius: 16px;
  box-shadow: 0 7px 18px rgba(35, 44, 70, 0.025);
}
.chart h3 {
  margin: 0;
  font-size: 16px;
}
.chart p {
  margin: 6px 0 0;
  color: var(--muted);
  font-size: 12px;
}
.graph {
  position: relative;
  height: 184px;
  margin-top: 17px;
  border-bottom: 1px solid var(--line);
}
</style>
