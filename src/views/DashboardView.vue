<script setup lang="ts">
import { computed, defineAsyncComponent, onMounted, onUnmounted, ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { useUsageDashboard } from "@/composables/useUsageDashboard";
import QuotaCard from "@/components/QuotaCard.vue";
import TokenSummaryCard from "@/components/TokenSummaryCard.vue";
import SettingsPanel from "@/components/SettingsPanel.vue";
import { isTauriDesktop } from "@/connectors/types";
import type { RangePreset } from "@/types/usage";

const {
  state,
  quotas,
  summary,
  trend,
  platformDaily,
  loading,
  syncError,
  setRange,
  sync: syncNow,
} = useUsageDashboard();

const TokenTrendChart = defineAsyncComponent(() => import("@/components/TokenTrendChart.vue"));
const PlatformDailyChart = defineAsyncComponent(
  () => import("@/components/PlatformDailyChart.vue"),
);

const showSettings = ref(false);
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
];

const syncText = computed(() => {
  if (loading.value || state.sync.syncing) return "同步中…";
  if (!state.sync.lastSyncAt) return "未同步";
  const diff = Date.now() - new Date(state.sync.lastSyncAt).getTime();
  const min = Math.floor(diff / 60000);
  const relative =
    min < 1
      ? "刚刚同步"
      : min < 60
        ? `${min} 分钟前同步`
        : min < 24 * 60
          ? `${Math.floor(min / 60)} 小时前同步`
          : "较久前同步";
  return state.sync.outcome === "partial" ? `${relative} · 部分更新` : relative;
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
const isPartialSync = computed(() => state.sync.outcome === "partial");
const syncIssueText = computed(() =>
  (syncError.value ?? "").replace(/^部分连接器同步失败：\s*/, ""),
);
</script>

<template>
  <main class="app-shell">
    <header>
      <div class="title-group">
        <span class="eyebrow">USAGE OVERVIEW</span>
        <h1>AI 用量看板</h1>
        <p>统一查看各平台订阅额度与 Token 使用情况</p>
      </div>
      <div class="head-actions">
        <button class="settings-btn" @click="showSettings = true">设置</button>
        <button
          type="button"
          class="sync"
          :class="{ syncing: loading || state.sync.syncing }"
          :title="syncError ? `${isPartialSync ? '部分数据未更新' : '同步出错'}：${syncError}` : '点击立即同步'"
          @click="syncNow"
        >
          <i />{{ syncText }}
        </button>
      </div>
    </header>

    <div v-if="syncError && !isPartialSync" class="error-banner" @click="syncNow">
      <span class="err-dot" />
      <span class="err-text">同步出错：{{ syncError }}</span>
      <span class="err-hint">点击重试</span>
    </div>

    <div v-else-if="syncError" class="partial-banner" @click="syncNow">
      <span class="partial-dot" />
      <span class="err-text">部分数据未更新：{{ syncIssueText }}</span>
      <span class="err-hint">点击重试</span>
    </div>

    <div
      v-else-if="!state.sync.lastSyncAt && !quotas.some((q) => q.windows.length || q.credits)"
      class="empty-banner"
      @click="syncNow"
    >
      暂无数据 · 点击右上角「同步」从本机 CLI 拉取真实用量
    </div>

    <section>
      <div class="head">
        <h2>订阅额度</h2>
        <span>{{ quotas.length }} 个平台</span>
      </div>
      <div class="quota-grid">
        <QuotaCard
          v-for="view in quotas"
          :key="view.platform"
          :view="view"
        />
      </div>
    </section>

    <section class="token">
      <div class="head">
        <h2>Token 使用量</h2>
        <div class="toolbar">
          <button
            v-for="r in RANGES"
            :key="r.preset"
            :class="{ on: state.range === r.preset }"
            @click="setRange(r.preset)"
          >
            {{ r.label }}
          </button>
        </div>
      </div>

      <div class="stats">
        <TokenSummaryCard
          label="总 Token"
          :value="summary.total"
          :delta-pct="summary.deltaPct"
        />
        <TokenSummaryCard
          label="输入 Token"
          :value="summary.input"
          :hint="`占总用量 ${inputPct}%`"
        />
        <TokenSummaryCard
          label="输出 Token"
          :value="summary.output"
          :hint="`占总用量 ${outputPct}%`"
        />
      </div>

      <div class="charts">
        <template v-if="chartsReady">
          <TokenTrendChart :data="trend" />
          <PlatformDailyChart :data="platformDaily" />
        </template>
      </div>
    </section>

    <SettingsPanel v-model:open="showSettings" @synced="syncNow" />
  </main>
</template>

<style scoped>
main {
  width: min(1240px, calc(100% - 56px));
  margin: auto;
  padding: 34px 0 48px;
}
header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  min-height: 126px;
  margin-bottom: 30px;
  padding: 25px 28px;
  border: 1px solid rgba(222, 229, 240, 0.9);
  border-radius: 20px;
  background: linear-gradient(115deg, #ffffff 10%, #f6f9ff 100%);
  box-shadow: 0 12px 30px rgba(49, 72, 114, 0.045);
}
.eyebrow {
  display: block;
  margin-bottom: 6px;
  color: var(--blue);
  font-size: 10px;
  font-weight: 800;
  letter-spacing: 1.1px;
}
h1 {
  margin: 0;
  font-size: 28px;
  letter-spacing: -0.9px;
}
header p {
  margin: 6px 0 0;
  color: var(--muted);
  font-size: 14px;
}
.sync {
  display: inline-flex;
  align-items: center;
  padding: 9px 12px;
  border: 1px solid #dce8f7;
  border-radius: 10px;
  background: #f4f8ff;
  color: #527197;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  user-select: none;
}
.head-actions {
  display: flex;
  align-items: center;
  gap: 16px;
}
.settings-btn {
  padding: 9px 14px;
  border: 1px solid var(--line);
  border-radius: 10px;
  background: var(--card);
  color: var(--text);
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
}
.settings-btn:hover {
  border-color: var(--blue);
  color: var(--blue);
}
.sync:hover {
  border-color: #b9d4f7;
  background: #edf5ff;
}
.error-banner {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 22px;
  padding: 11px 16px;
  border-radius: 12px;
  background: #fdece6;
  border: 1px solid #f6cbbf;
  color: #b53b28;
  font-size: 13px;
  cursor: pointer;
}
.partial-banner {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 22px;
  padding: 11px 16px;
  border-radius: 12px;
  background: #fff6e5;
  border: 1px solid #f5dcaa;
  color: #9a650b;
  font-size: 13px;
  cursor: pointer;
}
.empty-banner {
  margin-bottom: 22px;
  padding: 11px 16px;
  border-radius: 12px;
  background: #eef4ff;
  border: 1px solid #d4e4ff;
  color: #2f6fd0;
  font-size: 13px;
  cursor: pointer;
}
.partial-banner .err-hint {
  color: #9a650b;
}
.partial-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--orange);
  flex-shrink: 0;
}
.err-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #d65745;
  flex-shrink: 0;
}
.err-text {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.err-hint {
  color: #b53b28;
  font-size: 12px;
  opacity: 0.7;
  flex-shrink: 0;
}
.sync i {
  display: inline-block;
  width: 8px;
  height: 8px;
  margin-right: 8px;
  border-radius: 50%;
  background: var(--green);
}
.sync.syncing i {
  background: var(--orange);
  animation: pulse 1s ease-in-out infinite;
}
@keyframes pulse {
  50% {
    opacity: 0.35;
  }
}
.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 14px;
}
h2 {
  margin: 0;
  font-size: 19px;
  letter-spacing: -0.4px;
}
.head > span {
  color: var(--muted);
  font-size: 13px;
}
.quota-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 16px;
  align-items: start;
}
.token {
  margin-top: 32px;
}
.toolbar {
  display: flex;
  padding: 3px;
  border: 1px solid #e5eaf2;
  border-radius: 11px;
  background: #f0f3f8;
}
.toolbar button {
  padding: 7px 13px;
  border: 0;
  border-radius: 7px;
  background: transparent;
  color: #727886;
  font-weight: 600;
  font-size: 13px;
  cursor: pointer;
}
.toolbar button.on {
  color: white;
  background: var(--blue);
  box-shadow: 0 2px 6px rgba(37, 131, 235, 0.28);
}
.stats {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 14px;
  margin-top: 14px;
}
.charts {
  display: grid;
  grid-template-columns: 1.08fr 0.92fr;
  gap: 14px;
  margin-top: 14px;
}
@media (min-width: 1320px) {
  .quota-grid {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }
}
@media (max-width: 1100px) {
  .stats {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
  .charts {
    grid-template-columns: 1fr;
  }
}
@media (max-width: 900px) {
  .quota-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
@media (max-width: 720px) {
  .stats {
    grid-template-columns: 1fr;
  }
}
@media (max-width: 600px) {
  main {
    width: calc(100% - 28px);
    padding-top: 20px;
  }
  header {
    align-items: flex-start;
    min-height: 0;
    padding: 20px;
  }
  .sync {
    padding: 9px;
    font-size: 0;
  }
  .sync i {
    margin: 0;
  }
  .head {
    display: block;
  }
  .toolbar {
    width: max-content;
    margin-top: 13px;
  }
  .quota-grid,
  .stats {
    grid-template-columns: 1fr;
  }
}
</style>
