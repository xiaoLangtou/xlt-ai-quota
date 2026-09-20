<script setup lang="ts">
import { computed, defineAsyncComponent, onMounted, onUnmounted, ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { useUsageDashboard } from "@/composables/useUsageDashboard";
import { useSubscriptions } from "@/composables/useSubscriptions";
import { useTheme } from "@/composables/useTheme";
import QuotaCard from "@/components/QuotaCard.vue";
import QuotaSummary from "@/components/QuotaSummary.vue";
import ToolBreakdown from "@/components/ToolBreakdown.vue";
import ToolDonutChart from "@/components/ToolDonutChart.vue";
import ModelBreakdown from "@/components/ModelBreakdown.vue";
import CalendarHeatmap from "@/components/CalendarHeatmap.vue";
import TokenSummaryCard from "@/components/TokenSummaryCard.vue";
import SettingsPanel from "@/components/SettingsPanel.vue";
import SubscriptionManager from "@/components/SubscriptionManager.vue";
import AboutDialog from "@/components/AboutDialog.vue";
import SkeletonBlock from "@/components/SkeletonBlock.vue";
import DailyReportView from "@/views/DailyReportView.vue";
import SnippetLibraryView from "@/views/SnippetLibraryView.vue";
import ClipboardHistoryView from "@/views/ClipboardHistoryView.vue";
import VaultView from "@/views/VaultView.vue";
import TodayWorkCard from "@/components/TodayWorkCard.vue";
import OilPriceCard from "@/components/OilPriceCard.vue";
import { useOilMonitor } from "@/composables/useOilMonitor";
import { isTauriDesktop } from "@/connectors/types";
import { formatTokens } from "@/utils/format";
import type { RangePreset } from "@/types/usage";
import { Button } from "@/components/ui/button";

const {
  state,
  quotas,
  summary,
  trend,
  platformDaily,
  toolUsage,
  modelUsage,
  heatmap,
  todayHourly,
  loading,
  syncError,
  setRange,
  sync: syncNow,
} = useUsageDashboard();
const {
  subscriptions,
  monthlyCny,
  monthTotalCny,
  usdToCnyRate,
  setUsdToCnyRate,
} = useSubscriptions();
const currentMonthKey = new Date().toISOString().slice(0, 7);
const actualSpend = computed(() => monthTotalCny(currentMonthKey));
const { pref: themePref, cycle: cycleTheme } = useTheme();
const { sync: syncOil } = useOilMonitor();

const TokenTrendChart = defineAsyncComponent(() => import("@/components/TokenTrendChart.vue"));
const PlatformDailyChart = defineAsyncComponent(
  () => import("@/components/PlatformDailyChart.vue"),
);

type WorkspaceSection = "overview" | "daily-report" | "snippets" | "clipboard" | "vault" | "analytics" | "subscriptions" | "settings";
const activeWorkspace = ref<WorkspaceSection>("overview");
const subscriptionCreateSignal = ref(0);
const settingsSyncSignal = ref(0);
const aboutOpen = ref(false);
const exchangeRate = ref(usdToCnyRate.value);
const chartsReady = ref(!isTauriDesktop());
let unlistenFocus: UnlistenFn | undefined;

function loadCharts(): void {
  chartsReady.value = true;
}

onMounted(async () => {
  if (!isTauriDesktop()) return;
  const appWindow = getCurrentWindow();
  if (await appWindow.isVisible()) loadCharts();
  unlistenFocus = await appWindow.onFocusChanged(({ payload: focused }) => {
    if (focused) loadCharts();
  });
});

onUnmounted(() => {
  unlistenFocus?.();
});

const RANGES: { preset: RangePreset; label: string }[] = [
  { preset: "today", label: "今天" },
  { preset: "7d", label: "7 天" },
  { preset: "30d", label: "30 天" },
  { preset: "90d", label: "90 天" },
];

const sparkTotal = computed(() => trend.value.map((p) => p.total));
const sparkInput = computed(() => trend.value.map((p) => p.input));
const sparkOutput = computed(() => trend.value.map((p) => p.output));
const sparkRequests = computed(() => trend.value.map((p) => p.requests));

const activeSubs = computed(() =>
  subscriptions.value.filter((item) => item.status === "active"),
);
const hasData = computed(() =>
  quotas.value.some((q) => q.windows.length || q.credits),
);
const quotaPlatformCount = computed(() => new Set(quotas.value.map((item) => item.platform)).size);

const isSyncing = computed(() => loading.value || state.sync.syncing);
/** 首次同步且暂无 Token 数据时，用骨架占位替代空白。 */
const firstLoading = computed(() => loading.value && summary.value.total === 0);
const isPartialSync = computed(() => state.sync.outcome === "partial");
const syncIssueText = computed(() =>
  (syncError.value ?? "").replace(/^部分连接器同步失败：\s*/, ""),
);

const syncText = computed(() => {
  if (isSyncing.value) return "同步中…";
  if (!state.sync.lastSyncAt) return "未同步";
  const diff = Date.now() - new Date(state.sync.lastSyncAt).getTime();
  const min = Math.floor(diff / 60000);
  const relative =
    min < 1
      ? "刚刚同步"
      : min < 60
        ? `${min} 分钟前`
        : min < 24 * 60
          ? `${Math.floor(min / 60)} 小时前`
          : "较久前";
  return isPartialSync.value ? `${relative} · 部分` : relative;
});

const inputPct = computed(() =>
  summary.value.total > 0
    ? Math.round((summary.value.input / summary.value.total) * 100)
    : 0,
);
const outputPct = computed(() =>
  summary.value.total > 0
    ? Math.round((summary.value.output / summary.value.total) * 100)
    : 0,
);
const rangeLabel = computed(
  () => RANGES.find((r) => r.preset === state.range)?.label ?? "",
);

/** 全平台里利用率最高的一个额度窗口，作为驾驶舱「峰值」信号。 */
const peakUsage = computed(() => {
  let best: { name: string; pct: number } | null = null;
  for (const q of quotas.value) {
    for (const w of q.windows) {
      if (!best || w.usedPct > best.pct) best = { name: q.name, pct: w.usedPct };
    }
  }
  return best;
});
const peakTone = computed(() => {
  const pct = peakUsage.value?.pct ?? 0;
  if (pct >= 100) return "crit";
  if (pct >= 80) return "warn";
  return "ok";
});

const themeLabel = computed(() =>
  themePref.value === "system" ? "跟随系统" : themePref.value === "dark" ? "暗色" : "亮色",
);

const pageMeta = computed(() => {
  const pages: Record<WorkspaceSection, { crumb: string; title: string }> = {
    overview: { crumb: "Overview", title: "用量概览" },
    "daily-report": { crumb: "Git Reports", title: "Git 报告" },
    snippets: { crumb: "Snippets", title: "片段库" },
    clipboard: { crumb: "Clipboard", title: "剪贴板历史" },
    vault: { crumb: "Vault", title: "密钥库" },
    analytics: { crumb: "Analytics", title: "用量分析" },
    subscriptions: { crumb: "Billing", title: "订阅与账单" },
    settings: { crumb: "Connectors", title: "连接与设置" },
  };
  return pages[activeWorkspace.value];
});

function saveExchangeRate(): void {
  setUsdToCnyRate(exchangeRate.value);
  exchangeRate.value = usdToCnyRate.value;
}

async function syncEverything(): Promise<void> {
  await Promise.all([syncNow(), syncOil()]);
}

function formatCny(value: number): string {
  return new Intl.NumberFormat("zh-CN", {
    style: "currency",
    currency: "CNY",
    maximumFractionDigits: value >= 100 ? 0 : 2,
  }).format(value);
}
</script>

<template>
  <main class="workspace-shell">
    <aside class="rail-wrap">
      <div class="rail">
        <div class="rail-logo">/_</div>
        <nav class="rail-nav" aria-label="主导航">
          <button class="rail-btn" :class="{ active: activeWorkspace === 'overview' }" type="button"
            @click="activeWorkspace = 'overview'">
            <span class="tip">用量概览</span>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"
              stroke-linejoin="round">
              <rect x="3" y="3" width="7" height="9" rx="2" />
              <rect x="14" y="3" width="7" height="5" rx="2" />
              <rect x="14" y="12" width="7" height="9" rx="2" />
              <rect x="3" y="16" width="7" height="5" rx="2" />
            </svg>
          </button>
          <button class="rail-btn" :class="{ active: activeWorkspace === 'daily-report' }" type="button"
            @click="activeWorkspace = 'daily-report'">
            <span class="tip">Git 报告</span>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"
              stroke-linejoin="round">
              <rect x="4" y="3" width="16" height="18" rx="3" />
              <path d="M8 8h8M8 12h8M8 16h5" />
            </svg>
          </button>
          <button class="rail-btn" :class="{ active: activeWorkspace === 'snippets' }" type="button"
            @click="activeWorkspace = 'snippets'">
            <span class="tip">片段库</span>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"
              stroke-linejoin="round">
              <path d="M8 4.5H5.8A1.8 1.8 0 0 0 4 6.3v11.9A1.8 1.8 0 0 0 5.8 20H16.2A1.8 1.8 0 0 0 18 18.2V14" />
              <path d="M14 4h6v6M12 12l8-8" />
              <path d="M8 10h4M8 14h6" />
            </svg>
          </button>
          <button class="rail-btn" :class="{ active: activeWorkspace === 'clipboard' }" type="button"
            @click="activeWorkspace = 'clipboard'">
            <span class="tip">剪贴板历史</span>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"
              stroke-linejoin="round">
              <rect x="5" y="4" width="14" height="17" rx="3" />
              <path d="M9 4.5V3h6v1.5M9 9h6M9 13h6M9 17h4" />
            </svg>
          </button>
          <button class="rail-btn" :class="{ active: activeWorkspace === 'vault' }" type="button"
            @click="activeWorkspace = 'vault'">
            <span class="tip">密钥库</span>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
              <circle cx="8" cy="15" r="4" />
              <path d="m11 12 8-8M16 4l4 4M14 6l4 4" />
            </svg>
          </button>
          <button class="rail-btn" :class="{ active: activeWorkspace === 'analytics' }" type="button"
            @click="activeWorkspace = 'analytics'">
            <span class="tip">用量分析</span>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"
              stroke-linejoin="round">
              <path d="M4 19V10" />
              <path d="M11 19V5" />
              <path d="M18 19v-7" />
              <path d="M3 19h18" />
            </svg>
          </button>
          <button class="rail-btn" :class="{ active: activeWorkspace === 'subscriptions' }" type="button"
            @click="activeWorkspace = 'subscriptions'">
            <span class="tip">订阅与账单</span>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"
              stroke-linejoin="round">
              <rect x="2.5" y="5.5" width="19" height="14" rx="3" />
              <path d="M2.5 10h19" />
              <path d="M6.5 15h4" />
            </svg>
          </button>
          <button class="rail-btn" :class="{ active: activeWorkspace === 'settings' }" type="button"
            @click="activeWorkspace = 'settings'">
            <span class="tip">连接与设置</span>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"
              stroke-linejoin="round">
              <path d="M8 3v4" />
              <path d="M16 3v4" />
              <path d="M4 10h16" />
              <rect x="4" y="6" width="16" height="15" rx="3" />
              <path d="M9 15.5l2 2 4-4.5" />
            </svg>
          </button>
        </nav>
        <div class="rail-divider" />
        <button class="rail-btn" type="button" :title="`主题：${themeLabel}（点击切换）`" @click="cycleTheme">
          <span class="tip">{{ themeLabel }}</span>
          <svg v-if="themePref === 'light'" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"
            stroke-linecap="round">
            <circle cx="12" cy="12" r="4.2" />
            <path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M19.1 4.9L17 7M7 17l-2.1 2.1" />
          </svg>
          <svg v-else-if="themePref === 'dark'" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"
            stroke-linecap="round" stroke-linejoin="round">
            <path d="M20 14.5A8 8 0 1 1 9.5 4a6.3 6.3 0 0 0 10.5 10.5Z" />
          </svg>
          <svg v-else viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"
            stroke-linejoin="round">
            <rect x="3" y="4" width="18" height="12" rx="2" />
            <path d="M8 20h8" />
            <path d="M12 16v4" />
          </svg>
        </button>
        <button class="rail-btn" type="button" title="关于" @click="aboutOpen = true">
          <span class="tip">关于</span>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"
            stroke-linejoin="round">
            <circle cx="12" cy="12" r="9" />
            <path d="M12 11v5" />
            <circle cx="12" cy="7.5" r="0.6" fill="currentColor" />
          </svg>
        </button>
        <button class="sync-chip" type="button"
          :class="{ syncing: isSyncing, error: syncError && !isPartialSync, warn: isPartialSync }"
          :title="syncError ? `${isPartialSync ? '部分数据未更新' : '同步出错'}：${syncError}` : '点击立即同步'" @click="syncEverything">
          <i class="sync-dot" />
          <span class="t1">{{ syncText }}</span>
          <span class="t2">点击同步</span>
        </button>
      </div>
    </aside>

    <section class="workspace-main content" :class="{ 'snippet-workspace': activeWorkspace === 'snippets', 'clipboard-workspace-shell': activeWorkspace === 'clipboard' }">
      <header v-if="activeWorkspace !== 'snippets' && activeWorkspace !== 'clipboard'" class="topbar">
        <div>
          <div class="crumb">用量看板 <span>/</span> {{ pageMeta.crumb }}</div>
          <h1>{{ pageMeta.title }}</h1>
        </div>
        <div class="topbar-actions">
          <div v-if="activeWorkspace === 'analytics'" class="range-switch" role="group" aria-label="统计周期">
            <button v-for="r in RANGES" :key="r.preset" type="button" :class="{ on: state.range === r.preset }"
              @click="setRange(r.preset)">{{ r.label }}</button>
          </div>
          <template v-else-if="activeWorkspace === 'subscriptions'">
            <label class="rate-field">1 USD = ¥ <input v-model.number="exchangeRate" type="number" min="0.01"
                step="0.01" @change="saveExchangeRate" /></label>
            <button class="btn-primary" type="button" @click="subscriptionCreateSignal += 1">+ 新增订阅</button>
          </template>
          <Button v-else-if="activeWorkspace === 'settings'" type="button"
            @click="settingsSyncSignal += 1">立即同步</Button>
        </div>
      </header>
      <div class="page-scroll">
        <template v-if="activeWorkspace === 'overview'">
          <div v-if="syncError && !isPartialSync" class="banner banner-error" @click="syncEverything">
            <span class="banner-dot" />
            <span class="banner-text">同步出错：{{ syncError }}</span>
            <span class="banner-hint">点击重试</span>
          </div>
          <div v-else-if="syncError" class="banner banner-warn" @click="syncEverything">
            <span class="banner-dot" />
            <span class="banner-text">部分数据未更新：{{ syncIssueText }}</span>
            <span class="banner-hint">点击重试</span>
          </div>
          <div v-else-if="!state.sync.lastSyncAt && !hasData" class="banner banner-empty" @click="syncEverything">
            <span class="banner-dot" />
            <span class="banner-text">暂无数据 · 从本机 CLI 拉取真实用量</span>
            <span class="banner-hint">立即同步</span>
          </div>

          <!-- 账务指标优先，用量指标作为补充运营信号。 -->
          <section class="cockpit" aria-label="关键指标">
            <div class="cell">
              <span class="cell-label">订阅月支出 · 计划</span>
              <strong class="cell-value">{{ formatCny(monthlyCny) }}</strong>
              <button class="cell-meta link" type="button" @click="activeWorkspace = 'subscriptions'">
                {{ activeSubs.length }} 项活跃订阅 →
              </button>
            </div>
            <div class="cell">
              <span class="cell-label">本月实际支出 · 已记</span>
              <strong class="cell-value">{{ formatCny(actualSpend) }}</strong>
              <button class="cell-meta link" type="button" @click="activeWorkspace = 'subscriptions'">
                查看账单流水 →
              </button>
            </div>
            <div class="cell">
              <span class="cell-label">{{ rangeLabel }} TOKEN</span>
              <strong class="cell-value">{{ formatTokens(summary.total) }}</strong>
              <span v-if="summary.deltaPct != null" class="cell-delta" :class="summary.deltaPct >= 0 ? 'up' : 'down'">
                {{ summary.deltaPct >= 0 ? "↑" : "↓" }} {{ Math.abs(summary.deltaPct) }}%
              </span>
              <span v-else class="cell-meta">较上一周期</span>
            </div>
            <div class="cell">
              <span class="cell-label">峰值利用率</span>
              <strong v-if="peakUsage" class="cell-value" :class="`tone-${peakTone}`">
                {{ peakUsage.pct }}<small>%</small>
              </strong>
              <strong v-else class="cell-value muted">—</strong>
              <span class="cell-meta">{{ peakUsage ? peakUsage.name : "暂无额度" }}</span>
            </div>
          </section>

          <div class="overview-stack">
            <OilPriceCard @configure="activeWorkspace = 'settings'" />
            <TodayWorkCard @open="activeWorkspace = 'daily-report'" />
            <QuotaSummary :views="quotas" @navigate="activeWorkspace = 'subscriptions'" />
          </div>
        </template>

        <DailyReportView v-else-if="activeWorkspace === 'daily-report'" />

        <SnippetLibraryView v-else-if="activeWorkspace === 'snippets'" />

        <ClipboardHistoryView v-else-if="activeWorkspace === 'clipboard'" />

        <VaultView v-else-if="activeWorkspace === 'vault'" />

        <template v-else-if="activeWorkspace === 'analytics'">
          <section v-if="firstLoading" class="stats stats-5" aria-label="加载中">
            <div v-for="i in 5" :key="i" class="skeleton-card">
              <SkeletonBlock width="40%" height="11px" />
              <SkeletonBlock width="66%" height="26px" radius="10px" />
              <SkeletonBlock width="50%" height="11px" />
            </div>
          </section>
          <section v-else class="stats stats-5" aria-label="Token 概要">
            <TokenSummaryCard label="总 TOKEN" :value="summary.total" :delta-pct="summary.deltaPct"
              :spark="sparkTotal" />
            <TokenSummaryCard label="输入 TOKEN" :value="summary.input" :delta-pct="summary.inputDeltaPct"
              :hint="`占总量 ${inputPct}%`" :spark="sparkInput" />
            <TokenSummaryCard label="输出 TOKEN" :value="summary.output" :delta-pct="summary.outputDeltaPct"
              :hint="`占总量 ${outputPct}%`" :spark="sparkOutput" />
            <TokenSummaryCard label="请求数" :value="summary.requests" :delta-pct="summary.requestsDeltaPct"
              :spark="sparkRequests" />
            <TokenSummaryCard label="预估费用" :value="summary.costUsd" format="usd" :delta-pct="summary.costDeltaPct"
              hint="按模型单价估算" />
          </section>

          <section class="block">
            <div class="block-head">
              <div>
                <h2>活跃热力图</h2>
                <p>每日 Token 用量分布（近一年）</p>
              </div>
            </div>
            <div class="heatmap-card">
              <CalendarHeatmap :days="heatmap" />
            </div>
          </section>

          <section class="block">
            <div class="charts">
              <template v-if="chartsReady">
                <TokenTrendChart :data="state.range === 'today' ? todayHourly.trend : trend"
                  :labels="state.range === 'today' ? todayHourly.labels : undefined" />
                <PlatformDailyChart :data="state.range === 'today' ? todayHourly.platform : platformDaily"
                  :labels="state.range === 'today' ? todayHourly.labels : undefined" />
              </template>
              <div v-else class="charts-placeholder">图表将在窗口激活后加载</div>
            </div>
          </section>

          <section class="block">
            <div class="analytics-grid">
              <ToolBreakdown :tools="toolUsage" :range-label="rangeLabel" />
              <ToolDonutChart :tools="toolUsage" />
            </div>
          </section>

          <section class="block">
            <ModelBreakdown :models="modelUsage" :range-label="rangeLabel" />
          </section>
        </template>

        <template v-else-if="activeWorkspace === 'subscriptions'">
          <section class="block quota-section">
            <div class="block-head">
              <div>
                <h2>套餐额度</h2>
                <p>{{ quotaPlatformCount }} 个平台 · {{ quotas.length }} 个套餐的可用额度与重置时间</p>
              </div>
              <Button type="button" variant="outline" size="sm" @click="activeWorkspace = 'settings'">
                显示设置
              </Button>
            </div>
            <div class="quota-grid">
              <QuotaCard v-for="view in quotas" :key="view.id" :view="view" />
            </div>
          </section>
          <div class="subscription-manager-wrap">
            <SubscriptionManager embedded :create-signal="subscriptionCreateSignal" />
          </div>
        </template>

        <SettingsPanel v-else embedded :sync-signal="settingsSyncSignal" @synced="syncEverything" />
      </div>
    </section>

    <AboutDialog v-model:open="aboutOpen" />
  </main>
</template>

<style scoped>
.workspace-shell {
  display: flex;
  width: 100%;
  height: 100vh;
  min-height: 0;
  gap: 14px;
  padding: 16px;
  overflow: hidden;
  background: var(--bg);
}

.rail-wrap {
  position: relative;
  z-index: 20;
  display: flex;
  width: 68px;
  height: 100%;
  flex: 0 0 68px;
}

.rail {
  display: flex;
  width: 100%;
  min-height: 0;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  padding: 14px 0 12px;
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  background: var(--surface);
  box-shadow: var(--shadow-card);
}

.rail-logo {
  display: grid;
  width: 38px;
  height: 38px;
  place-items: center;
  margin-bottom: 10px;
  border-radius: 10px;
  background: var(--accent);
  color: var(--accent-contrast);
  font-family: var(--font-mono);
  font-size: 13px;
  font-weight: 750;
  letter-spacing: -0.04em;
}

.rail-nav {
  display: flex;
  width: 100%;
  min-height: 0;
  flex: 1;
  flex-direction: column;
  align-items: center;
  gap: 3px;
}

.rail-btn {
  position: relative;
  display: grid;
  width: 42px;
  height: 42px;
  flex: 0 0 42px;
  place-items: center;
  padding: 0;
  border: 0;
  border-radius: 10px;
  background: transparent;
  color: var(--text-subtle);
  cursor: pointer;
  transition: background-color 0.16s ease, color 0.16s ease;
}

.rail-btn svg {
  width: 18px;
  height: 18px;
}

.rail-btn:hover {
  background: var(--surface-2);
  color: var(--text);
}

.rail-btn.active {
  background: var(--accent-weak);
  color: var(--accent);
}

.rail-btn.active::before {
  position: absolute;
  top: 10px;
  bottom: 10px;
  left: -13px;
  width: 3px;
  border-radius: 0 3px 3px 0;
  background: var(--accent);
  content: "";
}

.rail-btn:focus-visible,
.sync-chip:focus-visible,
.range-switch button:focus-visible,
.btn-primary:focus-visible,
.cell-meta.link:focus-visible {
  outline: 2px solid var(--accent-ring);
  outline-offset: 2px;
}

.tip {
  position: absolute;
  top: 50%;
  left: 54px;
  z-index: 30;
  padding: 6px 9px;
  transform: translateY(-50%);
  border-radius: 6px;
  background: var(--text);
  box-shadow: var(--shadow-pop);
  color: var(--surface);
  font-size: 11px;
  line-height: 1;
  white-space: nowrap;
  opacity: 0;
  pointer-events: none;
  transition: opacity 0.14s ease;
}

.rail-btn:hover .tip {
  opacity: 1;
}

.rail-divider {
  width: 24px;
  height: 1px;
  margin: 7px 0;
  background: var(--border);
}

.sync-chip {
  display: flex;
  width: 56px;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  margin-top: 3px;
  padding: 8px 2px 7px;
  border: 0;
  border-radius: 10px;
  background: transparent;
  color: var(--text);
  font: inherit;
  cursor: pointer;
}

.sync-chip:hover {
  background: var(--surface-2);
}

.sync-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--u-ok);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--u-ok) 15%, transparent);
}

.sync-chip.warn .sync-dot {
  background: var(--u-warn);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--u-warn) 15%, transparent);
}

.sync-chip.error .sync-dot {
  background: var(--u-crit);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--u-crit) 15%, transparent);
}

.sync-chip.syncing .sync-dot {
  animation: pulse 1s ease-in-out infinite;
}

.sync-chip .t1 {
  max-width: 52px;
  overflow: hidden;
  font-size: 9.5px;
  font-weight: 650;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.sync-chip .t2 {
  color: var(--text-subtle);
  font-size: 9px;
}

.workspace-main {
  display: flex;
  min-width: 0;
  min-height: 0;
  flex: 1;
  flex-direction: column;
  gap: 12px;
}

.topbar {
  display: flex;
  min-height: 64px;
  flex: 0 0 auto;
  align-items: center;
  justify-content: space-between;
  gap: 18px;
  padding: 11px 18px;
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  background: var(--surface);
  box-shadow: var(--shadow-card);
}

.crumb {
  display: flex;
  align-items: center;
  gap: 5px;
  margin-bottom: 2px;
  color: var(--text-subtle);
  font-size: 11px;
}

.crumb span {
  opacity: 0.55;
}

.topbar h1 {
  margin: 0;
  color: var(--text);
  font-size: 18px;
  font-weight: 720;
  letter-spacing: -0.02em;
}

.topbar-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 9px;
}

.rate-field {
  display: flex;
  align-items: center;
  gap: 6px;
  min-height: 34px;
  padding: 0 11px;
  border: 1px solid var(--border);
  border-radius: var(--r-sm);
  background: var(--surface-2);
  color: var(--text-muted);
  font-size: 12px;
  white-space: nowrap;
}

.rate-field:focus-within {
  border-color: var(--accent);
  box-shadow: 0 0 0 3px var(--accent-weak);
}

.rate-field input {
  width: 52px;
  padding: 0;
  border: 0;
  outline: 0;
  background: transparent;
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 12px;
  font-weight: 650;
}

.btn-primary {
  min-height: 34px;
  padding: 0 14px;
  border: 1px solid var(--accent);
  border-radius: var(--r-sm);
  background: var(--accent);
  color: var(--accent-contrast);
  font: inherit;
  font-size: 12.5px;
  font-weight: 650;
  cursor: pointer;
  transition: background-color 0.16s ease, border-color 0.16s ease;
}

.btn-primary:hover {
  border-color: var(--accent-hover);
  background: var(--accent-hover);
}

.range-switch {
  display: flex;
  padding: 3px;
  border: 1px solid var(--border);
  border-radius: var(--r-sm);
  background: var(--surface-2);
}

.range-switch button {
  min-height: 28px;
  padding: 0 12px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--text-muted);
  font: inherit;
  font-size: 12px;
  font-weight: 550;
  cursor: pointer;
}

.range-switch button:hover {
  color: var(--text);
}

.range-switch button.on {
  background: var(--surface);
  box-shadow: var(--shadow-sm);
  color: var(--accent);
  font-weight: 650;
}

.page-scroll {
  min-height: 0;
  flex: 1;
  overflow-y: auto;
  padding: 0 4px 10px 0;
  -ms-overflow-style: none;
  scrollbar-width: none;
}

.page-scroll::-webkit-scrollbar {
  display: none;
}

.snippet-workspace .page-scroll {
  padding: 0;
}

.clipboard-workspace-shell .page-scroll {
  overflow: hidden;
}

.overview-stack {
  display: grid;
  gap: 12px;
  margin-top: 14px;
}

.banner {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 12px;
  padding: 10px 13px;
  border: 1px solid var(--border);
  border-radius: var(--r-sm);
  background: var(--surface);
  font-size: 12px;
  cursor: pointer;
}

.banner-dot {
  width: 7px;
  height: 7px;
  flex: 0 0 7px;
  border-radius: 50%;
}

.banner-text {
  min-width: 0;
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.banner-hint {
  flex: 0 0 auto;
  font-weight: 600;
}

.banner-error {
  border-color: color-mix(in srgb, var(--u-crit) 35%, var(--border));
  background: color-mix(in srgb, var(--u-crit) 8%, var(--surface));
  color: var(--u-crit);
}

.banner-warn {
  border-color: color-mix(in srgb, var(--u-warn) 35%, var(--border));
  background: color-mix(in srgb, var(--u-warn) 8%, var(--surface));
  color: var(--u-warn);
}

.banner-empty {
  color: var(--text-muted);
}

.banner-error .banner-dot {
  background: var(--u-crit);
}

.banner-warn .banner-dot {
  background: var(--u-warn);
}

.banner-empty .banner-dot {
  background: var(--accent);
}

.cockpit {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  margin-bottom: 12px;
  overflow: hidden;
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  background: var(--surface);
  box-shadow: var(--shadow-card);
}

.cell {
  position: relative;
  display: flex;
  min-width: 0;
  min-height: 108px;
  flex-direction: column;
  gap: 7px;
  padding: 15px 18px;
  border-left: 1px solid var(--border);
}

.cell:first-child {
  border-left: 0;
}

.cell:nth-child(-n + 2) {
  background: color-mix(in srgb, var(--accent) 3%, var(--surface));
}

.cell:nth-child(-n + 2)::before {
  position: absolute;
  top: 0;
  right: 18px;
  left: 18px;
  height: 2px;
  border-radius: 0 0 2px 2px;
  background: var(--accent);
  content: "";
  opacity: 0.72;
}

.cell-label {
  color: var(--text-subtle);
  font-size: 11px;
  font-weight: 650;
  letter-spacing: 0.03em;
}

.cell-value {
  max-width: 100%;
  overflow: hidden;
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 24px;
  font-weight: 720;
  line-height: 1.2;
  letter-spacing: -0.04em;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-variant-numeric: tabular-nums;
}

.cell-value small {
  margin-left: 1px;
  font-size: 13px;
}

.cell-value.muted {
  color: var(--text-subtle);
}

.cell-value.tone-ok {
  color: var(--u-ok);
}

.cell-value.tone-warn {
  color: var(--u-warn);
}

.cell-value.tone-crit {
  color: var(--u-crit);
}

.cell-delta {
  width: fit-content;
  padding: 2px 7px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--u-ok) 12%, transparent);
  color: var(--u-ok);
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 650;
}

.cell-delta.up {
  background: color-mix(in srgb, var(--u-warn) 12%, transparent);
  color: var(--u-warn);
}

.cell-meta {
  color: var(--text-subtle);
  font-size: 11.5px;
}

.cell-meta.link {
  width: fit-content;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--accent);
  font: inherit;
  font-size: 11.5px;
  font-weight: 600;
  text-align: left;
  cursor: pointer;
}

.cell-meta.link:hover {
  text-decoration: underline;
  text-underline-offset: 3px;
}

.block {
  margin-top: 16px;
}

.block-head {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 11px;
}

.block-head h2 {
  margin: 0;
  color: var(--text);
  font-size: 15px;
  font-weight: 680;
}

.block-head p {
  margin: 3px 0 0;
  color: var(--text-subtle);
  font-size: 11.5px;
}

.quota-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 12px;
  align-items: stretch;
}

.stats {
  display: grid;
  gap: 12px;
}

.stats-5 {
  grid-template-columns: repeat(5, minmax(0, 1fr));
}

.skeleton-card {
  display: flex;
  min-height: 106px;
  flex-direction: column;
  gap: 12px;
  padding: 18px;
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  background: var(--surface);
  box-shadow: var(--shadow-card);
}

.heatmap-card {
  padding: 18px 20px;
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  background: var(--surface);
  box-shadow: var(--shadow-card);
}

.charts {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
}

.charts > * {
  min-width: 0;
}

.charts-placeholder {
  grid-column: 1 / -1;
  display: grid;
  min-height: 220px;
  place-items: center;
  border: 1px dashed var(--border-strong);
  border-radius: var(--r-lg);
  background: var(--surface);
  color: var(--text-subtle);
  font-size: 12px;
}

.analytics-grid {
  display: grid;
  grid-template-columns: minmax(0, 1.55fr) minmax(300px, 1fr);
  gap: 12px;
}

.subscription-manager-wrap {
  margin-top: 16px;
}

.quota-section {
  margin-top: 0;
}

@keyframes pulse {
  50% {
    opacity: 0.3;
  }
}

@media (max-width: 1180px) {
  .cockpit,
  .stats-5,
  .quota-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .cockpit .cell:nth-child(odd) {
    border-left: 0;
  }

  .cockpit .cell:nth-child(n + 3) {
    border-top: 1px solid var(--border);
  }
}

@media (max-width: 980px) {
  .analytics-grid,
  .charts {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 760px) {
  .workspace-shell {
    height: 100vh;
    flex-direction: column;
    gap: 10px;
    padding: 10px;
  }

  .rail-wrap {
    width: 100%;
    height: 58px;
    flex: 0 0 58px;
  }

  .rail {
    flex-direction: row;
    gap: 3px;
    padding: 7px 8px;
    overflow-x: auto;
    border-radius: var(--r-md);
  }

  .rail-logo {
    width: 36px;
    height: 36px;
    flex: 0 0 36px;
    margin: 0 5px 0 0;
  }

  .rail-nav {
    width: auto;
    flex-direction: row;
  }

  .rail-btn {
    width: 38px;
    height: 38px;
    flex-basis: 38px;
  }

  .rail-btn.active::before {
    top: auto;
    right: 10px;
    bottom: -7px;
    left: 10px;
    width: auto;
    height: 3px;
    border-radius: 3px 3px 0 0;
  }

  .tip,
  .rail-divider,
  .sync-chip .t2 {
    display: none;
  }

  .sync-chip {
    width: 44px;
    flex: 0 0 44px;
    padding: 5px 2px;
  }

  .topbar {
    min-height: auto;
    align-items: flex-start;
    padding: 11px 13px;
    border-radius: var(--r-md);
  }

  .topbar-actions {
    flex-wrap: wrap;
  }

  .page-scroll {
    padding-right: 0;
  }
}

@media (max-width: 620px) {
  .topbar {
    flex-direction: column;
  }

  .topbar-actions {
    width: 100%;
    justify-content: flex-start;
  }

  .rate-field {
    flex: 1;
  }

  .cockpit,
  .stats-5,
  .quota-grid {
    grid-template-columns: 1fr;
  }

  .cockpit .cell {
    border-top: 1px solid var(--border);
    border-left: 0;
  }

  .cockpit .cell:first-child {
    border-top: 0;
  }

  .block-head {
    align-items: flex-start;
    flex-direction: column;
  }
}
</style>
