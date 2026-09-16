# AI 用量看板

AI 用量看板是一款本地优先的桌面应用，用于统一查看多个 AI 编程工具的套餐额度、Token 用量和预估成本。它从本机 CLI、会话文件与本地数据库采集数据，并通过 Vue 3 看板展示趋势、平台占比、模型分布和活跃热力图。

项目同时提供订阅与账单管理、Git 工作报告、加密密钥库和国内油价监控，适合希望在一个界面中管理日常开发工具与相关支出的个人开发者。

## 功能概览

- 汇总火山方舟、Codex、Kiro 和 Qoder 的套餐额度或 Credits。
- 统计 Codex、Claude Code、OpenCode、Kiro、Qoder、Gemini CLI 和 GitHub Copilot CLI 的 Token 用量。
- 按今天、7 天、30 天或 90 天查看 Token、请求数、模型、平台和成本趋势。
- 管理 AI 订阅、续费日期、汇率和实际账单流水。
- 从一个或多个 Git 仓库读取提交，并通过 MiniMax、Kimi 或 DeepSeek 生成日报与周报。
- 使用主密码加密保存账号、AI Key 和其他凭据，支持自动锁定与加密备份。
- 查看省级油价、下一调价窗口和国家发改委调价公告。
- 提供亮色、暗色和跟随系统三种主题，以及桌面托盘面板。

应用不包含演示数据。首次打开后，请点击左下角的「同步」读取本机真实用量。

## 支持的数据源

| 工具 | 套餐额度 | Token 用量 | 数据来源或准备工作 |
| --- | --- | --- | --- |
| 火山方舟 | 支持 | 支持 | 运行 `arkcli auth login volc-sso`，应用通过 `arkcli` 读取套餐与统计数据。 |
| Codex | 5 小时与周额度 | 支持 | 运行 `codex login`；额度来自账户用量接口，Token 来自 `~/.codex/sessions`。 |
| Kiro | Credits | 估算 | 运行 `kiro-cli login`；Token 根据本机会话文本估算。 |
| Qoder | 套餐与加购 Credits | 支持 | 运行 `qodercli login`；额度通过官方 Agent SDK 获取，Token 来自本地会话数据。 |
| OpenCode | 不支持 | 支持 | 从本机 OpenCode SQLite 数据库的 `usage` 字段聚合。 |
| Claude Code | 不支持 | 支持 | 从 `~/.claude/projects` 中的会话记录聚合。 |
| Gemini CLI | 不支持 | 支持 | 从 `~/.gemini/tmp` 中的会话记录聚合。 |
| GitHub Copilot CLI | 不支持 | 支持 | 从 `~/.copilot/session-state` 中的会话记录聚合。 |

没有对应工具或登录态时，相关连接器会显示同步错误，但不会影响其他数据源的采集。

## 快速开始

### 环境要求

- Node.js
- pnpm
- Rust 工具链和系统对应的 Tauri 依赖，仅桌面应用需要
- 你希望采集的 AI 工具及其本机登录态

### 浏览器开发模式

安装依赖并启动 Vite 开发服务器：

```bash
pnpm install
pnpm dev
```

浏览器访问 `http://localhost:1420`。开发服务器通过本地中间件读取 CLI、会话文件和数据库，因此应在运行这些工具的同一台电脑上启动。

### 桌面开发模式

安装 Rust 后，运行 Tauri 桌面应用：

```bash
pnpm install
pnpm tauri dev
```

Tauri 会自动启动 Vite，并打开桌面窗口。Git 报告、密钥库、原生目录选择和托盘交互需要桌面模式。

### 构建桌面应用

```bash
pnpm tauri build
```

构建产物位于 `src-tauri/target/release/bundle/`。具体安装包格式取决于当前操作系统和 Tauri 构建环境。

## 使用说明

1. 在终端完成所需 CLI 的安装与登录。
2. 打开「连接与设置」，选择需要展示的额度卡片，并设置统计时区和起始日期。
3. 点击「立即同步」采集额度、Token 和油价数据。
4. 在「用量分析」中切换统计周期，查看趋势、平台占比、工具与模型明细。
5. 在「订阅与账单」中维护计划支出和实际流水。

火山方舟控制面需要 Volc 签名认证。`ark-*` Bearer Key 只能用于推理，不能用于查询套餐与用量。

## 数据与隐私

- 用量、订阅、账单、设置和 Git 报告偏好保存在本机 Web Storage 中。
- 各工具的登录凭证由对应 CLI 管理，应用不会把它们写入项目文件。
- 密钥库使用 PBKDF2-SHA-256 派生密钥，并使用 AES-256-GCM 加密后写入 Tauri 应用数据目录。
- 生成 Git 报告时，应用会把所选时间范围内的提交信息发送给你选择的 AI 服务商。
- 国内油价监控与远程额度查询需要访问相应的数据源或服务接口。

密钥库的主密码无法恢复。请妥善保存主密码，并定期导出加密备份。

## 开发命令

| 命令 | 说明 |
| --- | --- |
| `pnpm dev` | 启动 Vite 开发服务器。 |
| `pnpm typecheck` | 运行 Vue 与 TypeScript 类型检查。 |
| `pnpm build` | 执行类型检查并构建 Web 资源。 |
| `pnpm preview` | 预览生产构建。 |
| `pnpm test:unit` | 运行数据聚合、映射、设置、计价和加密相关的冒烟测试。 |
| `pnpm test:e2e` | 启动测试用开发服务器，并通过本机 CLI 验证真实连接器。 |
| `pnpm test` | 依次运行类型检查、单元冒烟和端到端测试。 |
| `pnpm tauri dev` | 启动桌面开发模式。 |
| `pnpm tauri build` | 构建桌面安装包。 |

端到端测试依赖本机已安装并登录的 CLI，结果会随本机环境而变化。

## 技术栈

- Vue 3、TypeScript 和 Vite
- Tailwind CSS 4、Reka UI 和 Lucide
- Apache ECharts 与 vue-echarts
- Tauri 2 和 Rust
- Web Storage 与本地加密文件

## 项目结构

```text
src/
├── components/       # 看板、图表、订阅、设置与基础 UI
├── composables/      # 用量、订阅、主题和油价状态
├── connectors/       # 各平台额度与 Token 连接器
├── services/         # 用量、账单、Git 报告和密钥库服务
├── storage/          # 本地存储抽象与 Web Storage 实现
├── views/            # 总览、Git 报告与密钥库页面
└── types/            # 业务数据类型
src-tauri/
├── src/connectors.rs # 桌面端 CLI、文件、数据库与网络连接器
├── src/daily_report.rs
├── src/vault.rs
└── tauri.conf.json
scripts/              # 冒烟测试、端到端测试与 Qoder SDK 脚本
```

连接器通过统一接口返回额度快照和按日 Token 数据。`UsageService` 负责同步、持久化和聚合，界面层只消费标准化后的看板数据。
