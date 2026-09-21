import type {
  AiSubscription,
  BillEntry,
  QuotaDisplayTarget,
  QuotaSnapshot,
  SubscriptionPreferences,
  SyncInfo,
  TokenDailyUsage,
} from "@/types/usage";

/**
 * 本地业务存储抽象，当前由 Web Storage（localStorage）持久化。
 */
export interface UsageStorage {
  // 订阅额度快照
  listQuotas(): QuotaSnapshot[];
  listQuotasByPlatform(platform: string): QuotaSnapshot[];
  saveQuotas(snapshots: QuotaSnapshot[]): void;

  // Token 日统计
  listTokens(): TokenDailyUsage[];
  /** 按 [start, end] 区间（含）返回，date 格式 YYYY-MM-DD */
  listTokensByRange(start: string, end: string): TokenDailyUsage[];
  /** 写入（按 platform + date + model 去重 upsert） */
  saveTokens(records: TokenDailyUsage[]): void;
  /** 删除某平台在 [start, end] 区间内的记录（用于真实拉取覆盖种子） */
  replaceTokensForPlatform(
    platform: string,
    start: string,
    end: string,
    records: TokenDailyUsage[],
  ): void;

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
}
