import { invoke } from "@tauri-apps/api/core";
import type {
  AiSubscription,
  BillEntry,
  QuotaSnapshot,
  QuotaStateSnapshot,
  QueueBucket,
  SubscriptionPreferences,
  SyncInfo,
  TokenDailyUsage,
  UsageCacheStats,
  UsageConfig,
} from "@/types/usage";
import type { ConnectorConfigPersist, UsageStorage } from "./storage";
import { webStorage } from "./web-storage";
import { aggregateBucketsToDaily, dailyToBuckets, dayBoundsUtc, localDate, localNoonUtc } from "@/utils/usage-time";

interface UsageBootstrap {
  config: UsageConfig;
  buckets: QueueBucket[];
  quotas: QuotaStateSnapshot[];
  cacheStats: UsageCacheStats;
  dataDir: string;
}

const MIGRATION_FLAG = "xlt.usage-migrated";
const LEGACY_KEYS = ["xlt.tokens", "xlt.quotas", "xlt.bills"] as const;

const EMPTY_CONFIG: UsageConfig = {
  timezone: null,
  statsSince: null,
  retentionDays: 180,
  pricingUrl: null,
  lastCleanupAt: null,
};

const EMPTY_STATS: UsageCacheStats = {
  dataDir: "",
  totalRows: 0,
  totalBytes: 0,
  oldestHour: null,
  newestHour: null,
  months: [],
  cursorSources: [],
};

function readLegacy<T>(key: string): T[] {
  try {
    const raw = localStorage.getItem(key);
    if (!raw) return [];
    const parsed: unknown = JSON.parse(raw);
    return Array.isArray(parsed) ? (parsed as T[]) : [];
  } catch {
    return [];
  }
}

/**
 * Tauri 桌面端存储：用量桶落 `~/.xlt-workbench/queue/YYYY-MM.jsonl`，
 * 额度状态落 `quotas.json`，配置落 `config.json`；内存镜像保证 UI 读取同步。
 */
class TauriUsageStorage implements UsageStorage {
  private buckets: QueueBucket[] = [];
  private quotas: QuotaStateSnapshot[] = [];
  private config: UsageConfig = { ...EMPTY_CONFIG };
  private stats: UsageCacheStats = { ...EMPTY_STATS };
  private dailyCache: TokenDailyUsage[] | null = null;
  private initialized = false;

  async init(): Promise<void> {
    if (this.initialized) return;
    await this.reload();
    await this.migrateLegacy();
    this.initialized = true;
  }

  private async reload(): Promise<void> {
    const boot = await invoke<UsageBootstrap>("usage_init");
    this.buckets = boot.buckets;
    this.quotas = boot.quotas;
    this.config = { ...EMPTY_CONFIG, ...boot.config };
    this.stats = boot.cacheStats;
    this.dailyCache = null;
  }

  private async migrateLegacy(): Promise<void> {
    if (localStorage.getItem(MIGRATION_FLAG) === "1") return;
    const tokens = readLegacy<{
      platform: string;
      date: string;
      model?: string;
      inputTokens?: number;
      outputTokens?: number;
      cachedTokens?: number;
      requestCount?: number;
    }>("xlt.tokens");
    const quotas = readLegacy<{
      platform: string;
      accountName?: string;
      metric?: string;
      used?: number;
      limit?: number;
      unit?: string;
      resetsAt?: string;
      collectedAt?: string;
    }>("xlt.quotas");
    const bills = readLegacy<BillEntry>("xlt.bills");
    if (tokens.length === 0 && quotas.length === 0 && bills.length === 0) {
      localStorage.setItem(MIGRATION_FLAG, "1");
      return;
    }
    try {
      await invoke("usage_import_legacy", {
        payload: {
          tokens: tokens.map((row) => ({
            platform: row.platform,
            date: row.date,
            hourStart: localNoonUtc(row.date, this.config.timezone ?? undefined),
            model: row.model,
            inputTokens: row.inputTokens ?? 0,
            outputTokens: row.outputTokens ?? 0,
            cachedTokens: row.cachedTokens ?? 0,
            requestCount: row.requestCount ?? 0,
          })),
          quotas: quotas.map((row) => ({
            platform: row.platform,
            accountName: row.accountName,
            metric: row.metric,
            used: row.used ?? 0,
            limit: row.limit ?? 0,
            unit: row.unit,
            resetsAt: row.resetsAt,
            collectedAt: row.collectedAt,
          })),
          bills: bills.length > 0 ? bills : null,
        },
      });
      for (const key of LEGACY_KEYS) localStorage.removeItem(key);
      localStorage.setItem(MIGRATION_FLAG, "1");
      await this.reload();
    } catch {
      // 迁移失败时保留旧键，下次启动重试。
    }
  }

  listQuotas(): QuotaSnapshot[] {
    return this.quotas.flatMap((snapshot) =>
      snapshot.limits.map((limit) => ({
        platform: snapshot.channel,
        accountName: snapshot.account ?? snapshot.planLabel ?? "",
        metric: (limit.id as QuotaSnapshot["metric"]) ?? "account",
        used: limit.usedPercent,
        limit: limit.total ?? 100,
        unit: limit.total != null ? "credits" : "percent",
        resetsAt: limit.resetsAt ?? undefined,
        collectedAt: snapshot.fetchedAt ?? new Date(0).toISOString(),
        balance: snapshot.balance,
      })),
    );
  }

  listQuotasByPlatform(platform: string): QuotaSnapshot[] {
    return this.listQuotas().filter((quota) => quota.platform === platform);
  }

  saveQuotas(_snapshots: QuotaSnapshot[]): void {
    // 旧接口：connector 读数由 service 转成状态快照后调用 saveQuotaStates。
  }

  listQuotaStates(): QuotaStateSnapshot[] {
    return this.quotas;
  }

  async saveQuotaStates(snapshots: QuotaStateSnapshot[]): Promise<void> {
    await invoke("usage_save_quotas", { snapshots });
    // 以 Rust 侧为准重新加载：套餐变更/失败降级/旧卡清理都在后端完成。
    this.quotas = await invoke<QuotaStateSnapshot[]>("usage_list_quotas");
  }

  listTokens(): TokenDailyUsage[] {
    if (!this.dailyCache) {
      this.dailyCache = aggregateBucketsToDaily(this.buckets, this.config.timezone ?? undefined);
    }
    return this.dailyCache;
  }

  listTokensByRange(start: string, end: string): TokenDailyUsage[] {
    return this.listTokens().filter((record) => record.date >= start && record.date <= end);
  }

  listBucketsByRange(start: string, end: string): QueueBucket[] {
    const timezone = this.config.timezone ?? undefined;
    return this.buckets.filter((bucket) => {
      const date = localDate(bucket.hourStart, timezone);
      return date >= start && date <= end;
    });
  }

  async saveTokens(records: TokenDailyUsage[]): Promise<void> {
    const buckets = dailyToBuckets(records, this.config.timezone ?? undefined);
    this.buckets = mergeBuckets(this.buckets, buckets);
    this.dailyCache = null;
    await invoke("usage_append_buckets", { buckets });
  }

  async replaceTokensForPlatform(
    platform: string,
    start: string,
    end: string,
    records: TokenDailyUsage[],
  ): Promise<void> {
    const timezone = this.config.timezone ?? undefined;
    const bounds = {
      start: dayBoundsUtc(start).start,
      end: dayBoundsUtc(end).end,
    };
    const incoming = dailyToBuckets(records, timezone);
    const startMs = Date.parse(bounds.start);
    const endMs = Date.parse(bounds.end);
    const kept = this.buckets.filter((bucket) => {
      if (bucket.source !== platform) return true;
      const at = Date.parse(bucket.hourStart);
      return Number.isNaN(at) || at < startMs || at > endMs;
    });
    this.buckets = mergeBuckets(kept, incoming);
    this.dailyCache = null;
    await invoke("usage_replace_source_range", {
      source: platform,
      start: bounds.start,
      end: bounds.end,
      buckets: incoming,
    });
  }

  getSyncInfo(): SyncInfo {
    return webStorage.getSyncInfo();
  }

  saveSyncInfo(info: SyncInfo): void {
    webStorage.saveSyncInfo(info);
  }

  listSubscriptions(): AiSubscription[] {
    return webStorage.listSubscriptions();
  }

  saveSubscriptions(subscriptions: AiSubscription[]): void {
    webStorage.saveSubscriptions(subscriptions);
  }

  getSubscriptionPreferences(): SubscriptionPreferences {
    return webStorage.getSubscriptionPreferences();
  }

  saveSubscriptionPreferences(preferences: SubscriptionPreferences): void {
    webStorage.saveSubscriptionPreferences(preferences);
  }

  listBills(): BillEntry[] {
    return webStorage.listBills();
  }

  saveBills(bills: BillEntry[]): void {
    webStorage.saveBills(bills);
  }

  resetIfVersionChanged(version: string): boolean {
    return webStorage.resetIfVersionChanged(version);
  }

  getConnectorConfig(): ConnectorConfigPersist {
    return webStorage.getConnectorConfig();
  }

  saveConnectorConfig(config: ConnectorConfigPersist): void {
    webStorage.saveConnectorConfig(config);
  }

  cacheStats(): UsageCacheStats {
    return this.stats;
  }

  async cleanup(retentionDays?: number): Promise<UsageCacheStats> {
    this.stats = await invoke<UsageCacheStats>("usage_cleanup", {
      retentionDays: retentionDays ?? this.config.retentionDays,
    });
    await this.reload();
    return this.stats;
  }

  async reloadQuotas(): Promise<void> {
    this.quotas = await invoke<QuotaStateSnapshot[]>("usage_list_quotas");
  }

  async reloadBuckets(): Promise<void> {
    this.buckets = await invoke<QueueBucket[]>("usage_load_buckets", { since: null });
    this.dailyCache = null;
    this.stats = await invoke<UsageCacheStats>("usage_cache_stats");
  }

  async clearCache(): Promise<UsageCacheStats> {
    this.stats = await invoke<UsageCacheStats>("usage_clear_cache");
    this.buckets = [];
    this.quotas = [];
    this.dailyCache = null;
    return this.stats;
  }

  getUsageConfig(): UsageConfig {
    return this.config;
  }

  saveUsageConfig(patch: Partial<UsageConfig>): void {
    this.config = { ...this.config, ...patch };
    void invoke("usage_set_config", { patch }).catch(() => undefined);
  }
}

function mergeBuckets(base: QueueBucket[], incoming: QueueBucket[]): QueueBucket[] {
  const map = new Map(base.map((bucket) => [bucketKey(bucket), bucket]));
  for (const bucket of incoming) map.set(bucketKey(bucket), bucket);
  return [...map.values()];
}

function bucketKey(bucket: QueueBucket): string {
  return `${bucket.source}|${bucket.collector ?? ""}|${bucket.model}|${bucket.project}|${bucket.hourStart}`;
}

export function createUsageStorage(): UsageStorage {
  const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  return isTauri ? new TauriUsageStorage() : webStorage;
}
