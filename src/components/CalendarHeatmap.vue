<script setup lang="ts">
import { computed } from "vue";
import type { HeatmapDay } from "@/types/usage";
import { formatTokens } from "@/utils/format";

const props = defineProps<{ days: HeatmapDay[] }>();

const WEEKDAYS = ["日", "一", "二", "三", "四", "五", "六"];

interface Cell {
  date: string;
  total: number;
  level: 0 | 1 | 2 | 3 | 4;
  title: string;
}
interface Week {
  cells: Cell[];
  month: string; // 仅当该列是某月第一列时显示
}

const max = computed(() => Math.max(1, ...props.days.map((d) => d.total)));

function level(v: number): Cell["level"] {
  if (v <= 0) return 0;
  const r = v / max.value;
  if (r <= 0.25) return 1;
  if (r <= 0.5) return 2;
  if (r <= 0.75) return 3;
  return 4;
}

const weeks = computed<Week[]>(() => {
  const cols: Week[] = [];
  let prevMonth = -1;
  for (let i = 0; i < props.days.length; i += 7) {
    const slice = props.days.slice(i, i + 7);
    const cells: Cell[] = slice.map((d) => ({
      date: d.date,
      total: d.total,
      level: level(d.total),
      title: `${d.date} · ${d.total > 0 ? `${formatTokens(d.total)} tokens` : "无用量"}`,
    }));
    const first = slice[0];
    const monthIdx = first ? new Date(first.date).getMonth() : -1;
    const month = monthIdx !== prevMonth ? `${monthIdx + 1}月` : "";
    prevMonth = monthIdx;
    cols.push({ cells, month });
  }
  return cols;
});
</script>

<template>
  <div class="heatmap-scroll">
    <div class="heatmap">
      <div class="hm-months">
        <span class="hm-days-spacer" />
        <span v-for="(w, i) in weeks" :key="i" class="hm-month">{{ w.month }}</span>
      </div>
      <div class="hm-plot">
        <div class="hm-weekdays">
          <span v-for="(d, i) in WEEKDAYS" :key="i" :class="{ hidden: i % 2 === 0 }">{{ d }}</span>
        </div>
        <div class="hm-cols">
          <div v-for="(w, i) in weeks" :key="i" class="hm-col">
            <i
              v-for="cell in w.cells"
              :key="cell.date"
              class="hm-cell"
              :class="`lvl-${cell.level}`"
              :title="cell.title"
            />
          </div>
        </div>
      </div>
      <div class="hm-legend">
        <span>少</span>
        <i class="hm-cell lvl-0" />
        <i class="hm-cell lvl-1" />
        <i class="hm-cell lvl-2" />
        <i class="hm-cell lvl-3" />
        <i class="hm-cell lvl-4" />
        <span>多</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.heatmap-scroll {
  width: 100%;
  overflow-x: auto;
}
.heatmap {
  min-width: max-content;
}
.hm-months {
  display: flex;
  gap: 3px;
  margin-bottom: 6px;
  color: var(--text-subtle);
  font-size: 10px;
}
.hm-days-spacer {
  flex: 0 0 20px;
}
.hm-month {
  flex: 0 0 13px;
  overflow: visible;
  white-space: nowrap;
}
.hm-plot {
  display: flex;
  gap: 5px;
}
.hm-weekdays {
  display: flex;
  flex-direction: column;
  gap: 3px;
  flex: 0 0 15px;
  color: var(--text-subtle);
  font-size: 9px;
}
.hm-weekdays span {
  height: 13px;
  line-height: 13px;
}
.hm-weekdays span.hidden {
  visibility: hidden;
}
.hm-cols {
  display: flex;
  gap: 3px;
}
.hm-col {
  display: flex;
  flex-direction: column;
  gap: 3px;
}
.hm-cell {
  width: 13px;
  height: 13px;
  flex: 0 0 13px;
  border-radius: 3px;
  background: var(--surface-3);
}
.lvl-0 {
  background: var(--surface-2);
  border: 1px solid var(--border);
}
.lvl-1 {
  background: color-mix(in srgb, var(--accent) 28%, var(--surface-2));
}
.lvl-2 {
  background: color-mix(in srgb, var(--accent) 50%, var(--surface-2));
}
.lvl-3 {
  background: color-mix(in srgb, var(--accent) 74%, var(--surface-2));
}
.lvl-4 {
  background: var(--accent);
}
.hm-legend {
  display: flex;
  align-items: center;
  gap: 4px;
  justify-content: flex-end;
  margin-top: 10px;
  color: var(--text-subtle);
  font-size: 10px;
}
.hm-legend .hm-cell {
  width: 11px;
  height: 11px;
  flex: 0 0 11px;
}
</style>
