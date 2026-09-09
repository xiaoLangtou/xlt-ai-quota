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
import CalendarHeatmap from "@/components/CalendarHeatmap.vue";
import TokenSummaryCard from "@/components/TokenSummaryCard.vue";
import SettingsPanel from "@/components/SettingsPanel.vue";
import SubscriptionManager from "@/components/SubscriptionManager.vue";
import { isTauriDesktop } from "@/connectors/types";
import { formatTokens } from "@/utils/format";
import type { RangePreset } from "@/types/usage";

const {
  state,
  quotas,
  summary,
  trend,
  platformDaily,
  toolUsage,
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

const TokenTrendChart = defineAsyncComponent(() => import("@/components/TokenTrendChart.vue"));
const PlatformDailyChart = defineAsyncComponent(
  () => import("@/components/PlatformDailyChart.vue"),
);

type WorkspaceSection = "overview" | "analytics" | "subscriptions" | "settings";
const activeWorkspace = ref<WorkspaceSection>("overview");
const subscriptionCreateSignal = ref(0);
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

const isSyncing = computed(() => loading.value || state.sync.syncing);
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
    <div class="orbs" aria-hidden="true">
      <i class="orb o1" /><i class="orb o2" /><i class="orb o3" /><i class="orb o4" />
    </div>

    <aside class="rail-wrap">
      <div class="rail">
        <div class="rail-logo">/_</div>
        <nav class="rail-nav" aria-label="主导航">
          <button class="rail-btn" :class="{ active: activeWorkspace === 'overview' }" type="button" @click="activeWorkspace = 'overview'">
            <span class="tip">用量概览</span>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="7" height="9" rx="2"/><rect x="14" y="3" width="7" height="5" rx="2"/><rect x="14" y="12" width="7" height="9" rx="2"/><rect x="3" y="16" width="7" height="5" rx="2"/></svg>
          </button>
          <button class="rail-btn" :class="{ active: activeWorkspace === 'analytics' }" type="button" @click="activeWorkspace = 'analytics'">
            <span class="tip">用量分析</span>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M4 19V10"/><path d="M11 19V5"/><path d="M18 19v-7"/><path d="M3 19h18"/></svg>
          </button>
          <button class="rail-btn" :class="{ active: activeWorkspace === 'subscriptions' }" type="button" @click="activeWorkspace = 'subscriptions'">
            <span class="tip">订阅与账单</span>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="2.5" y="5.5" width="19" height="14" rx="3"/><path d="M2.5 10h19"/><path d="M6.5 15h4"/></svg>
          </button>
          <button class="rail-btn" :class="{ active: activeWorkspace === 'settings' }" type="button" @click="activeWorkspace = 'settings'">
            <span class="tip">连接与设置</span>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M8 3v4"/><path d="M16 3v4"/><path d="M4 10h16"/><rect x="4" y="6" width="16" height="15" rx="3"/><path d="M9 15.5l2 2 4-4.5"/></svg>
          </button>
        </nav>
        <div class="rail-divider" />
        <button class="rail-btn" type="button" :title="`主题：${themeLabel}（点击切换）`" @click="cycleTheme">
          <span class="tip">{{ themeLabel }}</span>
          <svg v-if="themePref === 'light'" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><circle cx="12" cy="12" r="4.2"/><path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M19.1 4.9L17 7M7 17l-2.1 2.1"/></svg>
          <svg v-else-if="themePref === 'dark'" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M20 14.5A8 8 0 1 1 9.5 4a6.3 6.3 0 0 0 10.5 10.5Z"/></svg>
          <svg v-else viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="4" width="18" height="12" rx="2"/><path d="M8 20h8"/><path d="M12 16v4"/></svg>
        </button>
        <button class="sync-chip" type="button" :class="{ syncing: isSyncing, error: syncError && !isPartialSync, warn: isPartialSync }" :title="syncError ? `${isPartialSync ? '部分数据未更新' : '同步出错'}：${syncError}` : '点击立即同步'" @click="syncNow">
          <i class="sync-dot" />
          <span class="t1">{{ syncText }}</span>
          <span class="t2">点击同步</span>
        </button>
      </div>
    </aside>

    <section class="workspace-main content">
      <header class="topbar">
        <div>
          <div class="crumb">用量看板 <span>/</span> {{ pageMeta.crumb }}</div>
          <h1>{{ pageMeta.title }}</h1>
        </div>
        <div class="topbar-actions">
          <div v-if="activeWorkspace === 'analytics'" class="range-switch" role="group" aria-label="统计周期">
            <button v-for="r in RANGES" :key="r.preset" type="button" :class="{ on: state.range === r.preset }" @click="setRange(r.preset)">{{ r.label }}</button>
          </div>
          <template v-else-if="activeWorkspace === 'subscriptions'">
            <label class="rate-field">1 USD = ¥ <input v-model.number="exchangeRate" type="number" min="0.01" step="0.01" @change="saveExchangeRate" /></label>
            <button class="btn-primary" type="button" @click="subscriptionCreateSignal += 1">+ 新增订阅</button>
          </template>
          <button v-else-if="activeWorkspace === 'settings'" class="btn-primary" type="button" @click="syncNow">立即同步</button>
        </div>
      </header>
      <div class="page-scroll">
        <template v-if="activeWorkspace === 'overview'">
          <div v-if="syncError && !isPartialSync" class="banner banner-error" @click="syncNow">
            <span class="banner-dot" />
            <span class="banner-text">同步出错：{{ syncError }}</span>
            <span class="banner-hint">点击重试</span>
          </div>
          <div v-else-if="syncError" class="banner banner-warn" @click="syncNow">
            <span class="banner-dot" />
            <span class="banner-text">部分数据未更新：{{ syncIssueText }}</span>
            <span class="banner-hint">点击重试</span>
          </div>
          <div v-else-if="!state.sync.lastSyncAt && !hasData" class="banner banner-empty" @click="syncNow">
            <span class="banner-dot" />
            <span class="banner-text">暂无数据 · 从本机 CLI 拉取真实用量</span>
            <span class="banner-hint">立即同步</span>
          </div>

          <header class="page-head">
            <div class="page-title">
              <span class="eyebrow">OVERVIEW</span>
              <h1>用量概览</h1>
            </div>
          </header>

          <!-- 驾驶舱仪表条：一眼看清消耗、支出、平台数与峰值 -->
          <section class="cockpit" aria-label="关键指标">
            <div class="cell">
              <span class="cell-label">{{ rangeLabel }} TOKEN</span>
              <strong class="cell-value">{{ formatTokens(summary.total) }}</strong>
              <span
                v-if="summary.deltaPct != null"
                class="cell-delta"
                :class="summary.deltaPct >= 0 ? 'up' : 'down'"
              >
                {{ summary.deltaPct >= 0 ? "↑" : "↓" }} {{ Math.abs(summary.deltaPct) }}%
              </span>
              <span v-else class="cell-meta">较上一周期</span>
            </div>
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
              <span class="cell-label">峰值利用率</span>
              <strong v-if="peakUsage" class="cell-value" :class="`tone-${peakTone}`">
                {{ peakUsage.pct }}<small>%</small>
              </strong>
              <strong v-else class="cell-value muted">—</strong>
              <span class="cell-meta">{{ peakUsage ? peakUsage.name : "暂无额度" }}</span>
            </div>
          </section>

          <QuotaSummary :views="quotas" @navigate="activeWorkspace = 'subscriptions'" />
        </template>

        <template v-else-if="activeWorkspace === 'analytics'">
          <header class="page-head">
            <div class="page-title">
              <span class="eyebrow">USAGE ANALYTICS</span>
              <h1>用量分析</h1>
            </div>
            <div class="range-switch" role="group" aria-label="统计周期">
              <button
                v-for="r in RANGES"
                :key="r.preset"
                type="button"
                :class="{ on: state.range === r.preset }"
                @click="setRange(r.preset)"
              >
                {{ r.label }}
              </button>
            </div>
          </header>
          <section class="stats stats-4" aria-label="Token 概要">
            <TokenSummaryCard label="总 TOKEN" :value="summary.total" :delta-pct="summary.deltaPct" :spark="sparkTotal" />
            <TokenSummaryCard label="输入 TOKEN" :value="summary.input" :delta-pct="summary.inputDeltaPct" :hint="`占总量 ${inputPct}%`" :spark="sparkInput" />
            <TokenSummaryCard label="输出 TOKEN" :value="summary.output" :delta-pct="summary.outputDeltaPct" :hint="`占总量 ${outputPct}%`" :spark="sparkOutput" />
            <TokenSummaryCard label="请求数" :value="summary.requests" :delta-pct="summary.requestsDeltaPct" :spark="sparkRequests" />
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
                <TokenTrendChart
                  :data="state.range === 'today' ? todayHourly.trend : trend"
                  :labels="state.range === 'today' ? todayHourly.labels : undefined"
                />
                <PlatformDailyChart
                  :data="state.range === 'today' ? todayHourly.platform : platformDaily"
                  :labels="state.range === 'today' ? todayHourly.labels : undefined"
                />
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
        </template>

        <template v-else-if="activeWorkspace === 'subscriptions'">
          <section class="block">
            <div class="block-head">
              <div>
                <h2>平台额度</h2>
                <p>{{ quotas.length }} 个平台的套餐、额度与重置时间</p>
              </div>
            </div>
            <div class="quota-grid">
              <QuotaCard v-for="view in quotas" :key="view.platform" :view="view" />
            </div>
          </section>
          <div class="subscription-manager-wrap">
            <SubscriptionManager embedded :create-signal="subscriptionCreateSignal" />
          </div>
        </template>

        <SettingsPanel v-else embedded @synced="syncNow" />
      </div>
    </section>
  </main>
</template>

<style scoped>
.workspace-shell {
  display: flex;
  width: 100%;
  height: 100vh;
  min-height: 0;
}

/* ---- 侧边栏 ---- */
.brand-lockup {
  display: flex;
  align-items: center;
  gap: 11px;
  padding: 4px 10px 0;
}
.brand-mark {
  display: grid;
  width: 36px;
  height: 36px;
  place-items: center;
  border-radius: 11px;
  background: var(--accent);
  color: var(--accent-contrast);
  font-family: var(--font-mono);
  font-size: 14px;
  font-weight: 700;
  letter-spacing: -1px;
  box-shadow: 0 4px 12px var(--accent-weak);
}
.brand-lockup strong {
  display: block;
  color: var(--text);
  font-size: 15px;
  letter-spacing: -0.3px;
}
.brand-lockup small {
  display: block;
  margin-top: 3px;
  color: var(--text-subtle);
  font-family: var(--font-mono);
  font-size: 9px;
  font-weight: 600;
  letter-spacing: 1px;
}
.workspace-nav {
  display: grid;
  gap: 3px;
  margin-top: 34px;
}
.nav-badge {
  min-width: 20px;
  margin-left: auto;
  padding: 1px 6px;
  border-radius: 999px;
  background: var(--accent-weak);
  color: var(--accent);
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 600;
  text-align: center;
}
.nav-icon {
  position: relative;
  width: 15px;
  height: 15px;
  flex: 0 0 15px;
  border: 1.6px solid currentColor;
  border-radius: 4px;
  opacity: 0.9;
}
.overview-icon::after {
  position: absolute;
  right: 2px;
  bottom: 2px;
  left: 2px;
  height: 4px;
  border-top: 1.6px solid currentColor;
  content: "";
}
.subscription-icon {
  border-radius: 50%;
}
.subscription-icon::after {
  position: absolute;
  top: 2px;
  right: 2px;
  width: 3.5px;
  height: 3.5px;
  border-radius: 50%;
  background: currentColor;
  content: "";
}
.settings-icon {
  border-radius: 50%;
}
.settings-icon::after {
  position: absolute;
  inset: 4px;
  border-radius: 50%;
  background: currentColor;
  content: "";
}
.analytics-icon::after {
  position: absolute;
  right: 3px;
  bottom: 3px;
  left: 3px;
  height: 6px;
  border-left: 1.6px solid currentColor;
  border-bottom: 1.6px solid currentColor;
  content: "";
}

.sidebar-footer {
  display: grid;
  gap: 8px;
  margin-top: auto;
  padding-top: 14px;
  border-top: 1px solid var(--border);
}
.theme-toggle {
  display: flex;
  align-items: center;
  gap: 9px;
  width: 100%;
  padding: 8px 10px;
  border: 1px solid transparent;
  border-radius: var(--r-sm);
  background: transparent;
  color: var(--text-muted);
  font: inherit;
  font-size: 12px;
  font-weight: 550;
  cursor: pointer;
  transition: background-color 0.15s ease, color 0.15s ease;
}
.theme-toggle:hover {
  background: var(--surface-2);
  color: var(--text);
}
.theme-toggle svg {
  width: 16px;
  height: 16px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.8;
  stroke-linecap: round;
  stroke-linejoin: round;
}
.sync-status {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 9px 10px;
  border: 1px solid var(--border);
  border-radius: var(--r-sm);
  background: var(--surface-2);
  color: var(--text);
  font: inherit;
  text-align: left;
  cursor: pointer;
  transition: border-color 0.15s ease, background-color 0.15s ease;
}
.sync-status:hover {
  border-color: var(--accent);
}
.sync-dot {
  width: 8px;
  height: 8px;
  flex: 0 0 8px;
  border-radius: 50%;
  background: var(--u-ok);
}
.sync-status.warn .sync-dot {
  background: var(--u-warn);
}
.sync-status.error .sync-dot {
  background: var(--u-crit);
}
.sync-status.syncing .sync-dot {
  background: var(--accent);
  animation: pulse 1s ease-in-out infinite;
}
@keyframes pulse {
  50% {
    opacity: 0.3;
  }
}
.sync-copy {
  display: flex;
  min-width: 0;
  flex-direction: column;
  line-height: 1.3;
}
.sync-copy b {
  overflow: hidden;
  font-size: 12px;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.sync-copy em {
  color: var(--text-subtle);
  font-size: 10px;
  font-style: normal;
}

/* ---- 主区 ---- */
.workspace-main {
  display: flex;
  min-width: 0;
  min-height: 0;
  flex: 1;
  flex-direction: column;
}
.page-scroll {
  min-height: 0;
  flex: 1;
  overflow-y: auto;
  padding: 32px clamp(24px, 4vw, 56px) 48px;
  background: var(--bg);
}
.page-scroll > :first-child {
  margin-top: 0;
}

.page-head {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 20px;
  margin-bottom: 22px;
}
.eyebrow {
  display: block;
  margin-bottom: 6px;
  color: var(--text-subtle);
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 1.5px;
}
.page-title h1 {
  margin: 0;
  font-size: 27px;
  font-weight: 700;
  letter-spacing: -0.6px;
}

/* ---- 驾驶舱仪表条 ---- */
.cockpit {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  margin-bottom: 30px;
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  background: var(--surface);
  box-shadow: var(--shadow-card);
  overflow: hidden;
}
.cell {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 7px;
  padding: 16px 18px;
  border-left: 1px solid var(--border);
}
.cell:first-child {
  border-left: 0;
}
.cell-label {
  color: var(--text-subtle);
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.8px;
}
.cell-value {
  max-width: 100%;
  overflow: hidden;
  font-family: var(--font-mono);
  font-size: 21px;
  font-weight: 650;
  line-height: 1.1;
  letter-spacing: -0.6px;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-variant-numeric: tabular-nums;
}
.cell-value small {
  margin-left: 1px;
  color: var(--text-subtle);
  font-size: 12px;
  letter-spacing: 0;
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
  font-family: var(--font-mono);
  font-size: 12px;
  font-weight: 600;
}
.cell-delta.up {
  color: var(--u-warn);
}
.cell-delta.down {
  color: var(--u-ok);
}
.cell-meta {
  color: var(--text-subtle);
  font-size: 12px;
}
.cell-meta.link {
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--accent);
  font: inherit;
  text-align: left;
  cursor: pointer;
}
.cell-meta.link:hover {
  text-decoration: underline;
}

/* ---- 区块 ---- */
.block {
  margin-top: 34px;
}
.block-head {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 20px;
  margin-bottom: 16px;
}
.block-head h2 {
  margin: 0;
  font-size: 18px;
  font-weight: 650;
  letter-spacing: -0.3px;
}
.block-head p {
  margin: 6px 0 0;
  color: var(--text-muted);
  font-size: 13px;
}

.quota-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 16px;
  align-items: stretch;
}
@media (max-width: 1240px) {
  .quota-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
@media (max-width: 560px) {
  .quota-grid {
    grid-template-columns: 1fr;
  }
}

.range-switch {
  display: flex;
  flex: 0 0 auto;
  padding: 3px;
  border: 1px solid var(--border);
  border-radius: var(--r-md);
  background: var(--surface-2);
}
.range-switch button {
  padding: 6px 14px;
  border: 0;
  border-radius: calc(var(--r-md) - 4px);
  background: transparent;
  color: var(--text-muted);
  font: inherit;
  font-size: 12.5px;
  font-weight: 600;
  cursor: pointer;
  transition: background-color 0.15s ease, color 0.15s ease;
}
.range-switch button:hover {
  color: var(--text);
}
.range-switch button.on {
  background: var(--accent);
  color: var(--accent-contrast);
}

.stats {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 1px;
  overflow: hidden;
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  background: var(--border);
  box-shadow: var(--shadow-card);
}
.stats-4 {
  grid-template-columns: repeat(4, minmax(0, 1fr));
}
.heatmap-card {
  padding: 18px 20px;
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  background: var(--surface);
  box-shadow: var(--shadow-card);
}
.analytics-grid {
  display: grid;
  grid-template-columns: minmax(0, 1.5fr) minmax(320px, 1fr);
  gap: 16px;
}
.analytics-note {
  margin: 34px 0 0;
  padding: 12px 16px;
  border: 1px dashed var(--border-strong);
  border-radius: var(--r-md);
  color: var(--text-subtle);
  font-size: 12px;
  line-height: 1.6;
}
.charts {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 16px;
  min-width: 0;
}
@media (max-width: 980px) {
  .analytics-grid {
    grid-template-columns: 1fr;
  }
}
@media (max-width: 1100px) {
  .stats-4 {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
@media (max-width: 560px) {
  .stats-4 {
    grid-template-columns: 1fr;
  }
}
.charts > * {
  min-width: 0;
}
.charts-placeholder {
  grid-column: 1 / -1;
  display: grid;
  place-items: center;
  min-height: 200px;
  border: 1px dashed var(--border-strong);
  border-radius: var(--r-lg);
  color: var(--text-subtle);
  font-size: 13px;
}

/* ---- 提示横幅 ---- */
.banner {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 20px;
  padding: 11px 16px;
  border-radius: var(--r-md);
  border: 1px solid var(--border);
  font-size: 13px;
  cursor: pointer;
}
.banner-dot {
  width: 8px;
  height: 8px;
  flex-shrink: 0;
  border-radius: 50%;
}
.banner-text {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.banner-hint {
  flex-shrink: 0;
  font-size: 12px;
  opacity: 0.75;
}
.banner-error {
  border-color: color-mix(in srgb, var(--u-crit) 40%, transparent);
  background: color-mix(in srgb, var(--u-crit) 12%, transparent);
  color: var(--u-crit);
}
.banner-error .banner-dot {
  background: var(--u-crit);
}
.banner-warn {
  border-color: color-mix(in srgb, var(--u-warn) 40%, transparent);
  background: color-mix(in srgb, var(--u-warn) 12%, transparent);
  color: var(--u-warn);
}
.banner-warn .banner-dot {
  background: var(--u-warn);
}
.banner-empty {
  border-color: var(--border-strong);
  background: var(--surface);
  color: var(--text-muted);
}
.banner-empty .banner-dot {
  background: var(--accent);
}

/* ---- 响应式 ---- */
@media (max-width: 1160px) {
  .cockpit {
    grid-template-columns: repeat(2, 1fr);
  }
  .cell:nth-child(3) {
    border-left: 0;
  }
  .cell:nth-child(n + 3) {
    border-top: 1px solid var(--border);
  }
}
@media (max-width: 980px) {
  .charts {
    grid-template-columns: 1fr;
  }
}
@media (max-width: 720px) {
  .stats {
    grid-template-columns: 1fr;
  }
}
@media (max-width: 760px) {
  .workspace-shell {
    display: block;
    height: auto;
    min-height: 0;
  }
  .sidebar {
    position: static;
    width: 100%;
    height: auto;
    padding: 12px 16px;
    border-right: 0;
    border-bottom: 1px solid var(--border);
  }
  .brand-lockup {
    display: none;
  }
  .workspace-nav {
    display: flex;
    gap: 4px;
    margin: 0;
    overflow-x: auto;
  }
  .workspace-nav :deep(.cn-button) {
    width: auto;
    flex: 0 0 auto;
  }
  .sidebar-footer {
    display: none;
  }
  .page-scroll {
    overflow: visible;
    padding: 20px 16px 36px;
  }
  .cockpit {
    grid-template-columns: 1fr;
  }
  .cell {
    border-left: 0;
    border-top: 1px solid var(--border);
  }
  .cell:first-child {
    border-top: 0;
  }
  .block-head {
    flex-direction: column;
    align-items: flex-start;
  }
}
@media (max-width: 520px) {
  .cockpit {
    grid-template-columns: 1fr;
  }
  .cell:nth-child(3) {
    border-left: 0;
  }
}

/* Glass prototype — exact shell, rail, top bar and page rhythm. */
.workspace-shell {
  position: relative;
  z-index: 1;
  display: flex;
  width: 100%;
  height: 100vh;
  min-height: 0;
  gap: 16px;
  padding: 20px;
  overflow: hidden;
  background: transparent;
}
.orbs {
  position: fixed;
  inset: 0;
  z-index: -1;
  overflow: hidden;
  pointer-events: none;
}
.orb {
  position: absolute;
  display: block;
  border-radius: 50%;
  filter: blur(100px);
  opacity: var(--orb-opacity);
}
.orb.o1 { top: -240px; left: -200px; width: 600px; height: 600px; background: radial-gradient(circle at 30% 30%, #b9c6fb, transparent 70%); }
.orb.o2 { top: -140px; right: -180px; width: 520px; height: 520px; background: radial-gradient(circle at 60% 40%, #f6c9de, transparent 70%); }
.orb.o3 { bottom: -280px; left: 38%; width: 560px; height: 560px; background: radial-gradient(circle at 50% 50%, #bfe9e0, transparent 70%); }
.orb.o4 { right: 10%; bottom: -200px; width: 380px; height: 380px; background: radial-gradient(circle at 50% 50%, #d8cbfb, transparent 70%); }
.rail-wrap {
  position: relative;
  z-index: 20;
  display: flex;
  width: 70px;
  height: 100%;
  flex: 0 0 70px;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 20px;
}
.rail {
  display: flex;
  width: 100%;
  flex-direction: column;
  align-items: center;
  gap: 5px;
  padding: 16px 0;
  border: 1px solid var(--glass-border);
  border-radius: 30px;
  background: var(--glass-fill);
  box-shadow: var(--glass-shadow);
  backdrop-filter: blur(20px) saturate(180%);
  -webkit-backdrop-filter: blur(20px) saturate(180%);
}
.rail-logo {
  display: flex;
  width: 38px;
  height: 38px;
  align-items: center;
  justify-content: center;
  margin-bottom: 12px;
  border-radius: 12px;
  background: linear-gradient(145deg, #6e7bf2, #5b5fef);
  box-shadow: 0 4px 12px rgba(91, 95, 239, 0.34);
  color: #fff;
  font-family: var(--font-mono);
  font-size: 13.5px;
  font-weight: 600;
}
.rail-nav {
  display: flex;
  width: 100%;
  flex-direction: column;
  align-items: center;
  gap: 5px;
}
.rail-btn {
  position: relative;
  display: flex;
  width: 44px;
  height: 44px;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  border-radius: 14px;
  background: transparent;
  color: var(--text-muted);
  cursor: pointer;
  transition: background 0.18s ease, color 0.18s ease, transform 0.12s ease;
}
.rail-btn svg { width: 18px; height: 18px; }
.rail-btn:hover { background: var(--icon-hover-bg); color: var(--text); }
.rail-btn.active {
  background: var(--segment-active);
  box-shadow: 0 4px 12px rgba(91, 95, 239, 0.18), inset 0 1px 0 #fff;
  color: #5b5fef;
}
.rail-btn .tip {
  position: absolute;
  top: 50%;
  left: 56px;
  z-index: 5;
  padding: 6px 10px;
  transform: translateY(-50%);
  border-radius: 8px;
  background: #20233a;
  color: #fff;
  font-size: 11.5px;
  white-space: nowrap;
  opacity: 0;
  pointer-events: none;
  transition: opacity 0.15s ease;
}
.rail-btn:hover .tip { opacity: 1; }
.rail-divider { width: 24px; height: 1px; margin: 9px 0; background: var(--border); }
.sync-chip {
  display: flex;
  width: 58px;
  flex-direction: column;
  align-items: center;
  gap: 5px;
  margin-top: 4px;
  padding: 10px 3px 8px;
  border: 0;
  border-radius: 14px;
  background: transparent;
  box-shadow: none;
  color: var(--text);
  font: inherit;
  cursor: pointer;
  transition: background 0.18s ease;
}
.sync-chip:hover { background: var(--icon-hover-bg); }
.sync-dot { width: 7px; height: 7px; border-radius: 50%; background: #2fbe8f; box-shadow: 0 0 0 3px rgba(47, 190, 143, 0.16); }
.sync-chip.warn .sync-dot { background: var(--u-warn); box-shadow: 0 0 0 3px rgba(242, 146, 74, 0.16); }
.sync-chip.error .sync-dot { background: var(--u-crit); box-shadow: 0 0 0 3px rgba(233, 85, 107, 0.16); }
.sync-chip.syncing .sync-dot { animation: pulse 1s ease-in-out infinite; }
.sync-chip .t1 { max-width: 52px; overflow: hidden; color: var(--text); font-size: 10.5px; font-weight: 600; text-align: center; text-overflow: ellipsis; white-space: nowrap; }
.sync-chip .t2 { color: var(--text-subtle); font-size: 9.5px; }
.content,
.workspace-main {
  position: relative;
  z-index: 1;
  display: flex;
  min-width: 0;
  min-height: 0;
  flex: 1;
  flex-direction: column;
  gap: 16px;
  background: transparent;
}
.topbar {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  justify-content: space-between;
  padding: 17px 26px;
  border: 1px solid var(--glass-border);
  border-radius: 20px;
  background: var(--glass-fill-strong);
  box-shadow: var(--glass-shadow);
  backdrop-filter: blur(20px) saturate(180%);
  -webkit-backdrop-filter: blur(20px) saturate(180%);
}
.crumb { display: flex; align-items: center; gap: 5px; margin-bottom: 3px; color: var(--text-subtle); font-size: 12px; }
.crumb span { opacity: 0.5; }
.topbar h1 { margin: 0; font-size: 19.5px; font-weight: 800; letter-spacing: -0.01em; }
.topbar-actions { display: flex; align-items: center; gap: 9px; }
.range-switch {
  display: flex;
  padding: 4px;
  border: 1px solid var(--border);
  border-radius: 13px;
  background: var(--segment-bg);
}
.range-switch button { padding: 7px 14px; border: 0; border-radius: 9px; background: transparent; color: var(--text-muted); font: inherit; font-size: 12.5px; font-weight: 500; cursor: pointer; }
.range-switch button.on { background: var(--segment-active); box-shadow: var(--shadow-sm); color: var(--text); font-weight: 600; }
.rate-field { display: flex; align-items: center; gap: 7px; padding: 7px 13px; border: 1px solid var(--glass-border); border-radius: 12px; background: var(--glass-control); color: var(--text-muted); font-size: 12.5px; }
.rate-field input { width: 48px; padding: 0; border: 0; outline: 0; background: transparent; color: var(--text); font-family: var(--font-mono); font-size: 12.5px; font-weight: 600; }
.btn-primary { padding: 9px 16px; border: 0; border-radius: 12px; background: linear-gradient(145deg, #6e7bf2, #5257e6); box-shadow: 0 6px 16px rgba(82, 87, 230, 0.3); color: #fff; font: inherit; font-size: 13px; font-weight: 600; cursor: pointer; }
.page-scroll {
  min-height: 0;
  flex: 1;
  overflow-y: auto;
  padding: 0 0 8px;
  background: transparent;
  scrollbar-width: none;
}
.page-scroll::-webkit-scrollbar { width: 0; height: 0; }
.page-head,
.banner { display: none; }
.cockpit {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 16px;
  margin: 0 0 16px;
  overflow: visible;
  border: 0;
  border-radius: 0;
  background: transparent;
  box-shadow: none;
}
.cell {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 9px;
  padding: 21px 24px;
  border: 1px solid var(--glass-border);
  border-radius: 20px;
  background: var(--glass-fill);
  box-shadow: var(--glass-shadow);
  backdrop-filter: blur(18px) saturate(180%);
  -webkit-backdrop-filter: blur(18px) saturate(180%);
}
.cell:first-child,
.cell:nth-child(3),
.cell:nth-child(n + 3) { border: 1px solid var(--glass-border); }
.cell-label { color: var(--text-subtle); font-family: var(--font-sans); font-size: 12.5px; font-weight: 500; letter-spacing: 0; }
.cell-value { color: var(--text); font-family: var(--font-mono); font-size: 27px; font-weight: 800; line-height: normal; letter-spacing: -0.02em; }
.cell-value.tone-crit { color: #d8405c; }
.cell-value small { color: inherit; font-size: inherit; }
.cell-delta { width: fit-content; padding: 3px 9px; border-radius: 20px; background: rgba(47, 190, 143, 0.14); color: #1e9a76; font-family: var(--font-mono); font-size: 11px; font-weight: 600; }
.cell-delta.up { background: rgba(233, 85, 107, 0.14); color: #d8405c; }
.cell-meta { color: var(--text-subtle); font-size: 12.5px; }
.cell-meta.link { color: #5b5fef; font-size: 12.5px; font-weight: 600; }
.stats,
.stats-4 {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 16px;
  overflow: visible;
  border: 0;
  border-radius: 0;
  background: transparent;
  box-shadow: none;
}
.block { margin-top: 16px; }
.stats + .block { padding: 21px 26px 26px; border: 1px solid var(--glass-border); border-radius: 20px; background: var(--glass-fill); box-shadow: var(--glass-shadow); backdrop-filter: blur(18px) saturate(180%); }
.stats + .block .block-head { margin: 0 0 12px; }
.stats + .block .heatmap-card { padding: 0; border: 0; border-radius: 0; background: transparent; box-shadow: none; }
.block-head h2 { font-size: 16.5px; font-weight: 700; }
.block-head p { margin-top: 3px; color: var(--text-subtle); font-size: 12.5px; }
.analytics-grid { grid-template-columns: minmax(0, 1.55fr) minmax(0, 1fr); gap: 16px; }
.analytics-note { display: none; }
.quota-grid { grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 16px; }
.page-scroll > .block:first-child { margin-top: 0; }
.page-scroll > .block:first-child > .block-head { display: none; }
.subscription-manager-wrap { margin-top: 16px; }

@media (max-width: 1100px) {
  .cockpit,
  .stats-4,
  .quota-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .analytics-grid,
  .charts { grid-template-columns: 1fr; }
}
</style>
