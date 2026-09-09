<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { usageService } from "@/services/usage-service";
import type { PlatformQuotaView, SyncInfo } from "@/types/usage";

const PANEL_WIDTH = 314;
const popoverEl = ref<HTMLElement | null>(null);
const quotas = ref<PlatformQuotaView[]>([]);
const sync = ref<SyncInfo>({ lastSyncAt: null, syncing: false });
const requesting = ref(false);
const unlisteners: UnlistenFn[] = [];

/**
 * 高度自适应：卡片随内容自然撑开、列表不滚动，渲染后把窗口尺寸精确贴到卡片高度，
 * 否则原生 vibrancy 会在卡片下方留出一段磨砂空白。
 */
async function fitWindowHeight(): Promise<void> {
  await nextTick();
  const height = Math.ceil(popoverEl.value?.offsetHeight ?? 0);
  if (height > 0) {
    await getCurrentWindow().setSize(new LogicalSize(PANEL_WIDTH, height)).catch(() => undefined);
  }
}

/** 平台圆点配色，对齐原型（logoClass → 品牌色）。 */
const DOT_COLOR: Record<string, string> = {
  codex: "#4C7CF3",
  ark: "#F2924A",
  kiro: "#7C6BF0",
  qoder: "#EF5A82",
  open: "#30C55A",
};

interface QuotaRow {
  key: string;
  name: string;
  planTag: string;
  color: string;
  pct: number | null;
  exhausted: boolean;
  note: string | null;
}

/** 复用主应用同款口径：有积分算积分利用率，否则取利用率最高的窗口。 */
const rows = computed<QuotaRow[]>(() =>
  quotas.value.map((v) => {
    let pct: number | null = null;
    if (v.credits) {
      pct =
        v.credits.total > 0
          ? Math.round(((v.credits.total - v.credits.remaining) / v.credits.total) * 100)
          : 0;
    } else if (v.windows.length) {
      pct = v.windows.reduce((a, b) => (b.usedPct > a.usedPct ? b : a)).usedPct;
    }
    return {
      key: v.platform,
      name: v.name,
      planTag: v.planTag,
      color: DOT_COLOR[v.logoClass] ?? "#8A8A8E",
      pct,
      exhausted: pct != null && pct >= 100,
      note: pct == null ? (v.account ? "已登录" : "暂无数据") : null,
    };
  }),
);

/** 面板与主窗口同源、共享 localStorage；直接从存储读取最新聚合结果。 */
function refresh(): void {
  quotas.value = usageService.getPlatformQuotaViews();
  sync.value = usageService.getSyncInfo();
  if (!sync.value.syncing) requesting.value = false;
  void fitWindowHeight();
}

const syncing = computed(() => requesting.value || sync.value.syncing);
const tone = computed<"ok" | "warn" | "error" | "syncing">(() => {
  if (syncing.value) return "syncing";
  if (sync.value.outcome === "failed") return "error";
  if (sync.value.outcome === "partial") return "warn";
  return "ok";
});
const statusText = computed(() => {
  if (syncing.value) return "同步中";
  if (!sync.value.lastSyncAt) return "未同步";
  const diff = Date.now() - new Date(sync.value.lastSyncAt).getTime();
  const min = Math.floor(diff / 60000);
  const relative =
    min < 1
      ? "刚刚同步"
      : min < 60
        ? `${min} 分钟前`
        : min < 24 * 60
          ? `${Math.floor(min / 60)} 小时前`
          : "较久前";
  return sync.value.outcome === "partial" ? `${relative} · 部分` : relative;
});
const hasData = computed(() => rows.value.length > 0);

async function openDashboard(): Promise<void> {
  await invoke("show_dashboard").catch(() => undefined);
  await getCurrentWindow().hide().catch(() => undefined);
}

async function requestSync(): Promise<void> {
  if (syncing.value) return;
  requesting.value = true;
  // 同步链路（连接器 + 本机 CLI）运行在主窗口的 composable 里，面板仅发起请求。
  await emit("tray:request-sync").catch(() => {
    requesting.value = false;
  });
}

function quit(): void {
  void invoke("quit_app");
}

onMounted(async () => {
  refresh();
  const win = getCurrentWindow();
  // 主窗口每次同步前后广播 usage-updated，面板据此重新读取存储。
  unlisteners.push(await listen("usage-updated", () => refresh()));
  // 失焦即收起，并留下时间戳供托盘点击去抖（避免点击图标时“隐藏又立即弹出”）。
  unlisteners.push(
    await win.onFocusChanged(({ payload: focused }) => {
      if (focused) {
        refresh();
      } else {
        localStorage.setItem("tray:lastHiddenAt", String(Date.now()));
        void win.hide();
      }
    }),
  );
});

onUnmounted(() => {
  for (const off of unlisteners) off();
});
</script>

<template>
  <main ref="popoverEl" class="popover">
    <!-- 磨砂玻璃细颗粒纹理 -->
    <div class="grain" aria-hidden="true" />

    <header class="pop-head">
      <div class="pop-title">
        <div class="pop-logo">/_</div>
        <h1>AI 用量</h1>
      </div>
      <button
        class="sync-pill"
        type="button"
        :class="tone"
        :title="sync.error ?? '点击立即同步'"
        @click="requestSync"
      >
        <span class="dot" />{{ statusText }}
      </button>
    </header>

    <div class="pop-divider" />

    <div v-if="hasData" class="quota-list">
      <div v-for="r in rows" :key="r.key" class="qrow">
        <div class="qrow-top">
          <span class="qdot" :style="{ background: r.color }" />
          <span class="qname">{{ r.name }}</span>
          <span class="qtag">{{ r.planTag }}</span>
        </div>
        <div class="qbar-row">
          <div class="qtrack">
            <div
              v-if="r.pct != null"
              class="qfill"
              :class="r.exhausted ? 'exhausted' : 'ok'"
              :style="r.exhausted ? undefined : { width: `${Math.min(100, Math.max(2, r.pct))}%` }"
            />
          </div>
          <div class="qpct" :class="{ exh: r.exhausted, muted: r.pct == null }">
            {{ r.pct == null ? (r.note ?? "—") : r.exhausted ? "已耗尽" : `${r.pct}%` }}
          </div>
        </div>
      </div>
    </div>
    <div v-else class="empty" @click="requestSync">
      <p>暂无额度数据</p>
      <small>点击「立即同步」从本机 CLI 拉取</small>
    </div>

    <div class="pop-divider" />

    <div class="pop-actions">
      <button class="act-btn" type="button" @click="openDashboard">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
          <rect x="3" y="3" width="7" height="9" rx="1.5" />
          <rect x="14" y="3" width="7" height="5" rx="1.5" />
          <rect x="14" y="12" width="7" height="9" rx="1.5" />
          <rect x="3" y="16" width="7" height="5" rx="1.5" />
        </svg>
        打开看板
      </button>
      <button
        class="act-btn primary"
        type="button"
        :class="{ syncing }"
        :disabled="syncing"
        @click="requestSync"
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M21 12a9 9 0 1 1-3-6.7" />
          <path d="M21 3v6h-6" />
        </svg>
        {{ syncing ? "同步中" : "立即同步" }}
      </button>
      <button class="act-btn quit" type="button" @click="quit">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
          <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4" />
          <path d="M16 17l5-5-5-5" />
          <path d="M21 12H9" />
        </svg>
        退出
      </button>
    </div>
  </main>
</template>

<style scoped>
.popover {
  --ink: #1d1d1f;
  --ink-soft: #48484d;
  --muted: #8a8a8e;
  --hairline: rgba(0, 0, 0, 0.085);
  --accent: #5a5fef;
  --green: #30c55a;
  --green-deep: #209948;
  --red: #ff3b30;
  --tint: rgba(246, 247, 250, 0.6);
  --chip: rgba(0, 0, 0, 0.045);
  --track: rgba(0, 0, 0, 0.08);
  --hover: rgba(0, 0, 0, 0.035);
  --press: rgba(0, 0, 0, 0.06);

  position: relative;
  display: flex;
  width: 100vw;
  /* 高度随内容自然撑开；再由 JS 把窗口尺寸精确贴到此高度 */
  flex-direction: column;
  overflow: hidden;
  border-radius: 14px;
  background: var(--tint);
  color: var(--ink);
  font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "PingFang SC",
    var(--font-sans), sans-serif;
  /* 仅内圈高光 + 发丝描边（外阴影由原生窗口投射） */
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.5),
    inset 0 0 0 0.5px rgba(255, 255, 255, 0.6);
  -webkit-font-smoothing: antialiased;
}

.grain {
  position: absolute;
  inset: 0;
  z-index: 0;
  opacity: 0.035;
  mix-blend-mode: overlay;
  pointer-events: none;
  background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='90' height='90'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='2' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)'/%3E%3C/svg%3E");
}

.pop-head {
  position: relative;
  z-index: 1;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 13px 14px 12px;
}
.pop-title {
  display: flex;
  align-items: center;
  gap: 9px;
}
.pop-logo {
  display: flex;
  width: 24px;
  height: 24px;
  align-items: center;
  justify-content: center;
  border-radius: 7px;
  background: linear-gradient(155deg, #6f79f5, #4c51e0);
  color: #fff;
  font-family: ui-monospace, "SF Mono", "JetBrains Mono", monospace;
  font-size: 10.5px;
  font-weight: 600;
  letter-spacing: -1px;
  box-shadow:
    0 1px 3px rgba(76, 81, 224, 0.4),
    inset 0 0 0 0.5px rgba(255, 255, 255, 0.25);
}
.pop-title h1 {
  margin: 0;
  font-size: 13.5px;
  font-weight: 600;
  letter-spacing: -0.01em;
}

.sync-pill {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px 4px 8px;
  border: 0;
  border-radius: 20px;
  background: rgba(48, 197, 90, 0.12);
  color: var(--green-deep);
  font: inherit;
  font-size: 11px;
  font-weight: 500;
  cursor: pointer;
  transition: filter 0.12s ease;
}
.sync-pill:hover {
  filter: brightness(0.97);
}
.sync-pill .dot {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: var(--green);
}
.sync-pill.warn {
  background: rgba(255, 149, 0, 0.14);
  color: #b25e00;
}
.sync-pill.warn .dot {
  background: #ff9500;
}
.sync-pill.error {
  background: rgba(255, 59, 48, 0.12);
  color: #d70015;
}
.sync-pill.error .dot {
  background: var(--red);
}
.sync-pill.syncing {
  background: rgba(90, 95, 239, 0.12);
  color: var(--accent);
}
.sync-pill.syncing .dot {
  background: var(--accent);
  animation: pulse 1s ease-in-out infinite;
}
@keyframes pulse {
  50% {
    opacity: 0.3;
  }
}

.pop-divider {
  position: relative;
  z-index: 1;
  height: 0.5px;
  background: var(--hairline);
}

.quota-list {
  position: relative;
  z-index: 1;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  scrollbar-width: thin;
  scrollbar-color: rgba(0, 0, 0, 0.16) transparent;
}
.quota-list::-webkit-scrollbar {
  width: 4px;
}
.quota-list::-webkit-scrollbar-thumb {
  border-radius: 10px;
  background: rgba(0, 0, 0, 0.16);
}
.quota-list::-webkit-scrollbar-track {
  background: transparent;
}

.qrow {
  padding: 11px 14px 12px;
  transition: background 0.1s ease;
}
.qrow:hover {
  background: var(--hover);
}
.qrow + .qrow {
  border-top: 0.5px solid var(--hairline);
}
.qrow-top {
  display: flex;
  align-items: center;
  gap: 7px;
  margin-bottom: 8px;
}
.qdot {
  width: 7px;
  height: 7px;
  flex-shrink: 0;
  border-radius: 50%;
}
.qname {
  font-size: 13px;
  font-weight: 600;
  color: var(--ink);
}
.qtag {
  padding: 2px 7px;
  border-radius: 6px;
  background: var(--chip);
  color: var(--muted);
  font-size: 9.5px;
  font-weight: 500;
  letter-spacing: 0.01em;
}
.qbar-row {
  display: flex;
  align-items: center;
  gap: 10px;
}
.qtrack {
  flex: 1;
  height: 5px;
  overflow: hidden;
  border-radius: 20px;
  background: var(--track);
  box-shadow: inset 0 0.5px 1.5px rgba(0, 0, 0, 0.08);
}
.qfill {
  height: 100%;
  border-radius: 20px;
  transition: width 0.4s ease;
}
.qfill.ok {
  background: linear-gradient(90deg, #3dd16a, var(--green-deep));
}
.qfill.exhausted {
  width: 100%;
  background: linear-gradient(90deg, #ff5a50, #e0231b);
}
.qpct {
  width: 38px;
  flex-shrink: 0;
  text-align: right;
  color: var(--ink-soft);
  font-size: 12px;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}
.qpct.exh {
  width: auto;
  color: var(--red);
  font-size: 11px;
  white-space: nowrap;
}
.qpct.muted {
  width: auto;
  color: var(--muted);
  font-size: 10.5px;
  font-weight: 500;
  white-space: nowrap;
}

.empty {
  position: relative;
  z-index: 1;
  display: grid;
  flex: 1;
  place-content: center;
  gap: 4px;
  padding: 24px;
  text-align: center;
  cursor: pointer;
}
.empty p {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
  color: var(--ink);
}
.empty small {
  color: var(--muted);
  font-size: 11px;
}

.pop-actions {
  position: relative;
  z-index: 1;
  display: flex;
}
.act-btn {
  position: relative;
  display: flex;
  height: 42px;
  flex: 1;
  align-items: center;
  justify-content: center;
  gap: 6px;
  border: 0;
  background: transparent;
  color: var(--ink-soft);
  font: inherit;
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: background 0.1s ease;
}
.act-btn svg {
  width: 13px;
  height: 13px;
  opacity: 0.75;
}
.act-btn + .act-btn::before {
  content: "";
  position: absolute;
  left: 0;
  top: 9px;
  bottom: 9px;
  width: 0.5px;
  background: var(--hairline);
}
.act-btn:hover {
  background: var(--hover);
}
.act-btn:active {
  background: var(--press);
}
.act-btn:first-child {
  border-bottom-left-radius: 14px;
}
.act-btn:last-child {
  border-bottom-right-radius: 14px;
}
.act-btn.primary {
  color: var(--accent);
  font-weight: 600;
}
.act-btn.primary svg {
  opacity: 1;
}
.act-btn.primary.syncing svg {
  animation: spin 0.8s linear infinite;
}
.act-btn.quit {
  color: var(--muted);
}
.act-btn:disabled {
  cursor: default;
}
@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

/* 系统暗色外观：vibrancy 变深，翻转墨色/发丝线/色块以保持可读 */
@media (prefers-color-scheme: dark) {
  .popover {
    --ink: #f5f5f7;
    --ink-soft: #c7c7cc;
    --muted: #8e8e93;
    --hairline: rgba(255, 255, 255, 0.1);
    --tint: rgba(28, 30, 38, 0.45);
    --chip: rgba(255, 255, 255, 0.1);
    --track: rgba(255, 255, 255, 0.14);
    --hover: rgba(255, 255, 255, 0.06);
    --press: rgba(255, 255, 255, 0.1);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.12),
      inset 0 0 0 0.5px rgba(255, 255, 255, 0.14);
  }
  .qtrack {
    box-shadow: inset 0 0.5px 1.5px rgba(0, 0, 0, 0.3);
  }
  .quota-list {
    scrollbar-color: rgba(255, 255, 255, 0.2) transparent;
  }
  .quota-list::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.2);
  }
}
</style>
