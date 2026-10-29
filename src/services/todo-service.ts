import { invoke } from "@tauri-apps/api/core";
import { isTauriDesktop } from "@/connectors/types";
import type { TodoDraft, TodoFilter, TodoStatus, TodoTask } from "@/types/productivity";

function requireDesktop(): void {
  if (!isTauriDesktop()) throw new Error("待办功能需要在 Tauri 桌面应用中使用。");
}

export const todoService = {
  list(filter: TodoFilter = {}): Promise<TodoTask[]> {
    requireDesktop();
    return invoke<TodoTask[]>("todo_list", { filter });
  },

  get(id: string): Promise<TodoTask> {
    requireDesktop();
    return invoke<TodoTask>("todo_get", { id });
  },

  save(draft: TodoDraft): Promise<TodoTask> {
    requireDesktop();
    return invoke<TodoTask>("todo_save", { draft });
  },

  setStatus(id: string, status: TodoStatus): Promise<TodoTask> {
    requireDesktop();
    return invoke<TodoTask>("todo_set_status", { id, status });
  },

  delete(id: string): Promise<void> {
    requireDesktop();
    return invoke<void>("todo_delete", { id });
  },
};
