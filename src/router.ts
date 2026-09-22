import { createRouter, createWebHashHistory } from "vue-router";

const UsageView = () => import("./views/DashboardView.vue");

// 统一路由：用量工作区 + Skills 管理 + MCP + 效率工具。页面按路由动态拆包，
// 导航由 AppLayout 的统一侧边栏驱动（配置见 src/config/navigation.ts）。
// 工具页各自独立成 View，不再由 DashboardView 按 route.name 分派。
export const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: "/", redirect: "/dashboard" },
    { path: "/dashboard", name: "dashboard", component: UsageView },
    { path: "/analytics", name: "analytics", component: UsageView },
    { path: "/subscriptions", name: "subscriptions", component: UsageView },
    { path: "/settings", name: "settings", component: UsageView },
    {
      path: "/daily-report",
      name: "daily-report",
      component: () => import("./views/tools/DailyReportPage.vue"),
    },
    {
      path: "/snippets",
      name: "snippets",
      component: () => import("./views/tools/SnippetsPage.vue"),
    },
    {
      path: "/clipboard",
      name: "clipboard",
      component: () => import("./views/tools/ClipboardPage.vue"),
    },
    {
      path: "/vault",
      name: "vault",
      component: () => import("./views/tools/VaultPage.vue"),
    },
    {
      path: "/mcp",
      name: "mcp",
      component: () => import("./views/mcp/McpServersView.vue"),
    },
    {
      path: "/mcp/new",
      name: "mcp-new",
      component: () => import("./views/mcp/McpServersView.vue"),
    },
    {
      path: "/mcp/library",
      name: "mcp-library",
      component: () => import("./views/mcp/McpLibraryView.vue"),
    },
    {
      path: "/skills",
      name: "skills-dashboard",
      component: () => import("./views/skills/SkillsDashboardView.vue"),
    },
    {
      path: "/skills/library",
      name: "skills-library",
      component: () => import("./views/skills/SkillListView.vue"),
    },
    {
      path: "/skills/library/:rootId/:name",
      name: "skills-detail",
      component: () => import("./views/skills/SkillDetailView.vue"),
      props: true,
    },
    {
      path: "/skills/install",
      name: "skills-install",
      component: () => import("./views/skills/InstallView.vue"),
    },
    {
      path: "/skills/remote",
      name: "skills-remote",
      component: () => import("./views/skills/RemoteSkillView.vue"),
    },
    // 来源目录 / 回收站：保留路由，入口收拢到技能库页。
    {
      path: "/skills/roots",
      name: "skills-roots",
      component: () => import("./views/skills/RootsView.vue"),
    },
    {
      path: "/skills/trash",
      name: "skills-trash",
      component: () => import("./views/skills/TrashView.vue"),
    },
    // 旧「问题中心」入口：重定向到技能库的「有问题」筛选。
    {
      path: "/skills/issues",
      redirect: { path: "/skills/library", query: { filter: "issues" } },
    },
    { path: "/:pathMatch(.*)*", redirect: "/dashboard" },
  ],
});
