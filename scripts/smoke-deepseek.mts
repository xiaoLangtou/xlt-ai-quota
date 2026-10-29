// 验证 DeepSeekConnector 对余额与 DSH 会话 Token 的映射（注入 connector mock，不依赖 Tauri 运行时）

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

const balanceJson = {
  is_available: true,
  balance_infos: [
    { currency: "CNY", total_balance: "80.00", granted_balance: "10.00", topped_up_balance: "70.00" },
    { currency: "USD", total_balance: "12.50", granted_balance: "2.50", topped_up_balance: "10.00" },
  ],
};

const statsJson = [
  { d: "2026-09-20", model: "deepseek-v4-pro", inp: 12000, outp: 3400, cache: 6000, requests: 4 },
  { d: "2026-09-21", model: "deepseek-reasoner", inp: 800, outp: 900, cache: 0, requests: 1 },
];

(globalThis as unknown as { __XLT_CONNECTOR_MOCK__: (opts: { path: string }) => Promise<unknown> })
  .__XLT_CONNECTOR_MOCK__ = async (opts) => {
    if (opts.path === "/balance") return balanceJson;
    if (opts.path === "/stats") return statsJson;
    throw new Error("not found");
  };

import { settings } from "@/config/settings";
import { DeepSeekConnector } from "@/connectors/deepseek";

let ok = true;
function assert(name: string, cond: boolean) {
  console.log(`${cond ? "PASS" : "FAIL"}  ${name}`);
  if (!cond) ok = false;
}

const c = new DeepSeekConnector();
assert("isConfigured 恒为 true（DSH 免登录）", c.isConfigured() === true);

// 未配置 Key 时不返回余额卡片，也不抛错
assert("未配置 Key 时余额为空", (await c.fetchQuotas()).length === 0);

settings.saveDeepseekApiKey("sk-test");
const quotas = await c.fetchQuotas();
assert("配置 Key 后映射 1 条余额快照", quotas.length === 1);
const snap = quotas[0];
assert("platform=deepseek", snap?.platform === "deepseek");
assert("metric=balance", snap?.metric === "balance");
assert("unit=currency", snap?.unit === "currency");
assert("优选 USD 余额", snap?.balance?.currency === "USD" && snap?.balance?.total === 12.5);
assert("赠金 / 充值拆分", snap?.balance?.granted === 2.5 && snap?.balance?.toppedUp === 10);
assert("available 透传", snap?.balance?.available === true);

const tokens = await c.fetchTokens({ start: "2026-09-20", end: "2026-09-21" });
assert("映射出 2 条 token 记录", tokens.length === 2);
assert("日期透传", tokens[0]?.date === "2026-09-20");
assert("input 数字透传", tokens[0]?.inputTokens === 12000);
assert("cached 透传", tokens[0]?.cachedTokens === 6000);
assert("requestCount 透传", tokens[0]?.requestCount === 4);
assert("model 透传", tokens[1]?.model === "deepseek-reasoner");
assert("platform=deepseek", tokens.every((t) => t.platform === "deepseek"));

console.log("\n余额:", JSON.stringify(snap?.balance));
console.log("Token:", tokens.map((t) => `${t.date} in=${t.inputTokens} out=${t.outputTokens}`).join(", "));
console.log("\n结果:", ok ? "全部通过 ✅" : "存在失败 ❌");
if (!ok) process.exit(1);
