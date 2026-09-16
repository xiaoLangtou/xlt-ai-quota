// 验证读取管线：用种子作为测试夹具直接注入内存 storage，再断言聚合/视图
import { UsageService } from "@/services/usage-service";
import type { UsageStorage } from "@/storage/storage";
import { buildSeedQuotas, buildSeedTokens } from "@/data/seed";
import type {
  AiSubscription,
  QuotaSnapshot,
  SyncInfo,
  TokenDailyUsage,
} from "@/types/usage";

function createMemoryStorage(seedQuotas: QuotaSnapshot[], seedTokens: TokenDailyUsage[]): UsageStorage {
  let quotas = [...seedQuotas];
  let tokens = [...seedTokens];
  let sync: SyncInfo = { lastSyncAt: null, syncing: false };
  let config: { baseUrl?: string } = {};
  let subscriptions: AiSubscription[] = [];
  let subscriptionPreferences = { usdToCnyRate: 7.2 };
  return {
    listQuotas: () => quotas,
    listQuotasByPlatform: (p) => quotas.filter((q) => q.platform === p),
    saveQuotas: (s) => {
      quotas = [...quotas, ...s];
    },
    listTokens: () => tokens,
    listTokensByRange: (start, end) => tokens.filter((t) => t.date >= start && t.date <= end),
    saveTokens: (r) => {
      const map = new Map(tokens.map((t) => [`${t.platform}|${t.date}|${t.model ?? ""}`, t]));
      for (const t of r) map.set(`${t.platform}|${t.date}|${t.model ?? ""}`, t);
      tokens = [...map.values()];
    },
    replaceTokensForPlatform: (platform, start, end, records) => {
      const kept = tokens.filter((t) => !(t.platform === platform && t.date >= start && t.date <= end));
      const map = new Map(kept.map((t) => [`${t.platform}|${t.date}|${t.model ?? ""}`, t]));
      for (const t of records) map.set(`${t.platform}|${t.date}|${t.model ?? ""}`, t);
      tokens = [...map.values()];
    },
    getSyncInfo: () => sync,
    saveSyncInfo: (i) => {
      sync = i;
    },
    listSubscriptions: () => subscriptions,
    saveSubscriptions: (items) => {
      subscriptions = items;
    },
    getSubscriptionPreferences: () => subscriptionPreferences,
    saveSubscriptionPreferences: (preferences) => {
      subscriptionPreferences = preferences;
    },
    getConnectorConfig: () => config,
    saveConnectorConfig: (c) => {
      config = c as { baseUrl?: string };
    },
    resetIfVersionChanged: () => false,
  };
}

const collectedAt = new Date().toISOString();
const storage = createMemoryStorage(buildSeedQuotas(collectedAt), buildSeedTokens(collectedAt));
const svc = new UsageService(storage);

const quotas = svc.getPlatformQuotaViews();
const summary = svc.getTokenSummary("7d");
const trend = svc.getTokenTrend("7d");
const daily = svc.getPlatformDaily("7d");

function fmt(n: number) {
  return (n / 1e6).toFixed(2) + "M";
}

let ok = true;
function assert(name: string, cond: boolean) {
  console.log(`${cond ? "PASS" : "FAIL"}  ${name}`);
  if (!cond) ok = false;
}

assert("4 个平台额度卡片", quotas.length === 4);
assert("Codex 5h 72%", quotas[0].windows[0]?.usedPct === 72);
assert("Ark weekly 83% warn", quotas[1].windows[1]?.usedPct === 83 && quotas[1].windows[1]?.tone === "orange");
assert("Kiro remaining 1240", quotas[2].credits?.remaining === 1240);
assert("Qoder remaining 1120", quotas[3].credits?.remaining === 1120);
assert("Qoder add-on remaining 1853", quotas[3].credits?.addOn?.remaining === 1853);

const arkCollectedAt = new Date(Date.parse(collectedAt) + 1_000).toISOString();
storage.saveQuotas([
  { platform: "ark", accountName: "coding-plan", metric: "session", used: 12, limit: 100, unit: "percent", collectedAt: arkCollectedAt },
  { platform: "ark", accountName: "coding-plan", metric: "weekly", used: 34, limit: 100, unit: "percent", collectedAt: arkCollectedAt },
  { platform: "ark", accountName: "coding-plan", metric: "monthly", used: 56, limit: 100, unit: "percent", collectedAt: arkCollectedAt },
  { platform: "ark", accountName: "agent-plan", metric: "five_hour", used: 23, limit: 100, unit: "percent", collectedAt: arkCollectedAt },
  { platform: "ark", accountName: "agent-plan", metric: "weekly", used: 45, limit: 100, unit: "percent", collectedAt: arkCollectedAt },
]);
const arkViews = svc.getPlatformQuotaViews().filter((item) => item.platform === "ark");
assert("方舟 Coding Plan 与 Agent Plan 分卡展示", arkViews.length === 2);
assert(
  "Coding Plan 保留 session/weekly/monthly",
  arkViews[0]?.planTag === "Coding Plan"
    && arkViews[0]?.windows.map((item) => item.metric).join(",") === "session,weekly,monthly",
);
assert(
  "Agent Plan 保留 five_hour/weekly",
  arkViews[1]?.planTag === "Agent Plan"
    && arkViews[1]?.windows.map((item) => item.metric).join(",") === "five_hour,weekly",
);
assert("总 Token ≈ 110.7M", Math.abs(summary.total - 110_700_000) < 200_000);
assert("输入 ≈ 86.3M", Math.abs(summary.input - 86_300_000) < 200_000);
assert("delta ≈ -12.6%", summary.deltaPct != null && Math.abs(summary.deltaPct - -12.6) < 0.5);
assert("趋势 7 点升序", trend.length === 7 && trend[0].date < trend[6].date);
assert("最旧日最大 ~36M", Math.abs(trend[0].total - 36_240_000) < 200_000);
assert("今日最小 ~1.19M", Math.abs(trend[6].total - 1_190_000) < 200_000);
assert("每日平台汇总 7 点", daily.length === 7);

storage.saveConnectorConfig({ quotaDisplay: { hiddenPlatforms: ["ark-agent"] } });
const codingOnlyViews = svc.getPlatformQuotaViews();
assert(
  "可单独隐藏方舟 Agent Plan",
  codingOnlyViews.filter((item) => item.platform === "ark").map((item) => item.planTag).join(",") === "Coding Plan",
);

storage.saveConnectorConfig({ quotaDisplay: { hiddenPlatforms: ["ark-coding", "ark-agent", "kiro"] } });
const visibleQuotas = svc.getPlatformQuotaViews();
assert(
  "隐藏未订阅的额度平台",
  visibleQuotas.map((item) => item.platform).join(",") === "codex,qoder",
);

console.log("\n总Token:", fmt(summary.total), "delta:", summary.deltaPct + "%");
console.log("结果:", ok ? "全部通过 ✅" : "存在失败 ❌");
if (!ok) process.exit(1);
