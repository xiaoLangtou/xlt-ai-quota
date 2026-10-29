# 禅道任务 → 项目待办事项 接入方案

> 2026-10-23 更新：已接入详情操作，Bug 支持指派/解决，任务支持开始/完成。同步状态以禅道为准；本页早期阶段设计以以下实现说明为准。

## 当前详情操作

- 待办抽屉与日历详情共用 `ZentaoActions`，打开操作表单时读取当前状态及人员/版本选项。
- 复用本机登录 Token，经 REST API v1 提交：`POST /bugs/{id}/assign`、`POST /bugs/{id}/resolve`、`POST /tasks/{id}/start`、`POST /tasks/{id}/finish`。指派接口依据官方 Go SDK 的 BugsService.AssignByID。
- 解决 Bug 必选解决方案；重复 Bug 必填其他 Bug 编号；已解决方案必选解决版本。任务开始填写累计消耗与剩余工时，任务完成填写本次消耗及完成时间。
- 每次只提交一次，回读详情核实状态后更新本地待办与日历；业务拒绝、回读未确认和本地保存失败分别提示。结果未确认时须刷新禅道状态后再操作。
- 首次仅导入未完成任务与未解决 Bug，忽略尚未同步过的已完成、关闭和取消项。已有条目继续更新状态，保留完成历史并跟随重新激活；同步明细只包含实际保留的条目，「更新」计数仅计入真实变化。
- 保存原始状态与指派人。列表中缺失的既有条目逐项查询详情，转交不标成完成，重新激活跟随远端恢复状态。读取失败保留已有镜像并报告未更新原因。
- 禅道条目的本地勾选、看板拖动及编辑表单状态修改不可用，状态统一走详情操作；本地排期仍可编辑。
- 启动桌面应用：`pnpm tauri dev`。不需要单独启动前端。验证可运行 `pnpm build` 和 `cargo test productivity:: --manifest-path src-tauri/Cargo.toml`；Rust 请求测试只启动本地 HTTP 测试替身，不访问真实禅道。

---

> 目标项目：你已有的 Node.js/TypeScript 业务项目（本仓库仅作为方案存档）
> 方案日期：2026-10-08

## 1. 目标与范围

- 从禅道拉取**指派给当前用户的 task / bug / story**，转换为你项目中的待办事项（Todo）
- 支持**手动触发 + 定时轮询**两种同步方式
- 禅道侧为只读源（本方案不含状态回写，见第 9 节扩展项）

## 2. 总体架构

```
┌─────────────── 你的 Node.js/TS 项目 ───────────────┐
│                                                    │
│  POST /api/sync/zentao     node-cron (每15分钟)    │
│          │                        │                │
│          └──────────┬─────────────┘                │
│                     ▼                              │
│            ZentaoSyncService                       │
│   ┌─────────────────────────────────────┐          │
│   │ 1. ZentaoClient（连接层，二选一）      │          │
│   │ 2. Normalize（task/bug/story→TodoItem）│        │
│   │ 3. Upsert（幂等写入 Todo 表）          │         │
│   └─────────────────────────────────────┘          │
│                     │                              │
│                     ▼                              │
│              Todo 模块（已有/新建）                  │
└────────────────────────┬───────────────────────────┘
                         │ HTTPS
                         ▼
                  禅道服务器 (REST API 或 zentao CLI)
```

## 3. 连接层选型（关键决策）

已验证：`zentao-cli`（npm 最新 0.3.1）**只提供 CLI bin，没有导出编程 API**（无 main/exports），只能子进程调用或直连禅道 REST API。

| | 方案 A：子进程调 `zentao` CLI | 方案 B：直连禅道 REST API |
|---|---|---|
| 实现 | `child_process.spawn('zentao', [...])` | 直接 fetch `/api/v1/*`，token 走 `/api/v1/tokens` |
| 认证 | CLI 自管（`~/.config/zentao/zentao.json`），代码不碰密码 | 需自己管理 token 的获取/续期/存储 |
| 多用户 | ❌ CLI profile 是全局单例，服务端多用户会互相踩 | ✅ 每用户一份 token，天然隔离 |
| 部署依赖 | 服务器需全局安装 zentao-cli 并预登录 | 无额外依赖 |
| 接口能力 | 受 CLI 模块限制 | 完整（禅道 18+ 自带 RESTful API） |

**决策建议：**

- 项目是**多用户系统**（每个用户同步自己的任务）→ **方案 B**，每用户存 token
- 项目是**单人工具/内部系统**（只同步一个账号）→ **方案 A** 起步最快，后续再迁移

无论哪种，都封装成统一的 `ZentaoClient` 接口：

```ts
interface ZentaoClient {
  getMyTasks(): Promise<ZentaoItem[]>
  getMyBugs(): Promise<ZentaoItem[]>
  getMyStories(): Promise<ZentaoItem[]>
}
```

## 4. 数据获取策略（注意 CLI/API 的范围约束）

禅道列表接口多数**要求指定归属范围**（task 按 execution、bug/story 按 product），没有现成的"我的全部"视图。两种取法：

1. **元数据遍历法**（通用）：

   ```
   拉产品列表 → 每个产品拉 bug/story (filter: assignedTo=me, status=active)
   拉项目列表 → 每个项目拉 execution → 每个执行拉 task (filter: assignedTo=me)
   ```

   可把产品/项目/执行 ID 缓存到本地（每天刷新一次），轮询时只遍历缓存的范围，控制请求数。

2. **my 模块 API**（如果禅道版本提供 `/api/v1/my-*` 或先验证 `zentao task --filter='assignedTo:xxx'` 是否免范围参数）：能用就优先，一条请求搞定。

> 落地第一步：用真实实例（`https://zentao.goktech.cn/zentao`）实际跑一遍 `zentao task --filter=...`、`zentao bug ...`，确认免范围过滤是否可行，这决定取数路径。

## 5. 数据模型与字段映射

Todo 表增加来源字段（以最小侵入方式扩展已有待办功能）：

```ts
// 新增字段（或独立表 zentao_sync_records）
{
  externalId: string,      // 幂等键："zentao:task:123"，唯一索引
  source: 'zentao',
  sourceType: 'task'|'bug'|'story',
  title: string,           // task.name / bug.title / story.title
  priority: 1|2|3|4,       // 映射禅道 pri；bug 额外融合 severity
  dueDate?: Date,          // task.deadline / bug.deadline
  externalUrl: string,     // 拼禅道跳转链接，点击直达原任务
  status: 'open'|'done'|'closed',
  syncedAt: Date,          // 上次同步确认时间
}
```

状态映射规则：

| 禅道状态 | 待办状态 |
|---|---|
| task: wait / doing | open |
| task: done / closed / cancel | done（标记完成，不删除） |
| bug: active | open |
| bug: resolved / closed | done |
| story: draft / active（指派给我评审） | open |
| story: closed | done |

## 6. 同步引擎核心逻辑

```
sync():
  1. 加分布式/进程锁（防止手动触发和定时任务并发跑）
  2. ZentaoClient 拉取三源数据 → 规范化为 TodoItem[]
  3. 逐条 upsert（按 externalId）：
     - 不存在 → 创建
     - 存在且禅道 updatedAt 更新 → 更新标题/优先级/截止/状态
     - 存在但本地已手动完成 → 不覆盖（默认策略，可配置）
  4. 本地 open 且本次拉取缺失的 → 标记 closed（禅道侧被取消/转派），
     保留记录不物理删除
  5. 记录 sync_log（各源条数、耗时、错误），释放锁
```

增量优化：拉取时按 `lastEditedDate` 过滤（禅道支持），只处理变化项；首次同步全量。

## 7. 触发机制

- **手动**：`POST /api/sync/zentao`（立即执行，返回本次同步摘要）
- **定时**：`node-cron`（`*/15 * * * *`），如果项目已有 BullMQ/Bull 则用其 repeatable job，顺便获得失败重试
- 两者共用 `sync()` 入口和同一把锁

## 8. 凭证与安全

- **方案 A**：服务账号在部署机执行一次 `zentao login`，凭证由 CLI 管理；代码只 spawn 命令，绝不读取 `~/.config/zentao/`（沿用 gw-teamai 仓库 `skills/common/zentao-cli` 的安全约束）
- **方案 B**：token 存数据库（加密列），`ZENTAO_URL` 走环境变量；token 过期（E1004 类错误）自动用账号密码续期或提示用户重新授权
- 日志与错误信息中不输出账号、token、任务正文

## 9. 分阶段实施

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P1 验证**（0.5 天） | 用真实实例验证取数路径：范围约束、assignedTo 过滤、分页、字段形状 | 拿到三源的真实 JSON 样本 |
| **P2 MVP**（2~3 天） | ZentaoClient + 规范化 + upsert + 手动触发 API + task/bug 两源 | 手动触发能把"我的任务"转成待办，重复触发不产生重复数据 |
| **P3 完善**（1~2 天） | story 源 + 定时轮询 + 状态同步/缺失处理 + sync_log | 定时跑稳定，禅道侧完成后待办自动变完成 |
| **P4 扩展（可选）** | 本地完成回写禅道（`task finish` / `bug resolve`）、多用户 token 管理、Webhook 实时推送 | — |

## 10. 主要风险

1. **禅道版本差异**：REST API 路径/字段随版本变化，Client 层要做版本探测和字段兼容，映射规则集中配置
2. **范围遍历的请求量**：产品/执行多时一次同步可能几十个请求，靠元数据缓存 + lastEditedDate 增量控制
3. **双向状态冲突**：本地手动完成的待办被同步覆盖回 open —— P2 就要定好策略（默认"本地完成优先"）
