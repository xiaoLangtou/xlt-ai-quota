import { invoke } from "@tauri-apps/api/core";
import { PhysicalPosition } from "@tauri-apps/api/dpi";
import { emit, listen } from "@tauri-apps/api/event";
import { Image } from "@tauri-apps/api/image";
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
let activeTrayIconKey: string | undefined;

type TrayIconState =
  | { kind: "quota"; usedPct: number }
  | { kind: "syncing" }
  | { kind: "warning" }
  | { kind: "error" }
  | { kind: "idle" };

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
  const quotaStatus = tightestQuota(input.quotas);
  const detail = input.syncing ? "同步中" : quotaStatus?.detail ?? "等待同步";
  const usageTitle = `AI 用量 · ${detail}`;
  const usageTooltip = input.sync.outcome === "partial"
    ? `${usageTitle}\n部分数据未更新，打开面板查看详情`
    : input.sync.outcome === "failed"
      ? `${usageTitle}\n同步失败，打开面板查看详情`
      : usageTitle;
  const iconState = oilAlert
    ? { kind: "warning" } as const
    : input.syncing
      ? { kind: "syncing" } as const
      : input.sync.outcome === "failed"
        ? { kind: "error" } as const
        : input.sync.outcome === "partial" || (quotaStatus?.usedPct ?? 0) >= 90
          ? { kind: "warning" } as const
          : quotaStatus
            ? { kind: "quota", usedPct: quotaStatus.usedPct } as const
            : { kind: "idle" } as const;
  await setTrayPresentation(iconState, oilAlert?.message ?? usageTooltip);
}

/** 油价或正式调价公告变化后，在菜单栏保留 6 小时提醒。 */
export async function announceOilUpdate(message: string): Promise<void> {
  oilAlert = { message, expiresAt: Date.now() + 6 * 60 * 60_000 };
  if (!tray) return;
  await setTrayPresentation({ kind: "warning" }, message);
}

async function createUsageTray(onSync: () => Promise<void>): Promise<void> {
  const appWindow = getCurrentWindow();
  await appWindow.onCloseRequested((event) => {
    event.preventDefault();
    void invoke("hide_dashboard");
  });
  await listen("tray:request-sync", () => void onSync());
  const initialState = { kind: "idle" } as const;
  const icon = await createTrayIcon(initialState);
  try {
    tray = await TrayIcon.new({
      id: "usage-status-tray",
      icon,
      iconAsTemplate: true,
      tooltip: "AI 用量看板",
      showMenuOnLeftClick: false,
      action: (event: TrayIconEvent) => {
        if (event.type === "Click" && event.button === "Left" && event.buttonState === "Up") void togglePanel(event.rect);
      },
    });
    activeTrayIconKey = trayIconKey(initialState);
  } finally {
    await icon.close();
  }
}

async function setTrayPresentation(state: TrayIconState, tooltip: string): Promise<void> {
  if (!tray) return;
  const key = trayIconKey(state);
  if (activeTrayIconKey === key) {
    await Promise.all([tray.setTooltip(tooltip), tray.setTitle(null)]);
    return;
  }
  const icon = await createTrayIcon(state);
  try {
    await Promise.all([
      tray.setTooltip(tooltip),
      tray.setTitle(null),
      tray.setIconWithAsTemplate(icon, true),
    ]);
    activeTrayIconKey = key;
  } finally {
    await icon.close();
  }
}

async function createTrayIcon(state: TrayIconState): Promise<Image> {
  const size = 44;
  const canvas = document.createElement("canvas");
  canvas.width = size;
  canvas.height = size;
  const context = canvas.getContext("2d");
  if (!context) throw new Error("无法创建菜单栏图标");

  context.fillStyle = "#000";
  context.strokeStyle = "#000";
  context.lineCap = "round";
  context.lineWidth = 5.5;
  context.beginPath();
  context.moveTo(9, 33);
  context.lineTo(9, 26);
  context.moveTo(19, 33);
  context.lineTo(19, 19);
  context.moveTo(29, 33);
  context.lineTo(29, 12);
  context.stroke();

  context.globalCompositeOperation = "destination-out";
  context.beginPath();
  context.arc(34, 10, 9.5, 0, Math.PI * 2);
  context.fill();
  context.globalCompositeOperation = "source-over";

  if (state.kind === "quota") {
    const progress = Math.min(Math.max(state.usedPct, 0), 100) / 100;
    context.lineWidth = 3.2;
    context.globalAlpha = 0.24;
    context.beginPath();
    context.arc(34, 10, 6.7, 0, Math.PI * 2);
    context.stroke();
    context.globalAlpha = 1;
    if (progress > 0) {
      context.beginPath();
      context.arc(34, 10, 6.7, -Math.PI / 2, -Math.PI / 2 + progress * Math.PI * 2);
      context.stroke();
    }
  } else if (state.kind === "idle") {
    context.beginPath();
    context.arc(34, 10, 3.6, 0, Math.PI * 2);
    context.fill();
  } else if (state.kind === "syncing") {
    context.lineWidth = 3.2;
    context.beginPath();
    context.arc(34, 10, 6.7, -Math.PI / 2, Math.PI);
    context.stroke();
  } else {
    context.beginPath();
    context.arc(34, 10, 7, 0, Math.PI * 2);
    context.fill();
    context.globalCompositeOperation = "destination-out";
    if (state.kind === "warning") {
      context.fillRect(33, 5.5, 2, 6);
      context.beginPath();
      context.arc(34, 14, 1.2, 0, Math.PI * 2);
      context.fill();
    } else {
      context.lineWidth = 2;
      context.beginPath();
      context.moveTo(31, 7);
      context.lineTo(37, 13);
      context.moveTo(37, 7);
      context.lineTo(31, 13);
      context.stroke();
    }
  }

  const pixels = context.getImageData(0, 0, size, size).data;
  return Image.new(new Uint8Array(pixels.buffer), size, size);
}

function trayIconKey(state: TrayIconState): string {
  if (state.kind !== "quota") return state.kind;
  return `quota:${Math.round(Math.min(Math.max(state.usedPct, 0), 100) / 5) * 5}`;
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

function tightestQuota(quotas: PlatformQuotaView[]): { detail: string; usedPct: number } | undefined {
  const candidates = quotas.flatMap((quota) => {
    const windows = quota.windows.map((window) => ({
      usedPct: window.usedPct,
      detail: `${trayPlatformName(quota.name)} ${shortMetric(window.metric)}${window.usedPct}%`,
    }));
    if (quota.credits && quota.credits.total > 0) {
      const usedPct = Math.round(((quota.credits.total - quota.credits.remaining) / quota.credits.total) * 100);
      windows.push({ usedPct, detail: `${trayPlatformName(quota.name)} 积分${usedPct}%` });
    }
    return windows;
  });
  return candidates.sort((a, b) => b.usedPct - a.usedPct)[0];
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
