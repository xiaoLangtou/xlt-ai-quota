<script setup lang="ts">
import { computed, defineAsyncComponent, onMounted, onUnmounted, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { useUsageDashboard } from "@/composables/useUsageDashboard";
import { useSubscriptions } from "@/composables/useSubscriptions";
import QuotaCard from "@/components/QuotaCard.vue";
import QuotaSummary from "@/components/QuotaSummary.vue";
import ToolBreakdown from "@/components/ToolBreakdown.vue";
import ToolDonutChart from "@/components/ToolDonutChart.vue";
import ModelBreakdown from "@/components/ModelBreakdown.vue";
import CalendarHeatmap from "@/components/CalendarHeatmap.vue";
import TokenSummaryCard from "@/components/TokenSummaryCard.vue";
import SettingsPanel from "@/components/SettingsPanel.vue";
import SubscriptionManager from "@/components/SubscriptionManager.vue";
import SkeletonBlock from "@/components/SkeletonBlock.vue";
import TodayWorkCard from "@/components/TodayWorkCard.vue";
import OilPriceCard from "@/components/OilPriceCard.vue";
import { useOilMonitor } from "@/composables/useOilMonitor";
import { isTauriDesktop } from "@/connectors/types";
import { formatTokens } from "@/utils/format";
import { USAGE_PAGE_META } from "@/config/navigation";
import type { RangePreset } from "@/types/usage";

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
const { sync: syncOil } = useOilMonitor();

const TokenTrendChart = defineAsyncComponent(() => import("@/components/TokenTrendChart.vue"));
const PlatformDailyChart = defineAsyncComponent(
  () => import("@/components/PlatformDailyChart.vue"),
);

type WorkspaceSection = "overview" | "analytics" | "subscriptions" | "settings";

/** 工作区 ↔ 路由名映射：用量与设置留在本组件，工具页已拆成独立 View。 */
const SECTION_BY_ROUTE: Record<string, WorkspaceSection> = {
  dashboard: "overview",
  analytics: "analytics",
  subscriptions: "subscriptions",
  settings: "settings",
};
const route = useRoute();
const router = useRouter();
const activeWorkspace = computed<WorkspaceSection>(
  () => SECTION_BY_ROUTE[String(route.name)] ?? "overview",
);
function go(name: "analytics" | "subscriptions" | "settings" | "daily-report"): void {
  void router.push({ name });
}
const subscriptionCreateSignal = ref(0);
const settingsSyncSignal = ref(0);
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

const pageMeta = computed(() => USAGE_PAGE_META[String(route.name)] ?? USAGE_PAGE_META.dashboard);

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
  <UDashboardPanel id="usage">
    <template #header>
      <UDashboardNavbar :title="pageMeta.title" :description="pageMeta.group">
        <template #leading>
          <UDashboardSidebarCollapse />
        </template>
        <template #right>
          <template v-if="activeWorkspace === 'overview'">
            <span class="hidden sm:inline-flex items-center gap-1.5 text-xs text-muted">
              <i class="size-1.5 rounded-full" :class="quotaPlatformCount ? 'bg-primary' : 'bg-dimmed'" />
              {{ quotaPlatformCount || "0" }} 个平台已连接
            </span>
            <span class="hidden md:inline text-xs text-dimmed">{{ syncText }}</span>
            <UButton
              icon="i-lucide-refresh-cw"
              color="neutral"
              variant="outline"
              size="sm"
              :loading="isSyncing"
              @click="syncEverything"
            >
              {{ isSyncing ? "同步中…" : "同步全部" }}
            </UButton>
          </template>
          <div v-else-if="activeWorkspace === 'analytics'" class="range-switch" role="group" aria-label="统计周期">
            <button v-for="r in RANGES" :key="r.preset" type="button" :class="{ on: state.range === r.preset }"
              @click="setRange(r.preset)">{{ r.label }}</button>
          </div>
          <template v-else-if="activeWorkspace === 'subscriptions'">
            <label class="rate-field">1 USD = ¥ <input v-model.number="exchangeRate" type="number" min="0.01"
                step="0.01" @change="saveExchangeRate" /></label>
            <UButton icon="i-lucide-plus" size="sm" @click="subscriptionCreateSignal += 1">新增订阅</UButton>
          </template>
          <UButton v-else-if="activeWorkspace === 'settings'" size="sm"
            @click="settingsSyncSignal += 1">立即同步</UButton>
        </template>
      </UDashboardNavbar>
    </template>
    <template #body>
      <div class="workspace-main">
      <div class="page-scroll">
        <template v-if="activeWorkspace === 'overview'">
          <UAlert
            v-if="syncError && !isPartialSync"
            color="error"
            variant="subtle"
            icon="i-lucide-triangle-alert"
            title="同步出错"
            :description="syncError"
            class="cursor-pointer"
            @click="syncEverything"
          />
          <UAlert
            v-else-if="syncError"
            color="warning"
            variant="subtle"
            icon="i-lucide-triangle-alert"
            title="部分数据未更新"
            :description="syncIssueText"
            class="cursor-pointer"
            @click="syncEverything"
          />
          <UAlert
            v-else-if="!state.sync.lastSyncAt && !hasData"
            color="neutral"
            variant="subtle"
            icon="i-lucide-cloud-download"
            title="暂无数据 · 从本机 CLI 拉取真实用量"
            description="点击立即同步"
            class="cursor-pointer"
            @click="syncEverything"
          />

          <!-- 账务指标优先，用量指标作为补充运营信号。 -->
          <section class="cockpit" aria-label="关键指标">
            <div class="cell">
              <span class="cell-label">订阅月支出 · 计划</span>
              <strong class="cell-value">{{ formatCny(monthlyCny) }}</strong>
              <button class="cell-meta link" type="button" @click="go('subscriptions')">
                {{ activeSubs.length }} 项活跃订阅 →
              </button>
            </div>
            <div class="cell">
              <span class="cell-label">本月实际支出 · 已记</span>
              <strong class="cell-value">{{ formatCny(actualSpend) }}</strong>
              <button class="cell-meta link" type="button" @click="go('subscriptions')">
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
            <OilPriceCard @configure="go('settings')" />
            <TodayWorkCard @open="go('daily-report')" />
            <QuotaSummary :views="quotas" @navigate="go('subscriptions')" />
          </div>
        </template>

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
              <UButton color="neutral" variant="outline" size="sm" @click="go('settings')">
                显示设置
              </UButton>
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
      </div>
    </template>
  </UDashboardPanel>
</template>

<style scoped>
.range-switch button:focus-visible,
.cell-meta.link:focus-visible {
  outline: 2px solid var(--accent-ring);
  outline-offset: 2px;
}
.workspace-main {
  display: flex;
  min-width: 0;
  min-height: 0;
  flex: 1;
  flex-direction: column;
  gap: 12px;
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
.overview-stack {
  display: grid;
  grid-template-columns: minmax(0, 1.1fr) minmax(0, 1fr) minmax(0, 1fr);
  gap: 14px;
  margin-top: 14px;
}
.overview-stack > * {
  min-width: 0;
}
.cockpit {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 14px;
  margin-bottom: 14px;
}
.cell {
  position: relative;
  display: flex;
  min-width: 0;
  min-height: 104px;
  flex-direction: column;
  gap: 7px;
  padding: 16px 18px;
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  background: var(--surface);
  box-shadow: var(--shadow-card);
}
.cell-label {
  color: var(--text-subtle);
  font-size: 11px;
  font-weight: 650;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
.cell-value {
  max-width: 100%;
  overflow: hidden;
  margin-top: 2px;
  color: var(--text);
  font-size: 27px;
  font-weight: 750;
  line-height: 1.15;
  letter-spacing: -0.02em;
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
  font-size: 10.5px;
  font-weight: 650;
}
.cell-delta.up {
  background: color-mix(in srgb, var(--u-warn) 12%, transparent);
  color: var(--u-warn);
}
.cell-meta {
  color: var(--text-muted);
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
  grid-template-columns: repeat(3, minmax(0, 1fr));
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
.stats-5 {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
.overview-stack {
    grid-template-columns: 1fr;
  }
.quota-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (max-width: 980px) {
.analytics-grid,
.charts {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 760px) {
.page-scroll {
    padding-right: 0;
  }
}

@media (max-width: 620px) {
.rate-field {
    flex: 1;
  }
.cockpit,
.stats-5,
.quota-grid {
    grid-template-columns: 1fr;
  }
.block-head {
    align-items: flex-start;
    flex-direction: column;
  }
}
</style>
