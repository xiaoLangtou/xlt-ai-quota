import { invoke } from "@tauri-apps/api/core";
import { Menu, MenuItem, PredefinedMenuItem } from "@tauri-apps/api/menu";
import { TrayIcon } from "@tauri-apps/api/tray";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauriDesktop } from "@/connectors/types";
import type { Platform, PlatformQuotaView, SyncInfo } from "@/types/usage";

let traySetup: Promise<void> | undefined;
let tray: TrayIcon | undefined;
let statusItem: MenuItem | undefined;
const quotaMenuItems = new Map<
  Platform,
  { heading: MenuItem; details: [MenuItem, MenuItem] }
>();

/** 在 macOS 菜单栏创建看板快捷入口；浏览器开发模式不执行任何原生调用。 */
export function setupUsageTray(onSync: () => Promise<void>): Promise<void> {
  if (!isTauriDesktop()) return Promise.resolve();
  if (!traySetup) traySetup = createUsageTray(onSync);
  return traySetup;
}

/** 将现有前端聚合结果投影为简短的原生菜单栏状态，不传递凭据或原始错误。 */
export async function updateUsageTrayStatus(input: {
  quotas: PlatformQuotaView[];
  sync: SyncInfo;
  syncing: boolean;
}): Promise<void> {
  if (!tray || !statusItem || quotaMenuItems.size === 0) return;

  const detail = input.syncing ? "同步中" : tightestQuota(input.quotas) ?? "等待同步";
  const partial = input.sync.outcome === "partial";
  const title = detail;
  const tooltipTitle = `AI 用量 · ${detail}`;
  const tooltip = partial
    ? `${tooltipTitle}\n部分数据未更新，打开看板查看详情`
    : input.sync.outcome === "failed"
      ? `${tooltipTitle}\n同步失败，打开看板查看详情`
      : tooltipTitle;
  const menuText = partial ? `状态：${detail} · 部分数据未更新` : `状态：${detail}`;

  const quotaUpdates = input.quotas.flatMap((quota) => {
    const items = quotaMenuItems.get(quota.platform);
    if (!items) return [];
    const lines = trayQuotaLines(quota);
    return [
      items.heading.setText(`${quota.name} · ${quota.planTag}`),
      items.details[0].setText(lines[0]),
      items.details[1].setText(lines[1]),
    ];
  });

  await Promise.all([
    tray.setTitle(title),
    tray.setTooltip(tooltip),
    statusItem.setText(menuText),
    ...quotaUpdates,
  ]);
}

async function createUsageTray(onSync: () => Promise<void>): Promise<void> {
  const appWindow = getCurrentWindow();
  await appWindow.onCloseRequested((event) => {
    event.preventDefault();
    void invoke("hide_dashboard");
  });

  statusItem = await MenuItem.new({
    id: "usage-status",
    text: "状态：等待同步",
    action: () => void revealDashboard(),
  });
  const quotaMenu = await createQuotaMenuItems();
  const openItem = await MenuItem.new({
    id: "open-dashboard",
    text: "打开 AI 用量看板",
    action: () => void revealDashboard(),
  });
  const syncItem = await MenuItem.new({
    id: "sync-usage",
    text: "立即同步",
    action: () => void onSync(),
  });
  const quitItem = await MenuItem.new({
    id: "quit-app",
    text: "退出",
    action: () => void invoke("quit_app"),
  });
  const separator = () => PredefinedMenuItem.new({ item: "Separator" });
  const menu = await Menu.new({
    items: [
      statusItem,
      await separator(),
      ...quotaMenu,
      await separator(),
      openItem,
      syncItem,
      await separator(),
      quitItem,
    ],
  });

  tray = await TrayIcon.new({
    id: "usage-status-tray",
    title: "AI 用量 · 等待同步",
    tooltip: "AI 用量看板",
    menu,
    showMenuOnLeftClick: true,
  });
}

/** 原生菜单项固定为每个平台两行，便于同步时原地刷新文字、保持菜单展开状态。 */
async function createQuotaMenuItems(): Promise<Array<MenuItem | PredefinedMenuItem>> {
  const items: Array<MenuItem | PredefinedMenuItem> = [];
  const platforms: Platform[] = ["codex", "ark", "kiro"];
  for (const platform of platforms) {
    const heading = await MenuItem.new({
      id: `quota-${platform}-heading`,
      text: platform,
      action: () => void revealDashboard(),
    });
    const primary = await MenuItem.new({
      id: `quota-${platform}-primary`,
      text: "  暂无额度数据",
      action: () => void revealDashboard(),
    });
    const secondary = await MenuItem.new({
      id: `quota-${platform}-secondary`,
      text: "  ",
      action: () => void revealDashboard(),
    });
    quotaMenuItems.set(platform, { heading, details: [primary, secondary] });
    items.push(heading, primary, secondary);
    if (platform !== "kiro") {
      items.push(await PredefinedMenuItem.new({ item: "Separator" }));
    }
  }
  return items;
}

function trayQuotaLines(view: PlatformQuotaView): [string, string] {
  if (view.credits) {
    const window = view.windows[0];
    const used = Math.max(0, view.credits.total - view.credits.remaining);
    return [
      `  总额度  ${view.credits.total.toLocaleString()}`,
      window
        ? `  已使用  ${used.toLocaleString()} / ${view.credits.total.toLocaleString()} · ${window.usedPct}%${resetSuffix(window.resetsIn)}`
        : "  已使用额度未同步",
    ];
  }

  const lines = view.windows.slice(0, 2).map(
    (window) =>
      `  ${trayMetricLabel(window.metric, window.label)}  ${window.usedPct}%${resetSuffix(window.resetsIn)}`,
  );
  return [lines[0] ?? "  暂无额度数据", lines[1] ?? "  "];
}

function resetSuffix(resetsIn?: string): string {
  if (!resetsIn) return "";
  const compact = resetsIn
    .replace(/(\d+) 小时 (\d+) 分钟后重置/, "$1h $2m 重置")
    .replace(/(\d+) 小时后重置/, "$1h 重置")
    .replace(/(\d+) 分钟后重置/, "$1m 重置")
    .replace(/(\d+) 天后重置/, "$1d 重置")
    .replace("后重置", "");
  return ` · ${compact}`;
}

function trayMetricLabel(
  metric: PlatformQuotaView["windows"][number]["metric"],
  label: string,
): string {
  if (metric === "five_hour") return "5h 额度";
  if (metric === "session") return "Session";
  return label;
}


async function revealDashboard(): Promise<void> {
  await invoke("show_dashboard");
}

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
