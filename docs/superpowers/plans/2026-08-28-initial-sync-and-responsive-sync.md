# 首次同步与响应式同步 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 首次打开自动同步，并在每个平台完成时立即显示结果，同时移除 OpenCode Go 的订阅额度卡片。

**Architecture:** `UsageService` 接收可选的同步进度回调，在每个 connector 完成后持久化结果并通知调用者。`useUsageDashboard` 只启动一次自动同步，并用该回调刷新响应式状态。Vite 中间件为 Ark 查询设置专用短超时，但保留其查询降级顺序。

**Tech Stack:** Vue 3 Composition API、TypeScript、Vite plugin middleware、tsx 冒烟脚本。

---

## 文件职责

- `src/services/usage-service.ts`：支持同步进度回调，按 connector 完成事件持久化并通知界面；排除 OpenCode Go 额度视图。
- `src/composables/useUsageDashboard.ts`：首次创建时自动执行一次同步，防止并发重复同步，并在每个进度事件后读取状态。
- `vite-local-connectors.ts`：为 Ark 的 CLI 调用设置专用短超时，降低慢失败的等待时间。
- `scripts/smoke-auto-sync.mts`：验证 composable 初始化会触发同步。
- `scripts/smoke-sync-progress.mts`：验证服务在部分 connector 完成时已写入数据并通知进度。
- `scripts/smoke-data.mts`：将额度卡片数量与 OpenCode Go 视图断言更新为当前产品范围。
- `package.json`：将新增冒烟脚本纳入 `test:unit`。

### Task 1: 为自动同步建立失败测试

**Files:**
- Create: `scripts/smoke-auto-sync.mts`
- Modify: `package.json`

- [ ] **Step 1: 写入失败测试**

在脚本中创建内存 `localStorage`、拦截 `fetch` 并返回 Ark、Codex、Kiro、OpenCode 的最小合法 JSON。动态导入 `useUsageDashboard`，调用后等待一个事件循环，再断言 `/api/ark/plan`、`/api/ark/stats`、`/api/codex/usage`、`/api/kiro/usage` 和 `/api/opencode/stats` 均被请求，并断言 `state.sync.lastSyncAt` 已写入。

- [ ] **Step 2: 确认测试失败**

运行：`pnpm exec tsx scripts/smoke-auto-sync.mts`

预期：因当前 `useUsageDashboard()` 未调用 `sync()` 而失败，至少一个 API 路径未出现。

- [ ] **Step 3: 实现首次自动同步**

在 `src/composables/useUsageDashboard.ts` 添加模块级 `autoSyncStarted` 标志。完成 `sync()` 声明后，在 `useUsageDashboard()` 首次调用时执行 `void sync()`。在 `sync()` 开头加入：

```ts
if (state.loading || state.sync.syncing) return;
```

以防首次自动同步与用户点击或设置面板共享状态时重复发起请求。

- [ ] **Step 4: 确认测试通过**

运行：`pnpm exec tsx scripts/smoke-auto-sync.mts`

预期：所有断言输出 `PASS`。

### Task 2: 为渐进刷新建立失败测试

**Files:**
- Create: `scripts/smoke-sync-progress.mts`
- Modify: `src/services/usage-service.ts`
- Modify: `src/composables/useUsageDashboard.ts`

- [ ] **Step 1: 写入失败测试**

在 `scripts/smoke-sync-progress.mts` 构造内存 `UsageStorage` 和两个可控 Promise connector：第一个立即返回一个 Ark 额度快照，第二个在测试显式释放前保持 pending。调用：

```ts
const syncing = svc.syncAll(() => progressEvents++);
await firstConnectorFinished;
assert("第一个 connector 完成后已持久化额度", storage.listQuotas().length === 1);
assert("第一个 connector 完成后已通知进度", progressEvents === 1);
releaseSecondConnector();
await syncing;
assert("全部 connector 完成后通知两次", progressEvents === 2);
```

- [ ] **Step 2: 确认测试失败**

运行：`pnpm exec tsx scripts/smoke-sync-progress.mts`

预期：当前 `syncAll()` 不接受回调，测试在编译阶段失败。

- [ ] **Step 3: 实现服务进度回调与可注入 connector**

在 `src/services/usage-service.ts`：

1. 声明内部 `SyncConnectorGroups`，包含可选的 `quotaConnectors` 与 `tokenConnectors` 数组。
2. 将构造函数扩展为 `constructor(storage: UsageStorage = createStorage(), connectorGroups?: SyncConnectorGroups)`；没有注入值时继续创建当前五个默认 connector。
3. 将方法签名改为：

```ts
async syncAll(onProgress?: () => void): Promise<void>
```

4. 每个 quota/token connector 的 `try/catch` 完成后都调用 `onProgress?.()`；token connector 应在 `replaceTokensForPlatform()` 后通知。该回调不应中断同步：用 `try { onProgress?.() } catch { /* ignore UI callback failure */ }` 包裹。
5. 在 composable 中调用 `await usageService.syncAll(readAll)`，保留结束后的最终 `readAll()`。

- [ ] **Step 4: 确认测试通过**

运行：`pnpm exec tsx scripts/smoke-sync-progress.mts`

预期：第一个 connector 未等待第二个 connector 时已写入数据且产生一次回调；全部结束后总回调数为二。

### Task 3: 缩短 Ark 慢失败等待

**Files:**
- Modify: `vite-local-connectors.ts`

- [ ] **Step 1: 写入可测试的命令超时单元**

将 `run()` 的选项提取为可导出的纯函数：

```ts
export function commandOptions(timeoutMs = TIMEOUT_MS) {
  return { timeout: timeoutMs, maxBuffer: MAX_BUFFER, env: process.env };
}
```

新增断言到 `scripts/smoke-ark-mapping.mts`：`commandOptions(15_000).timeout === 15_000`。

- [ ] **Step 2: 确认测试失败**

运行：`pnpm exec tsx scripts/smoke-ark-mapping.mts`

预期：`commandOptions` 尚未导出，脚本导入失败。

- [ ] **Step 3: 实现专用超时**

在 `vite-local-connectors.ts` 添加：

```ts
const ARK_PLAN_TIMEOUT_MS = 20_000;
const ARK_STATS_TIMEOUT_MS = 15_000;
```

将 `run(bin, args, timeoutMs = TIMEOUT_MS)` 改为使用 `commandOptions(timeoutMs)`。Ark status/plan 调用传 `ARK_PLAN_TIMEOUT_MS`，三次 Ark stats 查询调用传 `ARK_STATS_TIMEOUT_MS`。其余 CLI 保留当前 90 秒通用超时。

- [ ] **Step 4: 确认测试通过**

运行：`pnpm exec tsx scripts/smoke-ark-mapping.mts`

预期：既有 Ark 映射断言与新超时断言都通过。

### Task 4: 移除 OpenCode Go 额度卡片

**Files:**
- Modify: `src/services/usage-service.ts`
- Modify: `scripts/smoke-data.mts`

- [ ] **Step 1: 写入失败断言**

将 `scripts/smoke-data.mts` 的额度视图断言改为：

```ts
assert("仅展示 3 个平台额度卡片", quotas.length === 3);
assert("额度视图不含 OpenCode Go", !quotas.some((q) => q.platform === "opencode-go"));
```

- [ ] **Step 2: 确认测试失败**

运行：`pnpm exec tsx scripts/smoke-data.mts`

预期：当前服务仍返回四张卡片，两个断言失败。

- [ ] **Step 3: 实现视图排除**

在 `getPlatformQuotaViews()` 中将固定 `order` 收缩为：

```ts
const order: Platform[] = ["codex", "ark", "kiro"];
```

不要删除 OpenCode Token connector、类型或图表标签；本次仅去除未实现订阅额度的数据卡片。

- [ ] **Step 4: 确认测试通过**

运行：`pnpm exec tsx scripts/smoke-data.mts`

预期：额度卡片数量为三，既有 Codex、Ark、Kiro 聚合断言仍通过。

### Task 5: 执行完整的本地验证

**Files:**
- Modify: `package.json`

- [ ] **Step 1: 纳入新增的单元冒烟脚本**

将 `test:unit` 更新为依次执行：

```json
"test:unit": "pnpm exec tsx scripts/smoke-data.mts && pnpm exec tsx scripts/smoke-ark-mapping.mts && pnpm exec tsx scripts/smoke-settings.mts && pnpm exec tsx scripts/smoke-auto-sync.mts && pnpm exec tsx scripts/smoke-sync-progress.mts"
```

- [ ] **Step 2: 运行单元冒烟**

运行：`pnpm test:unit`

预期：五个脚本均通过，不调用真实账户或网络。

- [ ] **Step 3: 运行类型检查**

运行：`pnpm typecheck`

预期：`vue-tsc --noEmit` 以状态码 0 退出。

- [ ] **Step 4: 手动开发态验证**

用户在终端运行：`pnpm dev`。

验证：首次打开 `http://localhost:1420` 后马上显示“同步中”，已完成平台的卡片或 Token 图表先更新；Ark 延迟或失败时，其他平台不再等待其完成才显示；额度区仅显示 Codex、火山方舟和 Kiro 三张卡片。

## 计划自检

- 范围覆盖：自动同步、渐进刷新、Ark 慢失败、OpenCode Go 卡片移除均有对应任务。
- 无占位项：每项测试、实现、命令与预期均已明确。
- 类型一致：`syncAll(onProgress?)` 在服务与 composable 中一致；测试使用可选 connector 注入构造服务。