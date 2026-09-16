import { computed, reactive } from "vue";
import { pushToast } from "@/composables/useToast";
import { oilService } from "@/services/oil-service";
import { isTauriDesktop } from "@/connectors/types";
import { announceOilUpdate } from "@/desktop/tray";
import type { OilMonitorSnapshot } from "@/types/oil";

const state = reactive<{
  snapshot: OilMonitorSnapshot | null;
  loading: boolean;
  error: string | null;
  configured: boolean;
}>({
  snapshot: null,
  loading: false,
  error: null,
  configured: false,
});

let started = false;

function beijingDate(): string {
  return new Intl.DateTimeFormat("en-CA", {
    timeZone: "Asia/Shanghai",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).format(new Date());
}

function refreshDue(): boolean {
  if (!state.snapshot) return true;
  const adjustmentDay = state.snapshot.forecast.nextAdjustmentDate === beijingDate();
  const interval = adjustmentDay ? 5 * 60_000 : 30 * 60_000;
  return Date.now() - new Date(state.snapshot.collectedAt).getTime() >= interval;
}

function read(): void {
  state.configured = oilService.isConfigured();
  state.snapshot = oilService.getSnapshot();
}

export function useOilMonitor() {
  read();

  async function sync(quiet = false): Promise<void> {
    if (state.loading) return;
    read();
    if (!state.configured) return;
    state.loading = true;
    state.error = null;
    try {
      const result = await oilService.sync();
      state.snapshot = result.snapshot;
      if (result.priceChanges.length) {
        const detail = result.priceChanges
          .map((item) => `${item.name} ${item.delta > 0 ? "+" : ""}${item.delta.toFixed(2)} 元/升`)
          .join("，");
        pushToast(`${result.snapshot.province}油价已更新：${detail}`, "warn");
        void announceOilUpdate(`${result.snapshot.province}油价已更新：${detail}`);
      } else if (result.officialChanged) {
        pushToast(`国家发改委发布新调价公告：${result.snapshot.officialAdjustment.title}`, "warn");
        void announceOilUpdate(`国家发改委调价公告：${result.snapshot.officialAdjustment.title}`);
      } else if (!quiet) {
        pushToast("油价同步完成", "success");
      }
    } catch (error) {
      state.error = error instanceof Error ? error.message : String(error);
      if (!quiet) pushToast(`油价同步失败：${state.error}`, "error");
    } finally {
      state.loading = false;
    }
  }

  if (!started && typeof window !== "undefined") {
    started = true;
    if (oilService.isConfigured()) void sync(true);
    window.setInterval(() => {
      if ((isTauriDesktop() || document.visibilityState === "visible") && refreshDue()) void sync(true);
    }, 5 * 60_000);
    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "visible" && refreshDue()) void sync(true);
    });
  }

  return {
    snapshot: computed(() => state.snapshot),
    loading: computed(() => state.loading),
    error: computed(() => state.error),
    configured: computed(() => state.configured),
    sync,
  };
}
