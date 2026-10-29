import { ref } from "vue";
import { useCalendarTodo } from "@/composables/useCalendarTodo";
import { clearZentaoDetailCache, zentaoService } from "@/services/zentao-service";
import type { ZentaoStatus, ZentaoSyncResult } from "@/types/productivity";

const POLL_INTERVAL_MS = 15 * 60 * 1000;

// 模块级单例：手动触发与定时轮询共用同一状态，避免并发重复同步。
const configured = ref<ZentaoStatus>({ configured: false, url: "", account: "" });
const syncing = ref(false);
const lastResult = ref<ZentaoSyncResult | null>(null);
const lastError = ref("");
const refreshError = ref("");
let polling = false;
let pollTimer: ReturnType<typeof setInterval> | undefined;

/** 停止轮询（测试或登出时使用）。 */
function stopPolling(): void {
  polling = false;
  if (pollTimer !== undefined) {
    clearInterval(pollTimer);
    pollTimer = undefined;
  }
}

async function refreshStatus(): Promise<void> {
  try {
    configured.value = await zentaoService.status();
  } catch {
    configured.value = { configured: false, url: "", account: "" };
  }
}

/**
 * 执行一次禅道同步。轮询只在已登录时静默执行，失败不弹 toast。
 * @param silent 轮询触发时不打扰用户，仅记录结果。
 */
async function syncOnce(silent: boolean): Promise<ZentaoSyncResult | null> {
  if (syncing.value) return null;
  syncing.value = true;
  lastError.value = "";
  refreshError.value = "";
  try {
    await refreshStatus();
    if (!configured.value.configured) return null;
    const result = await zentaoService.sync();
    lastResult.value = result;
    // 同步可能改变条目内容，清掉详情缓存，避免打开详情看到旧数据。
    clearZentaoDetailCache();
    const dashboard = useCalendarTodo();
    await dashboard.refresh();
    refreshError.value = dashboard.error.value;
    return result;
  } catch (reason) {
    lastError.value = reason instanceof Error ? reason.message : String(reason);
    if (!silent) throw reason;
    return null;
  } finally {
    syncing.value = false;
  }
}

/** 启动 15 分钟轮询（仅在桌面端 + 已登录时执行实际同步）。 */
function ensurePolling(): void {
  if (polling) return;
  polling = true;
  pollTimer = setInterval(() => {
    if (document.visibilityState !== "visible") return;
    void syncOnce(true);
  }, POLL_INTERVAL_MS);
}

export function useZentaoSync() {
  return {
    configured,
    syncing,
    lastResult,
    lastError,
    refreshError,
    refreshStatus,
    syncOnce,
    ensurePolling,
    stopPolling,
  };
}
