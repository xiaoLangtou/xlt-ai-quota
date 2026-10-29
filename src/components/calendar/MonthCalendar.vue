<script setup lang="ts">
import { computed, ref } from "vue";
import { useCalendarTodo } from "@/composables/useCalendarTodo";
import type { CalendarEvent } from "@/types/productivity";
import {
  categoryClass,
  combineDateTime,
  dayOfMonth,
  eventStartDate,
  eventStartTime,
  monthGridDates,
  today as todayString,
  toDateString,
} from "@/utils/productivity";

const emit = defineEmits<{
  (event: "quick-create", payload: { date: string; x: number; y: number }): void;
  (event: "select-event", payload: CalendarEvent): void;
  (event: "move-event", payload: { id: string; startAt: string }): void;
}>();

const { anchor, visibleEvents, setAnchor } = useCalendarTodo();

const todayValue = todayString();
const dates = computed(() => monthGridDates(anchor.value));

const eventsByDate = computed(() => {
  const map = new Map<string, CalendarEvent[]>();
  for (const event of visibleEvents.value) {
    const start = eventStartDate(event);
    const end = event.allDay && event.endAt ? event.endAt.slice(0, 10) : start;
    // 事件覆盖的每一天都显示；单日事件只落一格。
    let cursor = start;
    let guard = 0;
    while (cursor <= end && guard < 62) {
      const list = map.get(cursor) ?? [];
      list.push(event);
      map.set(cursor, list);
      cursor = toDateString(new Date(new Date(`${cursor}T12:00:00`).getTime() + 86_400_000));
      guard += 1;
    }
  }
  for (const list of map.values()) {
    list.sort((a, b) => {
      if (a.allDay !== b.allDay) return a.allDay ? -1 : 1;
      return a.startAt.localeCompare(b.startAt);
    });
  }
  return map;
});

function dayEvents(date: string): CalendarEvent[] {
  return eventsByDate.value.get(date) ?? [];
}

function isCurrentMonth(date: string): boolean {
  return date.slice(0, 7) === anchor.value.slice(0, 7);
}

// 单击空白处快速创建待办；点事件查看详情；拖拽事件跨日期移动。
function onCellClick(date: string, event: MouseEvent): void {
  setAnchor(date);
  if ((event.target as HTMLElement).closest("[data-event-id]")) return;
  emit("quick-create", { date, x: event.clientX, y: event.clientY });
}

// 指针拖拽跨日期移动事件：Tauri WebView 会拦截原生 HTML5 拖拽。
interface MonthDrag {
  id: string;
  fromDate: string;
  x: number;
  y: number;
  moved: boolean;
}

const monthDrag = ref<MonthDrag | null>(null);
const dragOverDate = ref<string | null>(null);
let suppressClick = false;

function dateAt(x: number, y: number): string | null {
  for (const element of document.elementsFromPoint(x, y)) {
    const cell = (element as HTMLElement).closest?.("[data-date]") as HTMLElement | null;
    if (cell?.dataset.date) return cell.dataset.date;
  }
  return null;
}

function onEventPointerDown(item: CalendarEvent, event: PointerEvent): void {
  if (event.button !== 0) return;
  monthDrag.value = {
    id: item.id,
    fromDate: eventStartDate(item),
    x: event.clientX,
    y: event.clientY,
    moved: false,
  };
  window.addEventListener("pointermove", onMonthPointerMove);
  window.addEventListener("pointerup", onMonthPointerUp);
  window.addEventListener("pointercancel", onMonthPointerUp);
  event.preventDefault();
}

function onMonthPointerMove(event: PointerEvent): void {
  const state = monthDrag.value;
  if (!state) return;
  if (!state.moved) {
    if (Math.hypot(event.clientX - state.x, event.clientY - state.y) < 5) return;
    state.moved = true;
  }
  dragOverDate.value = dateAt(event.clientX, event.clientY);
}

function onMonthPointerUp(): void {
  window.removeEventListener("pointermove", onMonthPointerMove);
  window.removeEventListener("pointerup", onMonthPointerUp);
  window.removeEventListener("pointercancel", onMonthPointerUp);
  const state = monthDrag.value;
  const target = dragOverDate.value;
  monthDrag.value = null;
  dragOverDate.value = null;
  if (!state?.moved) return;
  suppressClick = true;
  setTimeout(() => {
    suppressClick = false;
  }, 0);
  if (!target || target === state.fromDate) return;
  const item = visibleEvents.value.find((value) => value.id === state.id);
  if (!item) return;
  const startAt = item.allDay ? target : combineDateTime(target, item.startAt.slice(11, 16));
  emit("move-event", { id: state.id, startAt });
}

function selectEvent(item: CalendarEvent, event: MouseEvent): void {
  event.stopPropagation();
  if (suppressClick) return;
  emit("select-event", item);
}
</script>

<template>
  <div class="month-card">
    <div class="weekdays">
      <span v-for="label in ['周一', '周二', '周三', '周四', '周五', '周六', '周日']" :key="label">
        {{ label }}
      </span>
    </div>
    <div class="month-grid">
      <div
        v-for="date in dates"
        :key="date"
        class="day-cell"
        :class="{
          other: !isCurrentMonth(date),
          today: date === todayValue,
          selected: date === anchor,
          'drop-target': dragOverDate === date && monthDrag?.moved,
        }"
        :data-date="date"
        @click="onCellClick(date, $event)"
      >
        <div class="day-number">{{ dayOfMonth(date) }}</div>
        <button
          v-for="item in dayEvents(date).slice(0, 3)"
          :key="`${item.id}-${date}`"
          type="button"
          class="event"
          :class="[categoryClass(item.category), { done: item.status === 'done', dragging: monthDrag?.id === item.id && monthDrag.moved }]"
          :data-event-id="item.id"
          @click="selectEvent(item, $event)"
          @pointerdown="onEventPointerDown(item, $event)"
        >
          <span v-if="!item.allDay" class="time">{{ eventStartTime(item) }}</span>
          <span class="event-title">{{ item.title }}</span>
        </button>
        <div v-if="dayEvents(date).length > 3" class="more">
          +{{ dayEvents(date).length - 3 }} 项
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.month-card {
  display: grid;
  grid-template-rows: 32px minmax(0, 1fr);
  height: 100%;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
  border: 1px solid var(--border);
  border-radius: 13px;
  background: var(--surface);
}
.weekdays {
  display: grid;
  grid-template-columns: repeat(7, minmax(0, 1fr));
  border-bottom: 1px solid var(--border);
}
.weekdays span {
  padding: 9px 10px 7px;
  color: var(--text-subtle);
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.05em;
  text-align: right;
}
.month-grid {
  display: grid;
  grid-template-columns: repeat(7, minmax(0, 1fr));
  grid-template-rows: repeat(6, minmax(72px, 1fr));
  min-height: 0;
}
.day-cell {
  position: relative;
  min-width: 0;
  min-height: 0;
  padding: 7px;
  overflow: hidden;
  border-right: 1px solid var(--border);
  border-bottom: 1px solid var(--border);
  background: var(--surface);
  cursor: pointer;
}
.day-cell:nth-child(7n) {
  border-right: 0;
}
.day-cell:nth-last-child(-n + 7) {
  border-bottom: 0;
}
.day-cell:hover {
  background: var(--surface-2);
}
.day-cell.other {
  background: var(--surface-2);
}
.day-cell.other .day-number {
  color: var(--text-subtle);
}
.day-cell.selected {
  background: var(--accent-weak);
  box-shadow: inset 0 0 0 1px var(--accent-ring);
}
.day-cell.drop-target {
  background: var(--accent-weak);
  box-shadow: inset 0 0 0 2px var(--accent);
}
.day-number {
  display: grid;
  place-items: center;
  width: 23px;
  height: 23px;
  border-radius: 50%;
  color: var(--text-muted);
  font-family: var(--font-mono);
  font-size: 11px;
}
.day-cell.today .day-number {
  background: var(--accent);
  color: var(--accent-contrast);
}
.event {
  display: flex;
  align-items: center;
  gap: 5px;
  width: 100%;
  min-width: 0;
  height: 21px;
  margin-top: 4px;
  padding: 0 6px;
  border: 0;
  border-left: 2px solid currentColor;
  border-radius: 4px;
  font-size: 10px;
  text-align: left;
  cursor: pointer;
}
.event.cat-work {
  color: var(--u-ok);
  background: color-mix(in srgb, var(--u-ok) 14%, transparent);
}
.event.cat-planning {
  color: var(--u-warn);
  background: color-mix(in srgb, var(--u-warn) 16%, transparent);
}
.event.cat-personal {
  color: #7c5bb7;
  background: color-mix(in srgb, #7c5bb7 15%, transparent);
}
.event.done {
  opacity: 0.56;
  text-decoration: line-through;
}
.event.dragging {
  opacity: 0.4;
}
.event .time {
  flex: none;
  font-family: var(--font-mono);
  font-size: 9px;
}
.event .event-title {
  min-width: 0;
  overflow: hidden;
  color: var(--text);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.more {
  margin-top: 5px;
  padding-left: 7px;
  color: var(--text-subtle);
  font-size: 9.5px;
}
</style>
