// 验证 ArkConnector 对 arkcli JSON 的映射（mock fetch，不依赖 dev 中间件）

const _store = new Map<string, string>();
(globalThis as unknown as { localStorage: Storage }).localStorage = {
  getItem: (k: string) => _store.get(k) ?? null,
  setItem: (k: string, v: string) => void _store.set(k, String(v)),
  removeItem: (k: string) => void _store.delete(k),
  clear: () => _store.clear(),
  key: () => null,
  get length() {
    return _store.size;
  },
} as Storage;

const planJson = {
  viewer: { auth_method: "sso" },
  items: [
    {
      product: "coding-plan",
      subscribed: true,
      periods: [
        { label: "session", percent: 98.52, reset_at: "2026-08-27T15:25:07+08:00" },
        { label: "weekly", percent: 58.01, reset_at: "2026-08-31T00:00:00+08:00" },
        { label: "monthly", percent: 71.45, reset_at: "2026-09-17T23:59:59+08:00" },
      ],
    },
    {
      product: "agent-plan",
      subscribed: true,
      periods: [
        { label: "5h", used: 250, total: 1000, percent: 25, reset_at: "2026-08-27T17:00:00+08:00" },
        { label: "weekly", used: 12500, total: 50000, percent: 25, reset_at: "2026-09-06T00:00:00+08:00" },
      ],
    },
    { product: "coding-plan-team", subscribed: false, periods: [] },
  ],
};

const statsJson = {
  fields: [],
  records: [
    { Day: "2026-08-25", InputTokens: "1200", OutputTokens: "380", CacheTokensHit: "0", ReqCnt: "5" },
    { Day: "2026-08-26", InputTokens: "980", OutputTokens: "410", CacheTokensHit: "12", ReqCnt: "3" },
  ],
  totals: { InputTokens: "2180", OutputTokens: "790", TotalTokens: "2970", ReqCnt: "8" },
  data_count: 2,
};

(globalThis as unknown as { fetch: typeof fetch }).fetch = ((url: string | URL | Request) => {
  const u = String(url);
  if (u.includes("/plan")) return Promise.resolve(new Response(JSON.stringify(planJson), { status: 200 }));
  if (u.includes("/stats")) return Promise.resolve(new Response(JSON.stringify(statsJson), { status: 200 }));
  return Promise.resolve(new Response("not found", { status: 404 }));
}) as typeof fetch;

import { ArkConnector } from "@/connectors/ark";

let ok = true;
function assert(name: string, cond: boolean) {
  console.log(`${cond ? "PASS" : "FAIL"}  ${name}`);
  if (!cond) ok = false;
}

const c = new ArkConnector();
assert("isConfigured 恒为 true（arkcli 路径）", c.isConfigured() === true);

const quotas = await c.fetchQuotas();
assert("映射出 5 条额度快照（coding 3 + agent 2）", quotas.length === 5);
const session = quotas.find((q) => q.metric === "session");
assert("session metric 映射", !!session);
assert("session percent 取整 99", session?.used === 99);
assert("session resetsAt 透传", session?.resetsAt === "2026-08-27T15:25:07+08:00");
const weekly = quotas.find((q) => q.metric === "weekly");
assert("weekly percent 取整 58", weekly?.used === 58);
const fiveH = quotas.find((q) => q.metric === "five_hour");
assert("5h metric 映射", !!fiveH && fiveH.used === 25);
const monthly = quotas.find((q) => q.metric === "monthly");
assert("monthly metric 映射", !!monthly && monthly.used === 71);
assert("accountName=product", quotas.every((q) => ["coding-plan", "agent-plan"].includes(q.accountName)));
assert("platform=ark", quotas.every((q) => q.platform === "ark"));
assert("unit=percent", quotas.every((q) => q.unit === "percent"));

const tokens = await c.fetchTokens({ start: "2026-08-25", end: "2026-08-26" });
assert("映射出 2 条 token 记录", tokens.length === 2);
assert("日期 8/25", tokens[0]?.date === "2026-08-25");
assert("input 字符串转数字", tokens[0]?.inputTokens === 1200);
assert("output 字符串转数字", tokens[0]?.outputTokens === 380);
assert("cached 透传", tokens[1]?.cachedTokens === 12);
assert("requestCount 透传", tokens[0]?.requestCount === 5);
assert("platform=ark", tokens.every((t) => t.platform === "ark"));

console.log("\n额度:", quotas.map((q) => `${q.accountName}/${q.metric}=${q.used}%`).join(", "));
console.log("Token:", tokens.map((t) => `${t.date} in=${t.inputTokens} out=${t.outputTokens}`).join(", "));
console.log("\n结果:", ok ? "全部通过 ✅" : "存在失败 ❌");
if (!ok) process.exit(1);
