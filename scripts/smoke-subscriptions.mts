// 验证 AI 订阅管理的本地增删改与状态切换。
const values = new Map<string, string>();
Object.defineProperty(globalThis, "localStorage", {
  value: {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => values.set(key, value),
    removeItem: (key: string) => values.delete(key),
  },
});

const { subscriptionService } = await import("@/services/subscription-service");

let ok = true;
function assert(name: string, condition: boolean): void {
  console.log(`${condition ? "PASS" : "FAIL"}  ${name}`);
  if (!condition) ok = false;
}

const created = subscriptionService.save({
  provider: "Qoder",
  planName: "Pro",
  account: "me@example.com",
  price: 20,
  currency: "USD",
  billingCycle: "monthly",
  nextBillingDate: "2026-10-06",
  status: "active",
  note: "",
});

assert("新增订阅", subscriptionService.list().length === 1);
assert("保留账号", subscriptionService.list()[0]?.account === "me@example.com");

subscriptionService.saveUsdToCnyRate(7.25);
assert("保存美元汇率", subscriptionService.getPreferences().usdToCnyRate === 7.25);

subscriptionService.save({
  ...created,
  planName: "Pro 年付",
  billingCycle: "yearly",
}, created.id);
assert("编辑订阅", subscriptionService.list()[0]?.planName === "Pro 年付");

subscriptionService.setStatus(created.id, "paused");
assert("暂停订阅", subscriptionService.list()[0]?.status === "paused");

subscriptionService.save({
  ...subscriptionService.list()[0],
  note: "已暂停后编辑",
}, created.id);
assert("编辑不改变暂停状态", subscriptionService.list()[0]?.status === "paused");

subscriptionService.remove(created.id);
assert("删除订阅", subscriptionService.list().length === 0);

console.log("结果:", ok ? "全部通过 ✅" : "存在失败 ❌");
if (!ok) process.exit(1);
