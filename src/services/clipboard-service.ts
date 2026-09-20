import { invoke } from "@tauri-apps/api/core";
import type {
  ClipboardItem,
  ClipboardFilePreview,
  ClipboardQuery,
  ClipboardSettingsDraft,
  ClipboardSequenceStatus,
  ClipboardSequenceStep,
  ClipboardStatus,
} from "@/types/clipboard";

export const clipboardService = {
  list(filter: ClipboardQuery): Promise<ClipboardItem[]> {
    return invoke<ClipboardItem[]>("clipboard_list", { filter });
  },

  status(): Promise<ClipboardStatus> {
    return invoke<ClipboardStatus>("clipboard_status");
  },

  setPinned(id: string, pinned: boolean): Promise<ClipboardItem> {
    return invoke<ClipboardItem>("clipboard_set_pinned", { id, pinned });
  },

  copy(id: string): Promise<ClipboardItem> {
    return invoke<ClipboardItem>("clipboard_copy", { id });
  },

  copyPlain(id: string): Promise<ClipboardItem> {
    return invoke<ClipboardItem>("clipboard_copy_plain", { id });
  },

  paste(id: string, plain = false): Promise<ClipboardItem> {
    return invoke<ClipboardItem>("clipboard_paste", { id, plain });
  },

  copyMerged(ids: string[], separator = "\n"): Promise<number> {
    return invoke<number>("clipboard_copy_merged", { ids, separator });
  },

  pasteMerged(ids: string[], separator = "\n"): Promise<number> {
    return invoke<number>("clipboard_paste_merged", { ids, separator });
  },

  writeText(content: string): Promise<void> {
    return invoke<void>("clipboard_write_text", { content });
  },

  sequenceStart(ids: string[]): Promise<ClipboardSequenceStatus> {
    return invoke<ClipboardSequenceStatus>("clipboard_sequence_start", { ids });
  },

  sequenceStatus(): Promise<ClipboardSequenceStatus> {
    return invoke<ClipboardSequenceStatus>("clipboard_sequence_status");
  },

  sequenceNext(): Promise<ClipboardSequenceStep> {
    return invoke<ClipboardSequenceStep>("clipboard_sequence_next");
  },

  sequencePasteNext(): Promise<ClipboardSequenceStep> {
    return invoke<ClipboardSequenceStep>("clipboard_sequence_paste_next");
  },

  showPanel(): Promise<void> {
    return invoke<void>("clipboard_show_panel");
  },

  sequenceCancel(): Promise<void> {
    return invoke<void>("clipboard_sequence_cancel");
  },

  delete(id: string): Promise<void> {
    return invoke<void>("clipboard_delete", { id });
  },

  clear(includePinned: boolean): Promise<number> {
    return invoke<number>("clipboard_clear", { includePinned });
  },

  updateSettings(settings: ClipboardSettingsDraft): Promise<ClipboardStatus> {
    return invoke<ClipboardStatus>("clipboard_update_settings", { settings });
  },

  saveAsSnippet(id: string): Promise<void> {
    return invoke<void>("clipboard_save_as_snippet", { id });
  },

  imageDataUrl(id: string): Promise<string> {
    return invoke<string>("clipboard_image_data_url", { id });
  },

  imageThumbnailDataUrl(id: string, size = 72): Promise<string> {
    return invoke<string>("clipboard_image_thumbnail_data_url", { id, size });
  },

  filePreview(id: string, index: number): Promise<ClipboardFilePreview> {
    return invoke<ClipboardFilePreview>("clipboard_file_preview", { id, index });
  },

  openSpecial(kind: "url" | "email", value: string): Promise<void> {
    return invoke<void>("clipboard_open_special", { kind, value });
  },
};
