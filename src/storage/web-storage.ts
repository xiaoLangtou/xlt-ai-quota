import type {
  AiSubscription,
  BillEntry,
  QuotaSnapshot,
  SubscriptionPreferences,
  SyncInfo,
  TokenDailyUsage,
} from "@/types/usage";
import type { ConnectorConfigPersist, UsageStorage } from "./storage";

const KEYS = {
  quotas: "xlt.quotas", tokens: "xlt.tokens", sync: "xlt.sync", config: "xlt.config",
  subscriptions: "xlt.subscriptions", bills: "xlt.bills",
  subscriptionPreferences: "xlt.subscription-preferences", version: "xlt.v",
} as const;

function read<T>(key: string, fallback: T): T {
  const raw = localStorage.getItem(key);
  if (!raw) return fallback;
  try { return JSON.parse(raw) as T; } catch { return fallback; }
}
function write<T>(key: string, value: T): void { localStorage.setItem(key, JSON.stringify(value)); }
function tokenKey(record: TokenDailyUsage): string { return `${record.platform}|${record.date}|${record.model ?? ""}`; }

/** 将旧版自动续费账单标记迁移为用量结算，并清理过期去重键。 */
function migrateLegacyBills(): void {
  try {
    const raw = localStorage.getItem(KEYS.bills);
    if (raw) {
      const bills = JSON.parse(raw) as BillEntry[];
      let changed = false;
      const migrated = bills.map((bill) => {
        if ((bill.source as string) !== "auto_renew") return bill;
        changed = true;
        return { ...bill, source: "usage_settlement" as const };
      });
      if (changed) localStorage.setItem(KEYS.bills, JSON.stringify(migrated));
    }
    localStorage.removeItem("xlt.bill-auto-keys");
  } catch {
    // 本地旧数据损坏时不阻断应用启动。
  }
}
migrateLegacyBills();

export const webStorage: UsageStorage = {
  listQuotas() { return read<QuotaSnapshot[]>(KEYS.quotas, []); },
  listQuotasByPlatform(platform) { return this.listQuotas().filter((quota) => quota.platform === platform); },
  saveQuotas(snapshots) {
    const buckets = new Map<string, QuotaSnapshot[]>();
    for (const snapshot of [...this.listQuotas(), ...snapshots]) {
      const key = `${snapshot.platform}|${snapshot.accountName}|${snapshot.metric}`;
      buckets.set(key, [...(buckets.get(key) ?? []), snapshot]);
    }
    const result: QuotaSnapshot[] = [];
    for (const bucket of buckets.values()) {
      bucket.sort((a, b) => (a.collectedAt < b.collectedAt ? 1 : -1)).slice(0, 200).forEach((item) => result.push(item));
    }
    write(KEYS.quotas, result);
  },
  listTokens() { return read<TokenDailyUsage[]>(KEYS.tokens, []); },
  listTokensByRange(start, end) { return this.listTokens().filter((record) => record.date >= start && record.date <= end); },
  saveTokens(records) {
    const recordsByKey = new Map<string, TokenDailyUsage>();
    for (const record of [...this.listTokens(), ...records]) recordsByKey.set(tokenKey(record), record);
    write(KEYS.tokens, [...recordsByKey.values()]);
  },
  replaceTokensForPlatform(platform, start, end, records) {
    const recordsByKey = new Map<string, TokenDailyUsage>();
    for (const record of this.listTokens().filter((item) => !(item.platform === platform && item.date >= start && item.date <= end))) recordsByKey.set(tokenKey(record), record);
    for (const record of records) recordsByKey.set(tokenKey(record), record);
    write(KEYS.tokens, [...recordsByKey.values()]);
  },
  getSyncInfo() { return read<SyncInfo>(KEYS.sync, { lastSyncAt: null, syncing: false }); },
  saveSyncInfo(info) { write(KEYS.sync, info); },
  listSubscriptions() { return read<AiSubscription[]>(KEYS.subscriptions, []); },
  saveSubscriptions(subscriptions) { write(KEYS.subscriptions, subscriptions); },
  listBills() { return read<BillEntry[]>(KEYS.bills, []); },
  saveBills(bills) { write(KEYS.bills, bills); },
  getSubscriptionPreferences() { return read<SubscriptionPreferences>(KEYS.subscriptionPreferences, { usdToCnyRate: 7.2 }); },
  saveSubscriptionPreferences(preferences) { write(KEYS.subscriptionPreferences, preferences); },
  getConnectorConfig() { return read<ConnectorConfigPersist>(KEYS.config, {}); },
  saveConnectorConfig(config) { write(KEYS.config, config); },
  resetIfVersionChanged(version) {
    if (localStorage.getItem(KEYS.version) === version) return false;
    localStorage.removeItem(KEYS.quotas);
    localStorage.removeItem(KEYS.tokens);
    localStorage.removeItem(KEYS.sync);
    localStorage.setItem(KEYS.version, version);
    return true;
  },
};

export function createStorage(): UsageStorage { return webStorage; }
