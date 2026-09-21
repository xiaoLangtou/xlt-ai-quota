# 合并方案：Skills 管理 × AI 用量看板（Rust 单应用）

> 目标：把 `xlt-local-skills`（Skills Manager）与 `xlt-workbench`（AI 用量看板）合并为一个 **Tauri 2 + Rust** 桌面应用。
> Skills 服务端逻辑**全量移植为 Rust**，删除 Electron / Fastify / Node 服务；前端**统一到 Nuxt UI 一套**。
>
> 文档状态：设计稿（不含代码改动）
> 日期：2026-09-21

---

## 1. 决策摘要

| 决策点 | 结论 |
| --- | --- |
| 交付形态 | 合并进 `xlt-workbench` 仓库，形成单一桌面应用（保留 workbench 的 Tauri 壳） |
| Skills 后端 | 全量 Rust 移植为 `src-tauri/src/skills/*`，通过 Tauri command 暴露，彻底移除 Node 依赖 |
| Skills 前端 | 迁入 workbench 前端，统一使用 **Nuxt UI v4 + vue-router + Pinia** |
| 存量 UI | workbench 现有视图逐步改造为 Nuxt UI，废弃自研 design token 体系 |
| 通信方式 | 统一 `invoke()`；Skills 不再走 HTTP / Fastify |
| 多入口面板 | `tray-panel` / `clipboard-panel` **维持现状**，不 Nuxt UI 化（体积敏感，保持轻量独立入口） |
| Skills 元数据 | 沿用 `~/.local-skills-manager/`（config/installs/trash/sources），老用户配置无损复用，与 Electron 版可共存 |
| workbench 业务数据 | 用量/订阅/账单/设置**本期仍用 localStorage**，不迁 SQLite |
| 浏览器 dev 模式 | Skills 功能仅在 Tauri 桌面模式可用（原生 FS/对话框依赖），浏览器模式隐藏入口；不做只读预览兜底 |
| 精选目录 | Rust `include_str!` 内嵌 JSON（沿用原 `catalog.ts` 数据），不做运行时更新 |
| 平台范围 | 本期仅 macOS；Windows/Linux 暂不考虑 |

### 1.1 实施进度

- [x] **P0 脚手架**（2026-09-21）
  - Rust：`src-tauri/src/skills/`（`error` / `paths` / `config` / `state` / `commands`），注册只读与根目录/项目管理共 10 个命令；`cargo check` / `cargo test`（24 passed）通过。
  - 前端：接入 Nuxt UI v4 + vue-router + Pinia；`src/types/skills.ts`、`src/api/skills.ts`、`src/stores/skills.ts`、`src/views/skills/SkillsHomeView.vue`；`pnpm typecheck` / `pnpm build` 通过。
  - 偏差：原生目录选择改用前端 `@tauri-apps/plugin-dialog`（`dialog:allow-open`），不在 Rust 侧实现 `skills_pick_dir`。
- [x] **P1 只读扫描**（2026-09-21）
  - Rust：`validation.rs`（frontmatter 解析 + 结构校验）、`scan.rs`（文件树 / 概览 / 失败项 / 目录大小）、`skill_io.rs`（详情 / 文件读取含软链越界防护）；新增命令 `skills_scan` / `skills_list_issues` / `skills_get_detail` / `skills_read_file`；`cargo test`（29 passed）通过。
  - 前端：`SkillsLayout`（UDashboardGroup + 侧边栏）、`SkillsDashboardView` / `SkillListView` / `SkillDetailView`（只读）/ `IssuesView`，`AgentAvatar` / `FileTree`，`skills-agents` / `skills-markdown`，`public/agent-icons`；新增依赖 `markdown-it`。`pnpm typecheck` / `pnpm build` 通过。
  - 暂缓到后续阶段：新建/编辑/重命名/删除（P2）、安装入口（P3）、来源与更新卡片（P4，详情页未展示）。
- [x] **P2 编辑闭环**（2026-09-21）
  - Rust：`trash.rs`（回收站移入/恢复/彻底删除 + 软链接保留）、`install/records.rs`（installs.json 记录增删改查）、`skill_io.rs` 增补保存/新建/重命名/删除（frontmatter 保序重组、重命名失败回滚、删除走回收站）；新增命令 `skills_save` / `skills_create` / `skills_rename` / `skills_delete` / `skills_list_trash` / `skills_restore_trash` / `skills_purge_trash`；`cargo test`（30 passed，含编辑-重命名-删除-恢复端到端用例）。
  - 前端：`CodeEditor` / `MarkdownEditor`（codemirror / md-editor-v3，异步加载）、`SkillDetailView` 完整编辑态（元信息折叠、文件编辑、脏检测与离开确认、重命名/删除确认弹窗）、`SkillListView` 新建弹窗、`TrashView` + 侧边栏/路由；`pnpm typecheck` / `pnpm build` 通过。
- [x] **P3 安装流水线**（2026-09-21）
  - Rust：`install/` 全量落地 — `security`（SSRF 私网判定 + 字符串/DNS 双层校验）、`download`（手动重定向 + 流式限额 + 空闲/总超时）、`extract`（ZIP/TAR.GZ 限额解压 + Zip Slip / 外部软链 / 特殊条目拒绝）、`github`（引用解析 / 最长 rev 验证 / subpath 快速通道 / tarball 回落）、`staging`（TTL + meta.json + 清理）、`inspector`（候选发现 / 内容哈希 / 脚本与二进制检测）、`planner`（不可变计划 + 冲突策略 + confirmationHash + 逐目标事务提交）、`transaction`（临时目录→哈希校验→备份→rename 切换→记录）、`sources`（local/upload/github/skills-sh/http/market 物化）、`catalog`（内嵌精选 JSON + star 缓存）、`sources_registry`（源登记/索引/同步）；新增 `skillssh` / `remote_detail`。新增 19 个安装/数据源/远程命令；`cargo test`（34 passed，含 SSRF / 归档限额 / Zip Slip 用例）。
  - 前端：`api/skills.ts` 重构为 `call` 包装（错误统一为 `Error`）+ 原生目录/文件选择；移植 `InstallView`（三种来源、多目标、冲突策略、目录/数据源、skills.sh 搜索）、`RemoteSkillView`、`RootsView`；新增 `/skills/install` `/skills/remote` `/skills/roots` 路由与侧边栏入口；`pnpm typecheck` / `pnpm build` 通过。
  - 偏差：上传归档改为原生文件对话框取路径（`skills_prepare_upload_source(path, filename)`），不再走 multipart；目录/文件选择的 SSE 与 osascript 由 `@tauri-apps/plugin-dialog` 承担。
- [x] **P4 更新与回滚**（2026-09-21）
  - Rust：`install/updates.rs`（更新检查四态、来源重取、候选定位、逐文件差异、应用更新复用事务安装器并保留备份、回滚原子交换）；新增命令 `skills_list_installs` / `skills_check_install_update` / `skills_preview_update` / `skills_apply_update` / `skills_rollback_install`；`cargo test`（36 passed，含 diff 与候选匹配用例）。
  - 前端：`SkillDetailView` 增补「来源与更新」卡片（检查更新 / 预览差异 / 应用更新 / 回滚），复用既有 `skillsApi` 更新接口。`pnpm typecheck` / `pnpm build` 通过。
- [ ] P5 UI 统一
  - [x] **P5a 统一导航与路由**（2026-09-21）：新增 `layouts/AppLayout.vue`（单一 Nuxt UI 侧边栏，分组：用量 / Skills / 工具 / 系统；含全局/项目工作区、主题切换、关于），打通 workbench `data-theme` 与 Nuxt UI color mode；用量各区块改为**路由驱动**（`/dashboard` `/analytics` `/subscriptions` `/daily-report` `/snippets` `/clipboard` `/vault` `/settings` 共用 `DashboardView`，`activeWorkspace` 由 `route.name` 推导，删除自研 rail 导航）；Skills 路由扁平化，移除 `SkillsLayout`；`App.vue` 挂载 `AppLayout`。`pnpm typecheck` / `pnpm build` 通过。
  - [ ] P5b 内容组件 Nuxt UI 化与自研 design token 下线（Dashboard 各卡片、SettingsPanel、SubscriptionManager 等按页替换；清理 DashboardView 中 rail 遗留样式）。
  - [x] **P5b 主题统一**（2026-09-21）：新增**主题 token 桥接**（`style.css` 把 `--ui-*` 指回 `--bg/--surface/--text/--border/--accent`，亮暗两套；暗色选择器同时兼容 `[data-theme="dark"]` 与 `.dark`），`useTheme` 同步切换 `.dark` 类；看板外壳转 Nuxt UI —— `AboutDialog`→`UModal`、`SkeletonBlock`→`USkeleton`、topbar 按钮→`UButton`、同步状态条→`UAlert`。自研内容卡片（QuotaCard 等）经桥接自动继承 Nuxt UI 配色，两侧风格与亮暗模式一致；如需改用 Nuxt UI 组件原语可后续增量替换。
- [x] **P6 收尾**（2026-09-21）
  - 移除 `DashboardView` 中 rail / banner / 旧按钮等遗留样式（scoped style 从 13.6KB 降至 7.3KB），保留 `clipboard-workspace-shell` 等在用规则。
  - `vite-local-connectors.ts` **保留**：作为浏览器 dev 模式（非 Tauri）下连接器的兜底实现，与 Skills 无关；后续如需彻底纯 Rust 可另行评估。
  - `xlt-local-skills` 的 Electron / server / shared 位于另一仓库，本仓库无残留；Skills 能力已全部由 `src-tauri/src/skills` 承载。
  - `pnpm typecheck` / `pnpm build` / `cargo test`（36 passed）通过。


---

## 2. 现状对照

### 2.1 xlt-workbench

- 前端：Vue 3 + Vite，导航靠 `DashboardView.vue:63` 的 `activeWorkspace` 状态（无 vue-router），自研 UI kit（Reka UI + Tailwind 4 + `style.css` 大量 CSS 变量）。
- 后端：Tauri 2 Rust。`connectors.rs`(2274) / `clipboard_history.rs`(1415) / `snippets.rs`(463) / `daily_report.rs`(358) / `database.rs`(155，SQLite) / `vault.rs`(50)。命令在 `main.rs` 注册。
- 浏览器 dev 兜底：`vite-local-connectors.ts`（~71KB Node 中间件，模拟 Rust connectors）。
- 多入口：主窗口 `index.html` + `tray-panel.html` + `clipboard-panel.html`。

### 2.2 xlt-local-skills

- 前端：Vue 3 + **Nuxt UI v4 + Pinia + vue-router（hash）**，入口 `packages/web`，dev 端口 5179，`/api` 代理到 5178。
- 后端：`packages/server` Fastify（~5300 行 TS），含 roots/projects/扫描/校验/编辑/回收站/安装流水线/数据源/更新回滚；持久化文件在 `~/.local-skills-manager/`。
- 壳：Electron（`electron/main.mjs`）内嵌 Fastify 到随机端口 + 托盘。

### 2.3 可复用资产

- `packages/shared/src/index.ts` 的领域类型（DTO）可直接照搬为 TS 类型 + Rust struct。
- Nuxt UI 的页面（Dashboard/SkillList/SkillDetail/Install/Roots/Issues/Trash/RemoteSkill）与 `agents.ts` 品牌图标可整体迁移。
- 安装流水线的五层架构与安全基线设计良好，移植时保持结构不变。

---

## 3. 目标架构

```
┌──────────────────────────── 单一 Tauri 应用 ────────────────────────────┐
│ 前端 (Vue 3 + Nuxt UI + vue-router + Pinia)                             │
│  ├─ 用量模块（原 workbench）    /dashboard /analytics /subscriptions ...│
│  ├─ Skills 模块（原 local-skills）/skills /install /roots /issues /trash│
│  └─ 工具模块 /daily-report /snippets /clipboard /vault /settings        │
│            │ invoke()                                                   │
│  Rust 后端 (src-tauri)                                                  │
│  ├─ connectors / clipboard_history / snippets / daily_report / vault    │
│  └─ skills/  ← 新移植                                                    │
│       config · scan · validation · skill_io · trash                     │
│       install/{security,staging,inspector,planner,transaction,records,   │
│                updates,download,extract,github,sources,catalog}         │
│       skillssh · remote_detail · sources_registry · fs_scan             │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 4. 仓库与目录结构

合并后仍用 `xlt-workbench` 仓库。`xlt-local-skills` 仓库冻结为历史参考，前端所需文件迁移后不再作为 workspace 依赖。

```
xlt-workbench/
├─ src/
│  ├─ main.ts                    # createApp + pinia + router + Nuxt UI
│  ├─ App.vue                    # UApp + UDashboardGroup
│  ├─ router.ts                  # 统一路由（原 skills/router.ts 扩展）
│  ├─ style.css                  # 保留至 UI 迁移完成，逐步瘦身
│  ├─ views/                     # 原 workbench 视图（逐步 Nuxt UI 化）
│  │  ├─ DashboardView.vue ...（用量）
│  │  └─ skills/                 # 迁移自 packages/web/src/views
│  │     ├─ SkillsDashboardView.vue
│  │     ├─ SkillListView.vue
│  │     ├─ SkillDetailView.vue
│  │     ├─ InstallView.vue
│  │     ├─ RemoteSkillView.vue
│  │     ├─ RootsView.vue
│  │     ├─ IssuesView.vue
│  │     └─ TrashView.vue
│  ├─ components/skills/         # 迁移自 packages/web/src/components
│  │  ├─ AgentAvatar.vue
│  │  ├─ CodeEditor.vue
│  │  ├─ FileTree.vue
│  │  ├─ MarkdownEditor.vue
│  │  └─ SkillNavPanel.vue
│  ├─ stores/skills.ts           # Pinia store（迁移）
│  ├─ api/skills.ts              # invoke 适配层（取代 packages/web/src/api.ts）
│  ├─ types/skills.ts            # 迁移自 @xlt-skills/shared
│  ├─ utils/skills-agents.ts     # 迁移自 packages/web/src/agents.ts
│  ├─ assets/agent-icons/        # 迁移 public/agent-icons
│  └─ tray-panel / clipboard-panel / main.ts 等保持多入口
├─ src-tauri/
│  └─ src/
│     ├─ main.rs                 # 追加 skills 命令注册 + app.manage(SkillsState)
│     ├─ connectors.rs / ...      # 不变
│     └─ skills/                 # 新增（见 §5.1）
├─ scripts/                      # 冒烟测试（保留）+ skills 相关 Rust 测试
└─ docs/
```

> `xlt-local-skills` 的 `packages/server`、`packages/shared`、`electron/`、`pnpm-workspace.yaml` 全部废弃。

---

## 5. Rust 后端设计

### 5.1 模块划分（`src-tauri/src/skills/`）

| Rust 模块 | 对应原 TS | 职责 |
| --- | --- | --- |
| `mod.rs` | — | 导出、`SkillsState`、命令注册宏/数组 |
| `commands.rs` | `routes.ts` | `#[tauri::command]` 薄封装 + 错误转换 |
| `paths.rs` | `paths.ts` | `safe_join` / `is_inside` / `is_valid_skill_name` / `to_posix` / `is_editable_file` |
| `config.rs` | `config.ts` | 默认根目录发现、agent 识别、项目登记、`config.json` 读写、`scan_dir_for_projects` |
| `validation.rs` | `validation.ts` | `validate_skill` + 超大文件/异常文件名扫描 |
| `scan.rs` | `skills.ts` 上半 | `scan_all_skills` / `read_summary` / `scan_failures` / 文件树 |
| `skill_io.rs` | `skills.ts` 下半 | detail / read_file / save / create / rename / delete |
| `trash.rs` | `trash.ts` | 回收站移入/恢复/彻底删除 |
| `install/security.rs` | `install/security.ts` | `LIMITS`、私网地址判定、主机校验（字符串层 + DNS 层） |
| `install/download.rs` | `install/download.ts` | 手动重定向 + 大小上限 + 双超时下载器 |
| `install/extract.rs` | `install/extract.ts` | ZIP/TAR.GZ 限额解压、Zip Slip / 外部软链 / 特殊条目拒绝 |
| `install/github.rs` | `install/github.ts` | 引用解析、`/tree/` 最长 rev 验证、subpath 快速通道、tarball 回落 |
| `install/staging.rs` | `install/staging.ts` | staging 生命周期（30min TTL、meta.json、启动清理） |
| `install/inspector.rs` | `install/inspector.ts` | 候选发现、`hash_dir`、脚本/二进制/外部软链检查 |
| `install/planner.rs` | `install/planner.ts` | 不可变安装计划、冲突策略、`confirmation_hash`、commit |
| `install/transaction.rs` | `install/transaction.ts` | 单目标原子落盘（临时目录→哈希校验→备份→rename→记录） |
| `install/records.rs` | `install/records.ts` | `installs.json` 增删改查 |
| `install/updates.rs` | `install/updates.ts` | 更新检查四态、差异预览、应用更新、回滚 |
| `install/sources.rs` | `install/sources.ts` | local / archive / github / skills-sh / http / market 物化到 staging |
| `install/catalog.rs` | `catalog.ts` | 内置精选目录（`include_str!` 嵌入 JSON）+ star 缓存 |
| `skillssh.rs` | `skillssh.ts` | skills.sh 搜索 + 打包产物下载/落盘 |
| `remote_detail.rs` | `remote-detail.ts` | 远程详情/文件预览（不落 staging） |
| `sources_registry.rs` | `sources.ts` | 数据源登记、索引缓存、同步（含 inflight 去重） |
| `fs_pick.rs` | `fs-pick.ts` | 改用 `tauri-plugin-dialog`，删除 osascript |

### 5.2 命令映射表（REST → Tauri command）

统一前缀 `skills_`，参数用 `#[serde(rename_all = "camelCase")]` 保持与 TS DTO 一致。

| 原路由 | 新 command |
| --- | --- |
| `GET /api/roots` | `skills_list_roots` |
| `POST /api/roots` | `skills_add_root { path }` |
| `DELETE /api/roots/:id` | `skills_remove_root { id }` |
| `PUT /api/roots/:id/label` | `skills_update_root_label { id, label }` |
| `POST /api/roots/:id/init` | `skills_init_root { id }` |
| `GET /api/projects` | `skills_list_projects` |
| `POST /api/projects` | `skills_add_project { path, label }` |
| `DELETE /api/projects/:id` | `skills_remove_project { id }` |
| `GET /api/projects/:id/agent-dirs` | `skills_list_project_agent_dirs { id }` |
| `GET /api/skills` | `skills_scan` |
| `GET /api/skills/:rootId/:name` | `skills_get_detail { rootId, name }` |
| `GET /api/skills/:rootId/:name/file` | `skills_read_file { rootId, name, path }` |
| `PUT /api/skills/:rootId/:name` | `skills_save { rootId, name, request }` |
| `POST /api/skills` | `skills_create { request }` |
| `PUT /api/skills/:rootId/:name/rename` | `skills_rename { rootId, name, newName }` |
| `DELETE /api/skills/:rootId/:name` | `skills_delete { rootId, name }` |
| `POST /api/fs/pick` | `skills_pick_dir`（`tauri-plugin-dialog`，返回 `path \| null`） |
| `GET /api/fs/scan` | `skills_scan_dir { path }` |
| `POST /api/install/sources/local` | `skills_prepare_local_source { sourcePath }` |
| `POST /api/install/sources/upload` | `skills_prepare_upload_source { archivePath, filename }`（对话框取路径，取消 multipart） |
| `GET /api/catalog` | `skills_get_catalog` |
| `GET /api/sources` | `skills_list_sources` |
| `POST /api/sources` | `skills_add_source { request }` |
| `DELETE /api/sources/:id` | `skills_remove_source { id }` |
| `POST /api/sources/:id/sync` | `skills_sync_source { id }` |
| `GET /api/skillssh/search` | `skills_search_skillssh { query, limit }` |
| `GET /api/remote/detail` | `skills_remote_detail { source, ref }` |
| `GET /api/remote/file` | `skills_remote_file { source, ref, path }` |
| `POST /api/install/sources/remote` | `skills_prepare_remote_source { source, ref }` |
| `DELETE /api/install/staging/:id` | `skills_remove_staging { id }` |
| `POST /api/install/plans` | `skills_create_install_plan { request }` |
| `GET /api/install/plans/:id` | `skills_get_install_plan { id }` |
| `POST /api/install/plans/:id/commit` | `skills_commit_install_plan { id, request }` |
| `DELETE /api/install/plans/:id` | `skills_cancel_install_plan { id }` |
| `GET /api/installs` | `skills_list_installs` |
| `POST /api/installs/:id/check-update` | `skills_check_install_update { id }` |
| `POST /api/installs/:id/update/preview` | `skills_preview_update { id }` |
| `POST /api/installs/:id/update/apply` | `skills_apply_update { id, request }` |
| `POST /api/installs/:id/rollback` | `skills_rollback_install { id, backupId }` |
| `GET /api/trash` | `skills_list_trash` |
| `POST /api/trash/:id/restore` | `skills_restore_trash { id }` |
| `DELETE /api/trash/:id` | `skills_purge_trash { id }` |
| `GET /api/issues` | `skills_list_issues` |

**错误模型**：定义 `SkillsError { code: String, message: String, status: u16 }` 并 `impl Serialize`，所有命令返回 `Result<T, SkillsError>`，替代 `HttpError`。前端 `api/skills.ts` 统一 catch 后抛出 `Error(message)`。

### 5.3 状态与并发

原 TS 的模块级可变状态改为 Tauri managed state：

```rust
pub struct SkillsState {
    staging: Mutex<HashMap<String, StagingRecord>>,
    plans: Mutex<HashMap<String, PlanEntry>>,
    sources_inflight: Mutex<HashMap<String, JoinHandle<...>>>, // 或记录同步中标志
    stars_cache: Mutex<Option<(HashMap<String,u32>, Instant)>>,
    paths: SkillsPaths,          // config_dir / staging_dir / default_roots（测试注入）
}
```

- 命令用 `async fn`（Tauri 默认 tokio 运行时）；阻塞 FS 用 `spawn_blocking` 或直接同步（命令可在非主线程执行）。
- `SkillsPaths` 取代 `process.env.LSM_*` 读取，便于单测隔离，避免环境变量全局竞争。

### 5.4 持久化

| 文件 | 位置 | 说明 |
| --- | --- | --- |
| `config.json` | `~/.local-skills-manager/` | `extraRoots` / `labelOverrides` / `projects` |
| `installs.json` | 同目录 | 安装来源记录 |
| `trash.json` + `trash/` | 同目录 | 回收站元数据与实体 |
| `sources.json` / `sources-index.json` | 同目录 | 数据源登记与索引缓存 |
| staging | 系统临时目录 `local-skills-manager/staging/<id>/` | `meta.json` + `content/**` |

> 默认目录可用环境变量 `LSM_CONFIG_DIR` / `LSM_STAGING_DIR` / `LSM_DEFAULT_ROOTS` 覆盖（为兼容原测试行为保留），但实现上优先走 `SkillsPaths` 注入。

### 5.5 Cargo 依赖新增

```toml
sha1 = "0.10"                     # 保持 rootId/contentHash 与原实现一致（原用 sha1）
serde_yml = "0.0.12"              # frontmatter 解析/序列化（serde_yaml 维护分支）
gray_matter = "0.3"               # 可选；或手写 --- 分隔解析
zip = { version = "2", default-features = false, features = ["deflate"] }
tar = "0.4"
flate2 = "1"
walkdir = "2"                     # 目录遍历/大小统计
url = "2"
futures-util = "0.3"              # reqwest 流式下载
hickory-resolver = "0.24"         # DNS 层 SSRF 校验
serde_json = "1"                  # 已有
uuid = { version = "1", features = ["v4"] }  # 已有
chrono = "0.4"                    # 已有
reqwest = { version = "0.12", features = ["json", "rustls-tls", "stream", "blocking"] } # 扩展 features
```

`tauri-plugin-dialog` 已在依赖中，目录/文件选择直接复用。

### 5.6 移植注意点（易踩坑）

1. **路径规范化**：`path.resolve` 是纯词法解析，不要求路径存在；Rust `canonicalize` 要求存在。需要手写词法 normalize（或 `path-clean`/`dunce` crate），`is_inside`/`safe_join` 全程用它。
2. **递归复制保留软链接**：Node `cpSync(..., { verbatimSymlinks: true })` 需用 `symlink_metadata` 判断 + `std::os::unix::fs::symlink` 重建，不能直接 `fs::copy`。
3. **YAML 往返保真**：`gray-matter` + `yaml` npm 的行为与 Rust 不同。用 `serde_yml::Mapping` 保序；`extraYaml` 原文透传合并；对样例 SKILL.md 做往返测（保存后未修改字段不被重排/改引号）。
4. **sha1 一致性**：`rootId` / `projectId` / `sourceId` / `contentHash` 都基于 sha1 前缀，用 `sha1` crate 保持老配置与安装记录可对齐。
5. **DNS 层 SSRF**：原实现 `dns.lookup(all)` + IP 段判断。Rust 用 `hickory-resolver` 或 `ToSocketAddrs` 解析全部地址后复用 `is_private_address`（IPv4/IPv6、CGNAT、IPv4-mapped 递归）。
6. **reqwest 重定向**：`redirect(Policy::none())`，手动逐跳校验+重定向，保持与原 `MAX_REDIRECTS = 5` / 空闲 30s / 总 5min 一致。
7. **原子 rename**：staging 与目标在同一文件系统才原子；跨设备回退为「复制+删除」（trash 已有此回退逻辑）。
8. **`.DS_Store`**：所有遍历统一跳过。
9. **JSON 序列化字段名**：Rust struct 一律 `#[serde(rename_all = "camelCase")]`，round-trip 用 `serde_json`，避免与前端 DTO 错位。

### 5.7 浏览器 dev 模式

Skills 依赖原生 FS / 原生对话框，浏览器无法实现。

- 方案：`isTauriDesktop()`（`src/connectors/types.ts:39`）为 false 时隐藏 Skills 导航并在直链页面显示「仅在桌面模式可用」提示。
- workbench 原有 `vite-local-connectors.ts` 只服务 connectors，不扩展到 skills（避免再维护一套 Node 实现）。

---

## 6. 前端设计（统一 Nuxt UI）

### 6.1 技术栈对齐

| 项 | workbench 现状 | 合并后 |
| --- | --- | --- |
| UI 库 | 自研 + Reka UI | **Nuxt UI v4**（`@nuxt/ui/vite` + `@nuxt/ui/vue-plugin`） |
| 路由 | 无（`activeWorkspace` 状态） | **vue-router 4**（hash history） |
| 状态 | ref/composable | **Pinia**（skills store；用量可后续迁 Pinia 或保留 composable） |
| 图标 | lucide-vue-next | `@iconify-json/lucide`（Nuxt UI 的 `i-lucide-*`） |
| 主题 | 自研 CSS 变量 + `data-theme` | Nuxt UI color mode（`useColorMode`）+ `--ui-*` token |
| 编辑器 | SnippetCodeEditor | 迁移 `CodeEditor.vue`（CodeMirror）+ `MarkdownEditor.vue`（md-editor-v3） |
| Markdown | shiki | `markdown-it`（或保留 shiki 做代码高亮） |
| 字体/滚动条 | 自研 | 迁入 skills 的 `assets/css/main.css` 排版与 CodeMirror 样式 |

### 6.2 依赖变更（`package.json`）

新增：
- `@nuxt/ui@^4.10`
- `vue-router@^4`
- `pinia@^2`
- `@iconify-json/lucide`、`@iconify-json/simple-icons`（dev）
- `@codemirror/*`、`codemirror`、`markdown-it`、`md-editor-v3`（迁移编辑器依赖）

调整：
- Vite 插件由 `@tailwindcss/vite` 切换/叠加到 `ui()`（Nuxt UI 内置 Tailwind v4 集成），`styles` 引入 `@import "@nuxt/ui"`。
- `main.ts` 追加 `.use(createPinia()).use(router).use(ui)`。
- `index.html` 保持；`tray-panel.html` / `clipboard-panel.html` **确认维持现状**，不引入 Nuxt UI，保持轻量独立入口。

### 6.3 导航与路由

用 `UDashboardGroup` + `UDashboardSidebar` 分组，取代 workbench 的 `rail-nav` 与 skills 的顶栏：

```
用量
  /dashboard        总览（原 overview）
  /analytics        用量分析
Skills            ← 桌面模式才显示
  /skills           概览（原 skills DashboardView）
  /skills/library   技能库（原 SkillList）
  /skills/install   安装
  /skills/roots     根目录
  /skills/issues    问题中心
  /skills/trash     回收站
/subscriptions    订阅与账单
工具
  /daily-report     日报
  /snippets         片段库
  /clipboard        剪贴板历史
  /vault            密钥库
/settings           设置
```

- 路由组件全部 `defineAsyncComponent` / 动态 import（沿用 skills 的按页拆包）。
- 技能详情 `/skills/library/:rootId/:name`，远程详情 `/skills/remote`。
- 原 `DashboardView.vue` 拆为「用量外壳 + 各路由页面」，`activeWorkspace` 删除。

### 6.4 API 适配层（`src/api/skills.ts`）

把 `packages/web/src/api.ts` 的 `request()` 换成 invoke：

```ts
import { invoke } from "@tauri-apps/api/core";
export const skillsApi = {
  listRoots: () => invoke<SkillRoot[]>("skills_list_roots"),
  addRoot: (path: string) => invoke<SkillRoot>("skills_add_root", { path }),
  getSkill: (rootId: string, name: string) =>
    invoke<SkillDetail>("skills_get_detail", { rootId, name }),
  // ...
};
```

- 请求体结构（`SaveSkillRequest` / `CreateInstallPlanRequest` 等）保持原 DTO，作为嵌套参数传入。
- 前端错误提示沿用 `SkillsError.message`。

### 6.5 迁移清单

| 来源（xlt-local-skills） | 去向（xlt-workbench） | 改动 |
| --- | --- | --- |
| `packages/shared/src/index.ts` | `src/types/skills.ts` | 原样迁移（去掉 workspace 引用） |
| `packages/web/src/api.ts` | `src/api/skills.ts` | fetch → invoke |
| `packages/web/src/stores/skills.ts` | `src/stores/skills.ts` | 基本不变 |
| `packages/web/src/router.ts` | `src/router.ts` | 合并 workbench 路由 |
| `packages/web/src/App.vue` | `src/App.vue` + 布局组件 | 侧边栏分组整合 |
| `views/*.vue` | `src/views/skills/*.vue` | 路径与返回逻辑调整 |
| `components/*.vue` | `src/components/skills/*.vue` | 保持 |
| `agents.ts` + `public/agent-icons` | `src/utils/skills-agents.ts` + `src/assets/agent-icons` | 保持 |
| `assets/css/main.css` | `src/style.css` | 合并 markdown/CodeMirror 样式 |
| `markdown.ts` | `src/utils/skills-markdown.ts` | 保持 |

### 6.6 主题统一策略

1. 先引入 Nuxt UI 与 `main.css`，Skills 页面直接可用。
2. workbench 现有页面按「路由页 → 子组件」顺序逐页替换为 Nuxt UI 组件（Button/Input/Table/Tabs/Dialog…），替换期间通过 `:ui` 覆盖保持视觉接近。
3. 现有 `style.css` 中的 brand 色（`--brand-ark` 等）保留为图表/品牌色，供 ECharts 与 Logo 使用；结构性 token（`--bg/--surface/--border/--text`）逐步映射到 `--ui-*` 后删除。
4. 亮/暗色统一用 Nuxt UI color mode，`useTheme` composable 下线或改为适配层。

---

## 7. 数据迁移

- **Skills 数据**：继续读写 `~/.local-skills-manager/`，用户从 Electron 版切到 Tauri 版后 roots/projects/回收站/安装记录/数据源全部保留。
- **workbench 数据**：用量/订阅/账单/设置仍用 localStorage（`UsageStorage`），**本期不迁移 SQLite**；后续若迁移另立专项。
- **ID 兼容**：sha1 前缀保持，安装记录与新扫描的 rootId 不发生漂移。

---

## 8. 分阶段实施计划

| 阶段 | 内容 | 验收 |
| --- | --- | --- |
| P0 脚手架 | 引入 Nuxt UI/vue-router/Pinia；`skills/` 模块骨架 + `SkillsState` + 错误模型；`package.json`/`Cargo.toml` 依赖 | 应用可启动，空命令可调用 |
| P1 只读 | `paths/config/validation/scan` + 只读命令；迁移 概览/技能库/根目录/问题中心 页面与 `api/skills.ts` | 能扫描并展示本机全部 skill 与校验问题 |
| P2 编辑 | `skill_io/trash` + 编辑器组件；创建/保存/重命名/删除/回收站恢复 | 编辑、重命名、删除恢复闭环，未保存提示生效 |
| P3 安装 | `install/{security,download,extract,github,staging,inspector,planner,transaction,records,sources} + skillssh + remote_detail + sources_registry + catalog`；安装向导页面 | local/归档/GitHub/skills.sh 安装全流程，冲突策略与多目标可用 |
| P4 更新 | `install/updates`；更新检查/差异/应用/回滚 UI | 四态检查、diff 预览、升级备份与回滚可用 |
| P5 UI 统一 | workbench 各页面 Nuxt UI 化；托盘/剪贴板面板适配；下线自研 token | 视觉与交互一致，亮暗色统一 |
| P6 收尾 | 删除 Electron/server/shared/`vite-local-connectors` 中 skills 残留；文档与测试补齐 | `pnpm typecheck`/`pnpm build`/Rust 测试通过 |

---

## 9. 测试策略

### 9.1 Rust 单元/集成测试（对应原 vitest）

- 直接移植：`paths`、`config`、`validation`、`skills`、`install/security`、`install/planner`、`install/transaction`、`install/install`、`sources`、`skillssh`。
- 用 `tempfile::TempDir` 隔离；`SkillsPaths` 注入临时 `config_dir`/`staging_dir`/`default_roots`，不读全局 env。
- 重点覆盖：路径穿越、越界软链、Zip Slip、压缩炸弹限额、SSRF 私网 URL、覆盖失败回滚、多目标部分失败、重命名回滚、保存原子性。
- `cargo test` 纳入 CI。

### 9.2 前端

- 保留现有冒烟测试（`scripts/smoke-*.mts`）。
- 新增 skills store / api 适配层单测（mock `invoke`）。
- Nuxt UI 迁移期加视觉回归基线（截图对比）以控制改造风险。

### 9.3 E2E

- 原 skills 的 HTTP e2e 改为 Tauri 集成测试；关键安装/升级流程走 Rust 端到端（真实临时目录）。
- 前端 e2e 浏览器模式仅覆盖用量/订阅等非 Skills 功能。

---

## 10. 风险与应对

| 风险 | 影响 | 应对 |
| --- | --- | --- |
| YAML 往返保真差异 | 保存后 SKILL.md 被重排/改引号 | 用保序 Mapping + `extraYaml` 透传 + 样例往返测试；发现偏差回退「仅替换结构化字段」策略 |
| workbench UI 全量 Nuxt UI 化工作量大 | 工期与视觉回归 | 分页面替换，过渡期双体系共存，`:ui` 覆盖保视觉 |
| `path.resolve` 词法语义移植 | 路径校验误判/越界 | 统一词法 normalize 工具 + 边界单测 |
| 软链接跨平台（Windows） | 复制/恢复失败 | 先保证 macOS；Windows 用 junction/权限降级并明确报错 |
| DNS 层 SSRF 实现差异 | 安全弱化 | 解析全部 A/AAAA 复用同一私网判定，白名单域名放行 |
| 内存态 staging/plan 重启丢失 | 未提交安装失效 | 与原实现一致（staging 有 meta 可恢复，plan 需重建），UI 明确提示 |
| `vite-local-connectors` 体积/维护 | 与纯 Rust 目标不符 | 本期保留；后续可评估用 Rust 侧 dev 命令替代 |

---

## 11. 已决事项与开放问题

### 11.1 已决（2026-09-21）

1. **托盘/剪贴板面板**：维持现状，不 Nuxt UI 化。
2. **workbench 业务数据**：本期不迁 SQLite，继续用 localStorage。
3. **Skills 元数据**：保留 `~/.local-skills-manager/`。

### 11.2 开放问题

1. 是否保留「浏览器模式下的 Skills 只读预览」能力 —— **暂不考虑**：Skills 仅在 Tauri 桌面模式提供，浏览器模式隐藏入口（见 §5.7）。
2. 内置精选目录（`catalog.ts` 的 CURATED）承载方式 —— **暂不考虑**：本期沿用最简单方案，Rust `include_str!` 内嵌 JSON（与原 `catalog.ts` 一致）；运行时可更新资源后续再评估。
3. Windows/Linux 支持 —— **暂不考虑**：本期仅面向 macOS，软链接与打包按 macOS 实现，跨平台留待后续。

---

## 12. 附录：领域类型移植对照

原 `packages/shared/src/index.ts` 的接口在 Rust 侧对应：

| TS 接口 | Rust struct |
| --- | --- |
| `SkillRoot` / `SkillProject` | `SkillRoot` / `SkillProject` |
| `SkillFileNode` | `SkillFileNode`（`#[serde(tag)]` 或 `type` 字段） |
| `ValidationIssue` / `ValidationLevel` | `ValidationIssue` / enum |
| `ScanFailure` / `TrashItem` | 同名 struct |
| `SkillSummary` / `SkillDetail` | 同名 struct（`SkillDetail` 含 `frontmatter: Value`、`frontmatterYaml`、`body`、`files`） |
| `SaveSkillRequest` / `CreateSkillRequest` | 同名 + `impl Deserialize` |
| `InstallPreview` / `StagingRecord` / `InspectedSkill` | 同名 struct |
| `InstallPlan` / `PlannedSkill` / `PlannedSkillTarget` / `PlannedTarget` | 同名 struct |
| `CommitPlanRequest` / `CommitPlanResponse` | 同名 struct |
| `InstallRecord` / `InstallUpdateInfo` / `UpdateDiffPreview` | 同名 struct |
| `SourceMetadata` / `CatalogSkill` / `SkillSource` | 同名 struct |
| `SkillsShSearchItem` / `RemoteSkillDetail` | 同名 struct |
| `HttpError` | `SkillsError` |

所有 struct 使用 `#[serde(rename_all = "camelCase")]`，`Option<T>` 对应可选字段，`ApiError` 统一由 `SkillsError` 承担。
