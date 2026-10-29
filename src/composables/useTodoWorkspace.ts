import { reactive } from "vue";
import type { TodoWorkView, TodoSort } from "@/utils/todo-workspace";

// 保留本次会话的浏览条件，从完整详情返回时继续原来的工作列表。
const filters = reactive({
  zentaoView: "pending" as TodoWorkView,
  manualView: "all" as TodoWorkView,
  sort: "suggested" as TodoSort,
  query: "",
  type: "all",
  priority: "all",
  category: "all",
  start: "",
  end: "",
});

export function useTodoWorkspace() {
  return filters;
}
