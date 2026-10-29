<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useNow } from "@vueuse/core";
import TodoTaskRow from "@/components/calendar/TodoTaskRow.vue";
import TodoBoardPanel from "@/components/calendar/TodoBoardPanel.vue";
import TodoViewSwitch from "@/components/calendar/TodoViewSwitch.vue";
import ZentaoBatchResolveModal from "@/components/calendar/ZentaoBatchResolveModal.vue";
import { DatePicker } from "@/components/ui/date-picker";
import { useCalendarTodo } from "@/composables/useCalendarTodo";
import { useTodoWorkspace } from "@/composables/useTodoWorkspace";
import { matchesWorkView, sortWorkTasks, type TodoWorkView } from "@/utils/todo-workspace";
import { CATEGORY_LABELS, PRIORITY_LABELS, TODO_CATEGORIES, TODO_PRIORITIES, type TodoTask, type TodoStatus, type ZentaoBatchResolveResult } from "@/types/productivity";

const props = defineProps<{ tasks: TodoTask[]; sourceTab: "zentao" | "manual"; selectedId?: string }>();
const emit = defineEmits<{
  (event: "edit-task", task: TodoTask): void;
  (event: "delete-task", task: TodoTask): void;
  (event: "create-task", status: TodoStatus): void;
  (event: "updated", task: TodoTask): void;
  (event: "visible-tasks", tasks: TodoTask[]): void;
}>();
const { allLoading, refresh, toggleTask, todoView } = useCalendarTodo();
const form = useTodoWorkspace();
const isZentao = computed(() => props.sourceTab === "zentao");
const now = useNow({ interval: 60_000 });
const today = computed(() => {
  const date = now.value;
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
});
const workView = computed({
  get: () => isZentao.value ? form.zentaoView : form.manualView,
  set: (value: TodoWorkView) => { if (isZentao.value) form.zentaoView = value; else form.manualView = value; },
});
const views = computed(() => [
  { value: "pending" as const, label: "待我处理", icon: "i-lucide-inbox" },
  { value: "today" as const, label: "今日计划", icon: "i-lucide-calendar-check" },
  ...(isZentao.value ? [{ value: "transferred" as const, label: "已转交", icon: "i-lucide-arrow-right-left" }] : []),
  { value: "processed" as const, label: isZentao.value ? "已处理" : "已完成", icon: "i-lucide-circle-check" },
  { value: "all" as const, label: "全部", icon: "i-lucide-list" },
].map((item) => ({ ...item, count: props.tasks.filter((task) => matchesWorkView(task, item.value, today.value)).length })));
const priorityOptions = [{ label: "全部优先级", value: "all" }, ...TODO_PRIORITIES.map((value) => ({ value, label: `${PRIORITY_LABELS[value]}优先级` }))];
const categoryOptions = [{ label: "全部分类", value: "all" }, ...TODO_CATEGORIES.map((value) => ({ value, label: CATEGORY_LABELS[value] }))];
const typeOptions = [{ label: "全部类型", value: "all" }, { label: "Bug", value: "bug" }, { label: "任务", value: "task" }];
const advanced = ref(false);
const hasFilter = computed(() => Boolean(form.query.trim() || form.priority !== "all" || (isZentao.value ? form.type !== "all" : form.category !== "all") || form.start || form.end));
const visibleTasks = computed(() => sortWorkTasks(props.tasks.filter((task) => {
  const query = form.query.trim().toLocaleLowerCase();
  return matchesWorkView(task, workView.value, today.value)
    && (!query || task.title.toLocaleLowerCase().includes(query))
    && (form.priority === "all" || task.priority === form.priority)
    && (!isZentao.value || form.type === "all" || task.externalType === form.type)
    && (isZentao.value || form.category === "all" || task.category === form.category)
    && (!form.start || Boolean(task.plannedDate && task.plannedDate >= form.start))
    && (!form.end || Boolean(task.plannedDate && task.plannedDate <= form.end));
}), workView.value, today.value, form.sort));
watch(visibleTasks, (tasks) => emit("visible-tasks", tasks), { immediate: true });
function resetFilters(): void {
  Object.assign(form, { query: "", type: "all", priority: "all", category: "all", start: "", end: "" });
}
const batchMode = ref(false);
const batchOpen = ref(false);
const selectedIds = ref<string[]>([]);
const selectedSet = computed(() => new Set(selectedIds.value));
function isBatchable(task: TodoTask): boolean {
  return task.sourceType === "zentao" && task.externalType === "bug" && task.externalStatus === "active" && !task.externalTransferred;
}
const bugCandidates = computed(() => visibleTasks.value.filter(isBatchable));
const selectedTasks = computed(() => bugCandidates.value.filter((task) => selectedSet.value.has(task.id)));
watch(bugCandidates, (tasks) => {
  const ids = new Set(tasks.map((task) => task.id));
  selectedIds.value = selectedIds.value.filter((id) => ids.has(id));
});
function exitBatch(): void { batchMode.value = false; batchOpen.value = false; selectedIds.value = []; }
function toggleSelect(task: TodoTask): void {
  selectedIds.value = selectedSet.value.has(task.id) ? selectedIds.value.filter((id) => id !== task.id) : [...selectedIds.value, task.id];
}
async function onBatchFinished(result: ZentaoBatchResolveResult): Promise<void> {
  await refresh();
  selectedIds.value = result.items.filter((item) => !item.ok).map((item) => item.taskId);
  if (!selectedIds.value.length) exitBatch();
}
watch(() => props.sourceTab, exitBatch);
watch(todoView, exitBatch);
</script>

<template>
  <section class="flex h-full min-h-0 flex-col bg-default">
    <header class="shrink-0 border-b border-default">
      <nav class="flex flex-nowrap gap-1 overflow-x-auto px-4 pt-2" aria-label="工作视图">
        <button v-for="item in views" :key="item.value" type="button"
          class="inline-flex shrink-0 items-center gap-2 whitespace-nowrap rounded-t-lg border-b-2 px-3 py-2.5 text-xs font-medium transition-colors"
          :class="workView === item.value ? 'border-primary text-primary bg-primary/5' : 'border-transparent text-muted hover:text-highlighted hover:bg-elevated'"
          :aria-current="workView === item.value ? 'page' : undefined" @click="workView = item.value">
          <UIcon :name="item.icon" class="size-3.5" />{{ item.label }}
          <span class="rounded px-1.5 py-0.5 text-[10px] tabular-nums" :class="workView === item.value ? 'bg-primary/10' : 'bg-elevated'">{{ item.count }}</span>
        </button>
      </nav>
      <div class="flex flex-wrap items-center gap-2 px-4 py-3">
        <UInput v-model="form.query" icon="i-lucide-search" :placeholder="isZentao ? '搜索编号或标题…' : '搜索待办标题…'" aria-label="搜索待办" class="min-w-40 flex-1" />
        <USelect v-if="isZentao" v-model="form.type" :items="typeOptions" aria-label="类型" class="w-28" />
        <USelect v-else v-model="form.category" :items="categoryOptions" aria-label="分类" class="w-32" />
        <USelect v-model="form.priority" :items="priorityOptions" aria-label="优先级" class="w-32" />
        <UButton icon="i-lucide-sliders-horizontal" color="neutral" :variant="advanced ? 'soft' : 'ghost'" :aria-expanded="advanced" aria-label="计划日期筛选" @click="advanced = !advanced" />
        <UButton v-if="hasFilter" color="neutral" variant="ghost" size="sm" @click="resetFilters">重置</UButton>
        <USelect v-model="form.sort" :items="[{ label: '建议处理顺序', value: 'suggested' }, { label: '最近更新', value: 'updated' }]" aria-label="排序" class="w-36" />
        <TodoViewSwitch />
      </div>
      <div v-if="advanced" class="flex flex-wrap items-center gap-2 border-t border-default px-4 py-2">
        <span class="text-xs text-muted">计划日期</span><DatePicker v-model="form.start" placeholder="开始日期" />
        <span class="text-xs text-muted">至</span><DatePicker v-model="form.end" placeholder="结束日期" />
      </div>
      <div class="flex min-h-10 flex-wrap items-center gap-2 border-t border-default bg-elevated/40 px-4 py-2 text-xs text-muted">
        <template v-if="batchMode">
          <span>已选 {{ selectedTasks.length }} 个 Bug</span>
          <UButton size="xs" variant="ghost" color="neutral" @click="selectedIds = bugCandidates.map(task => task.id)">全选当前结果</UButton>
          <UButton size="xs" variant="ghost" color="neutral" @click="selectedIds = []">清空</UButton>
          <UButton class="ml-auto" size="xs" color="neutral" variant="ghost" @click="exitBatch">退出选择</UButton>
          <UButton size="xs" :disabled="!selectedTasks.length" @click="batchOpen = true">解决选中</UButton>
        </template>
        <template v-else>
          <span>{{ views.find(item => item.value === workView)?.label }} · {{ visibleTasks.length }} 条</span>
          <span v-if="isZentao && workView === 'today'" class="text-dimmed">按本地计划日期展示</span>
          <UButton v-if="isZentao && todoView === 'list' && bugCandidates.length" class="ml-auto" size="xs" variant="ghost" color="neutral" icon="i-lucide-list-checks" @click="batchMode = true">批量解决</UButton>
        </template>
      </div>
    </header>
    <TodoBoardPanel v-show="todoView === 'board'" class="min-h-0 flex-1" :tasks="visibleTasks" :source-tab="sourceTab" @edit-task="emit('edit-task', $event)" @delete-task="emit('delete-task', $event)" @create-task="emit('create-task', $event)" />
    <div v-show="todoView !== 'board'" class="min-h-0 flex-1 overflow-y-auto">
      <div v-if="visibleTasks.length" class="divide-y divide-default">
        <TodoTaskRow v-for="task in visibleTasks" :key="task.id" :task="task" show-date
          :active="selectedId === task.id" :selectable="batchMode && isBatchable(task)" :selected="selectedSet.has(task.id)"
          :class="batchMode && !isBatchable(task) ? 'opacity-50' : ''"
          @toggle="toggleTask" @select="toggleSelect" @edit="emit('edit-task', $event)" @delete="emit('delete-task', $event)" @updated="emit('updated', $event)" />
      </div>
      <div v-else class="flex flex-col items-center gap-3 px-6 py-20 text-center">
        <UIcon :name="allLoading ? 'i-lucide-loader-circle' : 'i-lucide-inbox'" class="size-8 text-dimmed" :class="allLoading && 'animate-spin'" />
        <p class="text-sm font-medium text-highlighted">{{ allLoading ? '正在读取待办…' : hasFilter ? '没有匹配的条目' : workView === 'today' ? '今天还没有安排任务' : workView === 'pending' ? '当前没有待处理条目' : '当前视图暂无条目' }}</p>
        <p class="max-w-xs text-xs leading-5 text-muted">{{ hasFilter ? '调整搜索词或筛选条件后再查看。' : isZentao ? '同步禅道获取最新条目，或切换工作视图查看其他记录。' : '新建待办，安排接下来要做的事。' }}</p>
        <UButton v-if="hasFilter" color="neutral" variant="outline" size="sm" @click="resetFilters">清除筛选</UButton>
      </div>
    </div>
    <ZentaoBatchResolveModal v-model:open="batchOpen" :tasks="selectedTasks" @finished="onBatchFinished" />
  </section>
</template>
