import {
  fetchOilPayloads,
  parseOfficialAdjustment,
  parseOilForecast,
  parseOilPrices,
} from "@/connectors/oil-price";

let ok = true;
function assert(name: string, condition: boolean): void {
  console.log(`${condition ? "PASS" : "FAIL"}  ${name}`);
  if (!condition) ok = false;
}

const prices = parseOilPrices({
  code: 0,
  msg: "成功",
  data: {
    province: "广东",
    price_date: "2026-09-13",
    source_url: "https://www.chajiage.com/youjia/guangdong.html",
    prices: {
      "92号汽油": "7.68 元/升",
      "95号汽油": "8.32 元/升",
      "98号汽油": "9.43 元/升",
      "0号柴油": "7.30 元/升",
    },
  },
});
assert("解析四种省级油价", prices.prices.length === 4 && prices.prices[0].price === 7.68);
assert("保留价格日期与来源", prices.priceDate === "2026-09-13" && prices.sourceUrl.includes("chajiage.com"));

const forecast = parseOilForecast({
  code: 0,
  msg: "成功",
  data: {
    crude_oil: { brent: 104.246, brent_change: -3.384 },
    next_adjust_date: "2026-09-24",
    days_remaining: 11,
    prediction: {
      direction: "下跌",
      estimated_change_per_ton: -1723,
      estimated_change_per_liter: 1.276,
      confidence: "低",
      analysis: "预计本轮油价将下调。",
    },
  },
});
assert("预测方向映射为 down", forecast.direction === "down");
assert("解析下一调价日", forecast.nextAdjustmentDate === "2026-09-24");
assert("下调的每升预测保留负号", forecast.estimatedChangePerLiter === -1.276);

const official = parseOfficialAdjustment({
  title: "2026年9月11日国家对成品油价格实施调控",
  publishedAt: "2026-09-11",
  effectiveAt: "2026-09-12T00:00:00+08:00",
  direction: "up",
  gasolineChangePerTon: 260,
  dieselChangePerTon: 250,
  sourceUrl: "https://www.ndrc.gov.cn/example.html",
});
assert("解析国家发改委正式公告", official.direction === "up" && official.gasolineChangePerTon === 260);

const requestedPaths: string[] = [];
await fetchOilPayloads(
  { province: "福建", apiKey: undefined },
  async ({ path }) => {
    requestedPaths.push(path);
    return {};
  },
);
assert(
  "价格、预测与正式公告独立请求",
  ["/price", "/forecast", "/adjustment"].every((path) => requestedPaths.includes(path)),
);

const memory = new Map<string, string>();
Object.defineProperty(globalThis, "localStorage", {
  value: {
    getItem: (key: string) => memory.get(key) ?? null,
    setItem: (key: string, value: string) => memory.set(key, value),
    removeItem: (key: string) => memory.delete(key),
  },
});
const { settings } = await import("@/config/settings");
settings.saveOilConfig({ province: "浙江", apiKey: "test-key" });
settings.savePreferences({ timezone: "Asia/Shanghai", statsSince: "2026-09-01" });
const savedOil = settings.getOilConfig();
assert("保存其他设置后仍保留油价省份", savedOil.province === "浙江");
assert("重新读取配置仍保留油价 API Key", savedOil.apiKey === "test-key");

console.log("结果:", ok ? "全部通过 ✅" : "存在失败 ❌");
if (!ok) process.exit(1);
