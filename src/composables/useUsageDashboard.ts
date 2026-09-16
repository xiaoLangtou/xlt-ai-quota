import { computed, reactive, type Reactive } from "vue";
import { setupUsageTray, updateUsageTrayStatus } from "@/desktop/tray";
import { pushToast } from "@/composables/useToast";
import { usageService, type TodayHourly } from "@/services/usage-service";
import { isTauriDesktop } from "@/connectors/types";
import type {
  PlatformQuotaView,
  RangePreset,
  SyncInfo,
  TokenSummary,
  TrendPoint,
  PlatformDailyPoint,
  ToolUsage,
  ModelUsage,
  HeatmapDay,
  QuotaDisplayTarget,
} from "@/types/usage";

interface DashboardState {
  range: RangePreset;
  quotas: PlatformQuotaView[];
  summary: TokenSummary;
  trend: TrendPoint[];
  platformDaily: PlatformDailyPoint[];
  toolUsage: ToolUsage[];
  modelUsage: ModelUsage[];
  heatmap: HeatmapDay[];
  todayHourly: TodayHourly;
  sync: SyncInfo;
  loading: boolean;
  syncError: string | null;
}

const state = reactive<DashboardState>({
  range: "7d",
  quotas: [],
  summary: { total: 0, input: 0, output: 0, requests: 0, costUsd: 0 },
  trend: [],
  platformDaily: [],
  toolUsage: [],
  modelUsage: [],
  heatmap: [],
  todayHourly: { labels: [], trend: [], platform: [] },
  sync: { lastSyncAt: null, syncing: false },
  loading: false,
  syncError: null,
});

let autoSyncStarted = false;
let autoRefreshStarted = false;
let todayRefreshing = false;

function readAll(): void {
  state.quotas = usageService.getPlatformQuotaViews();
  state.summary = usageService.getTokenSummary(state.range);
  state.trend = usageService.getTokenTrend(state.range);
  state.platformDaily = usageService.getPlatformDaily(state.range);
  state.toolUsage = usageService.getToolBreakdown(state.range);
  state.modelUsage = usageService.getModelBreakdown(state.range);
  state.heatmap = usageService.getDailyHeatmap();
  state.sync = usageService.getSyncInfo();
  state.syncError = state.sync.error ?? null;
}

export function useUsageDashboard() {
  usageService.ensureSeeded();
  readAll();

  function updateTray(): void {
    void updateUsageTrayStatus({
      quotas: state.quotas,
      sync: state.sync,
      syncing: state.loading || state.sync.syncing,
    }).catch(() => undefined);
  }

  function setRange(preset: RangePreset): void {
    state.range = preset;
    state.summary = usageService.getTokenSummary(preset);
    state.trend = usageService.getTokenTrend(preset);
    state.platformDaily = usageService.getPlatformDaily(preset);
    state.toolUsage = usageService.getToolBreakdown(preset);
    state.modelUsage = usageService.getModelBreakdown(preset);
    // 切到「今天」时立即拉最新本地用量 + 按小时时间轴，避免看到过期数据。
    if (preset === "today") {
      void syncToday();
      void loadTodayHourly();
    }
  }

  /** 加载「今天」按小时的趋势/平台分布（分析页时间轴）。 */
  async function loadTodayHourly(): Promise<void> {
    if (state.range !== "today") return;
    try {
      state.todayHourly = await usageService.getTodayHourly();
    } catch {
      // 忽略：拉取失败时保留上一次数据
    }
  }

  /** 轻量自动刷新：静默重拉本地会话类 Token 的「今天」数据（不含 arkcli，不动 loading）。 */
  async function syncToday(): Promise<void> {
    if (todayRefreshing || state.loading) return;
    todayRefreshing = true;
    try {
      await usageService.syncTodayTokens();
      readAll();
      updateTray();
      if (state.range === "today") void loadTodayHourly();
    } catch {
      // 自动刷新失败不打扰用户
    } finally {
      todayRefreshing = false;
    }
  }

  async function sync(): Promise<void> {
    // `syncing` 会被持久化；应用被强制关闭后它可能是上次遗留状态，
    // 不能用作本次运行的并发锁。
    if (state.loading) return;

    state.loading = true;
    state.syncError = null;
    usageService.setSyncing(true);
    state.sync = usageService.getSyncInfo();
    updateTray();
    try {
      await usageService.syncAll(readAll);
      readAll();
      // 依据同步结果给出反馈。
      if (state.sync.outcome === "success") pushToast("同步完成", "success");
      else if (state.sync.outcome === "partial") pushToast(state.syncError || "部分连接器同步失败", "warn");
      else if (state.sync.outcome === "failed") pushToast(state.syncError || "同步失败", "error");
    } catch (e) {
      state.syncError = e instanceof Error ? e.message : String(e);
      pushToast(state.syncError, "error");
    } finally {
      state.loading = false;
      // 防止未预期异常或窗口中断后把“同步中”遗留到下次启动。
      if (state.sync.syncing) {
        usageService.setSyncing(false);
        state.sync = usageService.getSyncInfo();
      }
      updateTray();
    }
  }

  async function syncSubscriptionQuotas(): Promise<void> {
    if (state.loading) return;

    state.loading = true;
    state.syncError = null;
    usageService.setSyncing(true);
    state.sync = usageService.getSyncInfo();
    updateTray();
    try {
      await usageService.syncQuotas(readAll);
      readAll();
    } catch (e) {
      state.syncError = e instanceof Error ? e.message : String(e);
    } finally {
      state.loading = false;
      if (state.sync.syncing) {
        usageService.setSyncing(false);
        state.sync = usageService.getSyncInfo();
      }
      updateTray();
    }
  }

  function loadSettings() {
    return {
      ark: usageService.getArkConfigView(),
      oil: usageService.getOilConfigView(),
      quotaDisplay: usageService.getQuotaDisplayConfigView(),
      preferences: usageService.getPreferencesView(),
      systemTimezone: usageService.getSystemTimezone(),
    };
  }

  function saveSettings(patch: {
    ark?: { baseUrl?: string };
    oil?: { province: string; apiKey: string };
    quotaDisplay?: { hiddenPlatforms: QuotaDisplayTarget[] };
    preferences?: { timezone?: string; statsSince?: string | null };
  }): void {
    if (patch.ark) usageService.saveArkBaseUrl(patch.ark.baseUrl ?? "");
    if (patch.oil) usageService.saveOilConfig(patch.oil);
    if (patch.quotaDisplay) {
      usageService.saveQuotaDisplayConfig(patch.quotaDisplay);
      readAll();
      updateTray();
    }
    if (patch.preferences) usageService.savePreferences(patch.preferences);
  }

  if (!autoSyncStarted) {
    autoSyncStarted = true;
    void setupUsageTray(sync).then(updateTray).catch(() => undefined);
    void (isTauriDesktop() ? syncSubscriptionQuotas() : sync());
  }

  // 自动刷新「今天」：每 60s + 窗口可见/聚焦时，静默更新本地用量，实现近实时。
  if (!autoRefreshStarted && typeof window !== "undefined") {
    autoRefreshStarted = true;
    window.setInterval(() => {
      if (document.visibilityState === "visible") void syncToday();
    }, 60_000);
    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "visible") void syncToday();
    });
    window.addEventListener("focus", () => void syncToday());
  }

  return {
    state: state as Reactive<DashboardState>,
    range: computed(() => state.range),
    quotas: computed(() => state.quotas),
    summary: computed(() => state.summary),
    trend: computed(() => state.trend),
    platformDaily: computed(() => state.platformDaily),
    toolUsage: computed(() => state.toolUsage),
    modelUsage: computed(() => state.modelUsage),
    heatmap: computed(() => state.heatmap),
    todayHourly: computed(() => state.todayHourly),
    loading: computed(() => state.loading),
    syncError: computed(() => state.syncError),
    setRange,
    sync,
    syncToday,
    syncSubscriptionQuotas,
    loadSettings,
    saveSettings,
  };
}
