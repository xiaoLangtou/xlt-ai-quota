# AI 用量看板 (xlt-workbench)

本地优先的 AI 用量看板桌面应用，统一展示各平台订阅额度与 Token 使用量。

## 开发命令

```bash
pnpm install      # 安装依赖
pnpm dev          # 启动 Vite 前端开发服务器 (http://localhost:1420)
pnpm typecheck    # vue-tsc 类型检查
pnpm build        # 类型检查 + 构建 dist/
pnpm preview      # 预览生产构建
pnpm test:unit    # 单元冒烟（数据聚合 / 映射 / 设置），无需 dev server
pnpm test:e2e     # 端到端：自动起 dev server，经本机 CLI 拉真实数据
pnpm test         # typecheck + unit + e2e
```

## 桌面壳（Tauri）

需要 Rust 工具链。安装 Rust 后：

```bash
pnpm add -D @tauri-apps/cli   # 安装 Tauri CLI（ deferred，安装 Rust 后执行）
pnpm tauri dev                # 开发模式启动桌面应用（会自动 pnpm dev + 打开窗口）
pnpm tauri build              # 打包 macOS / Windows 应用
```

`src-tauri/` 配置已就绪，但本机未安装 Rust，桌面壳暂未编译；前端可独立运行。

## 配置本机连接器

- 火山方舟：运行 `arkcli auth login volc-sso` 后，由本机 arkcli 读取套餐与用量。
- 各连接器的登录态只保存在本机 CLI；应用不要求配置第三方 API Key。

## 架构

- 界面：Vue 3 + TypeScript + Vite，Composition API / script setup
- 图表：Apache ECharts（vue-echarts）
- 存储：`UsageStorage` 抽象，当前为 localStorage 实现；待 Tauri 就绪换 SQLite（tauri-plugin-sql）
- 采集器：`src/connectors/` 下各平台独立 Connector
  - `ark.ts`：火山方舟，经本机 `arkcli`（dev 由 `vite-local-connectors.ts` 的 `/api/ark/*` 中间件转发；需 `arkcli auth login volc-sso`）。套餐额度 + Token 用量
  - `kiro.ts`：Kiro Credits，经 `kiro-cli chat /usage --no-interactive`（`/api/kiro/usage`）。需 `kiro-cli login`
  - `qoder.ts`：Qoder 套餐与加购 Credits，经官方 Agent SDK 复用本机 qodercli 登录态（`/api/qoder/usage`）；需 `qodercli login`。
  - `codex.ts`：Codex 账户信息，经 `codex login status` + `~/.codex/auth.json` JWT（`/api/codex/status`）。5h/weekly 额度仅在 TUI /status 可见，需浏览器连接器
  - `opencode.ts`：OpenCode Go Token 用量，查本机 `opencode db` SQLite（`/api/opencode/stats`）。订阅额度无 CLI/API，仍待浏览器连接器

> 注：`ark-` Bearer Key 只能用于数据面推理，无法查询用量；Ark 用量需 Volc 签名（SSO），由 arkcli 承担。
> CLI 在无 TTY 时常把输出写到 stderr，中间件已合并 stdout+stderr 解析。
> 看板不含任何种子/默认数据；首次启动为空，点「同步」从本机 CLI 拉取真实数据。

## 目录

```
src/
├─ views/DashboardView.vue
├─ components/  QuotaCard / TokenSummaryCard / TokenTrendChart / PlatformDailyChart
├─ composables/useUsageDashboard.ts
├─ services/usage-service.ts
├─ connectors/  ark / kiro / qoder / codex / opencode / *-browser
├─ storage/     storage.ts(接口) + web-storage.ts(实现)
├─ config/settings.ts
├─ data/seed.ts
├─ types/usage.ts
├─ utils/format.ts
└─ echarts.ts
```
