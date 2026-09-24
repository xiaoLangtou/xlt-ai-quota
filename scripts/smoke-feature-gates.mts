import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createMemoryHistory } from "vue-router";
import { skillsMcp } from "../src/config/features.json";
import { NAV_GROUPS, skillsErrorCount, mcpIssueCount } from "../src/config/navigation";
import { createAppRouter } from "../src/router";
import { skillsApi } from "../src/api/skills";
import { mcpApi } from "../src/api/mcp";

const read = (path: string) => readFileSync(new URL(`../${path}`, import.meta.url), "utf8");
assert.equal(skillsMcp, false, "本轮排查必须关闭 Skills/MCP");
assert.deepEqual(NAV_GROUPS.map((group) => group.title), ["洞察", "工作", "工具箱"]);
assert.equal(skillsErrorCount(), 0);
assert.equal(mcpIssueCount(), 0);

// 用内存路由和空概览组件验证旧链接；不加载业务页面或创建原生窗口。
const router = createAppRouter(createMemoryHistory());
assert.ok(router.getRoutes().every((route) => !/^\/(skills|mcp)(\/|$)/.test(route.path)));
for (const name of ["dashboard", "analytics", "subscriptions", "settings", "daily-report", "snippets", "clipboard", "vault"]) {
  assert.ok(router.hasRoute(name), `其他功能路由 ${name} 必须保留`);
}
router.addRoute({ path: "/dashboard", name: "dashboard", component: { render: () => null } });
for (const path of ["/skills", "/skills/library/root/name", "/skills/install", "/skills/remote", "/skills/roots", "/skills/trash", "/skills/issues?filter=issues", "/mcp", "/mcp/new", "/mcp/library", "/MCP/new"]) {
  await router.push(path);
  assert.equal(router.currentRoute.value.path, "/dashboard", `旧入口 ${path} 不得激活页面`);
}

// 即使旧组件误调用客户端，也必须在进入 Tauri IPC 前失败。
let ipcCalls = 0;
Object.defineProperty(globalThis, "window", {
  configurable: true,
  value: { __TAURI_INTERNALS__: { invoke: () => { ipcCalls++; throw new Error("不应调用 IPC"); } } },
});
try {
  for (const api of [skillsApi, mcpApi]) {
    for (const [name, call] of Object.entries(api)) {
      await assert.rejects(() => (call as () => Promise<unknown>)(), /已暂时停用/, name);
    }
  }
  assert.equal(ipcCalls, 0);
} finally {
  Reflect.deleteProperty(globalThis, "window");
}

const layout = read("src/layouts/AppLayout.vue");
assert.match(layout, /if \(!isDesktop \|\| !skillsMcpEnabled\) return;\s+void useSkillsStore\(\)\.refresh\(\);\s+void useMcpStore\(\)\.refresh\(\);/);
assert.doesNotMatch(layout, /const \w+ = use(?:Skills|Mcp)Store\(\)/);

// 原生模块与每一项注册命令都受相同 cfg 约束，遗漏任意一项即失败。
const native = read("src-tauri/src/main.rs");
for (const module of ["skills", "mcp"]) {
  assert.ok(native.includes(`#[cfg(skills_mcp)]\n#[cfg(not(feature = "diagnostic"))]\nmod ${module};`));
}
const commands = [...native.matchAll(/^\s+(?:skills|mcp)::commands::\w+,/gm)];
assert.ok(commands.length > 0);
for (const command of commands) {
  assert.ok(native.slice(0, command.index).trimEnd().endsWith("#[cfg(skills_mcp)]"), command[0]);
}
assert.match(native, /#\[cfg\(skills_mcp\)\]\s+\{\s+app\.manage\(skills::SkillsState::new\(\)\);\s+app\.manage\(mcp::McpState::new\(\)\);\s+\}/);
for (const required of ["clipboard_history::start_listener", "clipboard_history::register_shortcut", "connectors::connector_get", "snippets::snippet_list", "vault::vault_read"]) {
  assert.ok(native.includes(required), `${required} 不应在本轮屏蔽`);
}
const build = read("src-tauri/build.rs");
assert.ok(build.includes('include_str!("../src/config/features.json")'));
assert.ok(build.includes('cargo:rerun-if-changed=../src/config/features.json'));
assert.ok(build.includes('cargo:rustc-cfg=skills_mcp'));
console.log(`Skills/MCP 屏蔽检查通过：导航、旧路由、零 IPC、启动扫描及 ${commands.length} 项原生命令`);
