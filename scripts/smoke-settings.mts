// Node 端验证 OpenAI 配置保存 -> connector isConfigured 路径
// 用内存 localStorage polyfill 模拟浏览器环境，跑的是真实 webStorage + settings 代码

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

import { usageService } from "@/services/usage-service";

let ok = true;
function assert(name: string, cond: boolean) {
  console.log(`${cond ? "PASS" : "FAIL"}  ${name}`);
  if (!cond) ok = false;
}

const before = usageService.getConnectorStatus();
usageService.saveOpenAIConfig({ adminKey: "sk-admin-test", orgId: "org-1", baseUrl: "/proxy-openai" });
const after = usageService.getConnectorStatus();
const view = usageService.getOpenAIConfigView();

assert("保存前 openai-api 未配置", before.find((c) => c.id === "openai-api")?.configured === false);
assert("保存后 openai-api 已配置", after.find((c) => c.id === "openai-api")?.configured === true);
assert("回读 adminKey 一致", view.adminKey === "sk-admin-test");
assert("回读 orgId 一致", view.orgId === "org-1");

console.log("\n结果:", ok ? "全部通过 ✅" : "存在失败 ❌");
if (!ok) process.exit(1);
