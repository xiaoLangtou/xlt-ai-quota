import { useSkillsStore } from "@/stores/skills";
import { useMcpStore } from "@/stores/mcp";
import { skillsMcp as skillsMcpEnabled } from "./features.json";

/**
 * 侧边栏导航配置。AppLayout 只负责渲染，新增入口（如 MCP）只改这里。
 * 侧边栏只放一级入口：页面内的筛选、维护、配置不占导航。
 */

export interface NavItem {
  /** 菜单文案 */
  label: string;
  /** 目标路由 */
  to: string;
  /** Lucide 图标名 */
  icon: string;
  /** 键盘快捷键提示，如「⌘1」，与 AppLayout 注册的快捷键对应 */
  kbd?: string;
  /** 徽章来源；返回 0 或 undefined 时不显示徽章 */
  badge?: () => number | undefined;
}

export interface NavGroup {
  /** 分组标题 */
  title: string;
  items: NavItem[];
}

/** Skills 校验错误数：只统计 error（`!valid`），告警不计数，避免常驻徽章失去提醒作用。 */
export function skillsErrorCount(): number {
  if (!skillsMcpEnabled) return 0;
  const store = useSkillsStore();
  return store.skills.filter((skill) => !skill.valid).length;
}

/** MCP 配置异常数：配置解析失败 + 命令不存在（含扫描文件级问题）。 */
export function mcpIssueCount(): number {
  if (!skillsMcpEnabled) return 0;
  return useMcpStore().errorCount;
}

export const NAV_GROUPS: NavGroup[] = [
  {
    title: "洞察",
    items: [
      { label: "概览", icon: "i-lucide-layout-dashboard", to: "/dashboard", kbd: "⌘1" },
      { label: "分析", icon: "i-lucide-chart-line", to: "/analytics", kbd: "⌘2" },
      { label: "费用中心", icon: "i-lucide-wallet", to: "/subscriptions", kbd: "⌘3" },
    ],
  },
  ...(skillsMcpEnabled ? [
    {
      title: "Skills",
      items: [
        { label: "概览", icon: "i-lucide-layers", to: "/skills" },
        { label: "技能库", icon: "i-lucide-library", to: "/skills/library", badge: skillsErrorCount },
        { label: "安装", icon: "i-lucide-download", to: "/skills/install" },
      ],
    },
    {
      title: "MCP",
      items: [
        { label: "MCP 服务", icon: "i-lucide-plug", to: "/mcp", badge: mcpIssueCount },
        { label: "MCP 库", icon: "i-lucide-library-big", to: "/mcp/library" },
      ],
    },
  ] : []),
  {
    title: "工作",
    items: [
      { label: "Git 报告", icon: "i-lucide-git-branch", to: "/daily-report", kbd: "⌘4" },
    ],
  },
  {
    title: "工具箱",
    items: [
      { label: "剪贴板历史", icon: "i-lucide-clipboard-list", to: "/clipboard", kbd: "⌘5" },
      { label: "代码片段", icon: "i-lucide-code", to: "/snippets", kbd: "⌘6" },
      { label: "密钥库", icon: "i-lucide-lock", to: "/vault", kbd: "⌘7" },
    ],
  },
];

/** 页头标题 / 分组：用量与设置仍由 DashboardView 按路由名渲染。 */
export interface PageMeta {
  group: string;
  crumb: string;
  title: string;
}

export const USAGE_PAGE_META: Record<string, PageMeta> = {
  dashboard: { group: "用量看板", crumb: "Overview", title: "概览" },
  analytics: { group: "用量看板", crumb: "Analytics", title: "分析" },
  subscriptions: { group: "套餐额度、订阅计划与账单流水统一对账", crumb: "Billing", title: "费用中心" },
  settings: { group: "系统", crumb: "Settings", title: "设置" },
};
