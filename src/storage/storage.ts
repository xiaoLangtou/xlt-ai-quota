import type {
  AiSubscription,
  BillEntry,
  QuotaDisplayTarget,
  QuotaSnapshot,
  QuotaStateSnapshot,
  QueueBucket,
  SubscriptionPreferences,
  SyncInfo,
  TokenDailyUsage,
  UsageCacheStats,
  UsageConfig,
} from "@/types/usage";

/**
 * 本地业务存储抽象。
 *
 * 当前实现分两类：
 * - Tauri 桌面：用量桶 / 额度状态 / 缓存统计走 `~/.xlt-workbench`（Rust command）。
 * - 浏览器 / 测试：回退到 localStorage 的旧实现。
 */
export interface UsageStorage {
  /** 初始化远端/文件存储；浏览器实现为空操作。 */
  init?(): Promise<void>;

  // 订阅额度快照（connector 原始读数，旧路径）
  listQuotas(): QuotaSnapshot[];
  listQuotasByPlatform(platform: string): QuotaSnapshot[];
  saveQuotas(snapshots: QuotaSnapshot[]): void | Promise<void>;

  /** 归一化额度状态快照（新模型）。旧实现可省略。 */
  listQuotaStates?(): QuotaStateSnapshot[];
  saveQuotaStates?(snapshots: QuotaStateSnapshot[]): void | Promise<void>;

  // Token 日统计
  listTokens(): TokenDailyUsage[];
  /** 按 [start, end] 区间（含）返回，date 格式 YYYY-MM-DD */
  listTokensByRange(start: string, end: string): TokenDailyUsage[];
  /** 写入（按 platform + date + model 去重 upsert） */
  saveTokens(records: TokenDailyUsage[]): void | Promise<void>;
  /** 删除某平台在 [start, end] 区间内的记录（用于真实拉取覆盖种子） */
  replaceTokensForPlatform(
    platform: string,
    start: string,
    end: string,
    records: TokenDailyUsage[],
  ): void | Promise<void>;

  // 同步状态
  getSyncInfo(): SyncInfo;
  saveSyncInfo(info: SyncInfo): void;

  // AI 订阅管理
  listSubscriptions(): AiSubscription[];
  saveSubscriptions(subscriptions: AiSubscription[]): void;
  getSubscriptionPreferences(): SubscriptionPreferences;
  saveSubscriptionPreferences(preferences: SubscriptionPreferences): void;

  // 账单流水台账（真实持久化：手动记账 / 用量结算）
  listBills(): BillEntry[];
  saveBills(bills: BillEntry[]): void;

  /** 数据版本变更时清空全部本地数据（含旧种子）并写入新版本，返回是否发生了重置 */
  resetIfVersionChanged(version: string): boolean;

  // 配置（API Key 等敏感信息；本机保存，不入库不入 git）
  getConnectorConfig(): ConnectorConfigPersist;
  saveConnectorConfig(config: ConnectorConfigPersist): void;

  /** 本地用量缓存统计（新存储）。 */
  cacheStats?(): UsageCacheStats;
  /** 按保留天数清理过期月份。 */
  cleanup?(retentionDays?: number): Promise<UsageCacheStats>;
  /** 清空全部用量缓存（保留配置）。 */
  clearCache?(): Promise<UsageCacheStats>;
  /** 从磁盘重新加载用量桶（Rust 采集器自行落盘后使用）。 */
  reloadBuckets?(): Promise<void>;
  /** 从磁盘重新加载额度快照（托盘面板与主窗口共用文件）。 */
  reloadQuotas?(): Promise<void>;
  /** 按本地日期区间读取原始半小时桶（小时级图表用）。 */
  listBucketsByRange?(start: string, end: string): QueueBucket[];

  /** 用量数据目录配置（时区 / 起始日 / 保留天数 / 定价地址）。 */
  getUsageConfig?(): UsageConfig;
  saveUsageConfig?(patch: Partial<UsageConfig>): void;
}

export interface UsagePreferences {
  /** IANA 时区名（如 Asia/Shanghai）；空表示跟随系统。用于日/小时分桶。 */
  timezone?: string;
  /** 统计起始日 YYYY-MM-DD；早于此日期的数据不纳入分析与同步。 */
  statsSince?: string;
}

export interface ConnectorConfigPersist {
  /** 用量统计偏好（时区 / 起始日）。 */
  preferences?: UsagePreferences;
  ark?: { baseUrl?: string };
  /** 国内油价监控；API Key 可选，省份为空时不启动采集。 */
  oil?: { province?: string; apiKey?: string; grade?: string };
  /** 套餐额度区域中由用户隐藏的平台或方舟套餐。 */
  quotaDisplay?: { hiddenPlatforms?: QuotaDisplayTarget[] };
  /** DeepSeek 账户余额查询用的 API Key（仅本机保存，可选）。 */
  deepseek?: { apiKey?: string };
}
