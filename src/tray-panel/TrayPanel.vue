<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import ToolLogo from "@/components/ToolLogo.vue";
import { usageService } from "@/services/usage-service";
import type { PlatformQuotaView, QuotaDisplayTarget, SyncInfo } from "@/types/usage";

const PANEL_WIDTH = 360;
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
  const height = Math.min(Math.ceil(popoverEl.value?.offsetHeight ?? 0), 720);
  if (height > 0) {
    await getCurrentWindow().setSize(new LogicalSize(PANEL_WIDTH, height)).catch(() => undefined);
  }
}

interface QuotaRow {
  key: string;
  platform: string;
  name: string;
  planTag: string;
  pct: number | null;
  exhausted: boolean;
  tone: "ok" | "warn" | "critical" | "muted";
  detail: string;
  reset: string | null;
  valueText: string;
}

function money(value: number, currency: string): string {
  const symbol = currency === "USD" ? "$" : "¥";
  return `${symbol}${value.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
}

/** 复用主应用同款口径：有积分算积分利用率，否则取利用率最高的窗口。 */
const rows = computed<QuotaRow[]>(() =>
  quotas.value.map((v) => {
    let pct: number | null = null;
    let detail = v.account ? "已登录" : "暂无额度数据";
    let reset: string | null = null;
    let valueText = "未读取";
    let tone: QuotaRow["tone"] = "muted";
    if (v.balance) {
      detail = `余额 · 赠金 ${money(v.balance.granted, v.balance.currency)}`;
      valueText = money(v.balance.total, v.balance.currency);
      tone = v.balance.available ? "ok" : "critical";
    } else if (v.credits) {
      const used = Math.max(0, v.credits.total - v.credits.remaining);
      pct =
        v.credits.total > 0
          ? Math.round((used / v.credits.total) * 100)
          : 0;
      detail = `已用 ${used.toLocaleString()} / ${v.credits.total.toLocaleString()} Credits`;
      reset = compactReset(v.credits.expiresIn ?? v.credits.refreshIn ?? v.windows[0]?.resetsIn);
      valueText = pct == null ? "未读取" : `${pct}%`;
      tone = pct != null && pct >= 100 ? "critical" : pct != null && pct >= 80 ? "warn" : "ok";
    } else if (v.windows.length) {
      const window = v.windows.reduce((a, b) => (b.usedPct > a.usedPct ? b : a));
      pct = window.usedPct;
      detail = window.label;
      reset = compactReset(window.resetsIn);
      valueText = pct >= 100 ? "已耗尽" : `${pct}%`;
      tone = pct >= 100 ? "critical" : pct >= 80 ? "warn" : "ok";
    }
    const exhausted = pct != null && pct >= 100;
    return {
      key: v.id,
      platform: v.platform,
      name: v.name,
      planTag: v.planTag,
      pct,
      exhausted,
      tone,
      detail,
      reset,
      valueText,
    };
  }),
);

function compactReset(value?: string): string | null {
  if (!value) return null;
  return value
    .replace(/(\d+) 小时 (\d+) 分钟后重置/, "$1h $2m 后")
    .replace(/(\d+) 小时后重置/, "$1h 后")
    .replace(/(\d+) 分钟后重置/, "$1m 后")
    .replace(/(\d+) 天后重置/, "$1d 后")
    .replace("后重置", "后")
    .replace("后刷新", "后")
    .replace("后到期", "后");
}

/** 面板与主窗口同源；从磁盘重新读取额度快照，避免托盘内存与主窗口不同步。 */
async function refresh(): Promise<void> {
  await usageService.reloadQuotas().catch(() => undefined);
  const hidden = new Set(usageService.getQuotaDisplayConfigView().hiddenPlatforms);
  quotas.value = usageService.getPlatformQuotaViews().filter((view) => {
    if (view.platform === "ark") {
      const target = view.planTag.includes("Coding")
        ? "ark-coding"
        : view.planTag.includes("Agent")
          ? "ark-agent"
          : null;
      return !target || !hidden.has(target);
    }
    return !hidden.has(view.platform as QuotaDisplayTarget);
  });
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
const availableCount = computed(() => rows.value.filter((row) => row.pct != null).length);
const tightest = computed(
  () =>
    [...rows.value]
      .filter((row) => row.pct != null)
      .sort((a, b) => (b.pct ?? 0) - (a.pct ?? 0))[0] ?? null,
);
/** 临时：预览页用 localStorage 强制暗色，便于无头截图走查（不影响 Tauri 运行时）。 */
const previewDark = typeof localStorage !== "undefined" && localStorage.getItem("xlt.tray-preview-theme") === "dark";

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

const onStorage = (event: StorageEvent): void => {
  if (!event.key || event.key === "xlt.config") void refresh();
};

onMounted(async () => {
  await usageService.init().catch(() => undefined);
  await refresh();
  window.addEventListener("storage", onStorage);
  const win = getCurrentWindow();
  // 主窗口每次同步前后广播 usage-updated，面板据此更新摘要。
  unlisteners.push(await listen("usage-updated", () => void refresh()));
  // 失焦即收起，并留下时间戳供托盘点击去抖（避免点击图标时“隐藏又立即弹出”）。
  unlisteners.push(
    await win.onFocusChanged(({ payload: focused }) => {
      if (focused) {
        void refresh();
      } else {
        localStorage.setItem("tray:lastHiddenAt", String(Date.now()));
        void win.hide();
      }
    }),
  );
});

onUnmounted(() => {
  window.removeEventListener("storage", onStorage);
  for (const off of unlisteners) off();
});
</script>

<template>
  <main ref="popoverEl" class="popover" :class="{ dark: previewDark }">
    <!-- 磨砂玻璃细颗粒纹理 -->
    <div class="grain" aria-hidden="true" />

    <header class="pop-head">
      <div class="pop-title">
        <div class="pop-logo">/_</div>
        <div>
          <h1>AI 用量</h1>
          <p>{{ rows.length }} 个平台 · {{ availableCount }} 个有额度</p>
        </div>
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

    <div v-if="tightest" class="pop-summary">
      <span class="sum-label">最紧额度</span>
      <strong class="sum-value" :class="tightest.tone">{{ tightest.pct }}%</strong>
      <span class="sum-name">{{ tightest.name }}</span>
      <span v-if="tightest.reset" class="sum-reset">{{ tightest.reset }}</span>
    </div>

    <div v-if="hasData" class="quota-list">
      <div v-for="r in rows" :key="r.key" class="qrow">
        <div class="qrow-top">
          <span class="qicon"><ToolLogo :platform="r.platform" :size="19" /></span>
          <div class="qcopy">
            <div class="qidentity">
              <span class="qname">{{ r.name }}</span>
              <span class="qtag">{{ r.planTag }}</span>
            </div>
            <span class="qmeta">
              {{ r.detail }}<template v-if="r.reset"> · {{ r.reset }}</template>
            </span>
          </div>
          <span class="qvalue" :class="r.tone">
            {{ r.valueText }}
          </span>
        </div>
        <div class="qtrack">
          <div
            v-if="r.pct != null"
            class="qfill"
            :class="r.tone"
            :style="{ width: `${Math.min(100, Math.max(2, r.pct))}%` }"
          />
        </div>
      </div>
    </div>
    <div v-else class="empty" @click="requestSync">
      <p>暂无额度数据</p>
      <small>点击下方同步按钮从本机 CLI 拉取</small>
    </div>

    <div class="pop-divider" />

    <div class="pop-actions">
      <button class="act-btn" type="button" title="打开看板" aria-label="打开看板" @click="openDashboard">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
          <rect x="3" y="3" width="7" height="9" rx="1.5" />
          <rect x="14" y="3" width="7" height="5" rx="1.5" />
          <rect x="14" y="12" width="7" height="9" rx="1.5" />
          <rect x="3" y="16" width="7" height="5" rx="1.5" />
        </svg>
        <span>看板</span>
      </button>
      <button
        class="act-btn primary"
        type="button"
        :class="{ syncing }"
        :disabled="syncing"
        :title="syncing ? '同步中' : '立即同步'"
        :aria-label="syncing ? '同步中' : '立即同步'"
        @click="requestSync"
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M21 12a9 9 0 1 1-3-6.7" />
          <path d="M21 3v6h-6" />
        </svg>
        <span>同步</span>
      </button>
      <button class="act-btn quit" type="button" title="退出" aria-label="退出" @click="quit">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
          <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4" />
          <path d="M16 17l5-5-5-5" />
          <path d="M21 12H9" />
        </svg>
        <span>退出</span>
      </button>
    </div>
  </main>
</template>

<style scoped>
.popover {
  --ink: #1d1d1f;
  --ink-soft: rgba(29, 29, 31, 0.76);
  --muted: rgba(29, 29, 31, 0.52);
  --hairline: rgba(0, 0, 0, 0.11);
  --accent: #5a5fef;
  --green: #30c55a;
  --green-deep: #209948;
  --red: #ff3b30;
  --orange: #e88316;
  --tint: rgba(246, 248, 249, 0.36);
  --rim: rgba(255, 255, 255, 0.66);
  --edge: rgba(66, 74, 80, 0.2);
  --sheen: rgba(255, 255, 255, 0.32);
  --footer: rgba(255, 255, 255, 0.08);
  --track: rgba(0, 0, 0, 0.08);
  --hover: rgba(0, 0, 0, 0.035);
  --press: rgba(0, 0, 0, 0.06);

  position: relative;
  display: flex;
  width: 100vw;
  /* 高度随内容自然撑开；再由 JS 把窗口尺寸精确贴到此高度 */
  flex-direction: column;
  overflow: hidden;
  border-radius: 20px;
  background:
    linear-gradient(165deg, var(--sheen), transparent 28%),
    var(--tint);
  -webkit-backdrop-filter: blur(32px) saturate(145%);
  backdrop-filter: blur(32px) saturate(145%);
  isolation: isolate;
  color: var(--ink);
  font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "PingFang SC",
    var(--font-sans), sans-serif;
  /* 内圈描边贴合原生圆角；桌面磨砂与外阴影由透明原生窗口提供。 */
  box-shadow:
    inset 0 1px 0 var(--rim),
    inset 0 0 0 0.5px var(--edge);
  -webkit-font-smoothing: antialiased;
}

.grain {
  position: absolute;
  inset: 0;
  z-index: 0;
  opacity: 0.016;
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
  padding: 16px 16px 14px;
}
.pop-title {
  display: flex;
  align-items: center;
  gap: 10px;
}
.pop-logo {
  display: flex;
  width: 30px;
  height: 30px;
  align-items: center;
  justify-content: center;
  border-radius: 9px;
  background: linear-gradient(155deg, #6f79f5, #4c51e0);
  color: #fff;
  font-family: ui-monospace, "SF Mono", "JetBrains Mono", monospace;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: -1px;
  box-shadow:
    0 1px 3px rgba(76, 81, 224, 0.4),
    inset 0 0 0 0.5px rgba(255, 255, 255, 0.25);
}
.pop-title h1 {
  margin: 0;
  font-size: 15px;
  font-weight: 650;
  letter-spacing: -0.01em;
}
.pop-title p {
  margin: 2px 0 0;
  color: var(--muted);
  font-size: 10.5px;
  font-weight: 500;
}

.sync-pill {
  display: flex;
  align-items: center;
  gap: 6px;
  max-width: 128px;
  padding: 4px 9px 4px 8px;
  border: 0;
  border-radius: 20px;
  background: rgba(48, 197, 90, 0.12);
  color: var(--green-deep);
  font: inherit;
  overflow: hidden;
  font-size: 10px;
  font-weight: 500;
  text-overflow: ellipsis;
  white-space: nowrap;
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
  margin-inline: 16px;
  background: var(--hairline);
}

.pop-summary {
  position: relative;
  z-index: 1;
  display: flex;
  align-items: baseline;
  gap: 7px;
  padding: 10px 16px;
  border-bottom: 0.5px solid var(--hairline);
  background: var(--footer);
}
.sum-label {
  color: var(--muted);
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.04em;
}
.sum-value {
  color: var(--ink);
  font-family: ui-monospace, "SF Mono", "JetBrains Mono", monospace;
  font-size: 15px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
}
.sum-value.warn { color: var(--orange); }
.sum-value.critical { color: var(--red); }
.sum-name {
  color: var(--ink-soft);
  font-size: 11px;
  font-weight: 600;
}
.sum-reset {
  margin-left: auto;
  color: var(--muted);
  font-size: 10px;
  white-space: nowrap;
}

.quota-list {
  position: relative;
  z-index: 1;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  display: grid;
  gap: 2px;
  padding: 7px 8px;
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
  padding: 10px 12px 11px;
  border: 0.5px solid var(--hairline);
  border-radius: 12px;
  background: var(--hover);
  transition: background 0.12s ease, border-color 0.12s ease;
}
.qrow:hover {
  background: var(--press);
  border-color: var(--edge);
}
.qrow-top {
  display: flex;
  align-items: center;
  gap: 9px;
}
.qicon {
  display: grid;
  width: 22px;
  height: 30px;
  flex: 0 0 22px;
  place-items: center;
  opacity: 0.92;
}
.qcopy {
  min-width: 0;
  flex: 1;
}
.qidentity {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 6px;
}
.qname {
  color: var(--ink);
  font-size: 13px;
  font-weight: 630;
}
.qtag {
  overflow: hidden;
  color: var(--muted);
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.04em;
  text-overflow: ellipsis;
  text-transform: uppercase;
  white-space: nowrap;
}
.qtag::before {
  content: "·";
  margin: 0 4px 0 1px;
  color: rgba(0, 0, 0, 0.22);
  font-weight: 700;
}
.qmeta {
  display: block;
  margin-top: 2px;
  overflow: hidden;
  color: var(--muted);
  font-size: 10px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.qvalue {
  flex: 0 0 auto;
  color: var(--ink-soft);
  font-family: ui-monospace, "SF Mono", "JetBrains Mono", monospace;
  font-size: 13.5px;
  font-weight: 650;
  font-variant-numeric: tabular-nums;
}
.qvalue.warn {
  color: var(--orange);
}
.qvalue.critical {
  color: var(--red);
  font-family: inherit;
  font-size: 10px;
}
.qvalue.muted {
  color: var(--muted);
  font-family: inherit;
  font-size: 10px;
  font-weight: 500;
}
.qtrack {
  height: 3px;
  margin: 8px 0 0;
  overflow: hidden;
  border-radius: 20px;
  background: var(--track);
}
.qfill {
  height: 100%;
  border-radius: 20px;
  transition: width 0.4s ease;
}
.qfill.ok {
  background: linear-gradient(90deg, #7c85f5, var(--accent));
}
.qfill.warn {
  background: linear-gradient(90deg, #ffb24d, var(--orange));
}
.qfill.critical {
  background: linear-gradient(90deg, #ff5a50, #e0231b);
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
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  padding: 7px 10px 9px;
  border-top: 0.5px solid var(--hairline);
  background: var(--footer);
}
.act-btn {
  position: relative;
  display: flex;
  height: 34px;
  flex: 1;
  align-items: center;
  justify-content: center;
  gap: 5px;
  border: 0;
  background: transparent;
  color: var(--ink-soft);
  border-radius: 7px;
  cursor: pointer;
  transition:
    background 0.1s ease,
    color 0.1s ease;
}
.act-btn svg {
  width: 14px;
  height: 14px;
  opacity: 0.75;
}
.act-btn span {
  font-size: 10.5px;
  font-weight: 550;
}
.act-btn + .act-btn::before {
  content: none;
}
.act-btn:hover {
  background: var(--hover);
}
.act-btn:active {
  background: var(--press);
}
.act-btn.primary {
  color: var(--accent);
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

/* 静态设计预览复用同一套暗色材质。 */
.popover.dark {
    --ink: rgba(255, 255, 255, 0.9);
    --ink-soft: rgba(255, 255, 255, 0.76);
    --muted: rgba(235, 239, 241, 0.46);
    --hairline: rgba(255, 255, 255, 0.13);
    --tint: rgba(41, 45, 47, 0.48);
    --rim: rgba(220, 239, 247, 0.23);
    --edge: rgba(255, 255, 255, 0.16);
    --sheen: rgba(171, 211, 226, 0.1);
    --footer: rgba(255, 255, 255, 0.015);
    --track: rgba(255, 255, 255, 0.12);
    --hover: rgba(255, 255, 255, 0.075);
    --press: rgba(255, 255, 255, 0.12);
    --green-deep: #64d98b;
}
.popover.dark .qtag::before { color: rgba(255, 255, 255, 0.28); }
.popover.dark .sync-pill.warn { color: #ffc278; }
.popover.dark .sync-pill.error { color: #ff8e86; }

/* 跟随系统菜单外观，与原生 popover 磨砂材质一致。 */
@media (prefers-color-scheme: dark) {
  .popover {
    --ink: rgba(255, 255, 255, 0.9);
    --ink-soft: rgba(255, 255, 255, 0.76);
    --muted: rgba(235, 239, 241, 0.46);
    --hairline: rgba(255, 255, 255, 0.13);
    --tint: rgba(41, 45, 47, 0.48);
    --rim: rgba(220, 239, 247, 0.23);
    --edge: rgba(255, 255, 255, 0.16);
    --sheen: rgba(171, 211, 226, 0.1);
    --footer: rgba(255, 255, 255, 0.015);
    --track: rgba(255, 255, 255, 0.12);
    --hover: rgba(255, 255, 255, 0.075);
    --press: rgba(255, 255, 255, 0.12);
    --green-deep: #64d98b;
  }
  .qtag::before {
    color: rgba(255, 255, 255, 0.28);
  }
  .qrow:hover {
    background: var(--hover);
  }
  .quota-list {
    scrollbar-color: rgba(255, 255, 255, 0.2) transparent;
  }
  .quota-list::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.2);
  }
  .sync-pill.warn {
    color: #ffc278;
  }
  .sync-pill.error {
    color: #ff8e86;
  }
}
</style>
