import { computed, reactive, type Reactive } from "vue";
import { setupUsageTray, updateUsageTrayStatus } from "@/desktop/tray";
import { usageService } from "@/services/usage-service";
import { isTauriDesktop } from "@/connectors/types";
import type {
  PlatformQuotaView,
  RangePreset,
  SyncInfo,
  TokenSummary,
  TrendPoint,
  PlatformDailyPoint,
  ToolUsage,
  HeatmapDay,
} from "@/types/usage";

interface DashboardState {
  range: RangePreset;
  quotas: PlatformQuotaView[];
  summary: TokenSummary;
  trend: TrendPoint[];
  platformDaily: PlatformDailyPoint[];
  toolUsage: ToolUsage[];
  heatmap: HeatmapDay[];
  sync: SyncInfo;
  loading: boolean;
  syncError: string | null;
}

const state = reactive<DashboardState>({
  range: "7d",
  quotas: [],
  summary: { total: 0, input: 0, output: 0, requests: 0 },
  trend: [],
  platformDaily: [],
  toolUsage: [],
  heatmap: [],
  sync: { lastSyncAt: null, syncing: false },
  loading: false,
  syncError: null,
});

let autoSyncStarted = false;

function readAll(): void {
  state.quotas = usageService.getPlatformQuotaViews();
  state.summary = usageService.getTokenSummary(state.range);
  state.trend = usageService.getTokenTrend(state.range);
  state.platformDaily = usageService.getPlatformDaily(state.range);
  state.toolUsage = usageService.getToolBreakdown(state.range);
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
    } catch (e) {
      state.syncError = e instanceof Error ? e.message : String(e);
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
    };
  }

  function saveSettings(patch: {
    ark?: { baseUrl?: string };
  }): void {
    if (patch.ark) usageService.saveArkBaseUrl(patch.ark.baseUrl ?? "");
  }

  if (!autoSyncStarted) {
    autoSyncStarted = true;
    void setupUsageTray(sync).then(updateTray).catch(() => undefined);
    void (isTauriDesktop() ? syncSubscriptionQuotas() : sync());
  }

  return {
    state: state as Reactive<DashboardState>,
    range: computed(() => state.range),
    quotas: computed(() => state.quotas),
    summary: computed(() => state.summary),
    trend: computed(() => state.trend),
    platformDaily: computed(() => state.platformDaily),
    toolUsage: computed(() => state.toolUsage),
    heatmap: computed(() => state.heatmap),
    loading: computed(() => state.loading),
    syncError: computed(() => state.syncError),
    setRange,
    sync,
    syncSubscriptionQuotas,
    loadSettings,
    saveSettings,
  };
}
