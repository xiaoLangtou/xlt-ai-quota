<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import CalendarToolbar from "@/components/calendar/CalendarToolbar.vue";
import CategoryFilter from "@/components/calendar/CategoryFilter.vue";
import DayCalendar from "@/components/calendar/DayCalendar.vue";
import MiniCalendar from "@/components/calendar/MiniCalendar.vue";
import MonthCalendar from "@/components/calendar/MonthCalendar.vue";
import NewTaskMenu from "@/components/calendar/NewTaskMenu.vue";
import QuickTaskMenu from "@/components/calendar/QuickTaskMenu.vue";
import TodoDetailPopover from "@/components/calendar/TodoDetailPopover.vue";
import TodoEditModal from "@/components/calendar/TodoEditModal.vue";
import WeekCalendar from "@/components/calendar/WeekCalendar.vue";
import { useCalendarTodo } from "@/composables/useCalendarTodo";
import { pushToast } from "@/composables/useToast";
import type { CalendarEvent, TodoCategory, TodoDraft, TodoStatus, TodoTask } from "@/types/productivity";
import { buildQuickTaskDraft, durationMinutesBetween, parseQuickEvent } from "@/utils/productivity";

// 日历页 = 待办的时间线视图。单击空白快速创建，拖画时间块精确安排，点事件查看待办详情。
const {
  view,
  anchor,
  error,
  initialize,
  setView,
  setAnchor,
  goToday,
  navigate,
  moveEvent,
  deleteTask,
  deleteEvent,
  createTask,
  loadTask,
} = useCalendarTodo();

const router = useRouter();
const editOpen = ref(false);
const editingTask = ref<TodoTask | null>(null);
const defaultDate = ref<string | null>(null);
const defaultTime = ref<string | null>(null);
const defaultEstimate = ref<number | null>(null);
const defaultStatus = ref<TodoStatus>("open");
const quickCreate = ref<{ date: string; time?: string; x: number; y: number } | null>(null);
const miniOpen = ref(false);
const editingEvent = ref<CalendarEvent | null>(null);
const detailTask = ref<TodoTask | null>(null);
const eventAnchorEl = ref<HTMLElement | null>(null);

const editorActive = computed(
  () => editOpen.value || quickCreate.value !== null || editingEvent.value !== null,
);

onMounted(async () => {
  await initialize();
});

watch(error, (value) => {
  if (value) pushToast(value, "error");
});

watch(anchor, () => {
  miniOpen.value = false;
});

// —— 快捷键：t 今天 / d w m 切换视图 / n 新建 / ← → 翻页 ——
function whenIdle(handler: () => void): () => void {
  return () => {
    if (!editorActive.value) handler();
  };
}

defineShortcuts({
  t: whenIdle(goToday),
  d: whenIdle(() => setView("day")),
  w: whenIdle(() => setView("week")),
  m: whenIdle(() => setView("month")),
  n: whenIdle(() => openCreateAtAnchor()),
  arrowleft: whenIdle(() => navigate(-1)),
  arrowright: whenIdle(() => navigate(1)),
});

function openCreate(date?: string, time?: string, estimate?: number | null, status: TodoStatus = "open"): void {
  editingTask.value = null;
  defaultDate.value = date ?? null;
  defaultTime.value = time ?? null;
  defaultEstimate.value = estimate ?? null;
  defaultStatus.value = status;
  editOpen.value = true;
}

function openCreateAtAnchor(): void {
  const now = new Date();
  const hour = Math.min(23, now.getHours() + 1);
  openCreate(anchor.value, `${String(hour).padStart(2, "0")}:00`, 60);
}

function openEdit(task: TodoTask): void {
  editingTask.value = task;
  defaultDate.value = null;
  defaultTime.value = null;
  defaultEstimate.value = null;
  defaultStatus.value = task.status;
  editOpen.value = true;
}

function onQuickOpen(payload: { date: string; time?: string; x: number; y: number }): void {
  if (quickCreate.value?.date === payload.date && quickCreate.value.x === payload.x) {
    quickCreate.value = null;
    return;
  }
  quickCreate.value = payload;
}

async function onQuickCreate(payload: { title: string; time: string; category: TodoCategory }): Promise<void> {
  const target = quickCreate.value;
  quickCreate.value = null;
  if (!target) return;
  const task = await createTask(buildQuickTaskDraft({ ...payload, date: target.date }));
  if (task) pushToast(`已在 ${target.date} 创建待办`);
}

function onQuickFull(payload: { date: string; time: string }): void {
  quickCreate.value = null;
  openCreate(payload.date, payload.time || "09:00", null);
}

async function onQuickCreateText(text: string): Promise<void> {
  const parsed = parseQuickEvent(text);
  if (!parsed) return;
  const date = parsed.date ?? anchor.value;
  const task = await createTask({
    title: parsed.title,
    priority: "medium",
    category: "work",
    plannedDate: date,
    plannedTime: parsed.allDay ? null : parsed.time ?? "09:00",
    estimateMinutes: parsed.allDay ? null : 60,
    sourceType: "manual",
  });
  if (task) pushToast(`已创建「${parsed.title}」· ${date}`);
}

async function onMoveEvent(payload: { id: string; startAt: string; endAt?: string }): Promise<void> {
  await moveEvent({ id: payload.id, startAt: payload.startAt, endAt: payload.endAt });
}

// 拖画时间块 → 完整表单（预填时间 + 估算时长）。
function onCreateEvent(payload: { date: string; time: string; endTime?: string }): void {
  const estimate = payload.endTime
    ? durationMinutesBetween(`${payload.date} ${payload.time}`, `${payload.date} ${payload.endTime}`)
    : null;
  openCreate(payload.date, payload.time || "09:00", estimate ?? 60);
}

function openDay(date: string): void {
  setAnchor(date);
  setView("day");
}

async function selectEvent(event: CalendarEvent): Promise<void> {
  editingEvent.value = event;
  detailTask.value = null;
  eventAnchorEl.value = document.querySelector<HTMLElement>(`[data-event-id="${event.id}"]`);
  if (event.taskId) {
    detailTask.value = await loadTask(event.taskId);
  }
}

function closeEventDetail(): void {
  editingEvent.value = null;
  detailTask.value = null;
  eventAnchorEl.value = null;
}

function onDetailPopoverOpen(value: boolean): void {
  if (!value) closeEventDetail();
}

/** 详情浮层里改计划日期/时间：写回待办，后端同步移动日历事件。 */
async function onPatchPlanned(payload: { task: TodoTask; date: string; time: string }): Promise<void> {
  const task = payload.task;
  const draft: TodoDraft = {
    id: task.id,
    title: task.title,
    description: task.description,
    status: task.status,
    priority: task.priority,
    category: task.category,
    plannedDate: payload.date || null,
    plannedTime: payload.time || null,
    estimateMinutes: task.estimateMinutes,
    sourceType: task.sourceType,
    sourceName: task.sourceName,
    acceptanceCriteria: task.acceptanceCriteria.map((item) => item.content),
    dependencyIds: task.dependencyIds,
  };
  const updated = await createTask(draft);
  if (updated) detailTask.value = updated;
}

function onEditFromDetail(task: TodoTask): void {
  closeEventDetail();
  openEdit(task);
}

async function onRemoveFromDetail(payload: { event: CalendarEvent; task: TodoTask | null }): Promise<void> {
  closeEventDetail();
  if (payload.task) {
    const ok = await deleteTask(payload.task.id);
    if (ok) pushToast("待办已删除");
    return;
  }
  if (await deleteEvent(payload.event.id)) pushToast("日历事件已删除");
}
</script>

<template>
  <UDashboardPanel id="calendar" :ui="{ body: 'p-0 sm:p-0 gap-0 overflow-hidden' }">
    <template #header>
      <UDashboardNavbar
        title="日历"
        description="待办的时间线：单击空白快速创建，拖画时间块精确安排"
      >
        <template #leading>
          <UDashboardSidebarCollapse />
        </template>

        <template #right>
          <div class="header-bar">
            <CalendarToolbar />
            <div class="header-actions">
              <UPopover v-model:open="miniOpen" :content="{ align: 'end' }">
                <UButton icon="i-lucide-calendar-days" color="neutral" variant="outline" size="sm" square aria-label="迷你日历" />
                <template #content>
                  <div class="mini-wrap"><MiniCalendar /></div>
                </template>
              </UPopover>
              <UButton
                icon="i-lucide-file-scan"
                color="neutral"
                variant="outline"
                size="sm"
                class="whitespace-nowrap"
                @click="router.push('/calendar/decompose')"
              >
                文档拆解
              </UButton>
              <NewTaskMenu @full="openCreateAtAnchor" @quick="onQuickCreateText" />
            </div>
          </div>
        </template>
      </UDashboardNavbar>
    </template>

    <template #body>
      <div class="calendar-page">
        <section class="calendar-pane">
          <div class="calendar-subbar">
            <CategoryFilter />
            <span class="shortcut-hint">
              <UKbd value="t" variant="soft" /> 今天
              <UKbd value="d" variant="soft" /> 日
              <UKbd value="w" variant="soft" /> 周
              <UKbd value="m" variant="soft" /> 月
              <UKbd value="n" variant="soft" /> 新建
            </span>
          </div>

          <div class="calendar-body">
            <Transition name="view-fade" mode="out-in">
              <MonthCalendar
                v-if="view === 'month'"
                key="month"
                @quick-create="onQuickOpen"
                @select-event="selectEvent"
                @move-event="onMoveEvent"
              />
              <WeekCalendar
                v-else-if="view === 'week'"
                key="week"
                @quick-create="onQuickOpen"
                @select-event="selectEvent"
                @move-event="onMoveEvent"
                @create-event="onCreateEvent"
                @open-day="openDay"
              />
              <DayCalendar
                v-else
                key="day"
                @quick-create="onQuickOpen"
                @select-event="selectEvent"
                @move-event="onMoveEvent"
                @create-event="onCreateEvent"
              />
            </Transition>
          </div>
        </section>

        <QuickTaskMenu
          v-if="quickCreate"
          :date="quickCreate.date"
          :time="quickCreate.time"
          :x="quickCreate.x"
          :y="quickCreate.y"
          @create="onQuickCreate"
          @full="onQuickFull"
          @close="quickCreate = null"
        />

        <TodoEditModal
          v-model:open="editOpen"
          :task="editingTask"
          :default-date="defaultDate"
          :default-time="defaultTime"
          :default-estimate="defaultEstimate"
          :default-status="defaultStatus"
        />

        <UPopover
          :open="editingEvent !== null"
          :reference="eventAnchorEl ?? undefined"
          :content="{ side: 'right', align: 'start' }"
          @update:open="onDetailPopoverOpen"
        >
          <template #content>
            <TodoDetailPopover
              v-if="editingEvent"
              :key="editingEvent.id"
              :event="editingEvent"
              :task="detailTask"
              @updated="detailTask = $event"
              @close="closeEventDetail"
              @patch-planned="onPatchPlanned"
              @edit="onEditFromDetail"
              @remove="onRemoveFromDetail"
            />
          </template>
        </UPopover>
      </div>
    </template>
  </UDashboardPanel>
</template>

<style scoped>
.header-bar {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 8px 12px;
  flex: 1 1 auto;
  min-width: 0;
}
.header-bar :deep(.calendar-toolbar) {
  flex: 1 1 auto;
  justify-content: flex-start;
}
.header-actions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
  flex: none;
}
.calendar-page {
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  min-width: 0;
  min-height: 0;
  height: 100%;
  background: var(--bg);
}
.calendar-pane {
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
  gap: 10px;
  min-width: 0;
  min-height: 0;
  padding: 12px 14px 16px;
}
.calendar-subbar {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
  flex-wrap: wrap;
}
.shortcut-hint {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  margin-left: auto;
  color: var(--text-subtle);
  font-size: 10.5px;
  white-space: nowrap;
}
.calendar-body {
  position: relative;
  min-width: 0;
  min-height: 0;
}
.mini-wrap {
  padding: 4px;
}
.view-fade-enter-active,
.view-fade-leave-active {
  transition: opacity 0.16s ease, transform 0.16s ease;
}
.view-fade-enter-from {
  opacity: 0;
  transform: translateX(10px);
}
.view-fade-leave-to {
  opacity: 0;
  transform: translateX(-10px);
}
@media (max-width: 1180px) {
  .shortcut-hint {
    display: none;
  }
}
</style>
