<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { useElementSize, useEventListener, useLocalStorage } from "@vueuse/core";
import { useRouter } from "vue-router";
import AllTodoPanel from "@/components/calendar/AllTodoPanel.vue";
import TodoDetailDrawer from "@/components/calendar/TodoDetailDrawer.vue";
import TodoEditModal from "@/components/calendar/TodoEditModal.vue";
import ZentaoWorkDetail from "@/components/calendar/ZentaoWorkDetail.vue";
import ZentaoSyncResultModal from "@/components/calendar/ZentaoSyncResultModal.vue";
import { useCalendarTodo } from "@/composables/useCalendarTodo";
import { useZentaoSync } from "@/composables/useZentaoSync";
import { pushToast } from "@/composables/useToast";
import { nextPendingTaskId } from "@/utils/todo-workspace";
import { isTauriDesktop } from "@/connectors/types";
import type { TodoStatus, TodoTask, ZentaoSyncResult } from "@/types/productivity";

const { allTasks, allFilters, initialize, deleteTask, findTask } = useCalendarTodo();
const { configured, syncing, lastResult, lastError, refreshError, refreshStatus, syncOnce, ensurePolling } = useZentaoSync();
const router = useRouter();
const sourceTab = ref<"zentao" | "manual">("zentao");
const sourceTabs = [
  { label: "禅道", value: "zentao", icon: "i-lucide-refresh-cw" },
  { label: "本地待办", value: "manual", icon: "i-lucide-list-todo" },
];
const sourceTasks = computed(() => allTasks.value.filter((task) => sourceTab.value === "zentao" ? task.sourceType === "zentao" : task.sourceType !== "zentao"));
const selectedId = ref<string | null>(null);
const selectedTask = computed(() => sourceTasks.value.find((task) => task.id === selectedId.value) ?? null);
const workspace = ref<HTMLElement | null>(null);
const { width: workspaceWidth } = useElementSize(workspace);
// 按扣除侧边栏后的可用宽度决定布局，确保列表与详情都有足够阅读空间。
const focusedDetail = computed(() => Boolean(selectedTask.value) && workspaceWidth.value < 1120);
const splitView = computed(() => sourceTab.value === "zentao" && Boolean(selectedTask.value) && !focusedDetail.value);
const savedListWidth = useLocalStorage("xlt.todo.list-width", 0);
const draftListWidth = ref<number | null>(null);
const minListWidth = 320;
const maxListWidth = computed(() => Math.max(minListWidth, workspaceWidth.value - 480 - 8));
function constrainListWidth(width: number): number {
  return Math.min(maxListWidth.value, Math.max(minListWidth, width));
}
const listWidth = computed(() => constrainListWidth(draftListWidth.value ?? (
  savedListWidth.value > 0 ? savedListWidth.value : Math.min(480, Math.max(380, workspaceWidth.value * 0.34))
)));
let resizeOrigin: { x: number; width: number } | null = null;
const resizing = ref(false);
function startResize(event: MouseEvent): void {
  if (event.button !== 0 || !splitView.value) return;
  event.preventDefault();
  resizeOrigin = { x: event.clientX, width: listWidth.value };
  draftListWidth.value = listWidth.value;
  resizing.value = true;
}
function finishResize(): void {
  if (resizing.value && draftListWidth.value !== null) savedListWidth.value = constrainListWidth(draftListWidth.value);
  resizeOrigin = null;
  draftListWidth.value = null;
  resizing.value = false;
}
function resizeWithKeyboard(event: KeyboardEvent): void {
  const step = event.shiftKey ? 48 : 16;
  let width: number;
  switch (event.key) {
    case "ArrowLeft": width = listWidth.value - step; break;
    case "ArrowRight": width = listWidth.value + step; break;
    case "Home": width = minListWidth; break;
    case "End": width = maxListWidth.value; break;
    default: return;
  }
  event.preventDefault();
  savedListWidth.value = constrainListWidth(width);
}
// WKWebView 内使用鼠标拖动；监听随组件卸载自动清理。
useEventListener("mousemove", (event: MouseEvent) => {
  if (resizeOrigin) draftListWidth.value = constrainListWidth(resizeOrigin.width + event.clientX - resizeOrigin.x);
});
useEventListener("mouseup", finishResize);
useEventListener("blur", finishResize);
watch(splitView, (visible) => { if (!visible) finishResize(); });
const hasZentaoNotice = computed(() => sourceTab.value === "zentao" && Boolean(
  lastError.value || refreshError.value || lastResult.value?.skipped || !configured.value.configured,
));
const visibleTasks = ref<TodoTask[]>([]);
const selectedIndex = computed(() => visibleTasks.value.findIndex((task) => task.id === selectedId.value));
const detailOpen = ref(false);
const detailTask = ref<TodoTask | null>(null);
const editOpen = ref(false);
const editingTask = ref<TodoTask | null>(null);
const defaultStatus = ref<TodoStatus>("open");
const syncDetailOpen = ref(false);
const syncDetail = ref<ZentaoSyncResult | null>(null);
const syncLabel = computed(() => {
  if (syncing.value) return "正在同步禅道…";
  if (!lastResult.value) return "尚未同步";
  return `上次同步 ${new Date(lastResult.value.syncedAt).toLocaleString("zh-CN", { month: "numeric", day: "numeric", hour: "2-digit", minute: "2-digit" })}`;
});
watch(sourceTab, () => { selectedId.value = null; detailOpen.value = false; detailTask.value = null; });
onMounted(async () => {
  // 页面统一读取完整任务库，工作视图、列表和看板使用同一组本地筛选。
  for (const key of Object.keys(allFilters)) delete allFilters[key as keyof typeof allFilters];
  await initialize();
  if (!isTauriDesktop()) return;
  await refreshStatus();
  if (configured.value.configured) {
    ensurePolling();
    if (!lastResult.value) await syncOnce(true);
  }
});
async function syncZentao(): Promise<void> {
  try { await syncOnce(false); }
  catch (reason) { pushToast(reason instanceof Error ? reason.message : String(reason), "error"); }
}
function openCreate(status: TodoStatus = "open"): void {
  editingTask.value = null; defaultStatus.value = status; editOpen.value = true;
}
function openDetail(task: TodoTask): void {
  if (task.sourceType === "zentao") { selectedId.value = task.id; return; }
  detailTask.value = task; detailOpen.value = true;
}
function openEdit(task: TodoTask): void {
  detailOpen.value = false; editingTask.value = task; defaultStatus.value = task.status; editOpen.value = true;
}
function navigate(direction: number): void {
  const next = visibleTasks.value[selectedIndex.value + direction];
  if (next) selectedId.value = next.id;
}
function updated(task: TodoTask): void {
  const index = allTasks.value.findIndex((item) => item.id === task.id);
  if (index >= 0) allTasks.value[index] = task;
  if (detailTask.value?.id === task.id) detailTask.value = task;
}
let handlingOrder: { id: string; order: string[] } | null = null;
function submitting(task: TodoTask): void {
  handlingOrder = { id: task.id, order: visibleTasks.value.map(item => item.id) };
}
async function handled(task: TodoTask): Promise<void> {
  const current = selectedId.value;
  const order = handlingOrder?.id === task.id ? handlingOrder.order : [];
  handlingOrder = null;
  updated(task);
  await nextTick();
  if (selectedId.value !== current || current !== task.id) return;
  selectedId.value = nextPendingTaskId(order, task.id, visibleTasks.value);
  if (!selectedId.value) pushToast("本轮可见事项已处理完成");
}
function onSaved(): void {
  if (editingTask.value) {
    const task = findTask(editingTask.value.id);
    if (task) updated(task);
  }
}
async function confirmDelete(task: TodoTask): Promise<void> {
  if (task.sourceType === "zentao") return;
  if (!window.confirm(`删除「${task.title}」及其关联日历事件？`)) return;
  if (await deleteTask(task.id)) pushToast("待办已删除");
}
</script>

<template>
  <UDashboardPanel id="todos" :ui="{ body: 'p-0 sm:p-0 gap-0 sm:gap-0 overflow-hidden' }">
    <template #header>
      <UDashboardNavbar title="待办" description="安排今天的工作，跟进每一项处理结果">
        <template #leading><UDashboardSidebarCollapse /></template>
        <template #right>
          <template v-if="sourceTab === 'zentao'">
            <span class="hidden text-xs text-muted sm:inline">{{ syncLabel }}</span>
            <UButton v-if="configured.configured" icon="i-lucide-refresh-cw" size="sm" variant="outline" color="neutral" :loading="syncing" :disabled="syncing" @click="syncZentao">同步禅道</UButton>
            <UButton v-if="lastResult" icon="i-lucide-list" size="sm" variant="ghost" color="neutral" @click="syncDetail = lastResult; syncDetailOpen = true">同步明细</UButton>
          </template>
          <UButton v-else icon="i-lucide-plus" size="sm" @click="openCreate()">新建待办</UButton>
        </template>
      </UDashboardNavbar>
    </template>
    <template #body>
      <div class="shrink-0 border-b border-default px-4">
        <UTabs v-model="sourceTab" :items="sourceTabs" :content="false" variant="link" class="w-fit" />
      </div>
      <div v-if="hasZentaoNotice" class="shrink-0">
        <UAlert v-if="lastError || refreshError" color="warning" variant="subtle" :title="lastError ? '禅道同步未完成' : '禅道已同步，页面刷新失败'" :description="lastError || refreshError" class="rounded-none" />
        <UAlert v-else-if="lastResult?.skipped" color="warning" variant="subtle" title="部分范围未同步" :description="lastResult.skipped" class="rounded-none" />
        <div v-if="!configured.configured" class="flex flex-wrap items-center gap-3 border-b border-default bg-elevated/40 px-4 py-3 text-sm">
          <UIcon name="i-lucide-plug" class="size-4 text-muted" />
          <span class="text-muted">连接禅道后，在这里查看和处理指派给你的任务与 Bug。</span>
          <UButton size="xs" color="neutral" variant="outline" @click="router.push({ name: 'settings' })">前往设置</UButton>
        </div>
      </div>
      <div ref="workspace" class="todo-workspace min-h-0 flex-1" :style="{ '--todo-list-width': `${listWidth}px` }" :class="{ 'has-detail': selectedTask && sourceTab === 'zentao', 'detail-focused': focusedDetail, 'is-resizing': resizing }">
        <AllTodoPanel id="todo-task-list" class="task-list min-h-0 min-w-0" :tasks="sourceTasks" :source-tab="sourceTab" :selected-id="selectedId ?? undefined"
          @edit-task="openDetail" @delete-task="confirmDelete" @create-task="openCreate" @updated="updated" @visible-tasks="visibleTasks = $event" />
        <div v-if="splitView" class="detail-resize-handle" role="separator" tabindex="0" aria-label="调整待办列表与详情宽度" aria-orientation="vertical"
          aria-controls="todo-task-list todo-task-detail" :aria-valuemin="minListWidth" :aria-valuemax="Math.round(maxListWidth)" :aria-valuenow="Math.round(listWidth)"
          title="拖动调整宽度，双击恢复默认；方向键微调"
          @mousedown="startResize" @keydown="resizeWithKeyboard" @dblclick="savedListWidth = 0" />
        <ZentaoWorkDetail v-if="sourceTab === 'zentao' && selectedTask" id="todo-task-detail" class="task-detail min-h-0 min-w-0 border-l border-default"
          :task="selectedTask" :focused="focusedDetail" :previous="selectedIndex > 0" :next="selectedIndex >= 0 && selectedIndex < visibleTasks.length - 1" :sync-version="lastResult?.syncedAt"
          @submitting="submitting" @handled="handled" @close="selectedId = null" @navigate="navigate" @updated="updated" @schedule="openEdit" />
      </div>
      <TodoDetailDrawer v-model:open="detailOpen" :task="detailTask" @edit="openEdit" @updated="updated" />
      <TodoEditModal v-model:open="editOpen" :task="editingTask" :default-status="defaultStatus" @saved="onSaved" />
      <ZentaoSyncResultModal v-model:open="syncDetailOpen" :result="syncDetail" />
    </template>
  </UDashboardPanel>
</template>

<style scoped>
.todo-workspace {
  position: relative;
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  overflow: hidden;
}
.todo-workspace.has-detail:not(.detail-focused) {
  grid-template-columns: var(--todo-list-width) 8px minmax(0, 1fr);
}
.detail-resize-handle {
  position: relative;
  z-index: 41;
  cursor: col-resize;
  background: var(--surface-2);
  outline: none;
}
.detail-resize-handle::after {
  content: "";
  position: absolute;
  top: 50%;
  left: 3px;
  width: 2px;
  height: 32px;
  border-radius: 2px;
  background: var(--border-strong);
  transform: translateY(-50%);
  transition: background-color 0.15s;
}
.detail-resize-handle:hover::after,
.detail-resize-handle:focus-visible::after {
  background: var(--accent);
}
.is-resizing::after {
  content: "";
  position: absolute;
  inset: 0;
  z-index: 40;
  cursor: col-resize;
  user-select: none;
}
.detail-focused > .task-list {
  display: none;
}
.detail-focused > .task-detail {
  border-left: 0;
}
</style>
