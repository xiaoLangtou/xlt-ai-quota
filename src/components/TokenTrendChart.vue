<script setup lang="ts">
import { computed } from "vue";
import type { TrendPoint } from "@/types/usage";
import { shortDate } from "@/utils/format";
import { VChart } from "@/echarts";

const props = defineProps<{ data: TrendPoint[] }>();

const option = computed(() => {
  const xData = props.data.map((p) => shortDate(p.date));
  const totals = props.data.map((p) => p.total);
  return {
    grid: { left: 8, right: 8, top: 10, bottom: 28, containLabel: false },
    tooltip: {
      trigger: "axis",
      formatter: (params: { name: string; value: number }[]) => {
        const p = params[0];
        return `${p.name}<br/>总 Token: <b>${p.value.toLocaleString()}</b>`;
      },
    },
    xAxis: {
      type: "category",
      data: xData,
      boundaryGap: false,
      axisLine: { lineStyle: { color: "#e8ebf0" } },
      axisTick: { show: false },
      axisLabel: { color: "#9197a4", fontSize: 11 },
    },
    yAxis: {
      type: "value",
      show: false,
    },
    series: [
      {
        type: "line",
        smooth: true,
        symbol: "none",
        data: totals,
        lineStyle: { color: "#1683f5", width: 4 },
        areaStyle: {
          color: {
            type: "linear",
            x: 0,
            y: 0,
            x2: 0,
            y2: 1,
            colorStops: [
              { offset: 0, color: "rgba(22,131,245,0.24)" },
              { offset: 1, color: "rgba(22,131,245,0)" },
            ],
          },
        },
      },
    ],
  };
});
</script>

<template>
  <article class="chart">
    <h3>Token 用量趋势</h3>
    <p>所选范围的总 Token 消耗</p>
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
