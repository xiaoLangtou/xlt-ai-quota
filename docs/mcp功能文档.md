# XLT Workbench · MCP 功能设计文档

> 适用版本：XLT Workbench v0.1.3 起
> 范围：MCP 服务（配置管理）+ MCP 库（发现与安装）
> 关联文档：《侧边栏整改方案》

---

## 一、概述

### 1.1 目标

| 模块 | 一句话定位 |
|---|---|
| MCP 服务 | 一处管理，多处生效：统一查看、添加、编辑、启停各 Agent 的 MCP 配置，并可同步到其他 Agent |
| MCP 库 | 发现 → 一键装进 MCP 服务：聚合多个数据源，浏览、搜索、生成配置 |

### 1.2 设计原则

1. **各 Agent 自己的配置文件是唯一真源**，本应用只做读写和元数据补充，不另存一份配置真身。
2. **MCP 库不自己写配置**，安装动作全部复用 MCP 服务的写入流水线。
3. **所有写入可预览、可备份、可回滚**。
4. **装 MCP = 执行第三方代码**，安全提示优先于便利性。
5. 沿用现有约定：本地优先、桌面端专属（Tauri）、Rust 侧处理网络与文件、SQLite 存元数据。

### 1.3 非目标（当前阶段不做）

- 不内置 MCP 运行时/网关，不代理 MCP 流量。
- 不做多用户/云同步。
- 不批量镜像第三方站点数据。
- 不自动更新已安装的 MCP 服务版本。

---

## 二、侧边栏与路由

```
MCP
  MCP 服务  [异常数]   /mcp
  MCP 库               /mcp/library
```

| 路由 | 页面 |
|---|---|
| `/mcp` | MCP 服务列表 |
| `/mcp/library` | MCP 库 |
| `/mcp/new`（可选） | 添加 MCP，也可用抽屉，不必独立路由 |

- 「MCP 服务」徽章只统计**配置错误 + 命令不存在**，红色，为 0 不显示。
- 「工作区」筛选（全局 / 项目 / Agent）做成页内组件，与技能库共用，不进侧边栏。

---

## 三、MCP 服务

### 3.1 数据模型

一个服务可出现在多个 Agent、多个范围里，按"名称 + 配置哈希"合并展示。

```ts
McpServer {                    // 归一化后的服务定义
  name: string
  transport: 'stdio' | 'http' | 'sse'
  command?: string             // stdio
  args?: string[]
  env?: Record<string, string>
  url?: string                 // http / sse
  headers?: Record<string, string>
  extra: Record<string, any>   // 未识别字段，原样保留
}

McpInstance {                  // 该服务在某处的落点
  server: McpServer
  agent: AgentId
  scope: 'global' | 'project'
  projectPath?: string
  configFile: string
  enabled: boolean
  status: 'ok' | 'disabled' | 'config_error' | 'command_missing' | 'unchecked'
}
```

本地 SQLite 只存补充信息：备注、标签、来源（来自 MCP 库哪一条）、被"暂存"停用的服务。**不存生效配置**。

### 3.2 页面

#### 3.2.1 服务列表 `/mcp`

- **工具条**：工作区下拉、Agent 筛选、搜索、状态筛选、「添加」、「重新扫描」。
- **列**：名称 / 类型 / 命令或 URL / 已安装到（Agent 头像组）/ 范围 / 状态 / 操作（编辑、启停、同步到…、删除）。
- **状态标签**：正常、已停用、配置错误、命令不存在、未检测。
- 进入页面自动扫描各 Agent 现有配置并导入列表，无需手动录入。

#### 3.2.2 添加抽屉

| Tab | 说明 |
|---|---|
| 粘贴 JSON / 命令 | 支持 `{"mcpServers": {...}}` 等常见格式，也支持粘贴 `claude mcp add ...`，自动解析。最常用入口，MCP 库的"手动配置"也送到这里 |
| 手动填写 | 名称、类型；stdio：command / args / env；http：url / headers |

底部固定「安装目标」：Agent × 范围（全局 / 选择项目）。确认前展示**写入预览**（每个目标文件的 diff）。

#### 3.2.3 编辑抽屉

- 表单与原始 JSON/TOML 两种视图切换。
- env / headers 的值默认遮罩，点击显示。
- 「测试连接」按钮，结果展示工具列表。
- 底部提示"该服务同时存在于：Claude 全局、Codex 全局"，修改时可选「同步更新全部」或「仅此处」。

### 3.3 Agent 适配层

统一接口，每个 Agent 一个适配器：

```
detect()             → 配置文件是否存在
read(scope)          → 解析并归一化为 McpServer[]
write(scope, ops)    → 增/改/删，保留未知字段
supportsDisable()    → 是否原生支持停用字段
```

**配置位置映射**（基于已知格式，实现前必须逐个实测；标"待实测"的先只读）：

| Agent | 全局配置 | 项目配置 | 格式 / 备注 |
|---|---|---|---|
| Claude Code | `~/.claude.json` → `mcpServers` | `<项目>/.mcp.json`；个人项目级在 `~/.claude.json` 的 `projects.<路径>.mcpServers` | JSON。该文件很大且含运行时状态，**写入必须最小改动** |
| Codex | `~/.codex/config.toml` → `[mcp_servers.<name>]` | 视版本而定 | TOML，需保留注释（Rust 用 `toml_edit`） |
| Gemini CLI | `~/.gemini/settings.json` → `mcpServers` | `.gemini/settings.json` | JSON |
| Kiro | `~/.kiro/settings/mcp.json` | `.kiro/settings/mcp.json` | JSON，含 `disabled` 字段 |
| OpenCode | `~/.config/opencode/opencode.json` → `mcp` | 项目 `opencode.json` | 结构差异大（`type: local/remote`、`command` 为数组），归一化要专门处理 |
| Qoder / Kimi / Copilot / 其他 | 待实测 | 待实测 | 先只读，确认后再开放写入 |

**启停**：原生支持停用字段的 Agent 直接改字段；不支持的，把服务从配置中移出并存入本地"暂存区"，恢复时写回。

### 3.4 写入流水线

增 / 改 / 删 / 启停 / 同步 / 库安装，统一走同一条流水线：

1. 生成变更计划（每个目标文件一个 diff）
2. 用户确认
3. 备份原文件到 `~/.xlt/backups/mcp/<agent>/<时间戳>/`
4. 原子写入（写临时文件 → rename）
5. 重新读取校验，失败则自动回滚
6. 刷新列表

**同步到其他 Agent**：选中服务 → 选目标 Agent 与范围 → 适配器转换格式 → 走上述流水线。目标不支持某字段时（如不支持 headers），在预览中明确标出。

### 3.5 状态检测

分两层，避免打开页面就到处启动进程：

| 层级 | 检查内容 | 触发 |
|---|---|---|
| 配置层（自动） | JSON/TOML 是否可解析、必填字段、stdio 的 command 是否在 PATH、URL 格式 | 扫描时 |
| 运行层（手动） | 启动进程或连接 URL，发送 `initialize` + `tools/list`，超时 10 秒，展示工具列表，用完即关 | 用户点「测试连接」 |

---

## 四、MCP 库

### 4.1 数据源

库本身不绑定站点，靠适配器接入，与「安装 Skills → 数据源管理」同一思路。

| 数据源 | 说明 | 阶段 |
|---|---|---|
| 内置精选 | 随应用打包 20 条左右常用服务，离线可用，字段完整（含运行方式） | 一期 |
| 官方 MCP Registry | 据我所知官方有公开的注册表与 REST 接口，字段规范。接口地址与字段需实测确认 | 一期 |
| GitCode（AtomGit） | 前端聚合接口，仅有仓库信息，见 4.3 | 一期 |
| 自定义源 | 用户填符合约定格式的 JSON URL（团队内部库） | 二期 |
| GitHub 精选列表 | 解析 Awesome 类仓库 README 或 JSON | 三期 |

### 4.2 统一数据模型

```ts
McpPackage {
  id: string                       // source + 规范化仓库名
  name: string
  description: string
  source: 'builtin' | 'registry' | 'gitcode' | 'custom' | ...
  homepage: string
  repo?: string                    // 规范化后的 owner/name，用于跨源去重
  tags: string[]
  language?: { name: string; color?: string }
  license?: string
  platforms?: string[]
  stars?: number
  forks?: number
  updatedAt?: string
  verified?: boolean
  mirror?: boolean                 // 是否为镜像仓库
  kind: 'server' | 'client' | 'other'   // 库里默认只展示 server
  install: InstallInfo             // 见下
}

InstallInfo =
  | { level: 'ready';                // 可一键安装
      runtimes: Runtime[];
      envSpec: EnvVar[] }
  | { level: 'manual' }              // 只有仓库信息，需读 README

Runtime {
  type: 'npx' | 'uvx' | 'pip' | 'docker' | 'remote'
  command?: string; args?: string[]; url?: string; version?: string
}

EnvVar { name: string; description?: string; required: boolean; secret: boolean }
```

### 4.3 GitCode 适配器

#### 4.3.1 接口概况

| 项 | 内容 |
|---|---|
| 地址 | `https://web-api.gitcode.com/api/v1/agg/index` |
| 性质 | 网站前端使用的聚合接口，**无官方文档**，字段与参数可能随时变化 |
| 鉴权 | 无需登录或 Token |
| 反爬 | 云 WAF 拦截：不带 `Referer` 返回 418；带 `Referer: https://gitcode.com/` 与浏览器 UA 返回 200 |
| 分页 | `page` / `per_page`（页面使用 18），响应含 `total`、`page_count`、`content[]` |

#### 4.3.2 请求参数

| 参数 | 取值 | 说明 |
|---|---|---|
| `channel_id` / `c_id` | 配置项（默认 `6a55a5361944323916325288`） | 两者取相同值 |
| `sub_channel_id` | 配置项，默认空 | 见 4.3.4 |
| `sort` | `all` / `star_desc` / `updated_at_desc` | 综合 / Star 数 / 更新时间 |
| `repo_type` | `-1` | 固定 |
| `m_code` | `recommendList` | 固定 |
| `d_code` | `projects` | 固定 |
| `page` | 从 1 起 | 分页 |
| `per_page` | `18` | 沿用页面取值，更大值未验证 |
| `keyword` | 用户输入 | 为空时不传 |

**Fork 数排序**：页面上有该选项，但取值未确认，因此**首版只开放 综合 / Star / 更新时间 三种排序**。

#### 4.3.4 频道与 MCP 过滤（重要）

已知该 `channel_id` 返回的是「智能体」大类下的混合内容（Agent 框架、Skills 仓库、Agent 应用等），并非纯 MCP 服务；MCP、Skills、Agents 三个 Tab 很可能靠 `sub_channel_id` 区分，但**当前未拿到 MCP Tab 对应的值**。

因此首版采用兜底方案，并把频道参数做成可配置项，之后填入正确值即可切换，无需改代码：

1. `channel_id`、`sub_channel_id` 放进数据源配置，默认值如上。
2. **本地过滤**：只保留 `topic[].name` 含 `MCP 服务` 或 `MCP` 的条目；含 `MCP Clients` 的标为 `kind: client`，默认隐藏（可通过筛选开关显示）。
3. **补页策略**：过滤后一页条数会不足。单次用户操作内，若命中数不足 18，自动继续请求下一页，**最多连续 3 页**，且遵守限流间隔；超过仍不足则展示已有结果并提供「加载更多」。
4. 浏览态（无关键词）时是否用 `keyword=mcp` 缩小范围，**需实测其是否匹配 topic**，验证前不启用。
5. 频道配置填入正确的 `sub_channel_id` 后，本地过滤降级为可选（保留客户端过滤）。

#### 4.3.5 字段映射

| 接口字段 | 库内字段 | 处理 |
|---|---|---|
| `namespace` | `repo` | 去空格转小写；镜像仓库的 `url` 是镜像路径，`namespace` 才是原仓库名，去重以此为准 |
| `url` | `homepage` | 直接用 |
| `description` | `description` | 直接用 |
| `star_count` / `fork_count` | `stars` / `forks` | 直接用 |
| `updated_at` | `updatedAt` | ISO 时间 |
| `language[0]` / `language[1]` | `language.name` / `language.color` | 缺失则为空 |
| `topic[].name` | `tags` | 过滤 `频道-` 前缀；许可证类（如 MIT、Apache-2.0）提为 `license`；`Windows` / `macOS` / `Linux` 提为 `platforms` |
| `is_mirrors` / `is_gh_mirrors` | `mirror` | 任一为真标"镜像" |
| —— | `install` | 固定为 `{ level: 'manual' }` |

所有字段按可选处理，缺字段不报错。

#### 4.3.6 安装信息的获取（懒加载）

GitCode 条目没有运行方式，安装信息来自仓库 README：

1. 用户打开详情抽屉时才拉 README，并缓存。
2. 从代码块中识别 `"mcpServers"` JSON、`claude mcp add ...`、`npx` / `uvx` / `docker run` 命令，**复用「添加 → 粘贴 JSON」的解析器**。
3. 解析出的命令必须完整展示并要求用户确认，**永不自动执行**。
4. README 的获取方式（开放接口路径、是否需要 Token）**尚未验证**，实现前实测。拉不到时降级为「在浏览器打开仓库」。
5. README 属不可信内容，只作为文本展示，不渲染其中脚本。

#### 4.3.7 请求封装

```rust
const BASE: &str = "https://web-api.gitcode.com/api/v1/agg/index";

#[derive(Clone, Copy)]
pub enum Sort { All, Star, Updated }

impl Sort {
    fn as_str(self) -> &'static str {
        match self {
            Sort::All => "all",
            Sort::Star => "star_desc",
            Sort::Updated => "updated_at_desc",
        }
    }
}

pub async fn fetch_page(
    client: &reqwest::Client,
    cfg: &GitcodeSourceCfg,        // channel_id / sub_channel_id
    sort: Sort,
    page: u32,
    keyword: Option<&str>,
) -> Result<AggPage, CatalogError> {
    let page_s = page.to_string();
    let mut q = vec![
        ("channel_id", cfg.channel_id.as_str()),
        ("sub_channel_id", cfg.sub_channel_id.as_str()),
        ("sort", sort.as_str()),
        ("repo_type", "-1"),
        ("m_code", "recommendList"),
        ("d_code", "projects"),
        ("c_id", cfg.channel_id.as_str()),
        ("page", page_s.as_str()),
        ("per_page", "18"),
    ];
    if let Some(k) = keyword.filter(|k| !k.trim().is_empty()) {
        q.push(("keyword", k));
    }

    let resp = client
        .get(BASE)
        .query(&q)
        .header("Referer", "https://gitcode.com/")
        .header("User-Agent", UA)              // 常量：浏览器 UA
        .header("Accept", "application/json")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await?;

    match resp.status().as_u16() {
        200 => Ok(resp.json::<AggPage>().await?),
        418 | 403 | 429 => Err(CatalogError::Blocked), // 立即停止，不重试，降级缓存
        s => Err(CatalogError::Http(s)),
    }
}
```

#### 4.3.8 限流与降级

| 项 | 规则 |
|---|---|
| 缓存键 | `channel + sub_channel + sort + keyword + page` |
| TTL | 浏览 6 小时；关键词搜索 30 分钟 |
| 请求间隔 | 同一源 ≥ 1 秒；搜索输入防抖 500ms |
| 分页 | 滚动到底再拉下一页，不预取，不全量爬取（样本显示该频道有上千条、上百页） |
| 被拦截 | 返回 `Blocked` 后该源冷却 10 分钟，界面显示缓存数据并提示"数据源暂时不可用"，不重试 |

### 4.4 页面

#### 4.4.1 库列表 `/mcp/library`

- **工具条**：搜索、来源筛选、分类、运行时（npx / uvx / docker / 远程）、排序（综合 / Star / 更新时间）、「显示客户端类项目」开关、「刷新」（显示上次同步时间）。
- **卡片**：名称、简介、来源标记、语言、Star、更新时间、镜像标记、安装档位（可一键安装 / 需手动配置）。
- 已安装的显示「已安装 · Claude、Codex」。支持收藏。
- 底部「加载更多」；数据源不可用时顶部提示条。

#### 4.4.2 详情抽屉

README 摘要、（如有）工具列表、可选运行方式、需要的环境变量、仓库链接、来源与镜像说明、风险提示条。

#### 4.4.3 安装向导（复用 MCP 服务的添加抽屉）

| 档位 | 流程 |
|---|---|
| 可一键安装 | 选运行方式 → 填变量（secret 项可"从密钥库选择"）→ 选目标（Agent × 范围）→ 预览并确认 |
| 需手动配置 | 展示 README 中解析出的配置片段 → 「送到添加抽屉」→ 用户核对修改 → 选目标 → 预览并确认 |

- 依赖检测：所需运行时（`npx` / `uvx` / `docker`）本机缺失时，对应运行方式置灰并提示。
- 安装成功后在 MCP 服务中记录来源（库条目 id）。

### 4.5 缓存与匹配

- **本地表**：`mcp_catalog`（条目）、`mcp_catalog_source`（源配置与同步状态）、`mcp_favorite`。
- **跨源去重**：以规范化 `repo` 为键，镜像与原仓库合并，优先展示信息更完整的一条。
- **已安装匹配**：按 `runtime.command + 包名` 或记录的来源 id 匹配库条目。
- **离线**：所有源失败时展示缓存，标注数据时间。
- 每个源独立超时，一个源失败不影响其他源。

---

## 五、安全

### 5.1 执行第三方代码

- stdio 服务本质是执行任意命令：添加、测试前都展示**完整命令行**，需用户确认；从粘贴内容或 README 导入时**不自动执行任何东西**。
- `npx` / `uvx` 可能每次拉取最新版，存在供应链风险：写入时默认**固定版本**，未固定则提示。
- 来源分级标记：内置精选 / 官方 Registry 已验证 / 社区 / 自定义源。

### 5.2 密钥

- Agent 读取的配置文件里必须是真实值（或 Agent 自身支持的环境变量引用），无法解析 `vault:` 这类引用，因此"引用密钥库就不落明文"**做不到**。
- v1：表单里"从密钥库选择"填充值，写入时仍是明文，界面明确提示。
- 后续：提供包装命令，运行时从密钥库取值注入环境变量，配置里只放包装命令。
- 备份目录权限 `0700`，备份可能含密钥，保留份数可配置（默认每文件 10 份）。
- env / headers 在日志和错误信息中一律脱敏。

### 5.3 网络

- 请求全部走 Rust 侧，复用 Skills 已有的 SSRF 防护（拒绝私网/本地地址）与响应大小限制，仅允许 https。
- 自定义源和 README 内容一律当数据展示，不执行、不渲染脚本。
- 界面注明数据来源并链接回原仓库；仅个人低频使用，不做批量镜像。

---

## 六、存储与代码结构

### 6.1 存储

| 数据 | 位置 |
|---|---|
| 生效的 MCP 配置 | 各 Agent 自己的配置文件（唯一真源） |
| 备注 / 标签 / 来源 / 暂存服务 | 本地 SQLite |
| 库条目 / 源配置 / 收藏 / 缓存 | 本地 SQLite |
| 配置备份 | `~/.xlt/backups/mcp/<agent>/<时间戳>/` |

### 6.2 目录结构

```
src-tauri/src/mcp/
  mod.rs                命令注册
  adapters/             claude.rs codex.rs gemini.rs kiro.rs opencode.rs
  scan.rs               扫描 + 归一化 + 合并
  plan.rs               生成 diff 计划
  write.rs              备份、原子写入、回滚
  probe.rs              测试连接
  catalog/
    mod.rs  sync.rs  matcher.rs  readme.rs
    sources/            builtin.rs registry.rs gitcode.rs custom.rs

src/api/mcp.ts
src/stores/mcp.ts
src/views/mcp/
  McpListView.vue  McpEditDrawer.vue  McpAddDrawer.vue
  McpLibraryView.vue  McpPackageDrawer.vue
```

### 6.3 Tauri 命令

| 模块 | 命令 |
|---|---|
| 服务 | `mcp_scan`、`mcp_plan`、`mcp_apply`、`mcp_probe`、`mcp_restore_backup` |
| 库 | `mcp_catalog_sync`、`mcp_catalog_list`、`mcp_catalog_detail`、`mcp_catalog_readme`、`mcp_catalog_source_save` |

---

## 七、分期

| 期 | MCP 服务 | MCP 库 |
|---|---|---|
| 一期 | Claude / Codex / Gemini **只读**扫描 + 列表 + 配置层状态检测 | 内置精选 + 官方 Registry + GitCode 源的**浏览与搜索**（不含安装） |
| 二期 | 写入流水线（备份 / diff / 回滚）+ 添加 / 编辑 / 删除 / 启停（仅全局范围） | 安装向导（可一键安装档 + 手动配置档）、README 解析、收藏、已安装标记 |
| 三期 | 同步到其他 Agent、测试连接、粘贴 JSON / 命令解析、项目级范围 | 自定义源、版本更新提示、跨源去重完善 |
| 四期 | 更多 Agent 适配、密钥库包装命令、导出分享 | 更多数据源、GitCode 频道参数正式化（填入 MCP 专属 `sub_channel_id`）、Fork 排序 |

**依赖关系**：MCP 库的安装环节依赖 MCP 服务的写入流水线，因此库的「安装」必须晚于服务二期；一期可先做只读浏览。

---

## 八、风险与应对

| # | 风险 | 应对 |
|---|---|---|
| 1 | `~/.claude.json` 是 Claude Code 的运行时状态文件，Agent 运行中可能同时写入 | 写前重读、检测 mtime 变化，冲突则中止并提示 |
| 2 | 各 Agent 配置格式随版本变化 | 适配器带版本容错，未知字段一律保留 |
| 3 | 项目级配置位置在不同 Agent 间差异最大 | 二期只做全局，项目级放三期 |
| 4 | GitCode 接口未公开，字段 / 参数 / WAF 策略随时可能变 | 适配器隔离，坏了只影响该源；频道参数可配置；失败降级缓存 |
| 5 | GitCode 频道混入非 MCP 内容，本地过滤后条数不稳定 | 补页策略（最多 3 页）+「加载更多」；后续填入正确 `sub_channel_id` |
| 6 | 镜像仓库与原仓库重复 | 以规范化 `repo` 去重 |
| 7 | README 解析出的命令被恶意构造 | 完整展示并强制确认，永不自动执行 |
| 8 | 高频请求触发 WAF 封禁 | 限流、缓存、冷却，不全量爬取 |

---

## 九、待确认事项

| # | 事项 | 影响 | 处理方式 |
|---|---|---|---|
| 1 | GitCode「MCP」Tab 对应的 `sub_channel_id`（或独立 `channel_id`） | 决定是否需要本地过滤 | 首版走本地过滤兜底，配置项预留 |
| 2 | Fork 数排序的 `sort` 取值 | 是否开放 Fork 排序 | 首版不开放 |
| 3 | 关键词是否匹配 topic / 描述 | 浏览态能否用关键词缩小范围 | 首版不启用，实测后决定 |
| 4 | README 的可用获取接口及是否需要 Token | 详情页解析安装信息 | 实现前实测，失败降级为浏览器打开 |
| 5 | 官方 MCP Registry 的接口地址与字段 | 官方源适配 | 实现前实测 |
| 6 | Qoder / Kimi / Copilot 等 Agent 的 MCP 配置位置与格式 | 适配范围 | 先只读，实测后开放写入 |
| 7 | `per_page` 是否支持大于 18 的值 | 分页效率 | 首版沿用 18 |

---

## 十、验收标准

**MCP 服务**
- [ ] 进入页面自动列出各 Agent 已有 MCP 配置，合并同名同配置项。
- [ ] 任意写入操作都经过 diff 预览，并在写入前生成备份。
- [ ] 写入后校验失败会自动回滚，且不丢失文件中的未知字段与注释。
- [ ] 「测试连接」超时 10 秒内返回结果，进程用完即关。
- [ ] 侧边栏徽章只统计错误类问题，为 0 不显示。

**MCP 库**
- [ ] 数据源失败（含 418 拦截）时展示缓存并给出可读提示，不无限重试。
- [ ] GitCode 源请求带 `Referer` 与浏览器 UA，同源请求间隔 ≥ 1 秒。
- [ ] 客户端类项目默认不出现在列表中，可通过开关显示。
- [ ] 镜像与原仓库不重复出现。
- [ ] 需手动配置的条目，其命令在写入前完整展示并需要用户确认。
- [ ] 本机缺少 `npx` / `uvx` / `docker` 时，对应运行方式置灰并给出提示。
- [ ] 新增数据源只需新增一个适配器文件并注册，不改列表与安装流程代码。
