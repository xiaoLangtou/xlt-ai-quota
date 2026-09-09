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
  quotas: "xlt.quotas",
  tokens: "xlt.tokens",
  sync: "xlt.sync",
  config: "xlt.config",
  subscriptions: "xlt.subscriptions",
  bills: "xlt.bills",
  subscriptionPreferences: "xlt.subscription-preferences",
  version: "xlt.v",
} as const;

function read<T>(key: string, fallback: T): T {
  const raw = localStorage.getItem(key);
  if (!raw) return fallback;
  try {
    return JSON.parse(raw) as T;
  } catch {
    return fallback;
  }
}

function write<T>(key: string, value: T): void {
  localStorage.setItem(key, JSON.stringify(value));
}

export const webStorage: UsageStorage = {
  listQuotas() {
    return read<QuotaSnapshot[]>(KEYS.quotas, []);
  },
  listQuotasByPlatform(platform) {
    return this.listQuotas().filter((q) => q.platform === platform);
  },
  saveQuotas(snapshots) {
    const existing = this.listQuotas();
    // 追加历史快照；每平台每指标仅保留最近 200 条，避免无限增长
    const merged = [...existing, ...snapshots];
    const buckets = new Map<string, QuotaSnapshot[]>();
    for (const q of merged) {
      const key = `${q.platform}|${q.metric}`;
      const arr = buckets.get(key) ?? [];
      arr.push(q);
      buckets.set(key, arr);
    }
    const trimmed: QuotaSnapshot[] = [];
    for (const arr of buckets.values()) {
      arr
        .sort((a, b) => (a.collectedAt < b.collectedAt ? 1 : -1))
        .slice(0, 200)
        .forEach((q) => trimmed.push(q));
    }
    write(KEYS.quotas, trimmed);
  },

  listTokens() {
    return read<TokenDailyUsage[]>(KEYS.tokens, []);
  },
  listTokensByRange(start, end) {
    return this.listTokens().filter((t) => t.date >= start && t.date <= end);
  },
  saveTokens(records) {
    const existing = this.listTokens();
    const map = new Map<string, TokenDailyUsage>();
    for (const r of existing) {
      map.set(tokenKey(r), r);
    }
    for (const r of records) {
      map.set(tokenKey(r), r);
    }
    write(KEYS.tokens, [...map.values()]);
  },
  replaceTokensForPlatform(platform, start, end, records) {
    const kept = this.listTokens().filter(
      (t) => !(t.platform === platform && t.date >= start && t.date <= end),
    );
    const map = new Map<string, TokenDailyUsage>();
    for (const t of kept) map.set(tokenKey(t), t);
    for (const r of records) map.set(tokenKey(r), r);
    write(KEYS.tokens, [...map.values()]);
  },

  getSyncInfo() {
    return read<SyncInfo>(KEYS.sync, { lastSyncAt: null, syncing: false });
  },
  saveSyncInfo(info) {
    write(KEYS.sync, info);
  },

  listSubscriptions() {
    return read<AiSubscription[]>(KEYS.subscriptions, []);
  },
  saveSubscriptions(subscriptions) {
    write(KEYS.subscriptions, subscriptions);
  },
  listBills() {
    return read<BillEntry[]>(KEYS.bills, []);
  },
  saveBills(bills) {
    write(KEYS.bills, bills);
  },
  getSubscriptionPreferences() {
    return read<SubscriptionPreferences>(KEYS.subscriptionPreferences, { usdToCnyRate: 7.2 });
  },
  saveSubscriptionPreferences(preferences) {
    write(KEYS.subscriptionPreferences, preferences);
  },

  getConnectorConfig() {
    return read<ConnectorConfigPersist>(KEYS.config, {});
  },
  saveConnectorConfig(config) {
    write(KEYS.config, config);
  },

  resetIfVersionChanged(version: string): boolean {
    const current = localStorage.getItem(KEYS.version);
    if (current === version) return false;
    localStorage.removeItem(KEYS.quotas);
    localStorage.removeItem(KEYS.tokens);
    localStorage.removeItem(KEYS.sync);
    localStorage.setItem(KEYS.version, version);
    return true;
  },
};

function tokenKey(r: TokenDailyUsage): string {
  return `${r.platform}|${r.date}|${r.model ?? ""}`;
}

/**
 * 一次性迁移：旧版「订阅自动生成」的账单 source=auto_renew 静默改为 usage_settlement
 * （语义最接近：系统曾推断的一笔结算），并清理已废弃的去重键 xlt.bill-auto-keys。
 * 历史账单是已发生的事实，保留不删除。
 */
function migrateLegacyBills(): void {
  try {
    const raw = localStorage.getItem(KEYS.bills);
    if (raw) {
      const bills = JSON.parse(raw) as BillEntry[];
      let changed = false;
      const migrated = bills.map((b) => {
        if ((b.source as string) === "auto_renew") {
          changed = true;
          return { ...b, source: "usage_settlement" as const };
        }
        return b;
      });
      if (changed) localStorage.setItem(KEYS.bills, JSON.stringify(migrated));
    }
    localStorage.removeItem("xlt.bill-auto-keys");
  } catch {
    /* 忽略迁移失败 */
  }
}
migrateLegacyBills();

/** 工厂：当前环境仅提供 web 实现 */
export function createStorage(): UsageStorage {
  return webStorage;
}
