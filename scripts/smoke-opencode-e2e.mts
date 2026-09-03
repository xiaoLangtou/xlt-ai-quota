// 端到端：通过 dev 中间件 /api/opencode/stats 拉真实 OpenCode 用量并映射
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

import { OpenCodeConnector } from "@/connectors/opencode";

// Node 端无 origin，给相对 URL 补上 dev server 地址
const ORIGIN = "http://localhost:1420";
const origFetch = globalThis.fetch;
(globalThis as unknown as { fetch: typeof fetch }).fetch = ((
  input: string | URL | Request,
  init?: RequestInit,
) => {
  const u = typeof input === "string" && input.startsWith("/") ? ORIGIN + input : input;
  return origFetch(u as RequestInfo, init);
}) as typeof fetch;

const c = new OpenCodeConnector();
const range = { start: "2026-08-20", end: "2026-08-27" };
const rows = await c.fetchTokens(range);

console.log("isConfigured:", c.isConfigured());
console.log("记录数:", rows.length);
console.log("前 5 天:");
for (const r of rows.slice(-5)) {
  console.log(
    `  ${r.date}  in=${(r.inputTokens / 1e6).toFixed(2)}M  out=${(r.outputTokens / 1e6).toFixed(2)}M  cache=${(r.cachedTokens! / 1e6).toFixed(2)}M`,
  );
}
const totalIn = rows.reduce((a, r) => a + r.inputTokens, 0);
const totalOut = rows.reduce((a, r) => a + r.outputTokens, 0);
console.log(`区间合计: in=${(totalIn / 1e6).toFixed(2)}M  out=${(totalOut / 1e6).toFixed(2)}M`);
console.log("platform:", rows[0]?.platform);
