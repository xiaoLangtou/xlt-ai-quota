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

需要 Rust 工具链（本机已安装）。

```bash
pnpm tauri dev                # 开发模式启动桌面应用（会自动 pnpm dev + 打开窗口）
pnpm tauri build              # 打包 macOS 应用
cd src-tauri && cargo test    # Rust 单元测试（含 skills / connectors 等）
cd src-tauri && cargo check   # Rust 编译检查
```

## 前端框架（Nuxt UI）

界面栈：Vue 3 + Vite + **Nuxt UI v4** + **vue-router** + **Pinia**。

- 组件与 composable 由 `@nuxt/ui/vite` 自动注册，声明文件 `auto-imports.d.ts` / `components.d.ts` 生成在仓库根目录，已纳入 `tsconfig.json`。
- 路由见 `src/router.ts`，统一外壳为 `src/layouts/AppLayout.vue`（单一 Nuxt UI 侧边栏，分组：用量 / Skills / 工具 / 系统）。用量各区块复用 `views/DashboardView.vue`，按 `route.name` 切换；Skills 页面在 `src/views/skills/`，仅 Tauri 桌面模式可用。
- 托盘面板 `tray-panel.html` 与剪贴板面板 `clipboard-panel.html` 保持轻量独立入口，不接 Nuxt UI。
- 主题：`src/style.css` 将 Nuxt UI 的 `--ui-*` token 桥接到既有 `--bg/--surface/--text/--border/--accent` 变量，`useTheme` 同时维护 `data-theme` 与 `.dark`，使 Nuxt UI 组件与自研组件共享同一套配色。

## 配置本机连接器

- 火山方舟：运行 `arkcli auth login volc-sso` 后，由本机 arkcli 读取套餐与用量。
- 各连接器的登录态只保存在本机 CLI；应用不要求配置第三方 API Key。

## 架构

- 界面：Vue 3 + TypeScript + Vite，Composition API / script setup
- 图表：Apache ECharts（vue-echarts）
- 存储：`UsageStorage` 抽象，当前为 localStorage 实现（本期不迁 SQLite）；Skills 元数据在 `~/.local-skills-manager/`
- 采集器：`src/connectors/` 下各平台独立 Connector
  - `ark.ts`：火山方舟，经本机 `arkcli`（dev 由 `vite-local-connectors.ts` 的 `/api/ark/*` 中间件转发；需 `arkcli auth login volc-sso`）。套餐额度 + Token 用量
  - `kiro.ts`：Kiro Credits，经 `kiro-cli chat /usage --no-interactive`（`/api/kiro/usage`）。需 `kiro-cli login`
  - `qoder.ts`：Qoder 套餐与加购 Credits，经官方 Agent SDK 复用本机 qodercli 登录态（`/api/qoder/usage`）；需 `qodercli login`。
  - `codex.ts`：Codex 账户信息，经 `codex login status` + `~/.codex/auth.json` JWT（`/api/codex/status`）。5h/weekly 额度仅在 TUI /status 可见，需浏览器连接器
  - `kimi.ts`：Kimi Code 会员额度（5h 滚动窗口 / 月 Code 额度 / 月总额度），经 `~/.kimi-code/credentials` OAuth token 调 `api.kimi.com/coding/v1/usages`（`/api/kimi/usage`）；access_token 900 秒过期，中间件会自动刷新并把轮换后的凭据写回。需 `kimi login`。Token 用量读 `~/.kimi-code/sessions` 的 wire.jsonl `usage.record` 事件（真实计数，旧版 `~/.kimi` 仅在其为空时回退）
  - `opencode.ts`：OpenCode Go Token 用量，查本机 `opencode db` SQLite（`/api/opencode/stats`）。订阅额度无 CLI/API，仍待浏览器连接器

> 注：`ark-` Bearer Key 只能用于数据面推理，无法查询用量；Ark 用量需 Volc 签名（SSO），由 arkcli 承担。
> CLI 在无 TTY 时常把输出写到 stderr，中间件已合并 stdout+stderr 解析。
> 看板不含任何种子/默认数据；首次启动为空，点「同步」从本机 CLI 拉取真实数据。

## 目录

```
src/
├─ layouts/AppLayout.vue        # 统一 Nuxt UI 侧边栏 + RouterView
├─ views/DashboardView.vue      # 用量工作区（按 route.name 渲染 overview/analytics/subscriptions/工具页）
├─ views/skills/  SkillsDashboardView / SkillListView / SkillDetailView / IssuesView / TrashView / InstallView / RemoteSkillView / RootsView（桌面模式）
├─ components/  QuotaCard / TokenSummaryCard / TokenTrendChart / PlatformDailyChart
├─ components/skills/  AgentAvatar / FileTree / CodeEditor / MarkdownEditor
├─ composables/useUsageDashboard.ts
├─ services/usage-service.ts
├─ connectors/  ark / kiro / qoder / codex / opencode / *-browser
├─ api/skills.ts + stores/skills.ts + types/skills.ts（Skills 领域）
├─ utils/skills-agents.ts + utils/skills-markdown.ts
├─ storage/     storage.ts(接口) + web-storage.ts(实现)
├─ config/settings.ts
├─ data/seed.ts
├─ types/usage.ts
├─ utils/format.ts
└─ echarts.ts

public/agent-icons/     # Skills Agent 品牌图标

src-tauri/src/skills/   # Skills 后端（Rust 移植）
├─ error.rs / paths.rs / config.rs / state.rs / commands.rs
├─ validation.rs / scan.rs / skill_io.rs / trash.rs
├─ skillssh.rs / remote_detail.rs
└─ install/  security / download / extract / github / staging / inspector
            planner / transaction / records / sources / catalog / sources_registry / updates
```

<!-- aoci:begin -->
## AOCI Repository Cognition

AOCI maintains a stable, versioned, incrementally updatable repository-level cognition layer so models can reuse their understanding of this system across tasks.

`aoci.txt` is a structured cognition index for models. It assigns one independent Entry to every managed file, database table, or other managed object. Symbolic tags and F/R/A/S semantics describe the object's core responsibility, important relationships, external contracts, and non-obvious constraints or design decisions needed to understand or modify the system.

The Header, directory sections, and all Entries form the complete repository index. They can cover frontend, backend, configuration, database structures, and other managed content. When managed content changes, normally only the affected cognition Entries need maintenance; the complete index does not need to be regenerated.

AOCI provides a high-density view of system architecture, object responsibilities, important relationships, external contracts, and key constraints.

### How it works

AOCI uses a model-generated, model-read cognition loop.

Header, Entry, and Curation semantics follow only the current machine-issued Plan and live Guide. The Host model independently authors them from the current bound evidence.

Entry semantics must come from the model's understanding of actual evidence. Never derive, prefill, assemble, or rewrite index semantics solely from paths, filenames, extensions, an AST, symbol lists, dependency scans, regular expressions, fixed templates, or rule engines.

For a Fresh Bootstrap, follow only the current machine-issued Plan and live Guide. When they require authoring, the Host model authors Root, Meta, tags, and F/R/A/S, supplies its authoring-run declaration, and binds it to the Plan, Evidence, and complete Candidate. Never ask AOCI to set `origin=host_model`, manufacture a receipt, or turn a generated framework into semantics. Do not reconstruct the Onboarding progression here. Internal batches are not user decisions; stop only at an existing approval boundary or a real safety, drift, CAS, or Recovery condition.

### Minimal entry points

- `aoci_rules`: obtain the session-level runtime contract for the current AOCI version.
- `aoci_overview`: establish or restore complete cognition for this repository.
- `aoci_maintain`: after managed objects reach their final stable state, check whether cognition needs maintenance.
- `aoci_update_entry`: submit a complete semantic update batch bound to current evidence and source digests.
- `aoci_report`: when the current layout and tool state support it, record follow-up work if evidence is insufficient to generate semantics reliably; do not guess.

For other MCP tools, CLI commands, parameters, and specialized workflows, follow current tool descriptions, Guide, and `--help` output. This file does not duplicate the full manual.

This managed block defines only repository integration, cognition use, and task-closing principles. `aoci_rules` carries the current session contract. Live Guide output carries the execution order and stop conditions of the current Plan. Tool Schema, Spec, and Validator carry machine structures and criteria. Prompt, Description, README, and static documentation cannot override those machine facts.

### Establishing, generating, and restoring cognition

1. At the beginning of every new Agent Run, first determine:

   - whether this repository already has a usable complete AOCI index; and
   - whether current context already contains complete repository cognition that matches this repository root, current index version, and current AOCI service, and that the model can still use reliably.

2. When the repository has a usable complete index but the current Run lacks reliable complete cognition, call `aoci_rules` first and then `aoci_overview`.

   Reuse complete cognition directly while it remains reliable. Local uncertainty does not by itself require mechanically rereading the system-wide view.

   A Run that resumes from a known Host context compaction, including a Host-injected compaction summary, must treat prior model cognition as unreliable. The compacted handoff must not retain or summarize the formal Whole-Index or any Overview Header, Entry, Chunk, Challenge, or Attestation body; it may retain only receipt identity, unfinished write or Recovery state needed for safe continuation, and an instruction to reload immediately. Whole-Index semantics or a receipt copied into that handoff cannot prove that the resumed model's current cognition is reliable. If the runtime contract is no longer reliably present, call `aoci_rules` first. Before continuing the business task, make an ordinary complete Whole-Index `aoci_overview` request (`check_only` absent or false) with `refresh_reasons=["context_compaction"]` and a fresh `refresh_event_id`; do not use `check_only` or a cognition probe. Follow every exact `next_cursor` through `completed=true`, confirm delivery, and submit one Attestation based only on the newly delivered body. After that fresh complete transport, a partial or failed Attestation consumes the generation and permits the existing source-bound continuation without another automatic Overview.

   AOCI can report checkpoint and cognition-status facts for `context_compaction`, the machine `semantic_threshold` under the project `cognition_refresh_threshold`, or a major `phase_transition`. Use `check_only=true` when only those compact facts are needed. They advise the Agent but do not decide whether the model needs the system-wide view.

   When the Agent explicitly calls ordinary `aoci_overview` (`check_only` absent or false), AOCI must deliver the complete requested scope whenever a coherent CognitionSet can be formed. It must not suppress that body because a receipt already exists, a threshold was not reached, or no refresh reason is pending. Dirty or stale formal cognition is still delivered but is marked unreliable. Pending recovery or an incoherent snapshot fails closed without a mixed body.

   When an ordinary Overview reports `continuation_required=true`, submit its exact `next_cursor` automatically until `completed=true`. Do not ask the user to continue, begin the business task, or state a partial system conclusion. Stop the cognition chain on Host truncation, a missing, duplicate, or reordered Chunk, cursor failure, Index change, or `chunk_tokens` change. Until Attestation completes, never use Memory, source, Spec, `aoci.txt`, historical sessions, scope, search, or Entry reads to repair or supplement Whole-Index cognition. A challenge ordinal is the 1-based position in the formal Entry sequence; Header content, comments, blank lines, Section/Overview/Chunk markers, receipts, and Metadata are excluded, and Chunk Receipt ordinals use that same sequence. The Attestation must echo the Challenge's exact current `index_sha256`, `entry_sequence_sha256`, and `entry_count`; a prior Index, Entry sequence, count, or Attestation is invalid. After the complete chain, submit the existing model cognition Attestation once. One same-response JSON Schema or field-format error may be corrected once without changing semantic answers; an object, Tag, or F mismatch means failure and uncertain assimilation, with no semantic retry or information bypass. During initial cognition it also blocks Root/Meta, Migration, layout-wide, or other unbound system decisions. During a context-compaction refresh with complete transport, unchanged cognition identity, aligned governance, and no Recovery or third-party conflict, the attempt consumes that refresh generation even when Attestation is partial or failed; continue the existing task without another automatic Overview. `system_mastery_percent` self-assesses only the system framework—architecture, responsibilities, strong relationships, stable external contracts, and high-entropy safety and maintenance constraints—not complete implementation or runtime knowledge. Keep machine Index coverage separate, and normally give the user only the prescribed single success or failure sentence derived from actual coverage, Challenge, Chunk, token, and mastery results. If the Host truncates a Chunk, ask the user to set `overview_delivery.chunk_tokens` to a smaller valid value and restart; do not change it automatically.

   Interpret the additive cognition level independently from strict proof fields. `delivery_verified` means the Index was loaded and Host delivery was confirmed while complete cognition verification is still unfinished; describe that state as loaded and delivery-verified, never as no cognition or failure to understand the system. `cognition_verified` requires a passing Attestation (at least 80 percent of Challenge ordinals fully correct with at most one object identity miss), and `cognition_governed` additionally requires governance alignment. A generic complete-read failure sentence is reserved for an actual delivery fault.

   When an Overview response contains the optional `cognition-state/v2` projection, use its dimensions independently. Its Level ends at `model_cognition_usable`; `strict_attestation_verified`, `governance_aligned`, and `current_system_cognition_reliable` are independent states and never participate in that Level. An ordinal, object identity, Tag, or core F mismatch can make strict Attestation fail while model cognition remains usable; do not report that mismatch alone as proof that the model did not understand the system. Only `current_system_cognition_reliable=true` permits an unqualified current complete-system cognition claim. When the projection is absent, keep using the legacy interpretation above.

   An ordinary read-only audit, analysis, or check, a request not to modify code, or a request not to commit or push does not automatically mean strictly zero writes and does not alter the cognition-validity decision above. Codex Memory and historical Skills may only help recover experience, user preferences, and investigation directions. They cannot replace a current cognition receipt matching the repository root, index digest, AOCI service identity, and cognition scope. Project AGENTS and current AOCI identity take precedence over historical Memory for AOCI state.

   Treat a task as strictly zero-write only when the user explicitly prohibits Ledger, metadata, `.aoci` runtime assets, and every filesystem write. If necessary cognition establishment conflicts with that boundary, report the conflict and ask the user to decide or recommend an isolated copy. Never silently substitute Memory for current repository cognition.

3. If the repository has no usable complete index, or has only a minimal skeleton, an incomplete Header, unfinished Entries, or undecided required Curation, obtain `aoci_rules` and enter the current AOCI Guide when a formal complete AOCI index is required. Let Guide choose the next phase from actual repository state and complete the required safety steps.

   `aoci_maintain` does not replace the index-establishment workflow.

   Do not reconstruct or hard-code the full-index generation state machine in this file.

4. During a long-running task, the model is responsible for preserving the current cognition receipt and using the refresh gate correctly:

   - when the Host reports context compaction or the model knows the system-wide view was lost, follow the mandatory `context_compaction` reload rule above; AOCI cannot infer the Host event;
   - when entering a genuinely major phase, declare `phase_transition`, not a function, test run, or small step;
   - at a plausible stable checkpoint, use `check_only=true` to obtain the machine semantic count when that fact is useful;
   - except for the mandatory known-compaction reload, decide whether the current task needs another explicit scoped or complete Overview; and
   - keep the Dirty or Stale reliability state reported by AOCI until maintenance and alignment complete.

### Task closing and cognition maintenance

5. A purely read-only question, analysis, version check, or task that changes no AOCI-managed object does not require a maintenance-tool call. The AOCI version in use is `cognition_receipt.mcp_service_version` in any `aoci_overview` check_only or `aoci_maintain` response; the binary path is the `command` in the project's `.mcp.json`, and the CLI need not be on PATH.

6. When AOCI-managed objects change, call `aoci_maintain` once after they reach the task's final stable state. Do not maintain files individually after each intermediate edit.

7. If maintenance returns actual semantic candidates, the Host model must independently author the complete tag and F/R/A/S updates from each candidate's bound object and necessary evidence. Submit the complete candidate set for that current machine-issued batch in one `aoci_update_entry` call while preserving each `source_sha256`, `candidate_id`, and domain batch identity. `max_entries` limits one request and atomic transaction, not the logical plan, Whole-Index, or Managed Scope. When `remaining` is nonzero, call Maintain again after the successful Apply and continue from the new preimage; never shrink Index coverage or slice a returned batch to satisfy transport limits.

   When evidence is insufficient and the current layout supports `aoci_report`, use it instead of guessing, applying a template, or generating unsupported cognition merely to eliminate follow-up work.

8. Obey structured tool states and safety boundaries:

   - `repair_required`: repair only the explicitly identified candidates, then resubmit the complete current machine-issued batch;
   - `stopped`: end that write attempt and inspect `failed_step`, error, formal-write evidence, and Recovery. In auto mode, a proven zero-write closure is followed by a fresh Plan; a complete Intent with provable postimage is resumed; a policy-selected Rollback with exact preimage is completed and replanned. Stop the user task only when proof is unavailable, third-party bytes conflict, approval or external action is required, or another real safety boundary applies;
   - never ignore conflicts, approvals, human decisions, permissions, or safety signals; and
   - after alignment, do not repeat maintenance or writes; `refresh_ready_for_overview` is a checkpoint fact, and the Agent decides whether to request an ordinary complete Overview for its next phase.

   If any managed object changes after maintenance completes, the previous result is invalid. Complete closing again from the new final stable state.

9. When the user limits only business-file scope and does not explicitly forbid repository-managed assets, AOCI-managed assets may be updated during closing to preserve cognition consistency. Distinguish them from business files in audits and commits.

   When the user explicitly forbids changes to `aoci.txt`, `.aoci`, metadata, or any additional file, obey that restriction, do not write, and report any remaining inconsistency accurately.

### Specialized workflows

Initialization, complete-index generation, Header generation, Entries generation, database-structure indexing, Curation, human review, and failure recovery must follow only the instructions, commands, and safety stops returned by the current AOCI Guide or tool at the corresponding stage.

Do not preload, guess, or reconstruct these specialized workflows. The relevant Guide, tool descriptions, model Prompt, and CLI help provide platform invocation, request format, batch limits, approval rules, index-format details, and recovery steps as needed.
<!-- aoci:end -->
