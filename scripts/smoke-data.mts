// 验证读取管线：用种子作为测试夹具直接注入内存 storage，再断言聚合/视图
import { UsageService } from "@/services/usage-service";
import type { UsageStorage } from "@/storage/storage";
import { buildSeedQuotas, buildSeedTokens } from "@/data/seed";
import type {
  QuotaSnapshot,
  SyncInfo,
  TokenDailyUsage,
} from "@/types/usage";

function createMemoryStorage(seedQuotas: QuotaSnapshot[], seedTokens: TokenDailyUsage[]): UsageStorage {
  let quotas = [...seedQuotas];
  let tokens = [...seedTokens];
  let sync: SyncInfo = { lastSyncAt: null, syncing: false };
  let config: { baseUrl?: string; adminKey?: string; orgId?: string } = {};
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
    getConnectorConfig: () => config,
    saveConnectorConfig: (c) => {
      config = c as { baseUrl?: string; adminKey?: string; orgId?: string };
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
assert("OpenCode weekly 64%", quotas[3].windows[1]?.usedPct === 64);
assert("总 Token ≈ 110.7M", Math.abs(summary.total - 110_700_000) < 200_000);
assert("输入 ≈ 86.3M", Math.abs(summary.input - 86_300_000) < 200_000);
assert("delta ≈ -12.6%", summary.deltaPct != null && Math.abs(summary.deltaPct - -12.6) < 0.5);
assert("趋势 7 点升序", trend.length === 7 && trend[0].date < trend[6].date);
assert("最旧日最大 ~36M", Math.abs(trend[0].total - 36_240_000) < 200_000);
assert("今日最小 ~1.19M", Math.abs(trend[6].total - 1_190_000) < 200_000);
assert("每日平台汇总 7 点", daily.length === 7);

console.log("\n总Token:", fmt(summary.total), "delta:", summary.deltaPct + "%");
console.log("结果:", ok ? "全部通过 ✅" : "存在失败 ❌");
if (!ok) process.exit(1);
