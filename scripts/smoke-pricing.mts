// 验证模型定价与费用估算：家族匹配、缓存计费、未知模型兜底。
import { computeCostUsd, lookupRates, isModelPriced, DEFAULT_RATES } from "@/pricing";

let ok = true;
function assert(name: string, condition: boolean, detail = ""): void {
  console.log(`${condition ? "PASS" : "FAIL"}  ${name}${detail ? ` — ${detail}` : ""}`);
  if (!condition) ok = false;
}

// 家族匹配：带日期戳 / provider 前缀 / 点号变体都应命中同一家族。
assert("claude-opus-4-8 命中 opus 家族", lookupRates("claude-opus-4-8").rates.output === 25);
assert("anthropic/claude-sonnet-4 命中 sonnet", lookupRates("anthropic/claude-sonnet-4").rates.output === 15);
assert("gpt-5.6-terra 命中 gpt-5", lookupRates("gpt-5.6-terra").rates.input === 1.25);
assert("gemini-2.5-flash 命中", lookupRates("gemini-2.5-flash").rates.output === 2.5);
assert("最长匹配优先 claude-3-5-haiku", lookupRates("claude-3-5-haiku").rates.output === 4);

// 未知模型走 default，且 isModelPriced=false。
assert("quest-ultimate 未命中→default", !isModelPriced("quest-ultimate"));
assert("big-pickle 未命中→default", lookupRates("big-pickle").rates.input === DEFAULT_RATES.input);
assert("空模型→default", lookupRates(undefined).matched === false);

// 费用计算：1M 输入 + 1M 输出（无缓存），claude-opus = 5 + 25 = 30 USD。
const opus = computeCostUsd({ model: "claude-opus-4-8", inputTokens: 1_000_000, outputTokens: 1_000_000 });
assert("opus 1M+1M ≈ 30 USD", Math.abs(opus - 30) < 1e-6, `cost=${opus}`);

// 缓存计费：input 含缓存时，(input-cached) 按输入价、cached 按缓存价。
// gpt-5：input=1.25, cacheRead=0.125。1M 输入含 0.5M 缓存 + 0 输出
// = 0.5M*1.25 + 0.5M*0.125 = 0.625 + 0.0625 = 0.6875 USD
const cachedCost = computeCostUsd({
  model: "gpt-5.6-terra",
  inputTokens: 1_000_000,
  outputTokens: 0,
  cachedTokens: 500_000,
});
assert("含缓存计费正确", Math.abs(cachedCost - 0.6875) < 1e-6, `cost=${cachedCost}`);

// input 已扣缓存（input < cached）时不重复扣减，仍为正。
const netInput = computeCostUsd({
  model: "gpt-5",
  inputTokens: 100,
  outputTokens: 0,
  cachedTokens: 500,
});
assert("input<cached 仍为正", netInput > 0, `cost=${netInput}`);

// 零用量 = 0。
assert("零用量 = 0", computeCostUsd({ model: "gpt-5", inputTokens: 0, outputTokens: 0 }) === 0);

console.log("结果:", ok ? "全部通过 ✅" : "存在失败 ❌");
if (!ok) process.exit(1);
