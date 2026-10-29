import type { TodoTask } from "@/types/productivity";

export type TodoWorkView = "pending" | "today" | "transferred" | "processed" | "all";

export function matchesWorkView(task: TodoTask, view: TodoWorkView, date: string): boolean {
  const active = task.status !== "done" && !task.externalTransferred;
  switch (view) {
    case "pending": return active;
    case "today": return active && task.plannedDate === date;
    case "transferred": return Boolean(task.externalTransferred);
    case "processed": return task.status === "done";
    case "all": return true;
  }
}

export type TodoSort = "suggested" | "updated";

export function zentaoCopyText(task: TodoTask): string {
  const id = task.externalId?.split(":").pop();
  if (!id || !/^\d+$/.test(id)) throw new Error("禅道编号无效");
  return `【${id}】${task.title.replace(/^【#\d+】\s*/, "")}：`;
}

export function nextPendingTaskId(order: string[], current: string, tasks: TodoTask[]): string | null {
  const position = order.indexOf(current);
  if (position < 0) return null;
  const pending = new Set(tasks.filter(task => task.status !== "done" && !task.externalTransferred).map(task => task.id));
  return order.slice(position + 1).find(id => pending.has(id)) ?? null;
}

export function attentionReasons(task: TodoTask, date: string): string[] {
  if (task.status === "done" || task.externalTransferred) return [];
  const reasons: string[] = [];
  const due = task.externalDue;
  const days = (value: string) => Math.floor((Date.parse(`${date}T00:00:00Z`) - Date.parse(`${value}T00:00:00Z`)) / 86400000);
  if (due && days(due) > 0) reasons.push(`逾期 ${days(due)} 天`);
  else if (!due && task.plannedDate && days(task.plannedDate) > 0) reasons.push(`计划已过期 ${days(task.plannedDate)} 天`);
  if (task.plannedDate === date) reasons.push("今日计划");
  if (task.priority === "high") reasons.push("高优先级");
  return reasons;
}

export function sortWorkTasks(tasks: TodoTask[], view: TodoWorkView, date?: string, sort: TodoSort = "suggested"): TodoTask[] {
  const priority = { high: 0, medium: 1, low: 2 };
  const group = (task: TodoTask) => {
    const reasons = date ? attentionReasons(task, date) : [];
    return reasons.some(reason => reason.includes("过期") || reason.startsWith("逾期")) ? 0
      : reasons.includes("今日计划") ? 1 : task.priority === "high" ? 2 : 3;
  };
  const updated = (task: TodoTask) => task.sourceType === "zentao" ? task.externalUpdatedDate ?? "" : String(task.updatedAt).padStart(16, "0");
  return [...tasks].sort((a, b) => {
    if (sort === "updated") return updated(b).localeCompare(updated(a)) || a.id.localeCompare(b.id);
    if (view === "processed") return (b.completedAt ?? b.updatedAt) - (a.completedAt ?? a.updatedAt);
    if (a.status === "done" && b.status !== "done") return 1;
    if (b.status === "done" && a.status !== "done") return -1;
    if (date && group(a) !== group(b)) return group(a) - group(b);
    if (date && priority[a.priority] !== priority[b.priority]) return priority[a.priority] - priority[b.priority];
    if (date && (a.externalDue || a.plannedDate) !== (b.externalDue || b.plannedDate)) {
      return (a.externalDue || a.plannedDate || "9999-12-31").localeCompare(b.externalDue || b.plannedDate || "9999-12-31");
    }
    if (a.plannedDate !== b.plannedDate) {
      if (!a.plannedDate) return 1;
      if (!b.plannedDate) return -1;
      return a.plannedDate.localeCompare(b.plannedDate);
    }
    return priority[a.priority] - priority[b.priority]
      || (a.plannedTime ?? "").localeCompare(b.plannedTime ?? "")
      || updated(b).localeCompare(updated(a))
      || a.createdAt - b.createdAt || a.id.localeCompare(b.id);
  });
}
