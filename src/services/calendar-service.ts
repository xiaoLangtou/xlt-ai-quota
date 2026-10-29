import { invoke } from "@tauri-apps/api/core";
import { isTauriDesktop } from "@/connectors/types";
import type { CalendarEvent, CalendarEventDraft, MoveEventInput } from "@/types/productivity";

function requireDesktop(): void {
  if (!isTauriDesktop()) throw new Error("日历功能需要在 Tauri 桌面应用中使用。");
}

export const calendarService = {
  listRange(start: string, end: string): Promise<CalendarEvent[]> {
    requireDesktop();
    return invoke<CalendarEvent[]>("calendar_list_range", { start, end });
  },

  save(draft: CalendarEventDraft): Promise<CalendarEvent> {
    requireDesktop();
    return invoke<CalendarEvent>("calendar_save_event", { draft });
  },

  move(input: MoveEventInput): Promise<CalendarEvent> {
    requireDesktop();
    return invoke<CalendarEvent>("calendar_move_event", { input });
  },

  delete(id: string): Promise<void> {
    requireDesktop();
    return invoke<void>("calendar_delete_event", { id });
  },
};
