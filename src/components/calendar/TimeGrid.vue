<script setup lang="ts">
import { computed, ref } from "vue";
import NowIndicator from "@/components/calendar/NowIndicator.vue";
import type { CalendarEvent } from "@/types/productivity";
import {
  HOUR_HEIGHT,
  TIMELINE_HOURS,
  type PositionedDayEvent,
  categoryClass,
  combineDateTime,
  dayOfMonth,
  eventDurationMinutes,
  eventStartDate,
  eventStartTime,
  layoutDayEvents,
  minutesToTime,
  snapMinutes,
  timeToMinutes,
  today as todayString,
  weekdayLabel,
} from "@/utils/productivity";

const props = defineProps<{
  dates: string[];
  events: CalendarEvent[];
}>();

const emit = defineEmits<{
  (event: "select-event", payload: CalendarEvent): void;
  (event: "move-event", payload: { id: string; startAt: string; endAt?: string }): void;
  (event: "create-event", payload: { date: string; time: string; endTime?: string }): void;
  (event: "quick-create", payload: { date: string; time: string; x: number; y: number }): void;
  (event: "open-day", payload: string): void;
}>();

const todayValue = todayString();
const gridTemplate = computed(() => `56px repeat(${props.dates.length}, minmax(0, 1fr))`);
const columnExpression = computed(() => `calc((100% - 56px) / ${props.dates.length})`);
const bodyRef = ref<HTMLElement | null>(null);

interface TimedBlock {
  event: CalendarEvent;
  layout: PositionedDayEvent;
  dayIndex: number;
}

// 同一天的定时事件按重叠列排布，重叠的任务并排显示而不是叠在一起。
const timedBlocks = computed<TimedBlock[]>(() => {
  const list: TimedBlock[] = [];
  props.dates.forEach((date, dayIndex) => {
    for (const layout of layoutDayEvents(props.events, date)) {
      list.push({ event: layout.event, layout, dayIndex });
    }
  });
  return list;
});

function covers(item: CalendarEvent, date: string): boolean {
  const start = eventStartDate(item);
  const end = item.allDay && item.endAt ? item.endAt.slice(0, 10) : start;
  return start <= date && end >= date;
}

function allDayEvents(date: string): CalendarEvent[] {
  return props.events.filter((item) => item.allDay && covers(item, date));
}

const showAllDay = computed(() => props.events.some((item) => item.allDay));

interface Preview {
  id: string;
  top: number;
  height: number;
  column: number;
  left: number;
  width: number;
}

const drag = ref<{
  item: CalendarEvent;
  mode: "move" | "resize";
  startX: number;
  startY: number;
  dateIndex: number;
  columnWidth: number;
  startMinutes: number;
  duration: number;
  left: number;
  width: number;
  moved: boolean;
} | null>(null);
const preview = ref<Preview | null>(null);

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

function blockStyle(block: TimedBlock): Record<string, string> {
  const dragging = preview.value?.id === block.event.id;
  const column = dragging ? preview.value!.column : block.dayIndex;
  const top = dragging ? preview.value!.top : block.layout.top;
  const height = dragging ? preview.value!.height : block.layout.height;
  const leftPct = dragging ? preview.value!.left : block.layout.left;
  const widthPct = dragging ? preview.value!.width : block.layout.width;
  return {
    top: `${top}px`,
    height: `${height}px`,
    left: `calc(56px + ${column} * ${columnExpression.value} + ${leftPct / 100} * ${columnExpression.value})`,
    width: `calc(${widthPct / 100} * ${columnExpression.value} - 4px)`,
  };
}

function clearDrag(): void {
  window.removeEventListener("pointermove", onPointerMove);
  window.removeEventListener("pointerup", onPointerUp);
  drag.value = null;
  preview.value = null;
}

function onPointerMove(event: PointerEvent): void {
  const state = drag.value;
  if (!state) return;
  const dy = event.clientY - state.startY;
  const dx = event.clientX - state.startX;
  if (Math.abs(dy) > 3 || Math.abs(dx) > 3) state.moved = true;
  if (state.mode === "resize") {
    const duration = clamp(snapMinutes(state.duration + (dy / HOUR_HEIGHT) * 60), 15, 1440 - state.startMinutes);
    preview.value = {
      id: state.item.id,
      top: (state.startMinutes / 60) * HOUR_HEIGHT,
      height: Math.max(15, (duration / 60) * HOUR_HEIGHT),
      column: state.dateIndex,
      left: state.left,
      width: state.width,
    };
    return;
  }
  const minutes = clamp(snapMinutes(state.startMinutes + (dy / HOUR_HEIGHT) * 60), 0, 1440 - state.duration);
  let column = state.dateIndex;
  if (props.dates.length > 1 && state.columnWidth > 0) {
    column = clamp(state.dateIndex + Math.round(dx / state.columnWidth), 0, props.dates.length - 1);
  }
  preview.value = {
    id: state.item.id,
    top: (minutes / 60) * HOUR_HEIGHT,
    height: Math.max(20, (state.duration / 60) * HOUR_HEIGHT),
    column,
    left: 0,
    width: 100,
  };
}

function onPointerUp(): void {
  const state = drag.value;
  const result = preview.value;
  if (state && result && state.moved) {
    const date = props.dates[result.column] ?? props.dates[state.dateIndex];
    if (state.mode === "resize") {
      const endTime = minutesToTime(((result.top + result.height) / HOUR_HEIGHT) * 60);
      emit("move-event", {
        id: state.item.id,
        startAt: state.item.startAt,
        endAt: combineDateTime(date, endTime),
      });
    } else {
      const startTime = minutesToTime((result.top / HOUR_HEIGHT) * 60);
      emit("move-event", { id: state.item.id, startAt: combineDateTime(date, startTime) });
    }
  } else if (state && !state.moved) {
    emit("select-event", state.item);
  }
  clearDrag();
}

function startDrag(block: TimedBlock, event: PointerEvent, mode: "move" | "resize"): void {
  if (event.button !== 0) return;
  const width = bodyRef.value ? (bodyRef.value.clientWidth - 56) / props.dates.length : 0;
  drag.value = {
    item: block.event,
    mode,
    startX: event.clientX,
    startY: event.clientY,
    dateIndex: block.dayIndex,
    columnWidth: width,
    startMinutes: timeToMinutes(eventStartTime(block.event) ?? "00:00"),
    duration: eventDurationMinutes(block.event, 30),
    left: block.layout.left,
    width: block.layout.width,
    moved: false,
  };
  preview.value = null;
  window.addEventListener("pointermove", onPointerMove);
  window.addEventListener("pointerup", onPointerUp);
  event.preventDefault();
}

// —— 参考模板：在空白时间轴上拖动即可画出待办 ——
interface CreateGesture {
  date: string;
  startMinutes: number;
  x: number;
  y: number;
  moved: boolean;
}

const createGesture = ref<CreateGesture | null>(null);
const createPreview = ref<{ date: string; start: number; end: number } | null>(null);

function columnAtPoint(x: number, y: number): HTMLElement | null {
  for (const element of document.elementsFromPoint(x, y)) {
    const column = (element as HTMLElement).closest?.(".day-column") as HTMLElement | null;
    if (column) return column;
  }
  return null;
}

function minutesAt(clientY: number, column: HTMLElement): number {
  return clamp(snapMinutes(((clientY - column.getBoundingClientRect().top) / HOUR_HEIGHT) * 60), 0, 1439);
}

function onColumnPointerDown(date: string, event: PointerEvent): void {
  if (event.button !== 0 || drag.value) return;
  const target = event.target as HTMLElement;
  if (target.closest("[data-event-id]")) return;
  const column = event.currentTarget as HTMLElement;
  createGesture.value = {
    date,
    startMinutes: minutesAt(event.clientY, column),
    x: event.clientX,
    y: event.clientY,
    moved: false,
  };
  window.addEventListener("pointermove", onColumnPointerMove);
  window.addEventListener("pointerup", onColumnPointerUp);
}

function clearCreate(): void {
  window.removeEventListener("pointermove", onColumnPointerMove);
  window.removeEventListener("pointerup", onColumnPointerUp);
  createGesture.value = null;
  createPreview.value = null;
}

function onColumnPointerMove(event: PointerEvent): void {
  const state = createGesture.value;
  if (!state) return;
  if (!state.moved) {
    if (Math.hypot(event.clientX - state.x, event.clientY - state.y) < 5) return;
    state.moved = true;
  }
  const column = columnAtPoint(event.clientX, event.clientY);
  const date = column?.dataset.date ?? state.date;
  const minutes = column ? minutesAt(event.clientY, column) : state.startMinutes;
  const start = Math.min(state.startMinutes, minutes);
  const end = Math.max(Math.max(state.startMinutes, minutes), start + 15);
  createPreview.value = { date, start, end };
}

function onColumnPointerUp(): void {
  const previewValue = createPreview.value;
  const state = createGesture.value;
  if (state?.moved && previewValue) {
    emit("create-event", {
      date: previewValue.date,
      time: minutesToTime(previewValue.start),
      endTime: minutesToTime(previewValue.end),
    });
  } else if (state && !state.moved) {
    // 单击空白：弹出快速创建；拖画时间块：走 create-event 打开完整表单。
    emit("quick-create", {
      date: state.date,
      time: minutesToTime(state.startMinutes),
      x: state.x,
      y: state.y,
    });
  }
  clearCreate();
}

function draftStyle(): Record<string, string> {
  const value = createPreview.value;
  if (!value) return {};
  const column = Math.max(0, props.dates.indexOf(value.date));
  return {
    top: `${(value.start / 60) * HOUR_HEIGHT}px`,
    height: `${Math.max(15, ((value.end - value.start) / 60) * HOUR_HEIGHT - 3)}px`,
    left: `calc(56px + ${column} * ${columnExpression.value} + 1px)`,
    width: `calc(${columnExpression.value} - 2px)`,
  };
}

const isDragging = computed(() => drag.value !== null);
</script>

<template>
  <div class="time-grid" :class="{ dragging: isDragging }">
    <div class="grid-header" :style="{ gridTemplateColumns: gridTemplate }">
      <div class="axis-spacer" />
      <button
        v-for="date in dates"
        :key="date"
        type="button"
        class="day-head"
        :class="{ today: date === todayValue }"
        @click="emit('open-day', date)"
      >
        <span class="weekday">{{ weekdayLabel(date) }}</span>
        <span class="day-number">{{ dayOfMonth(date) }}</span>
      </button>
    </div>

    <div v-if="showAllDay" class="allday-row" :style="{ gridTemplateColumns: gridTemplate }">
      <div class="axis-label">全天</div>
      <div v-for="date in dates" :key="`allday-${date}`" class="allday-cell">
        <button
          v-for="item in allDayEvents(date)"
          :key="item.id"
          type="button"
          class="event allday"
          :class="[categoryClass(item.category), { done: item.status === 'done' }]"
          :data-event-id="item.id"
          @click="emit('select-event', item)"
        >
          <span class="event-title">{{ item.title }}</span>
        </button>
      </div>
    </div>

    <div class="grid-scroll">
      <div ref="bodyRef" class="grid-body" :style="{ gridTemplateColumns: gridTemplate }">
        <div class="time-axis">
          <div v-for="hour in TIMELINE_HOURS" :key="hour" class="hour-cell">
            <span>{{ String(hour).padStart(2, "0") }}:00</span>
          </div>
        </div>
        <div
          v-for="(date, index) in dates"
          :key="date"
          class="day-column"
          :data-index="index"
          :data-date="date"
          :class="{ today: date === todayValue }"
          @pointerdown="onColumnPointerDown(date, $event)"
        >
          <div v-for="hour in TIMELINE_HOURS" :key="hour" class="hour-cell" />
          <NowIndicator v-if="date === todayValue" />
        </div>

        <div class="blocks-layer">
          <div v-if="createPreview" class="event block draft" :style="draftStyle()">
            <span class="event-title">新建待办</span>
            <span class="event-time">
              {{ minutesToTime(createPreview.start) }} – {{ minutesToTime(createPreview.end) }}
            </span>
          </div>
          <button
            v-for="block in timedBlocks"
            :key="block.event.id"
            type="button"
            class="event block"
            :class="[categoryClass(block.event.category), { done: block.event.status === 'done' }]"
            :data-event-id="block.event.id"
            :style="blockStyle(block)"
            @pointerdown="startDrag(block, $event, 'move')"
          >
            <span class="event-title">{{ block.event.title }}</span>
            <span class="event-time">{{ eventStartTime(block.event) }}</span>
            <span class="resize-handle" @pointerdown.stop="startDrag(block, $event, 'resize')" />
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.time-grid {
  display: grid;
  grid-template-rows: 44px auto minmax(0, 1fr);
  min-width: 0;
  min-height: 0;
  height: 100%;
  overflow: hidden;
  border: 1px solid var(--border);
  border-radius: 13px;
  background: var(--surface);
}
.time-grid.dragging {
  cursor: grabbing;
}
.grid-header {
  display: grid;
  border-bottom: 1px solid var(--border);
}
.axis-spacer {
  border-right: 1px solid var(--border);
}
.day-head {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  border: 0;
  border-right: 1px solid var(--border);
  background: transparent;
  color: var(--text-muted);
  font-size: 11px;
  font-weight: 600;
  cursor: pointer;
}
.day-head:last-child {
  border-right: 0;
}
.day-head:hover {
  background: var(--surface-2);
}
.day-head .day-number {
  display: grid;
  place-items: center;
  width: 22px;
  height: 22px;
  border-radius: 50%;
  font-family: var(--font-mono);
  font-size: 11px;
}
.day-head.today .day-number {
  background: var(--accent);
  color: var(--accent-contrast);
}
.allday-row {
  display: grid;
  border-bottom: 1px solid var(--border);
  background: var(--surface-2);
}
.axis-label {
  display: grid;
  place-items: center;
  border-right: 1px solid var(--border);
  color: var(--text-subtle);
  font-size: 9.5px;
  font-weight: 600;
}
.allday-cell {
  display: flex;
  flex-direction: column;
  gap: 3px;
  min-height: 30px;
  padding: 3px 4px;
  border-right: 1px solid var(--border);
}
.allday-cell:last-child {
  border-right: 0;
}
.grid-scroll {
  min-height: 0;
  overflow-y: auto;
}
.grid-body {
  position: relative;
  display: grid;
}
.time-axis {
  border-right: 1px solid var(--border);
}
.hour-cell {
  height: 48px;
  border-bottom: 1px solid var(--border);
}
.time-axis .hour-cell span {
  position: relative;
  top: -7px;
  display: block;
  padding-right: 6px;
  color: var(--text-subtle);
  font-family: var(--font-mono);
  font-size: 9px;
  text-align: right;
}
.day-column {
  position: relative;
  border-right: 1px solid var(--border);
}
.day-column:last-of-type {
  border-right: 0;
}
.day-column.today {
  background: color-mix(in srgb, var(--accent) 4%, transparent);
}
.blocks-layer {
  position: absolute;
  inset: 0;
  pointer-events: none;
}
.event.block {
  position: absolute;
  z-index: 2;
  display: flex;
  flex-direction: column;
  gap: 1px;
  padding: 3px 6px;
  overflow: hidden;
  border: 0;
  border-left: 3px solid currentColor;
  border-radius: 5px;
  text-align: left;
  pointer-events: auto;
  cursor: grab;
}
.event.allday {
  display: flex;
  align-items: center;
  gap: 5px;
  height: 22px;
  padding: 0 6px;
  overflow: hidden;
  border: 0;
  border-left: 2px solid currentColor;
  border-radius: 4px;
  text-align: left;
  cursor: pointer;
}
.event.cat-work {
  color: var(--u-ok);
  background: color-mix(in srgb, var(--u-ok) 16%, transparent);
}
.event.cat-planning {
  color: var(--u-warn);
  background: color-mix(in srgb, var(--u-warn) 18%, transparent);
}
.event.cat-personal {
  color: #7c5bb7;
  background: color-mix(in srgb, #7c5bb7 16%, transparent);
}
.event.done {
  opacity: 0.55;
  text-decoration: line-through;
}
.event.draft {
  z-index: 3;
  border-left-style: dashed;
  border-left-color: var(--accent);
  background: var(--accent-weak);
  color: var(--accent);
  pointer-events: none;
}
.event .event-title {
  min-width: 0;
  overflow: hidden;
  color: var(--text);
  font-size: 10.5px;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.event .event-time {
  color: var(--text-subtle);
  font-family: var(--font-mono);
  font-size: 9px;
}
.resize-handle {
  position: absolute;
  right: 0;
  bottom: 0;
  left: 0;
  height: 6px;
  cursor: ns-resize;
}
</style>
