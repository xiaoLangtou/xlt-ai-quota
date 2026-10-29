import { computed, watch } from "vue";
import { createSharedComposable, useLocalStorage, useNow } from "@vueuse/core";
import { historyEntry, pruneTranslationHistory, HISTORY_ENTRY_MAX_BYTES, type TranslationHistoryEntry } from "@/utils/translate-workspace";

export const useTranslationHistory = createSharedComposable(() => {
  const enabled = useLocalStorage("xlt.translate.history.enabled", false);
  const retentionDays = useLocalStorage("xlt.translate.history.retention", 30);
  const records = useLocalStorage<TranslationHistoryEntry[]>("xlt.translate.history.records.v1", []);
  const now = useNow({ interval: 60_000 });
  const entries = computed(() => pruneTranslationHistory(records.value, retentionDays.value, now.value.getTime()));
  watch([retentionDays, now], () => { records.value = entries.value; }, { immediate: true });
  function save(input: Omit<TranslationHistoryEntry, "status">, cancelled: boolean): "saved" | "disabled" | "too-large" {
    if (!enabled.value) return "disabled";
    const entry = historyEntry(input, cancelled);
    if (new TextEncoder().encode(JSON.stringify(entry)).length > HISTORY_ENTRY_MAX_BYTES) return "too-large";
    records.value = pruneTranslationHistory([entry, ...records.value.filter(item => item.id !== entry.id)], retentionDays.value);
    return "saved";
  }
  function remove(id: string): void { records.value = records.value.filter(item => item.id !== id); }
  function clear(): void { records.value = []; }
  return { enabled, retentionDays, entries, save, remove, clear };
});
