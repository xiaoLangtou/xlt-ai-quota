// 数据模型 - 与产品方案一致

export type Platform = "ark" | "codex" | "kiro" | "opencode-go";

export type QuotaMetric = "five_hour" | "weekly" | "monthly" | "session" | "credits" | "account";

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

// ---- 以下为界面层派生类型 ----

export type RangePreset = "today" | "7d" | "30d";

export interface TokenSummary {
  total: number;
  input: number;
  output: number;
  cached?: number;
  /** 与上一周期对比的总变化百分比，负值表示下降 */
  deltaPct?: number;
}

export interface TrendPoint {
  date: string;
  total: number;
  input: number;
  output: number;
}

export interface PlatformDailyPoint {
  date: string;
  /** key = 平台名，value = 当日该平台 Token */
  byPlatform: Record<string, number>;
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
  credits?: { remaining: number; total: number; refreshIn?: string };
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
  "opencode-go": { name: "OpenCode Go", logoChar: "O", logoClass: "open" },
};

/** 用于存储 connector 配置（API Key 等），仅保存在本机 */
export interface ConnectorConfig {
  /** Ark 走本机 arkcli，仅可覆盖 base URL */
  ark?: { baseUrl?: string };
  openai?: { adminKey?: string; orgId?: string; baseUrl?: string };
}
