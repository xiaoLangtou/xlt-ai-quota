import { invoke } from "@tauri-apps/api/core";
import { PhysicalPosition } from "@tauri-apps/api/dpi";
import { emit, listen } from "@tauri-apps/api/event";
import { TrayIcon, type TrayIconEvent } from "@tauri-apps/api/tray";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWindow, monitorFromPoint } from "@tauri-apps/api/window";
import { isTauriDesktop } from "@/connectors/types";
import type { PlatformQuotaView, SyncInfo } from "@/types/usage";

const PANEL_LABEL = "tray-panel";
/** 图标与面板的垂直间距（物理像素）。 */
const PANEL_GAP = 6;
/** 屏幕边缘安全内边距。 */
const EDGE_MARGIN = 8;
/** 失焦隐藏后的去抖窗口：期间的托盘点击视为“已收起”，不再重新弹出。 */
const HIDE_DEBOUNCE_MS = 260;

let traySetup: Promise<void> | undefined;
let tray: TrayIcon | undefined;

/** 在 macOS 菜单栏创建看板托盘图标；浏览器开发模式不执行任何原生调用。 */
export function setupUsageTray(onSync: () => Promise<void>): Promise<void> {
  if (!isTauriDesktop()) return Promise.resolve();
  if (!traySetup) traySetup = createUsageTray(onSync);
  return traySetup;
}

/**
 * 将聚合结果投影为托盘图标标题/悬浮提示，并广播 usage-updated，
 * 让常驻隐藏的托盘面板窗口重新从存储读取最新额度。
 */
export async function updateUsageTrayStatus(input: {
  quotas: PlatformQuotaView[];
  sync: SyncInfo;
  syncing: boolean;
}): Promise<void> {
  // 面板窗口独立于主窗口运行时，靠事件而非共享内存刷新。
  void emit("usage-updated").catch(() => undefined);
  if (!tray) return;

  const detail = input.syncing ? "同步中" : tightestQuota(input.quotas) ?? "等待同步";
  const tooltipTitle = `AI 用量 · ${detail}`;
  const tooltip =
    input.sync.outcome === "partial"
      ? `${tooltipTitle}\n部分数据未更新，打开面板查看详情`
      : input.sync.outcome === "failed"
        ? `${tooltipTitle}\n同步失败，打开面板查看详情`
        : tooltipTitle;

  await Promise.all([tray.setTitle(detail), tray.setTooltip(tooltip)]);
}

async function createUsageTray(onSync: () => Promise<void>): Promise<void> {
  const appWindow = getCurrentWindow();
  await appWindow.onCloseRequested((event) => {
    event.preventDefault();
    void invoke("hide_dashboard");
  });

  // 面板底部「立即同步」按钮发起请求 → 主窗口执行完整同步链路。
  await listen("tray:request-sync", () => void onSync());

  tray = await TrayIcon.new({
    id: "usage-status-tray",
    title: "AI 用量",
    tooltip: "AI 用量看板",
    // 不再使用原生菜单：左键点击切换自定义面板。
    showMenuOnLeftClick: false,
    action: (event: TrayIconEvent) => {
      if (
        event.type === "Click" &&
        event.button === "Left" &&
        event.buttonState === "Up"
      ) {
        void togglePanel(event.rect);
      }
    },
  });
}

/** 托盘点击：定位到图标正下方并显示，或在已展开时收起。 */
async function togglePanel(rect: TrayRect): Promise<void> {
  const panel = await WebviewWindow.getByLabel(PANEL_LABEL);
  if (!panel) return;

  // 面板失焦时会记录时间戳并自我隐藏；点击图标恰好触发失焦，
  // 若在去抖窗口内则视为本次点击已完成“收起”，避免立刻又弹出。
  const lastHidden = Number(localStorage.getItem("tray:lastHiddenAt") ?? "0");
  if (Date.now() - lastHidden < HIDE_DEBOUNCE_MS) return;

  if (await panel.isVisible()) {
    await panel.hide();
    return;
  }
  await positionPanelNearTray(panel, rect);
  await panel.show();
  await panel.setFocus();
}

interface TrayRect {
  position: { x: number; y: number };
  size: { width: number; height: number };
}

/** 将面板对齐到图标水平中点、正下方；并根据所在显示器边界向内裁剪。 */
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

/** 全平台里利用率最高的一项，作为托盘标题的“最吃紧”信号。 */
function tightestQuota(quotas: PlatformQuotaView[]): string | undefined {
  const candidates = quotas.flatMap((quota) => {
    const windows = quota.windows.map((window) => ({
      usedPct: window.usedPct,
      text: `${trayPlatformName(quota.name)} ${shortMetric(window.metric)}${window.usedPct}%`,
    }));
    if (quota.credits && quota.credits.total > 0) {
      const usedPct = Math.round(
        ((quota.credits.total - quota.credits.remaining) / quota.credits.total) * 100,
      );
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
    case "five_hour":
      return "5h";
    case "weekly":
      return "周";
    case "monthly":
      return "月";
    case "session":
      return "会话";
    default:
      return "额度";
  }
}
