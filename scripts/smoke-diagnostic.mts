import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { build, type ConfigEnv, type UserConfig } from "vite";
import viteConfig from "../vite.config";

const read = (path: string) => readFileSync(new URL(`../${path}`, import.meta.url), "utf8");
const normal = JSON.parse(read("src-tauri/tauri.conf.json"));
const diagnostic = JSON.parse(read("src-tauri/tauri.diagnostic.conf.json"));

// Tauri 使用 JSON Merge Patch；数组整体替换，不能残留原版隐藏面板。
function merge(base: any, patch: any): any {
  if (!patch || typeof patch !== "object" || Array.isArray(patch)) return patch;
  const result = { ...base };
  for (const [key, value] of Object.entries(patch)) {
    if (value === null) delete result[key];
    else result[key] = merge(result[key], value);
  }
  return result;
}

const merged = merge(normal, diagnostic);
assert.equal(merged.identifier, "com.xlt.workbench.diagnostic");
assert.notEqual(merged.identifier, normal.identifier);
assert.notEqual(merged.productName, normal.productName);
assert.deepEqual(merged.build.features, ["diagnostic"]);
assert.equal(merged.build.beforeBuildCommand, "pnpm build:diagnostic");
assert.equal(merged.build.frontendDist, "../dist-diagnostic");
assert.equal(merged.app.macOSPrivateApi, false);
assert.equal(merged.app.windows.length, 1);
const window = merged.app.windows[0];
assert.equal(window.label, "main");
assert.equal(window.url, "diagnostic.html");
assert.equal(window.visible, true);
assert.equal(window.decorations, true);
assert.equal(window.transparent, false);
assert.equal(window.alwaysOnTop, false);
assert.equal(window.windowEffects, undefined);
assert.deepEqual(merged.app.security.capabilities, [{
  identifier: "diagnostic-main", windows: ["main"], permissions: [],
}]);
assert.deepEqual(normal.app.windows.map((item: { label: string }) => item.label), [
  "main", "tray-panel", "clipboard-panel",
]);
assert.equal(normal.build.features, undefined);
// 诊断打包后必须恢复普通版依赖配置，避免破坏直接 cargo check/test。
assert.match(read("src-tauri/Cargo.toml"), /tauri = \{[^\n]*"macos-private-api"/);
assert.equal(merged.bundle.macOS.signingIdentity, "-");

const main = read("src-tauri/src/main.rs");
for (const module of ["clipboard_history", "connectors", "daily_report", "database", "mcp", "skills", "snippets", "vault"]) {
  assert.ok(main.includes(`#[cfg(not(feature = "diagnostic"))]\nmod ${module};`), `${module} 不得编入诊断入口`);
}
assert.ok(main.includes('#[cfg(feature = "diagnostic")]\nfn main() {\n    diagnostic::run();\n}'));
assert.ok(main.includes('#[cfg(not(feature = "diagnostic"))]\nfn main()'));
const native = read("src-tauri/src/diagnostic.rs");
assert.match(native, /generate_handler!\[quit_diagnostic\]/);
assert.doesNotMatch(native, /\.plugin\(|\.manage\(|Command::new|start_listener|set_activation_policy/);

assert.equal(typeof viteConfig, "function");
const configure = viteConfig as (env: ConfigEnv) => UserConfig;
const diagnosticVite = configure({ mode: "diagnostic", command: "build" });
const productionVite = configure({ mode: "production", command: "build" });
assert.equal(productionVite.build?.outDir, "dist");
assert.deepEqual(Object.keys(productionVite.build?.rollupOptions?.input ?? {}), ["main", "tray-panel", "clipboard-panel"]);
assert.equal(diagnosticVite.build?.outDir, "dist-diagnostic");
assert.deepEqual(diagnosticVite.plugins, []);
assert.deepEqual(Object.keys(diagnosticVite.build?.rollupOptions?.input ?? {}), ["diagnostic"]);

// 只在内存编译，不启动 Webview、不访问 CLI、不修改构建产物。
const result = await build({
  ...diagnosticVite,
  configFile: false,
  publicDir: false,
  logLevel: "silent",
  build: { ...diagnosticVite.build, write: false },
});
if (!("output" in result)) throw new Error("期望单次内存构建结果");
const chunks = result.output.filter((item) => item.type === "chunk");
assert.ok(chunks.length > 0);
for (const chunk of chunks) {
  assert.deepEqual(chunk.dynamicImports, []);
  for (const id of chunk.moduleIds) {
    assert.doesNotMatch(id, /\/src\/|qoder-agent-sdk|clipboard-panel|tray-panel|vue-router/);
  }
  assert.doesNotMatch(chunk.code, /connector_get|clipboard_status|mcp_scan|show_dashboard/);
}
assert.ok(result.output.some((item) => item.fileName === "diagnostic.html"));
console.log("诊断构建隔离检查通过：单窗口、独立标识、无业务模块及后台入口");
