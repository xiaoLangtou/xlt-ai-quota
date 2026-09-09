<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import ToolLogo from "@/components/ToolLogo.vue";
import type { HeatmapDay, HeatmapDetail } from "@/types/usage";
import { formatTokens } from "@/utils/format";

const props = defineProps<{ days: HeatmapDay[] }>();

const MIN_CELL_SIZE = 14;
const CELL_GAP = 3;
const LABEL_WIDTH = 22;
const EDGE_INSET = 7;
const WEEKDAYS = ["日", "一", "二", "三", "四", "五", "六"];
const TOOL_LABEL: Record<string, string> = {
  ark: "火山方舟",
  claude: "Claude Code",
  codex: "Codex",
  kiro: "Kiro",
  qoder: "Qoder",
  "opencode-go": "OpenCode Go",
};

interface Cell extends HeatmapDay {
  level: 0 | 1 | 2 | 3 | 4;
}

interface TooltipState {
  cell: Cell;
  left: number;
  top: number;
  below: boolean;
}

const container = ref<HTMLElement>();
const containerWidth = ref(0);
const tooltip = ref<TooltipState>();
let resizeObserver: ResizeObserver | undefined;

function quantile(values: number[], q: number): number {
  if (!values.length) return 0;
  const position = (values.length - 1) * q;
  const lower = Math.floor(position);
  const fraction = position - lower;
  const left = values[lower] ?? values[values.length - 1];
  const right = values[Math.min(values.length - 1, lower + 1)] ?? left;
  return left + (right - left) * fraction;
}

const thresholds = computed(() => {
  const values = props.days
    .map((day) => day.total)
    .filter((value) => value > 0)
    .sort((a, b) => a - b);
  return {
    first: quantile(values, 0.5),
    second: quantile(values, 0.75),
    third: quantile(values, 0.9),
  };
});

function level(total: number): Cell["level"] {
  if (total <= 0) return 0;
  if (total <= thresholds.value.first) return 1;
  if (total <= thresholds.value.second) return 2;
  if (total <= thresholds.value.third) return 3;
  return 4;
}

const allWeeks = computed<Cell[][]>(() => {
  const weeks: Cell[][] = [];
  for (let index = 0; index < props.days.length; index += 7) {
    weeks.push(
      props.days.slice(index, index + 7).map((day) => ({ ...day, level: level(day.total) })),
    );
  }
  return weeks;
});

const visibleWeekCount = computed(() => {
  const maximum = allWeeks.value.length;
  if (!maximum || !containerWidth.value) return maximum;
  const fitted = Math.floor(
    (containerWidth.value - EDGE_INSET - LABEL_WIDTH) / (MIN_CELL_SIZE + CELL_GAP),
  );
  return Math.max(1, Math.min(maximum, fitted));
});

const visibleWeeks = computed(() => allWeeks.value.slice(-visibleWeekCount.value));

const cellSize = computed(() => {
  const count = visibleWeeks.value.length;
  if (!count) return MIN_CELL_SIZE;
  if (count < allWeeks.value.length) return MIN_CELL_SIZE;
  const gaps = count * CELL_GAP;
  return Math.max(
    MIN_CELL_SIZE,
    Math.floor((containerWidth.value - EDGE_INSET - LABEL_WIDTH - gaps) / count),
  );
});

const gridStyle = computed(() => ({
  gridTemplateColumns: `${LABEL_WIDTH}px repeat(${visibleWeeks.value.length}, ${cellSize.value}px)`,
  columnGap: `${CELL_GAP}px`,
}));

const rowStyle = computed(() => ({
  gridTemplateRows: `repeat(7, ${cellSize.value}px)`,
  rowGap: `${CELL_GAP}px`,
}));

const cellsStyle = computed(() => ({
  ...rowStyle.value,
  gridTemplateColumns: `repeat(${visibleWeeks.value.length}, ${cellSize.value}px)`,
  columnGap: `${CELL_GAP}px`,
}));

const monthMarkers = computed(() => {
  const markers: { label: string; index: number }[] = [];
  visibleWeeks.value.forEach((week, index) => {
    const firstDay = week.find((cell) => cell.date.slice(8, 10) === "01");
    if (firstDay) {
      markers.push({ label: `${Number(firstDay.date.slice(5, 7))}月`, index });
    }
  });
  return markers;
});

const tooltipDetails = computed(() => tooltip.value?.cell.details.slice(0, 7) ?? []);

function showTooltip(cell: Cell, event: MouseEvent | FocusEvent) {
  const target = event.currentTarget as HTMLElement;
  const rect = target.getBoundingClientRect();
  const halfWidth = 156;
  tooltip.value = {
    cell,
    left: Math.min(Math.max(rect.left + rect.width / 2, halfWidth), window.innerWidth - halfWidth),
    top: rect.top < 340 ? rect.bottom + 9 : rect.top - 9,
    below: rect.top < 340,
  };
}

function hideTooltip() {
  tooltip.value = undefined;
}

function detailLabel(detail: HeatmapDetail): string {
  const tool = TOOL_LABEL[detail.platform];
  return detail.model ? `${tool} · ${detail.model}` : tool;
}

function detailPercent(total: number): number {
  const dayTotal = tooltip.value?.cell.total ?? 1;
  return Math.round((total / dayTotal) * 100);
}

function measure() {
  containerWidth.value = container.value?.clientWidth ?? 0;
}

onMounted(() => {
  measure();
  if (container.value) {
    resizeObserver = new ResizeObserver(measure);
    resizeObserver.observe(container.value);
  }
});

watch(() => props.days.length, () => nextTick(measure));
onBeforeUnmount(() => resizeObserver?.disconnect());
</script>

<template>
  <div ref="container" class="heatmap" @mouseleave="hideTooltip">
    <div v-if="visibleWeeks.length" class="hm-content">
      <div class="hm-months" :style="gridStyle">
        <span />
        <span
          v-for="marker in monthMarkers"
          :key="`${marker.label}-${marker.index}`"
          class="hm-month"
          :style="{ gridColumnStart: marker.index + 2 }"
        >{{ marker.label }}</span>
      </div>

      <div class="hm-grid" :style="gridStyle">
        <div class="hm-weekdays" :style="rowStyle">
          <span v-for="weekday in WEEKDAYS" :key="weekday">{{ weekday }}</span>
        </div>
        <div class="hm-cells" :style="cellsStyle">
          <template v-for="week in visibleWeeks" :key="week[0]?.date">
            <button
              v-for="cell in week"
              :key="cell.date"
              type="button"
              class="hm-cell"
              :class="`lvl-${cell.level}`"
              :style="{ width: `${cellSize}px`, height: `${cellSize}px` }"
              :aria-label="`${cell.date}，${cell.total.toLocaleString()} Token`"
              @mouseenter="showTooltip(cell, $event)"
              @focus="showTooltip(cell, $event)"
              @blur="hideTooltip"
            />
          </template>
        </div>
      </div>

      <footer class="hm-footer">
        <span>UTC+08:00</span>
        <div class="hm-legend">
          <span>少</span>
          <i v-for="value in 5" :key="value" class="hm-cell" :class="`lvl-${value - 1}`" />
          <span>多</span>
        </div>
      </footer>
    </div>
    <div v-else class="hm-empty">暂无用量数据</div>

    <Teleport to="body">
      <div
        v-if="tooltip"
        class="hm-tooltip"
        role="tooltip"
        :style="{
          left: `${tooltip.left}px`,
          top: `${tooltip.top}px`,
          transform: tooltip.below ? 'translateX(-50%)' : 'translate(-50%, -100%)',
        }"
      >
        <header>
          <strong>{{ tooltip.cell.date }}</strong>
          <span>等级 {{ tooltip.cell.level }}</span>
        </header>
        <div class="hm-tooltip-total">
          <i :class="`lvl-${tooltip.cell.level}`" />
          <span>总用量</span>
          <b>{{ formatTokens(tooltip.cell.total) }}</b>
        </div>
        <div v-if="tooltipDetails.length" class="hm-tooltip-details">
          <small>模型明细</small>
          <div v-for="detail in tooltipDetails" :key="`${detail.platform}-${detail.model}`" class="hm-detail">
            <div class="hm-detail-line">
              <span class="hm-detail-name">
                <ToolLogo :platform="detail.platform" :size="15" />
                <span :title="detailLabel(detail)">{{ detailLabel(detail) }}</span>
              </span>
              <b>{{ formatTokens(detail.total) }} · {{ detailPercent(detail.total) }}%</b>
            </div>
            <span class="hm-detail-track"><i :style="{ width: `${detailPercent(detail.total)}%` }" /></span>
          </div>
        </div>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.heatmap { width: 100%; min-width: 0; }
.hm-content {
  width: 100%;
  padding-right: 7px;
  overflow: visible;
}
.hm-months,
.hm-grid { display: grid; width: 100%; min-width: 0; }
.hm-months { margin-bottom: 7px; color: var(--text-subtle); font-size: 10px; font-weight: 600; }
.hm-month { white-space: nowrap; }
.hm-weekdays {
  display: grid;
  padding-right: 5px;
  color: var(--text-subtle);
  font-size: 10px;
  font-weight: 600;
}
.hm-weekdays span { display: flex; align-items: center; line-height: 1; }
.hm-cells { display: grid; grid-column: 2 / -1; grid-auto-flow: column; }
.hm-cell {
  display: block;
  padding: 0;
  border: 0;
  border-radius: 4px;
  background: var(--heatmap-0);
}
button.hm-cell { cursor: pointer; transition: transform 100ms ease, box-shadow 100ms ease; }
button.hm-cell:hover,
button.hm-cell:focus-visible {
  position: relative;
  z-index: 1;
  transform: scale(1.12);
  outline: none;
  box-shadow: 0 0 0 2px var(--surface), 0 0 0 3px var(--heatmap-4);
}
.lvl-0 { background: var(--heatmap-0); }
.lvl-1 { background: var(--heatmap-1); }
.lvl-2 { background: var(--heatmap-2); }
.lvl-3 { background: var(--heatmap-3); }
.lvl-4 { background: var(--heatmap-4); }
.hm-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-top: 13px;
  color: var(--text-subtle);
  font-size: 10px;
  font-weight: 500;
}
.hm-legend { display: flex; align-items: center; gap: 5px; }
.hm-legend .hm-cell { width: 12px; height: 12px; border-radius: 3px; }
.hm-empty { padding: 32px 0; color: var(--text-subtle); font-size: 13px; text-align: center; }
</style>

<style>
.hm-tooltip {
  position: fixed;
  z-index: 1000;
  width: 304px;
  max-width: calc(100vw - 24px);
  padding: 14px;
  border: 1px solid var(--glass-border);
  border-radius: 13px;
  background: var(--tooltip-bg);
  color: var(--text);
  box-shadow: var(--shadow-pop);
  pointer-events: none;
}
.hm-tooltip header,
.hm-tooltip-total,
.hm-detail-line { display: flex; align-items: center; }
.hm-tooltip header { justify-content: space-between; gap: 16px; }
.hm-tooltip header strong { font-family: var(--font-mono); font-size: 14px; }
.hm-tooltip header span { color: var(--text-subtle); font-size: 10px; }
.hm-tooltip-total { gap: 8px; margin-top: 12px; }
.hm-tooltip-total > i { width: 10px; height: 10px; flex: 0 0 10px; border-radius: 50%; }
.hm-tooltip-total span { flex: 1; color: var(--text-muted); font-size: 12px; }
.hm-tooltip-total b { font-family: var(--font-mono); font-size: 13px; }
.hm-tooltip-details { display: grid; gap: 10px; margin-top: 13px; }
.hm-tooltip-details > small { color: var(--text-subtle); font-size: 10px; font-weight: 600; }
.hm-detail { display: grid; gap: 5px; min-width: 0; }
.hm-detail-line { justify-content: space-between; gap: 12px; min-width: 0; }
.hm-detail-name { display: flex; align-items: center; gap: 7px; min-width: 0; font-size: 12px; }
.hm-detail-name > span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.hm-detail-line > b {
  flex: 0 0 auto;
  color: var(--text-muted);
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 500;
}
.hm-detail-track { height: 4px; overflow: hidden; border-radius: 999px; background: var(--surface-3); }
.hm-detail-track > i { display: block; height: 100%; border-radius: inherit; background: var(--heatmap-4); }
</style>
