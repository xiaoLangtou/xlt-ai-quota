import { computed, reactive, ref } from "vue";
import { calendarService } from "@/services/calendar-service";
import { todoService } from "@/services/todo-service";
import { shiftAnchor, today, viewRange } from "@/utils/productivity";
import type {
  CalendarEvent,
  CalendarViewMode,
  MoveEventInput,
  TodoCategory,
  TodoDraft,
  TodoFilter,
  TodoStatus,
  TodoTask,
} from "@/types/productivity";

const HIDDEN_CATEGORIES_KEY = "xlt.calendar.hidden-categories";
const TODO_VIEW_KEY = "xlt.todo.view";

export type TodoViewMode = "list" | "board";

function readTodoView(): TodoViewMode {
  try {
    return localStorage.getItem(TODO_VIEW_KEY) === "board" ? "board" : "list";
  } catch {
    return "list";
  }
}

function readHiddenCategories(): TodoCategory[] {
  try {
    const raw = localStorage.getItem(HIDDEN_CATEGORIES_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as unknown;
    return Array.isArray(parsed)
      ? parsed.filter((value): value is TodoCategory => value === "work" || value === "planning" || value === "personal")
      : [];
  } catch {
    return [];
  }
}

// 模块级单例状态：日历时间线与任务库共享同一份数据。
// 概念模型：日历是待办的时间可视化，不存在独立的“事件”编辑路径。
const view = ref<CalendarViewMode>("month");
const anchor = ref(today());
const events = ref<CalendarEvent[]>([]);
const allTasks = ref<TodoTask[]>([]);
const allFilters = reactive<TodoFilter>({});
const loading = ref(false);
const allLoading = ref(false);
const error = ref("");
const hiddenCategories = ref<TodoCategory[]>(readHiddenCategories());
const todoView = ref<TodoViewMode>(readTodoView());

function report(reason: unknown, fallback: string): void {
  error.value = reason instanceof Error ? reason.message : String(reason ?? fallback);
}

export function useCalendarTodo() {
  const range = computed(() => viewRange(view.value, anchor.value));

  // 隐藏某个分类后，日历不再渲染该分类的事件，待办列表不受影响。
  const visibleEvents = computed(() =>
    events.value.filter((event) => !hiddenCategories.value.includes(event.category)),
  );

  function toggleCategory(category: TodoCategory): void {
    hiddenCategories.value = hiddenCategories.value.includes(category)
      ? hiddenCategories.value.filter((value) => value !== category)
      : [...hiddenCategories.value, category];
    try {
      localStorage.setItem(HIDDEN_CATEGORIES_KEY, JSON.stringify(hiddenCategories.value));
    } catch {
      // 存储不可用时仅影响本次会话的可见性。
    }
  }

  async function refreshEvents(): Promise<void> {
    events.value = await calendarService.listRange(range.value.start, range.value.end);
  }

  async function refreshAll(): Promise<void> {
    allLoading.value = true;
    try {
      allTasks.value = await todoService.list(cleanFilters(allFilters));
    } finally {
      allLoading.value = false;
    }
  }

  function cleanFilters(filters: TodoFilter): TodoFilter {
    const result: TodoFilter = {};
    if (filters.status) result.status = filters.status;
    if (filters.priority) result.priority = filters.priority;
    if (filters.category) result.category = filters.category;
    if (filters.sourceType) result.sourceType = filters.sourceType;
    if (filters.query?.trim()) result.query = filters.query.trim();
    if (filters.start) result.start = filters.start;
    if (filters.end) result.end = filters.end;
    if (filters.scheduledOnly) result.scheduledOnly = true;
    return result;
  }

  async function refresh(): Promise<void> {
    error.value = "";
    loading.value = true;
    try {
      await refreshEvents();
      await refreshAll();
    } catch (reason) {
      report(reason, "无法读取本地日历与待办");
    } finally {
      loading.value = false;
    }
  }

  async function refreshAfterWrite(): Promise<void> {
    error.value = "";
    try {
      await refreshEvents();
      await refreshAll();
    } catch (reason) {
      report(reason, "刷新本地数据失败");
    }
  }

  function initialize(): Promise<void> {
    return refresh();
  }

  function setView(next: CalendarViewMode): void {
    if (view.value === next) return;
    view.value = next;
    void refreshEvents();
  }

  function setAnchor(date: string): void {
    anchor.value = date;
    void refreshEvents();
  }

  function goToday(): void {
    anchor.value = today();
    void refreshEvents();
  }

  function navigate(direction: number): void {
    anchor.value = shiftAnchor(view.value, anchor.value, direction);
    void refreshEvents();
  }

  /** 列表 / 看板两种形式，看板需要全部待办数据。 */
  function setTodoView(next: TodoViewMode): void {
    todoView.value = next;
    try {
      localStorage.setItem(TODO_VIEW_KEY, next);
    } catch {
      // 存储不可用时仅影响本次会话。
    }
    if (next === "board") void refreshAll();
  }

  /** 同时承担新建与更新：draft.id 为空则新建，否则覆盖保存。 */
  async function createTask(draft: TodoDraft): Promise<TodoTask | null> {
    error.value = "";
    try {
      const task = await todoService.save(draft);
      await refreshAfterWrite();
      return task;
    } catch (reason) {
      report(reason, "无法保存待办");
      return null;
    }
  }

  /** 先本地更新状态，再落库，失败回滚，保证勾选即时反馈。 */
  function applyStatusLocally(id: string, status: TodoStatus): void {
    const completedAt = status === "done" ? Date.now() : null;
    allTasks.value = allTasks.value.map((task) =>
      task.id === id ? { ...task, status, completedAt } : task,
    );
    events.value = events.value.map((event) => (event.taskId === id ? { ...event, status } : event));
  }

  async function toggleTask(task: TodoTask): Promise<void> {
    if (task.sourceType === "zentao") {
      report("请点开详情操作禅道状态", "无法更新禅道状态");
      return;
    }
    error.value = "";
    const next: TodoStatus = task.status === "done" ? "open" : "done";
    applyStatusLocally(task.id, next);
    try {
      await todoService.setStatus(task.id, next);
      await refreshAfterWrite();
    } catch (reason) {
      applyStatusLocally(task.id, task.status);
      report(reason, "无法更新待办状态");
    }
  }

  async function setTaskStatus(id: string, status: TodoStatus): Promise<void> {
    if (findTask(id)?.sourceType === "zentao") {
      report("请点开详情操作禅道状态", "无法更新禅道状态");
      return;
    }
    error.value = "";
    const previous = findTask(id)?.status;
    applyStatusLocally(id, status);
    try {
      await todoService.setStatus(id, status);
      await refreshAfterWrite();
    } catch (reason) {
      if (previous) applyStatusLocally(id, previous);
      report(reason, "无法更新待办状态");
    }
  }

  /** 在当前已加载的任务库中按 id 查找，用于日历事件联动编辑。 */
  function findTask(id: string): TodoTask | undefined {
    return allTasks.value.find((task) => task.id === id);
  }

  /** 按 id 读取单条待办（跨日期事件联动编辑时，当前列表可能还没加载它）。 */
  async function loadTask(id: string): Promise<TodoTask | null> {
    const cached = findTask(id);
    if (cached) return cached;
    try {
      return await todoService.get(id);
    } catch {
      return null;
    }
  }

  async function deleteTask(id: string): Promise<boolean> {
    error.value = "";
    try {
      await todoService.delete(id);
      await refreshAfterWrite();
      return true;
    } catch (reason) {
      report(reason, "无法删除待办");
      return false;
    }
  }

  /** 拖拽移动时间线上的事件：后端会同步回写关联待办的计划日期与时间。 */
  async function moveEvent(input: MoveEventInput): Promise<void> {
    error.value = "";
    try {
      await calendarService.move(input);
      await refreshAfterWrite();
    } catch (reason) {
      report(reason, "无法移动日历事件");
    }
  }

  /** 仅用于无关联待办的纯事件；关联待办的删除走 deleteTask 级联。 */
  async function deleteEvent(id: string): Promise<boolean> {
    error.value = "";
    try {
      await calendarService.delete(id);
      await refreshAfterWrite();
      return true;
    } catch (reason) {
      report(reason, "无法删除日历事件");
      return false;
    }
  }

  return {
    // state
    view,
    anchor,
    events,
    allTasks,
    allFilters,
    loading,
    allLoading,
    error,
    // derived
    range,
    visibleEvents,
    hiddenCategories,
    toggleCategory,
    todoView,
    // actions
    initialize,
    refresh,
    refreshAll,
    setView,
    setAnchor,
    goToday,
    navigate,
    setTodoView,
    createTask,
    toggleTask,
    setTaskStatus,
    findTask,
    loadTask,
    deleteTask,
    moveEvent,
    deleteEvent,
    clearError: () => {
      error.value = "";
    },
  };
}
