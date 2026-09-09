import type {
  AiSubscription,
  BillEntry,
  QuotaSnapshot,
  SubscriptionPreferences,
  SyncInfo,
  TokenDailyUsage,
} from "@/types/usage";

/**
 * 本地存储抽象。当前实现为 WebStorage（localStorage），用于 Vite dev / 浏览器。
 * 待 Tauri 桌面壳就绪后，提供基于 tauri-plugin-sql 的 SQLite 实现。
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

export interface ConnectorConfigPersist {
  ark?: { baseUrl?: string };
}
