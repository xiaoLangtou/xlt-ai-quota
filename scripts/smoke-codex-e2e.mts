// 端到端：经 dev 中间件 /api/codex/status 拉真实 Codex 账户信息并映射
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

import { CodexConnector } from "@/connectors/codex";

const ORIGIN = "http://localhost:1420";
const origFetch = globalThis.fetch;
(globalThis as unknown as { fetch: typeof fetch }).fetch = ((
  input: string | URL | Request,
  init?: RequestInit,
) => {
  const u = typeof input === "string" && input.startsWith("/") ? ORIGIN + input : input;
  return origFetch(u as RequestInfo, init);
}) as typeof fetch;

const c = new CodexConnector();
const quotas = await c.fetchQuotas();

let ok = true;
function assert(name: string, cond: boolean) {
  console.log(`${cond ? "PASS" : "FAIL"}  ${name}`);
  if (!cond) ok = false;
}

assert("isConfigured", c.isConfigured());
assert("platformId=codex", c.platformId() === "codex");
assert("返回 1 条 account 快照", quotas.length === 1);
const q = quotas[0];
assert("platform=codex", q?.platform === "codex");
assert("metric=account", q?.metric === "account");
assert("accountName 含 @", (q?.accountName ?? "").includes("@"));
console.log("accountName:", q?.accountName);
console.log("resetsAt(订阅续期):", q?.resetsAt);
console.log("\n结果:", ok ? "全部通过 ✅" : "存在失败 ❌");
if (!ok) process.exit(1);
