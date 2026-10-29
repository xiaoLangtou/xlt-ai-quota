// 数据模型 - 与产品方案一致

export type Platform = "ark" | "codex" | "kimi" | "kiro" | "qoder" | "opencode-go" | "deepseek";

/** 会在「套餐额度」区域展示的平台。 */
export const QUOTA_PLATFORMS = ["codex", "ark", "kiro", "qoder", "kimi", "deepseek"] as const;
export type QuotaPlatform = (typeof QUOTA_PLATFORMS)[number];

/** 套餐额度视图的显示开关；方舟的两种套餐可独立控制。 */
export const QUOTA_DISPLAY_TARGETS = ["codex", "ark-coding", "ark-agent", "kiro", "qoder", "kimi", "deepseek"] as const;
export type QuotaDisplayTarget = (typeof QUOTA_DISPLAY_TARGETS)[number];

export type QuotaMetric =
  | "five_hour"
  | "weekly"
  | "monthly"
  | "monthly_code"
  | "session"
  | "credits"
  | "addon_credits"
  | "account"
  | "balance";

export type QuotaUnit = "percent" | "usd_value" | "credits" | "currency";

/** DeepSeek 账户余额（赠送 + 充值），来自 /user/balance。 */
export interface QuotaBalance {
  total: number;
  currency: "CNY" | "USD";
  granted: number;
  toppedUp: number;
  /** 账户当前是否有余额可供调用 */
  available: boolean;
}

/** 订阅额度快照（connector 原始读数）。保存当前与历史订阅状态 */
export interface QuotaSnapshot {
  platform: Platform;
  accountName: string;
  metric: QuotaMetric;
  used: number;
  limit: number;
  unit: QuotaUnit;
  /** ISO 时间字符串 */
  resetsAt?: string;
  /** ISO 时间字符串，采集时间 */
  collectedAt: string;
  /** DeepSeek 等账户余额（metric=balance 时携带） */
  balance?: QuotaBalance;
}

/** 额度读取状态机；决定卡片展示额度、空态还是操作提示。 */
export type QuotaStatus =
  | "ready"
  | "not-installed"
  | "not-signed-in"
  | "expired"
  | "access-denied"
  | "custom-provider"
  | "unsupported-account"
  | "unsupported-plan"
  | "unsupported-version"
  | "temporarily-unavailable"
  | "unavailable";

/** 一个可展示的额度窗口（对应 juejin 的 limits[]）。 */
export interface QuotaWindow {
  id: string;
  label: string;
  usedPercent: number;
  resetsAt?: string | null;
  description?: string | null;
  remaining?: number | null;
  total?: number | null;
}

/**
 * 归一化后的额度快照（三类结构共用）：
 * - 命名窗口：fiveHour / weekly / sevenDay
 * - 模型池窗口：primaryModels / otherModels / plan
 * - 额度列表：limits[]
 */
export interface QuotaStateSnapshot {
  channel: Platform;
  account?: string | null;
  status: QuotaStatus;
  planLabel?: string | null;
  region?: "global" | "mainland" | null;
  limits: QuotaWindow[];
  fiveHour?: QuotaWindow;
  weekly?: QuotaWindow;
  sevenDay?: QuotaWindow;
  primaryModels?: QuotaWindow;
  otherModels?: QuotaWindow;
  plan?: QuotaWindow;
  fetchedAt?: string | null;
  stale: boolean;
  message?: string | null;
  balance?: QuotaBalance;
}

/** 本地半小时用量桶，落盘在 ~/.xlt-workbench/queue/YYYY-MM.jsonl。 */
export interface QueueBucket {
  hourStart: string;
  source: string;
  collector?: string;
  project: string;
  model: string;
  inputTokens: number;
  outputTokens: number;
  cachedInputTokens: number;
  cacheCreationInputTokens: number;
  reasoningOutputTokens: number;
  totalTokens: number;
  conversationCount: number;
  reportedCostUsd?: number;
  localMetrics?: {
    requestCount: number;
    cacheHitTokens: number;
    cacheMissTokens: number;
    requestCountComplete: boolean;
  };
}

/** 按天聚合后的 Token 使用量 */
export interface TokenDailyUsage {
  platform: string;
  /** YYYY-MM-DD */
  date: string;
  model?: string;
  inputTokens: number;
  outputTokens: number;
  cachedTokens?: number;
  /** 缓存写入（Claude cache_creation 等），单独计价 */
  cacheWriteTokens?: number;
  reasoningTokens?: number;
  requestCount?: number;
  collectedAt: string;
}

export type SubscriptionBillingCycle = "monthly" | "yearly";
export type SubscriptionStatus = "active" | "paused";

/** 账单分类。 */
export type BillCategory =
  | "model_subscription"
  | "coding_tool"
  | "api_relay"
  | "image_media"
  | "other";

/** 支付方式。 */
export type BillPaymentMethod =
  | "alipay"
  | "wechat"
  | "credit_card"
  | "paypal"
  | "balance"
  | "other";

/** 账单来源：用量结算 / 手动记账。账单只记录真实发生的花费，不再有系统自动生成。 */
export type BillSource = "usage_settlement" | "manual";

/** 用户手工管理的 AI 服务订阅，仅保存在本机。 */
export interface AiSubscription {
  id: string;
  provider: string;
  planName: string;
  account: string;
  price: number;
  currency: "CNY" | "USD";
  billingCycle: SubscriptionBillingCycle;
  /** YYYY-MM-DD */
  nextBillingDate: string;
  status: SubscriptionStatus;
  /** 账单分类（用于自动生成账单时归类；旧数据可能缺省） */
  category?: BillCategory;
  /** 默认支付方式（用于自动生成账单；旧数据可能缺省） */
  paymentMethod?: BillPaymentMethod;
  note: string;
  createdAt: string;
  updatedAt: string;
}

/** 一条账单流水记录，真实持久化在本机（手动记账或订阅到期自动生成）。 */
export interface BillEntry {
  id: string;
  /** YYYY-MM-DD 记账/扣费日期 */
  date: string;
  /** 关联订阅 id（自动生成时有；手动记账可空） */
  subscriptionId?: string;
  /** 服务名称（关联订阅名或手动填写） */
  service: string;
  category: BillCategory;
  amount: number;
  currency: "CNY" | "USD";
  paymentMethod: BillPaymentMethod;
  source: BillSource;
  note: string;
  createdAt: string;
  updatedAt: string;
}

/** 订阅统计的本机偏好设置。 */
export interface SubscriptionPreferences {
  /** 1 USD 可折算的 CNY 金额。 */
  usdToCnyRate: number;
}

export const BILL_CATEGORY_LABEL: Record<BillCategory, string> = {
  model_subscription: "模型订阅",
  coding_tool: "编程工具",
  api_relay: "API 中转",
  image_media: "图像/多媒体",
  other: "其他",
};

export const BILL_PAYMENT_LABEL: Record<BillPaymentMethod, string> = {
  alipay: "支付宝",
  wechat: "微信",
  credit_card: "信用卡",
  paypal: "PayPal",
  balance: "余额",
  other: "其他",
};

export const BILL_SOURCE_LABEL: Record<BillSource, string> = {
  usage_settlement: "用量结算",
  manual: "手动记账",
};

export const BILL_CATEGORY_OPTIONS = (Object.keys(BILL_CATEGORY_LABEL) as BillCategory[]).map(
  (value) => ({ value, label: BILL_CATEGORY_LABEL[value] }),
);

export const BILL_PAYMENT_OPTIONS = (Object.keys(BILL_PAYMENT_LABEL) as BillPaymentMethod[]).map(
  (value) => ({ value, label: BILL_PAYMENT_LABEL[value] }),
);

// ---- 以下为界面层派生类型 ----

export type RangePreset = "today" | "7d" | "30d" | "90d";

export interface TokenSummary {
  total: number;
  input: number;
  output: number;
  cached?: number;
  requests: number;
  /** 预估费用（USD），按模型单价估算 */
  costUsd: number;
  /** 与上一周期对比的总变化百分比，负值表示下降 */
  deltaPct?: number;
  inputDeltaPct?: number;
  outputDeltaPct?: number;
  requestsDeltaPct?: number;
  costDeltaPct?: number;
}

export interface TrendPoint {
  date: string;
  total: number;
  input: number;
  output: number;
  requests: number;
  /** 当日预估费用（USD） */
  costUsd: number;
}

/** 贡献热力图的单日数据。 */
export interface HeatmapDay {
  /** YYYY-MM-DD */
  date: string;
  total: number;
  details: HeatmapDetail[];
}

export interface HeatmapDetail {
  platform: string;
  model?: string;
  total: number;
}

export interface PlatformDailyPoint {
  date: string;
  /** key = 平台名，value = 当日该平台 Token */
  byPlatform: Record<string, number>;
}

/** 按 AI 工具（平台）聚合的用量，用于「按工具统计」。 */
export interface ToolUsage {
  /** 平台/工具标识（如 codex、claude、opencode-go、ark） */
  platform: string;
  total: number;
  input: number;
  output: number;
  requests: number;
  /** 预估费用（USD） */
  costUsd: number;
  /** 占所选周期总 Token 的百分比 */
  pct: number;
}

/** 按模型聚合的用量，用于「按模型统计」。 */
export interface ModelUsage {
  /** 规范化后的模型名（如 claude-opus-4-8、gpt-5.5、quest-ultimate） */
  model: string;
  /** 该模型主要归属的平台（Token 最多的那个），用于配色/图标 */
  platform: string;
  total: number;
  input: number;
  output: number;
  cached: number;
  requests: number;
  /** 预估费用（USD），按模型单价估算 */
  costUsd: number;
  /** 该模型是否命中内置定价（false 表示走默认价，UI 标注预估） */
  priced: boolean;
  /** 占所选周期总 Token 的百分比 */
  pct: number;
}

export type SyncOutcome = "success" | "partial" | "failed";

export interface SyncInfo {
  lastSyncAt: string | null;
  syncing: boolean;
  /** 最近一次同步的整体结果；旧本地数据可能没有该字段。 */
  outcome?: SyncOutcome;
  /** 最近一次同步的错误信息（若有） */
  error?: string;
}

/** 卡片展示用：单个平台聚合后的额度视图 */
export interface PlatformQuotaView {
  /** 视图唯一标识；同一平台有多个套餐时仍保持稳定。 */
  id: string;
  platform: Platform;
  name: string;
  planTag: string;
  logoChar: string;
  logoClass: string;
  /** 读取状态；缺省视为 ready（旧数据兼容）。 */
  status?: QuotaStatus;
  /** true 表示正在展示上一次成功读数。 */
  stale?: boolean;
  /** 面向用户的状态说明。 */
  message?: string | null;
  /** 当展示 Credits 时使用 */
  credits?: {
    remaining: number;
    total: number;
    refreshIn?: string;
    expiresIn?: string;
    addOn?: { remaining: number; total: number };
  };
  /** 登录/账户信息（无额度数值时仍可展示，如 Codex 经 CLI） */
  account?: { label: string };
  /** 账户余额（DeepSeek），与 windows / credits 互斥展示 */
  balance?: QuotaBalance;
  /** 时间窗口额度（5h / weekly / monthly / session） */
  windows: QuotaWindowView[];
  collectedAt: string;
}

export interface QuotaWindowView {
  metric: QuotaMetric;
  label: string;
  usedPct: number;
  /** 已用 / 上限，用于原生单位展示 */
  usedText?: string;
  resetsAt?: string;
  resetsIn?: string;
  /** bar 颜色主题 */
  tone: "blue" | "orange" | "purple" | "green";
}

export const PLATFORM_META: Record<
  Platform,
  { name: string; logoChar: string; logoClass: string }
> = {
  codex: { name: "Codex", logoChar: "⌘", logoClass: "codex" },
  ark: { name: "火山方舟", logoChar: "V", logoClass: "ark" },
  kiro: { name: "Kiro", logoChar: "K", logoClass: "kiro" },
  qoder: { name: "Qoder", logoChar: "Q", logoClass: "qoder" },
  kimi: { name: "Kimi", logoChar: "K", logoClass: "kimi" },
  "opencode-go": { name: "OpenCode Go", logoChar: "O", logoClass: "open" },
  deepseek: { name: "DeepSeek", logoChar: "D", logoClass: "deepseek" },
};

/** 用于存储 connector 配置（API Key 等），仅保存在本机 */
export interface ConnectorConfig {
  /** Ark 走本机 arkcli，仅可覆盖 base URL */
  ark?: { baseUrl?: string };
  /** DeepSeek 账户余额查询需要 API Key（仅本机保存，可选） */
  deepseek?: { apiKey?: string };
}

/** 本地用量缓存统计（设置页「缓存」区块）。 */
export interface UsageCacheMonthStats {
  month: string;
  rows: number;
  bytes: number;
  firstHour?: string | null;
  lastHour?: string | null;
}

export interface UsageCacheStats {
  dataDir: string;
  totalRows: number;
  totalBytes: number;
  oldestHour?: string | null;
  newestHour?: string | null;
  months: UsageCacheMonthStats[];
  cursorSources: string[];
}

/** 用量数据目录配置。 */
export interface UsageConfig {
  timezone?: string | null;
  statsSince?: string | null;
  retentionDays: number;
  pricingUrl?: string | null;
  lastCleanupAt?: string | null;
}
