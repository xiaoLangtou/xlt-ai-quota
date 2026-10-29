<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useCalendarTodo } from "@/composables/useCalendarTodo";
import {
  CATEGORY_LABELS,
  PRIORITY_LABELS,
  type TodoPriority,
  type TodoStatus,
  type TodoTask,
} from "@/types/productivity";
import { CATEGORY_DOT_BG, CATEGORY_TEXT, externalStatusChip, formatEstimate } from "@/utils/productivity";
import { openExternal } from "@/services/zentao-service";

const props = defineProps<{ tasks: TodoTask[]; sourceTab: "zentao" | "manual" }>();

const emit = defineEmits<{
  (event: "edit-task", payload: TodoTask): void;
  (event: "delete-task", payload: TodoTask): void;
  (event: "create-task", payload: TodoStatus): void;
}>();

const { refreshAll, setTaskStatus } = useCalendarTodo();

type BadgeColor = "primary" | "secondary" | "success" | "info" | "warning" | "error" | "neutral";

const COLUMNS: { status: TodoStatus; label: string; swatch: string }[] = [
  { status: "open", label: "未开始", swatch: "bg-accented" },
  { status: "doing", label: "进行中", swatch: "bg-warning" },
  { status: "done", label: "已完成", swatch: "bg-success" },
];

const PRIORITY_COLOR: Record<TodoPriority, BadgeColor> = {
  high: "error",
  medium: "warning",
  low: "neutral",
};

const columns = computed(() =>
  COLUMNS.map((column) => ({
    ...column,
    tasks: props.tasks.filter((task) => task.status === column.status),
  })),
);

// 拖动用鼠标事件 + 跟随光标的浮层：WKWebView 会取消 pointer 拖拽手势，鼠标事件更稳。
interface DragState {
  task: TodoTask;
  from: TodoStatus;
  startX: number;
  startY: number;
  x: number;
  y: number;
  moved: boolean;
  over: TodoStatus;
}

const drag = ref<DragState | null>(null);

function columnAt(x: number, y: number): TodoStatus | null {
  for (const element of document.elementsFromPoint(x, y)) {
    const column = (element as HTMLElement).closest?.("[data-column-status]") as HTMLElement | null;
    const status = column?.dataset.columnStatus as TodoStatus | undefined;
    if (status) return status;
  }
  return null;
}

function onCardMouseDown(task: TodoTask, event: MouseEvent): void {
  if (event.button !== 0) return;
  if ((event.target as HTMLElement).closest("[data-no-drag]")) return;
  if (task.sourceType === "zentao") {
    emit("edit-task", task);
    return;
  }
  drag.value = {
    task,
    from: task.status,
    startX: event.clientX,
    startY: event.clientY,
    x: event.clientX,
    y: event.clientY,
    moved: false,
    over: task.status,
  };
  document.addEventListener("mousemove", onMouseMove);
  document.addEventListener("mouseup", onMouseUp);
  event.preventDefault();
}

function onMouseMove(event: MouseEvent): void {
  const state = drag.value;
  if (!state) return;
  if (!state.moved) {
    if (Math.hypot(event.clientX - state.startX, event.clientY - state.startY) < 5) return;
    state.moved = true;
  }
  state.x = event.clientX;
  state.y = event.clientY;
  state.over = columnAt(event.clientX, event.clientY) ?? state.over;
}

function onMouseUp(): void {
  document.removeEventListener("mousemove", onMouseMove);
  document.removeEventListener("mouseup", onMouseUp);
  const state = drag.value;
  drag.value = null;
  if (!state) return;
  if (!state.moved) {
    emit("edit-task", state.task);
    return;
  }
  if (state.over !== state.from) void setTaskStatus(state.task.id, state.over);
}

onMounted(() => void refreshAll());
</script>

<template>
  <section class="grid h-full min-h-0 grid-rows-[minmax(0,1fr)] bg-default" :class="{ 'select-none': drag?.moved }">
    <div class="flex h-full min-h-0 gap-3 overflow-x-auto overflow-y-hidden p-3.5">
      <div
        v-for="column in columns"
        :key="column.status"
        class="flex h-full max-h-full min-h-0 min-w-72 flex-1 flex-col gap-2 rounded-xl bg-elevated p-2.5 ring ring-default transition-colors"
        :class="{ 'ring-primary bg-primary/5': drag?.moved && drag.over === column.status }"
        :data-column-status="column.status"
      >
        <div class="flex items-center gap-2 border-b border-dashed border-default pb-2">
          <span class="size-2 rounded-full" :class="column.swatch" />
          <span class="text-xs font-bold text-highlighted">{{ column.label }}</span>
          <UBadge class="ml-auto font-mono" color="neutral" variant="subtle" size="sm">{{ column.tasks.length }}</UBadge>
        </div>

        <div class="board-col-body flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto">
          <article
            v-for="task in column.tasks"
            :key="task.id"
            class="group flex touch-none flex-col gap-2 rounded-xl bg-default p-3 ring ring-default transition select-none hover:ring-accented"
            :class="[task.sourceType === 'zentao' ? 'cursor-pointer' : 'cursor-grab', { 'opacity-40': drag?.moved && drag.task.id === task.id }]"
            @mousedown="onCardMouseDown(task, $event)"
          >
            <div class="flex items-center gap-1.5">
              <span class="inline-flex items-center gap-1.5 text-[10px] font-semibold" :class="CATEGORY_TEXT[task.category]">
                <span class="size-1.5 rounded-full" :class="CATEGORY_DOT_BG[task.category]" />
                {{ CATEGORY_LABELS[task.category] }}
              </span>
              <UBadge class="ml-auto" :color="PRIORITY_COLOR[task.priority]" variant="subtle" size="sm">
                {{ PRIORITY_LABELS[task.priority] }}
              </UBadge>
              <UButton
                v-if="task.sourceType !== 'zentao'"
                icon="i-lucide-trash-2"
                color="error"
                variant="ghost"
                size="xs"
                square
                data-no-drag
                title="删除"
                class="opacity-0 transition group-hover:opacity-100 focus-visible:opacity-100"
                @mousedown.stop
                @click.stop="emit('delete-task', task)"
              />
            </div>
            <p class="line-clamp-3 text-xs leading-5 font-semibold break-words" :class="task.status === 'done' ? 'text-dimmed line-through decoration-1' : 'text-highlighted'">
              {{ task.title }}
            </p>
            <div class="flex flex-wrap items-center gap-x-2 gap-y-1 text-[10px] text-dimmed">
              <span v-if="task.plannedDate" class="inline-flex items-center gap-1">
                <UIcon name="i-lucide-calendar-days" class="size-3" />{{ task.plannedDate }}
              </span>
              <span v-if="task.plannedTime">{{ task.plannedTime }}</span>
              <span v-if="task.estimateMinutes" class="inline-flex items-center gap-1">
                <UIcon name="i-lucide-timer" class="size-3" />{{ formatEstimate(task.estimateMinutes) }}
              </span>
              <UBadge v-if="task.sourceType === 'document'" color="primary" variant="subtle" size="sm">文档</UBadge>
              <template v-if="task.sourceType === 'zentao'">
                <UBadge
                  v-if="externalStatusChip(task.externalStatus)"
                  :color="externalStatusChip(task.externalStatus)!.color"
                  variant="subtle"
                  size="sm"
                >
                  {{ externalStatusChip(task.externalStatus)!.label }}
                </UBadge>
                <UBadge v-if="task.externalTransferred" color="warning" variant="subtle" size="sm">已转交 · {{ task.externalAssignedTo || '未指派' }}</UBadge>
                <UBadge color="info" variant="soft" size="sm">禅道</UBadge>
                <UButton
                  v-if="task.externalUrl"
                  icon="i-lucide-external-link"
                  color="neutral"
                  variant="ghost"
                  size="xs"
                  square
                  title="在禅道中打开"
                  data-no-drag
                  @mousedown.stop
                  @click.stop="openExternal(task.externalUrl).catch(() => {})"
                />
              </template>
            </div>
          </article>

          <p v-if="!column.tasks.length" class="py-8 text-center text-xs text-dimmed">暂无待办</p>
        </div>

        <UButton
          v-if="sourceTab === 'manual'"
          block
          color="neutral"
          variant="outline"
          size="sm"
          icon="i-lucide-plus"
          @click="emit('create-task', column.status)"
        >
          新建卡片
        </UButton>
      </div>
    </div>

    <Teleport to="body">
      <div
        v-if="drag?.moved"
        class="drag-ghost"
        :style="{ left: `${drag.x}px`, top: `${drag.y}px` }"
      >
        <span class="size-1.5 shrink-0 rounded-full" :class="CATEGORY_DOT_BG[drag.task.category]" />
        <span class="min-w-0 truncate">{{ drag.task.title }}</span>
      </div>
    </Teleport>
  </section>
</template>

<style scoped>
.board-col-body {
  scrollbar-width: thin;
  scrollbar-color: var(--scroll-thumb) transparent;
}
.board-col-body::-webkit-scrollbar {
  width: 8px;
}
.board-col-body::-webkit-scrollbar-thumb {
  border-radius: 4px;
  background: var(--scroll-thumb);
}
.board-col-body::-webkit-scrollbar-thumb:hover {
  background: var(--scroll-thumb-hover);
}
.board-col-body::-webkit-scrollbar-track {
  background: transparent;
}
</style>
