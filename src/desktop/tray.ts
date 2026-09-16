import { invoke } from "@tauri-apps/api/core";
import { PhysicalPosition } from "@tauri-apps/api/dpi";
import { emit, listen } from "@tauri-apps/api/event";
import { TrayIcon, type TrayIconEvent } from "@tauri-apps/api/tray";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWindow, monitorFromPoint } from "@tauri-apps/api/window";
import { isTauriDesktop } from "@/connectors/types";
import type { PlatformQuotaView, SyncInfo } from "@/types/usage";

const PANEL_LABEL = "tray-panel";
const PANEL_GAP = 6;
const EDGE_MARGIN = 8;
const HIDE_DEBOUNCE_MS = 260;

let traySetup: Promise<void> | undefined;
let tray: TrayIcon | undefined;
let oilAlert: { message: string; expiresAt: number } | undefined;

export function setupUsageTray(onSync: () => Promise<void>): Promise<void> {
  if (!isTauriDesktop()) return Promise.resolve();
  if (!traySetup) traySetup = createUsageTray(onSync);
  return traySetup;
}

export async function updateUsageTrayStatus(input: {
  quotas: PlatformQuotaView[];
  sync: SyncInfo;
  syncing: boolean;
}): Promise<void> {
  void emit("usage-updated").catch(() => undefined);
  if (!tray) return;
  if (oilAlert && oilAlert.expiresAt <= Date.now()) oilAlert = undefined;
  const detail = input.syncing ? "同步中" : tightestQuota(input.quotas) ?? "等待同步";
  const usageTitle = `AI 用量 · ${detail}`;
  const usageTooltip = input.sync.outcome === "partial"
    ? `${usageTitle}\n部分数据未更新，打开面板查看详情`
    : input.sync.outcome === "failed"
      ? `${usageTitle}\n同步失败，打开面板查看详情`
      : usageTitle;
  await Promise.all([
    tray.setTitle(oilAlert ? "油价更新" : detail),
    tray.setTooltip(oilAlert?.message ?? usageTooltip),
  ]);
}

/** 油价或正式调价公告变化后，在菜单栏保留 6 小时提醒。 */
export async function announceOilUpdate(message: string): Promise<void> {
  oilAlert = { message, expiresAt: Date.now() + 6 * 60 * 60_000 };
  if (!tray) return;
  await Promise.all([tray.setTitle("油价更新"), tray.setTooltip(message)]);
}

async function createUsageTray(onSync: () => Promise<void>): Promise<void> {
  const appWindow = getCurrentWindow();
  await appWindow.onCloseRequested((event) => {
    event.preventDefault();
    void invoke("hide_dashboard");
  });
  await listen("tray:request-sync", () => void onSync());
  tray = await TrayIcon.new({
    id: "usage-status-tray",
    title: "AI 用量",
    tooltip: "AI 用量看板",
    showMenuOnLeftClick: false,
    action: (event: TrayIconEvent) => {
      if (event.type === "Click" && event.button === "Left" && event.buttonState === "Up") void togglePanel(event.rect);
    },
  });
}

async function togglePanel(rect: TrayRect): Promise<void> {
  const panel = await WebviewWindow.getByLabel(PANEL_LABEL);
  if (!panel) return;
  const lastHidden = Number(localStorage.getItem("tray:lastHiddenAt") ?? "0");
  if (Date.now() - lastHidden < HIDE_DEBOUNCE_MS) return;
  if (await panel.isVisible()) { await panel.hide(); return; }
  await positionPanelNearTray(panel, rect);
  await panel.show();
  await panel.setFocus();
}

interface TrayRect { position: { x: number; y: number }; size: { width: number; height: number }; }

async function positionPanelNearTray(panel: WebviewWindow, rect: TrayRect): Promise<void> {
  const size = await panel.outerSize();
  const centerX = rect.position.x + rect.size.width / 2;
  let x = Math.round(centerX - size.width / 2);
  let y = Math.round(rect.position.y + rect.size.height + PANEL_GAP);
  const monitor = await monitorFromPoint(centerX, rect.position.y).catch(() => null);
  if (monitor) {
    const minX = monitor.position.x + EDGE_MARGIN;
    const maxX = monitor.position.x + monitor.size.width - size.width - EDGE_MARGIN;
    x = Math.min(Math.max(x, minX), Math.max(minX, maxX));
    const maxY = monitor.position.y + monitor.size.height - size.height - EDGE_MARGIN;
    if (y > maxY) y = Math.max(monitor.position.y + EDGE_MARGIN, maxY);
  }
  await panel.setPosition(new PhysicalPosition(x, y));
}

function tightestQuota(quotas: PlatformQuotaView[]): string | undefined {
  const candidates = quotas.flatMap((quota) => {
    const windows = quota.windows.map((window) => ({ usedPct: window.usedPct, text: `${trayPlatformName(quota.name)} ${shortMetric(window.metric)}${window.usedPct}%` }));
    if (quota.credits && quota.credits.total > 0) {
      const usedPct = Math.round(((quota.credits.total - quota.credits.remaining) / quota.credits.total) * 100);
      windows.push({ usedPct, text: `${trayPlatformName(quota.name)} 积分${usedPct}%` });
    }
    return windows;
  });
  return candidates.sort((a, b) => b.usedPct - a.usedPct)[0]?.text;
}

function trayPlatformName(name: string): string {
  if (name === "火山方舟") return "方舟";
  if (name === "OpenCode Go") return "OpenCode";
  return name;
}
function shortMetric(metric: PlatformQuotaView["windows"][number]["metric"]): string {
  switch (metric) {
    case "five_hour": return "5h";
    case "weekly": return "周";
    case "monthly": return "月";
    case "session": return "会话";
    default: return "额度";
  }
}
