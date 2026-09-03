# AI 用量看板产品方案

AI 用量看板统一展示个人开发者在多个 AI 平台上的订阅额度和 Token 使用量。产品只聚焦使用状态，帮助用户判断某个平台当前是否可用，以及近期的 Token 消耗趋势。

产品不提供团队协作、预算、费用管理、告警或任务管理功能。

## 产品目标

看板解决两个问题：

1. 用户可以在同一个页面查看 Codex、火山方舟、Kiro、OpenCode Go 等平台的订阅额度。
2. 用户可以查看可获取的输入 Token、输出 Token、总 Token 和按日趋势。

订阅额度和 Token 使用量是两类独立数据。产品保留平台的原生计量单位，不将 Credits、美元价值额度或滚动时间窗口换算成虚构的 Token 配额。

## 产品形态

产品采用本地优先的 Web 应用形态，并配套浏览器连接器。

- Vue Web 应用展示看板。
- Node.js 本地服务运行同步任务，并保存本地历史数据。
- 浏览器连接器在用户已登录的官方账户页面读取订阅额度状态。
- 数据库与访问凭证保留在用户本机。

本地 Web 应用适合个人开发者在工作期间快速查看用量。浏览器连接器解决部分订阅产品仅在账户控制台展示用量状态的问题。

## 功能范围

### 各平台订阅额度

看板展示每个平台的套餐名称、当前额度进度、原生单位和重置时间。

| 平台 | 展示内容 |
| --- | --- |
| Codex | 5 小时额度、周额度、重置时间 |
| 火山方舟 | 5 小时额度、周额度、重置时间 |
| Kiro | 当前 Credits、周期已用 Credits、刷新时间 |
| OpenCode Go | 5 小时、周、月额度及重置时间 |

OpenCode Go 的额度按美元价值计算。官方当前规则为 5 小时 $12、每周 $30、每月 $60；模型价格不同，因此相同额度不能直接等同于固定 Token 数。[OpenCode Go 用量规则](https://dev.opencode.ai/docs/go/)

Kiro 使用 Credits 作为原生计量单位。其订阅面板提供月度 Credits 上限、已用 Credits 和剩余 Credits，数据至少每 5 分钟更新一次。[Kiro Credits 说明](https://kiro.dev/pricing/)

### Token 使用量

Token 区域只展示下列指标：

- 总 Token。
- 输入 Token。
- 输出 Token。
- 最近 7 天或 30 天的用量趋势。
- 按平台汇总的每日 Token。

产品只在数据源提供精确 Token 数据时展示 Token 统计。订阅额度页面中的百分比、Credits 或美元价值不参与 Token 聚合。

## 页面结构

看板保持单页，包含两个连续区块。

~~~text
AI 用量看板
├─ 订阅额度
│  ├─ Codex
│  ├─ 火山方舟
│  ├─ Kiro
│  └─ OpenCode Go
└─ Token 使用量
   ├─ 总 Token、输入 Token、输出 Token
   ├─ Token 用量趋势
   └─ 每日平台汇总
~~~

订阅额度卡片只显示必要数据：平台、套餐、额度进度和重置时间。Token 区域使用数据卡片、折线图和堆叠柱状图呈现统计结果。

当前静态界面原型位于 [token-usage-prototype.html](./token-usage-prototype.html)。

## 数据来源

产品按来源类型拆分采集方式。

| 来源类型 | 适用平台 | 获取的数据 |
| --- | --- | --- |
| 官方用量 API | 火山方舟、OpenAI API | Token、请求数、费用或套餐用量 |
| 官方账户控制台 | Codex、Kiro、OpenCode Go | 订阅额度、Credits、周期和重置时间 |
| 请求返回的 usage 字段 | OpenCode Go 等兼容 API | 输入 Token、输出 Token、缓存 Token |

### 火山方舟

火山方舟使用官方套餐用量接口获取订阅额度。官方 GetUsageDetails 接口用于获取套餐用量详情。[火山方舟 API 文档](https://www.volcengine.com/docs/82379/2479849?lang=zh)

采集器按平台提供的凭证拉取套餐用量和可用的用量统计，并将结果按天保存。

### OpenAI API

OpenAI API 使用组织级 Usage API 获取 Token 用量。该接口可提供输入 Token、输出 Token、缓存 Token、请求数，并支持按模型、项目和 API Key 分组；Costs API 提供费用聚合数据。[OpenAI Usage API](https://developers.openai.com/api/reference/resources/admin/subresources/organization/subresources/usage)

该连接需要组织级 Admin Key。Codex 的 ChatGPT 订阅额度与 OpenAI API 用量必须作为不同数据源处理。

### Codex

Codex 订阅额度由用户已登录的 Codex 或 ChatGPT 用量页面提供。浏览器连接器读取页面中显示的 5 小时额度、周额度和重置时间，并保存为额度快照。

Codex 订阅额度不等同于 API Token 用量。用户通过 OpenAI API 产生的 Token 用量由 OpenAI Usage API 采集。

### Kiro

Kiro 订阅状态由 Kiro Subscription Dashboard 提供。浏览器连接器读取当前套餐、Credits 上限、已用 Credits、余额和下个周期时间。

Kiro 卡片始终使用 Credits 展示，不参与 Token 总量计算。

### OpenCode Go

OpenCode Go 的订阅额度由 OpenCode Zen Console 提供。浏览器连接器读取 5 小时、周和月额度的当前状态。

当 OpenCode Go 的请求响应提供 usage 字段时，采集器将输入、输出和缓存 Token 保存为 Token 记录。额度进度仍按其美元价值窗口展示。

## 数据模型

~~~ts
type QuotaSnapshot = {
  platform: "ark" | "codex" | "kiro" | "opencode-go"
  accountName: string
  metric: "five_hour" | "weekly" | "monthly" | "credits"
  used: number
  limit: number
  unit: "percent" | "usd_value" | "credits"
  resetsAt?: string
  collectedAt: string
}

type TokenDailyUsage = {
  platform: string
  date: string
  model?: string
  inputTokens: number
  outputTokens: number
  cachedTokens?: number
  requestCount?: number
  collectedAt: string
}
~~~

QuotaSnapshot 保存当前与历史订阅状态。TokenDailyUsage 保存按天聚合后的 Token 使用量。界面根据 unit 渲染百分比、Credits 或美元价值。

## 系统架构

~~~text
官方 API ─────────────┐
                      ├─ Node.js 本地服务 ─→ SQLite ─→ Vue 看板
官方账户控制台页面 ────┘
~~~

本地采集器为每个平台提供独立 Connector。Connector 只负责获取和标准化自己的平台数据，界面不直接依赖任一平台的原始接口格式。

~~~text
connectors/
├─ ark.ts
├─ openai-api.ts
├─ codex-browser.ts
├─ kiro-browser.ts
└─ opencode-browser.ts
~~~

## 本地存储与安全

SQLite 保存平台配置、额度快照和 Token 日统计。系统钥匙串保存 API Key 和网页登录授权信息；数据库不保存明文访问凭证。

每条采集结果记录采集时间和来源。看板显示最后同步时间，用户可以识别当前数据是否已过期。

## 技术选型

当前原型使用 HTML 和 CSS。正式产品使用 Vue 3 和 TypeScript 开发界面，并由 Node.js 本地服务负责数据同步和存储。

| 层级 | 技术 |
| --- | --- |
| 界面 | Vue 3、TypeScript、Vite |
| 组件开发方式 | Composition API、script setup |
| 图表 | Apache ECharts |
| 本地服务 | Node.js、TypeScript、Hono |
| 本地数据 | SQLite |
| 凭证存储 | 操作系统凭证库 |
| 浏览器侧采集 | 浏览器扩展 |
| 平台同步 | 独立 Connector |

Vue 组件统一使用 script setup 和 TypeScript。页面状态通过 composable 管理；当前产品只有单页看板，不引入全局状态管理。

~~~text
src/
├─ views/
│  └─ DashboardView.vue
├─ components/
│  ├─ QuotaCard.vue
│  ├─ TokenSummaryCard.vue
│  ├─ TokenTrendChart.vue
│  └─ PlatformDailyChart.vue
├─ composables/
│  └─ useUsageDashboard.ts
├─ services/
│  └─ usage-service.ts
└─ types/
   └─ usage.ts
~~~

## 开发阶段

### 第一阶段：本地看板

完成 Vue 看板、Node.js 本地服务、数据库和当前看板界面。使用本地数据结构保存额度快照与 Token 日统计。

### 第二阶段：官方 API Connector

接入火山方舟和 OpenAI API。验证套餐额度、输入 Token、输出 Token 和日趋势的数据正确性。

### 第三阶段：浏览器连接器

接入 Codex、Kiro 和 OpenCode Go 的官方账户控制台。连接器读取用户账户中已展示的订阅状态，并同步到本地 Web 应用。

### 第四阶段：数据验证

核对看板数据与各平台官方页面或官方接口的结果。重点验证滚动周期的重置时间、Kiro Credits 单位和 OpenCode Go 的美元价值额度。

## 验收标准

- 看板可同时展示 Codex、火山方舟、Kiro、OpenCode Go 的当前订阅额度。
- 每个平台卡片显示原生单位和重置时间。
- 火山方舟与 OpenAI API 显示真实输入、输出和总 Token 日趋势。
- Kiro 使用 Credits 展示，OpenCode Go 使用美元价值额度展示。
- 用户的历史数据和访问凭证保留在本机。
