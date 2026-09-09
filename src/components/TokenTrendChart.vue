<script setup lang="ts">
import { computed } from "vue";
import type { TrendPoint } from "@/types/usage";
import { shortDate } from "@/utils/format";
import { useTheme } from "@/composables/useTheme";
import { VChart } from "@/echarts";

const props = defineProps<{ data: TrendPoint[] }>();
const { resolved } = useTheme();

function cssVar(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

const option = computed(() => {
  // 依赖 resolved，主题切换时重算配色
  void resolved.value;
  const accent = cssVar("--accent") || "#5457e6";
  const axis = cssVar("--border") || "#e5e7ec";
  const label = cssVar("--text-subtle") || "#8b93a2";
  const surface = cssVar("--surface") || "#ffffff";
  const border = cssVar("--border") || "#e5e7ec";
  const text = cssVar("--text") || "#171b23";

  const xData = props.data.map((p) => shortDate(p.date));
  const totals = props.data.map((p) => p.total);
  return {
    grid: { left: 8, right: 8, top: 10, bottom: 28, containLabel: false },
    tooltip: {
      trigger: "axis",
      backgroundColor: surface,
      borderColor: border,
      borderWidth: 1,
      textStyle: { color: text, fontSize: 12 },
      formatter: (params: { name: string; value: number }[]) => {
        const p = params[0];
        return `${p.name}<br/>总 Token: <b>${p.value.toLocaleString()}</b>`;
      },
    },
    xAxis: {
      type: "category",
      data: xData,
      boundaryGap: false,
      axisLine: { lineStyle: { color: axis } },
      axisTick: { show: false },
      axisLabel: { color: label, fontSize: 11 },
    },
    yAxis: { type: "value", show: false },
    series: [
      {
        type: "line",
        smooth: 0.2,
        symbol: "circle",
        symbolSize: 5,
        showSymbol: true,
        itemStyle: { color: accent, borderColor: surface, borderWidth: 1.5 },
        data: totals,
        lineStyle: { color: accent, width: 2.5 },
        areaStyle: {
          color: {
            type: "linear",
            x: 0,
            y: 0,
            x2: 0,
            y2: 1,
            colorStops: [
              { offset: 0, color: `${accent}40` },
              { offset: 1, color: `${accent}00` },
            ],
          },
        },
      },
    ],
  };
});
</script>

<template>
  <article class="chart trend-chart">
    <div class="chart-heading">
      <div>
        <h3>使用趋势</h3>
        <p>所选范围内的每日 Token 消耗</p>
      </div>
      <span class="chart-chip">总 Token</span>
    </div>
    <VChart class="graph" :option="option" autoresize />
  </article>
</template>

<style scoped>
.chart {
  min-width: 0;
  overflow: hidden;
  min-height: 292px;
  padding: 19px 20px 16px;
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
  height: 202px;
  margin-top: 12px;
  border-top: 1px solid var(--border);
}
@media (max-width: 600px) {
  .chart {
    min-height: 272px;
    padding: 18px;
  }
  .graph {
    height: 180px;
  }
}
</style>
