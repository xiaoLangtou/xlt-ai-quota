// 端到端：运行真实 UsageService.syncAll()（经 dev 中间件），验证看板拿到真实数据
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

const ORIGIN = "http://localhost:1420";
const origFetch = globalThis.fetch;
(globalThis as unknown as { fetch: typeof fetch }).fetch = ((
  input: string | URL | Request,
  init?: RequestInit,
) => {
  const u = typeof input === "string" && input.startsWith("/") ? ORIGIN + input : input;
  return origFetch(u as RequestInfo, init);
}) as typeof fetch;

import { usageService } from "@/services/usage-service";

usageService.ensureSeeded(); // 版本变更会清空旧种子
console.log("同步前 quotas:", usageService.getPlatformQuotaViews().map((v) => `${v.name}:${v.windows.length}窗口`).join(", "));

await usageService.syncAll();

const quotas = usageService.getPlatformQuotaViews();
const summary = usageService.getTokenSummary("7d");
const daily = usageService.getPlatformDaily("7d");

console.log("\n=== 同步后 额度卡片 ===");
for (const v of quotas) {
  if (v.credits) {
    console.log(`${v.name} [${v.planTag}]: remaining ${v.credits.remaining} / ${v.credits.total}, ${v.windows[0]?.usedPct}%`);
  } else if (v.account) {
    console.log(`${v.name} [${v.planTag}]: 已登录 · ${v.account.label}（额度窗口需浏览器）`);
  } else {
    console.log(`${v.name} [${v.planTag}]: ${v.windows.map((w) => `${w.label}=${w.usedPct}%`).join(", ") || "(无数据)"}`);
  }
}

console.log("\n=== Token 7 天 ===");
console.log("总:", (summary.total / 1e6).toFixed(2) + "M", "输入:", (summary.input / 1e6).toFixed(2) + "M", "输出:", (summary.output / 1e6).toFixed(2) + "M");
console.log("每日平台汇总天数:", daily.length);
if (daily.length) {
  const platforms = new Set<string>();
  daily.forEach((d) => Object.keys(d.byPlatform).forEach((p) => platforms.add(p)));
  console.log("涉及平台:", [...platforms].join(", "));
}

const sync = usageService.getSyncInfo();
console.log("\n同步时间:", sync.lastSyncAt, "错误:", sync.error || "无");
