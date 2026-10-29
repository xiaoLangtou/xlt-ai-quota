import type { TranslateSegment } from "@/types/translate";

export function mergeTranslationSegment(segments: TranslateSegment[], update: TranslateSegment): TranslateSegment[] {
  return [...segments.filter(item => item.index !== update.index), update].sort((a, b) => a.index - b.index);
}
export function completedTranslation(segments: TranslateSegment[], bilingual = false): string {
  return segments.filter(item => item.status === "done").map(item => bilingual ? `${item.original}\n${item.translated}` : item.translated).join("\n\n");
}
export interface TranslationHistoryEntry {
  id: string; createdAt: number; source: string; from: string; to: string; engine: string;
  status: "done" | "partial" | "failed" | "cancelled"; segments: TranslateSegment[];
}
export const HISTORY_MAX_BYTES = 1024 * 1024;
export const HISTORY_ENTRY_MAX_BYTES = 32 * 1024;
export function pruneTranslationHistory(entries: TranslationHistoryEntry[], days: number, now = Date.now()): TranslationHistoryEntry[] {
  const encoder = new TextEncoder();
  let used = 2;
  const ids = new Set<string>();
  return entries.filter(item => item.createdAt <= now && item.createdAt > now - days * 86400000)
    .sort((a, b) => b.createdAt - a.createdAt).filter(item => {
      const size = encoder.encode(JSON.stringify(item)).length + 1;
      if (ids.has(item.id) || size > HISTORY_ENTRY_MAX_BYTES || used + size > HISTORY_MAX_BYTES || ids.size >= 50) return false;
      ids.add(item.id); used += size; return true;
    });
}
/** 明确挑选保存字段，调用配置和密钥不进入历史。 */
export function historyEntry(input: Omit<TranslationHistoryEntry, "status">, cancelled: boolean): TranslationHistoryEntry {
  const segments = input.segments.map(({ index, original, translated, status, error }) => ({ index, original, translated, status, error }));
  const done = segments.filter(item => item.status === "done").length;
  return { id: input.id, createdAt: input.createdAt, source: input.source, from: input.from, to: input.to, engine: input.engine,
    status: cancelled ? "cancelled" : done === segments.length ? "done" : done ? "partial" : "failed", segments };
}
