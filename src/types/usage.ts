// 数据模型 - 与产品方案一致

export type Platform = "ark" | "codex" | "kiro" | "qoder" | "opencode-go";

export type QuotaMetric =
  | "five_hour"
  | "weekly"
  | "monthly"
  | "session"
  | "credits"
  | "addon_credits"
  | "account";

export type QuotaUnit = "percent" | "usd_value" | "credits";

/** 订阅额度快照，保存当前与历史订阅状态 */
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
  /** 与上一周期对比的总变化百分比，负值表示下降 */
  deltaPct?: number;
  inputDeltaPct?: number;
  outputDeltaPct?: number;
  requestsDeltaPct?: number;
}

export interface TrendPoint {
  date: string;
  total: number;
  input: number;
  output: number;
  requests: number;
}

/** 贡献热力图的单日数据。 */
export interface HeatmapDay {
  /** YYYY-MM-DD */
  date: string;
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
  platform: Platform;
  name: string;
  planTag: string;
  logoChar: string;
  logoClass: string;
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
  "opencode-go": { name: "OpenCode Go", logoChar: "O", logoClass: "open" },
};

/** 用于存储 connector 配置（API Key 等），仅保存在本机 */
export interface ConnectorConfig {
  /** Ark 走本机 arkcli，仅可覆盖 base URL */
  ark?: { baseUrl?: string };
}
