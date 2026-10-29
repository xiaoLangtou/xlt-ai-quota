/**
 * Shell 环境模块前端契约冒烟（无服务、不触发真实 IPC）。
 *
 * 覆盖（PRD 8.1 前端检查项）：
 * - invoke 契约：7 个命令名与参数名（camelCase）与 Rust 侧一一对应；
 * - 旧直写入口（shell_alias_save / shell_file_write 等）不得再出现；
 * - 预览 → 提交流程：提交必须携带计划 id 与快照版本，源码 / 恢复同走计划；
 * - 保存与复制是独立反馈：提交函数不得直接写剪贴板；页面卸载取消计划；
 * - 路由与导航入口保留；diagnostic 隔离（模块门控不变）。
 */
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { shellEnvService } from "../src/services/shell-env-service";
import { SHELL_RELOAD_COMMAND, type ShellSnapshot } from "../src/types/shell";

const read = (path: string) => readFileSync(new URL(`../${path}`, import.meta.url), "utf8");

// ---------------------------------------------------------------------------
// 1. invoke 契约：mock __TAURI_INTERNALS__ 捕获命令与参数
// ---------------------------------------------------------------------------

type Call = { command: string; args: Record<string, unknown> | undefined };
const calls: Call[] = [];

const aliasSnapshot: ShellSnapshot = {
  alias: {
    kind: "alias",
    path: "/Users/demo/.alias",
    exists: true,
    version: "a".repeat(64),
    content: "alias gcm='git commit -m'\n",
    aliases: [],
    envs: [],
    managedBlock: null,
    blockIssues: [],
    lineEnding: "lf",
  },
  zshrc: {
    kind: "zshrc",
    path: "/Users/demo/.zshrc",
    exists: true,
    version: "b".repeat(64),
    content: "export KEEP=1\n",
    aliases: [],
    envs: [],
    managedBlock: null,
    blockIssues: [],
    lineEnding: "lf",
  },
};

const fixtureByCommand: Record<string, unknown> = {
  shell_snapshot: aliasSnapshot,
  shell_change_preview: {
    id: "plan-1",
    kind: "alias",
    baseVersion: "a".repeat(64),
    ops: [{ action: "修改别名", detail: "gcm" }],
    sessionCommand: "source ~/.alias",
    postimage: "",
    createdAt: "",
  },
  shell_change_commit: {
    kind: "alias",
    version: "c".repeat(64),
    content: "",
    sessionCommand: "source ~/.alias",
    ops: [],
    snapshot: aliasSnapshot.alias,
  },
  shell_config_backup_list: [],
  shell_config_validate: { ok: true, errors: [] },
  shell_source_map: { files: [], envs: [], diagnostics: [] },
  shell_path_report: {
    contributions: [],
    merged: [],
    diagnostics: [],
    summary: {
      contributionCount: 0,
      dynamicCount: 0,
      mergedCount: 0,
      duplicateGroups: 0,
      missingCount: 0,
      relativeCount: 0,
      emptyCount: 0,
    },
  },
};

Object.defineProperty(globalThis, "window", {
  configurable: true,
  value: {
    __TAURI_INTERNALS__: {
      invoke: (command: string, args?: Record<string, unknown>) => {
        calls.push({ command, args });
        if (command in fixtureByCommand) return Promise.resolve(fixtureByCommand[command]);
        return Promise.resolve(undefined);
      },
    },
  },
});

const last = () => calls.at(-1)!;

try {
  // snapshot
  const snapshot = await shellEnvService.snapshot();
  assert.equal(last().command, "shell_snapshot");
  assert.deepEqual(last().args ?? {}, {});
  assert.equal(snapshot.alias.kind, "alias");

  // preview：请求体原样透传（camelCase 字段保持不变）
  await shellEnvService.previewChange({ type: "env_set", key: "HTTP_PROXY", value: "http://127.0.0.1:8798" });
  assert.equal(last().command, "shell_change_preview");
  assert.deepEqual(last().args, {
    request: { type: "env_set", key: "HTTP_PROXY", value: "http://127.0.0.1:8798" },
  });

  await shellEnvService.previewChange({ type: "env_set", key: "K", value: "v", confirmBlockCreate: true });
  assert.deepEqual((last().args!.request as { confirmBlockCreate?: boolean }).confirmBlockCreate, true);

  // commit：参数名必须是 planId / expectedVersion（camelCase → rust snake_case）
  await shellEnvService.commitChange("plan-1", "a".repeat(64));
  assert.equal(last().command, "shell_change_commit");
  assert.deepEqual(last().args, { planId: "plan-1", expectedVersion: "a".repeat(64) });

  // cancel
  await shellEnvService.cancelChange("plan-1");
  assert.equal(last().command, "shell_change_cancel");
  assert.deepEqual(last().args, { planId: "plan-1" });

  // 备份 / 校验 / 打开
  await shellEnvService.listBackups();
  assert.equal(last().command, "shell_config_backup_list");
  await shellEnvService.validate();
  assert.equal(last().command, "shell_config_validate");
  await shellEnvService.openFile("/Users/demo/.alias");
  assert.equal(last().command, "shell_file_open");
  assert.deepEqual(last().args, { path: "/Users/demo/.alias" });

  // R1 只读来源模型：includeExtended 必须透传
  await shellEnvService.sourceMap(false);
  assert.equal(last().command, "shell_source_map");
  assert.deepEqual(last().args, { includeExtended: false });
  await shellEnvService.pathReport(true);
  assert.equal(last().command, "shell_path_report");
  assert.deepEqual(last().args, { includeExtended: true });

  assert.equal(calls.length, 10, `预期 10 次 invoke，实际 ${calls.length}`);
  assert.ok(
    calls.every(({ command }) => command.startsWith("shell_")),
    "shell 服务不得调用非 shell_ 前缀命令",
  );
} finally {
  Reflect.deleteProperty(globalThis, "window");
}

// ---------------------------------------------------------------------------
// 2. 旧直写入口彻底移除（前后端同版本迁移，不得保留绕过预览的通道）
// ---------------------------------------------------------------------------

const mainRust = read("src-tauri/src/main.rs");
const serviceTs = read("src/services/shell-env-service.ts");
const viewVue = read("src/views/ShellEnvView.vue");
const shellenvDir = ["commands", "files", "parser", "plan", "serializer", "types", "tests"]
  .map((name) => read(`src-tauri/src/shellenv/${name}.rs`))
  .join("\n");
const shellenvAll = shellenvDir + mainRust;

for (const legacy of [
  "shell_alias_save",
  "shell_alias_delete",
  "shell_env_save",
  "shell_env_delete",
  "shell_config_status",
  "shell_config_restore",
  "shell_file_read",
  "shell_file_write",
]) {
  assert.ok(!shellenvAll.includes(legacy), `旧命令 ${legacy} 不得再出现在 Rust 侧`);
  assert.ok(!serviceTs.includes(legacy), `旧命令 ${legacy} 不得再出现在前端 service`);
  assert.ok(!viewVue.includes(legacy), `旧命令 ${legacy} 不得再出现在视图`);
}

// 7 个新命令全部注册且 service 全部引用
const commands = [
  "shell_snapshot",
  "shell_source_map",
  "shell_path_report",
  "shell_change_preview",
  "shell_change_commit",
  "shell_change_cancel",
  "shell_config_backup_list",
  "shell_config_validate",
  "shell_file_open",
];
for (const command of commands) {
  assert.ok(mainRust.includes(`shellenv::commands::${command},`), `命令 ${command} 必须注册在 invoke_handler`);
  if (!command.startsWith("shell_source_map") && !command.startsWith("shell_path_report")) {
    assert.ok(serviceTs.includes(`"${command}"`), `service 必须封装 ${command}`);
  }
}
// R1 只读命令由 shellEnvService.sourceMap / pathReport 封装
assert.ok(serviceTs.includes('"shell_source_map"') && serviceTs.includes('"shell_path_report"'), "service 必须封装 R1 只读命令");
assert.ok(
  !/shellenv::commands::\w+,/g.test(mainRust.replace(new RegExp(commands.join("|"), "g"), "")),
  "invoke_handler 中不得出现计划外的 shellenv 命令",
);

// R1 只读边界：来源模型与 PATH 报告不得走计划管线，也不得出现在变更请求类型中
assert.ok(!serviceTs.includes("shell_source_plan") && !serviceTs.includes("shell_path_plan"), "R1 只读命令不得引入计划命令");

// ---------------------------------------------------------------------------
// 3. 计划流：预览 → 确认 → 提交；提交带版本基线；恢复与源码同为操作类型
// ---------------------------------------------------------------------------

for (const marker of [
  "await shellEnvService.previewChange(request)",
  "shellEnvService.commitChange(plan.id, versionOf(plan.kind))",
  "type: \"source_write\"",
  "type: \"restore_backup\"",
  "type: \"block_normalize\"",
  "type: \"alias_rename\"",
]) {
  assert.ok(viewVue.includes(marker), `视图缺少计划流标记：${marker}`);
}

// 提交失败必须放弃已消费的计划（单次消费语义）
assert.ok(
  /confirmOpen\.value = false;\s*\n\s*pendingPlan\.value = null;/.test(viewVue),
  "提交失败后必须清空 pendingPlan（计划已单次消费）",
);

// 页面卸载取消未提交计划（资源边界：页面离开可取消任务）
assert.ok(
  viewVue.includes("onBeforeUnmount") && viewVue.includes("cancelChange(plan.id)"),
  "页面卸载必须取消 pending 计划",
);

// R1：变量 / PATH / 别名 / 诊断 Tab 组织；扩展只读开关与来源跳转
for (const marker of [
  'value="path"',
  'value="diagnostics"',
  "includeExtended",
  "openSourceAt",
  "shellEnvService.sourceMap(includeExtended.value)",
  "shellEnvService.pathReport(includeExtended.value)",
]) {
  assert.ok(viewVue.includes(marker), `视图缺少 R1 标记：${marker}`);
}

// ---------------------------------------------------------------------------
// 4. 保存与复制独立反馈（B09）：提交函数不得直接写剪贴板
// ---------------------------------------------------------------------------

function functionBody(source: string, name: string): string {
  const markerAt = source.indexOf(`function ${name}`);
  assert.notEqual(markerAt, -1, `缺少函数：${name}`);
  const openAt = source.indexOf("{", markerAt);
  let depth = 0;
  for (let index = openAt; index < source.length; index++) {
    if (source[index] === "{") depth++;
    if (source[index] === "}") depth--;
    if (depth === 0) return source.slice(openAt + 1, index);
  }
  assert.fail(`函数 ${name} 未闭合`);
}

const confirmBody = functionBody(viewVue, "confirmCommit");
assert.ok(!confirmBody.includes("writeText"), "确认提交函数不得直接写剪贴板（保存与复制分离）");
assert.ok(confirmBody.includes("applyCommitResult(result)"), "保存后必须应用提交结果");
const applyBody = functionBody(viewVue, "applyCommitResult");
assert.ok(
  applyBody.includes("lastSessionCommand.value = result.sessionCommand"),
  "保存后必须展示本次变更命令（不自动复制）",
);

const saveSourceBody = functionBody(viewVue, "saveSource");
assert.ok(!saveSourceBody.includes("writeText"), "源码保存函数不得直接写剪贴板");

// 脏源码互斥（B10）：runChange 与源码切换都必须检查
assert.ok(
  (viewVue.match(/源码有未保存修改/g) ?? []).length >= 3,
  "脏源码互斥提示必须在 runChange / 换 Tab / 跳转等入口出现",
);
assert.ok(viewVue.includes("isSourceDirtyFor(kind)"), "写操作入口必须检查同文件脏源码互斥");

// ---------------------------------------------------------------------------
// 5. 路由与导航保留；diagnostic 隔离不变
// ---------------------------------------------------------------------------

const routerTs = read("src/router.ts");
assert.ok(routerTs.includes('path: "/shell-env"') && routerTs.includes('name: "shell-env"'), "/shell-env 路由必须保留");
assert.ok(read("src/config/navigation.ts").includes('to: "/shell-env"'), "导航入口必须保留");
assert.ok(mainRust.includes('#[cfg(not(feature = "diagnostic"))]\nmod shellenv;'), "shellenv 模块必须保持 diagnostic 隔离");

// 手动整份刷新命令仍保留（顶栏按钮），删除类会话命令由后端生成
assert.equal(SHELL_RELOAD_COMMAND, "source ~/.alias && source ~/.zshrc");

console.log("Shell 环境前端契约冒烟通过：7 命令契约、旧入口移除、计划流、复制独立、路由与隔离 ✅");
