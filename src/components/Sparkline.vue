<script setup lang="ts">
import { computed } from "vue";

const props = withDefaults(
  defineProps<{ values: number[]; color?: string }>(),
  { color: "var(--u-ok)" },
);

const W = 100;
const H = 30;
const PAD = 3;

const paths = computed(() => {
  const v = props.values.filter((n) => Number.isFinite(n));
  if (v.length === 0) return { line: "", area: "" };
  const max = Math.max(...v);
  const min = Math.min(...v);
  const span = max - min || 1;
  const n = v.length;
  const pts = v.map((val, i) => {
    const x = n === 1 ? W / 2 : (i / (n - 1)) * (W - PAD * 2) + PAD;
    const y = H - PAD - ((val - min) / span) * (H - PAD * 2);
    return [x, y] as const;
  });
  const line = pts.map((p, i) => `${i ? "L" : "M"}${p[0].toFixed(1)} ${p[1].toFixed(1)}`).join(" ");
  const last = pts[pts.length - 1]!;
  const first = pts[0]!;
  const area = `${line} L${last[0].toFixed(1)} ${H} L${first[0].toFixed(1)} ${H} Z`;
  return { line, area };
});
</script>

<template>
  <svg class="spark" viewBox="0 0 100 30" preserveAspectRatio="none" aria-hidden="true">
    <path :d="paths.area" class="spark-area" :style="{ fill: color }" />
    <path :d="paths.line" class="spark-line" :style="{ stroke: color }" />
  </svg>
</template>

<style scoped>
.spark {
  width: 100%;
  height: 100%;
  display: block;
}
.spark-area {
  opacity: 0.16;
}
.spark-line {
  fill: none;
  stroke-width: 1.6;
  stroke-linejoin: round;
  stroke-linecap: round;
  vector-effect: non-scaling-stroke;
}
</style>
