// 模型定价与费用估算（纯前端、无副作用）。
//
// 费率单位：美元 / 每 1M token。数据参考 juejin-usage 内置定价表
// （docs/juejin-usage/packages/core/src/pricing/pricing.json）中常见家族的公开单价，
// 仅取家族级代表价，未覆盖到的模型走 fuzzy 家族匹配，再兜底 DEFAULT_RATES。
//
// 说明：本机会话里出现的模型名可能是匿名/未来代号（如 quest-ultimate、big-pickle），
// 无法精确定价，此时按家族模糊匹配或默认价估算，UI 会标注为「预估」。

export interface ModelRates {
  /** 输入 token 单价（USD / 1M） */
  input: number;
  /** 输出 token 单价（USD / 1M） */
  output: number;
  /** 缓存命中读取单价（USD / 1M） */
  cacheRead?: number;
}

/** 未知模型的兜底费率（对齐 juejin default）。 */
export const DEFAULT_RATES: ModelRates = { input: 1, output: 5, cacheRead: 0.1 };

/** 家族级内置费率（键为规范化后的家族片段，按最长匹配优先）。 */
const FAMILY_RATES: Record<string, ModelRates> = {
  // Anthropic Claude
  "claude-opus": { input: 5, output: 25, cacheRead: 0.5 },
  "claude-sonnet": { input: 3, output: 15, cacheRead: 0.3 },
  "claude-haiku": { input: 0.8, output: 4, cacheRead: 0.08 },
  "claude-3-5-sonnet": { input: 3, output: 15, cacheRead: 0.3 },
  "claude-3-5-haiku": { input: 0.8, output: 4, cacheRead: 0.08 },
  "claude-3-opus": { input: 15, output: 75, cacheRead: 1.5 },
  // OpenAI / Codex
  "gpt-5": { input: 1.25, output: 10, cacheRead: 0.125 },
  "gpt-4.1": { input: 2, output: 8, cacheRead: 0.5 },
  "gpt-4o": { input: 2.5, output: 10, cacheRead: 1.25 },
  "o3": { input: 2, output: 8, cacheRead: 0.5 },
  "o1": { input: 15, output: 60, cacheRead: 7.5 },
  // Google Gemini
  "gemini-2.5-pro": { input: 1.25, output: 10, cacheRead: 0.125 },
  "gemini-2.5-flash": { input: 0.3, output: 2.5, cacheRead: 0.03 },
  "gemini-2.0-flash": { input: 0.1, output: 0.4, cacheRead: 0.025 },
  "gemini-1.5-pro": { input: 3.5, output: 10.5 },
  "gemini": { input: 1.25, output: 10, cacheRead: 0.125 },
  // DeepSeek
  "deepseek-reasoner": { input: 0.14, output: 0.28, cacheRead: 0.0028 },
  "deepseek-chat": { input: 0.14, output: 0.28, cacheRead: 0.0028 },
  "deepseek": { input: 0.14, output: 0.28, cacheRead: 0.0028 },
  // 通义千问 / Kimi / 豆包 / 智谱
  "qwen": { input: 0.05, output: 0.4 },
  "kimi": { input: 0.6, output: 3, cacheRead: 0.1 },
  "doubao": { input: 0.11, output: 0.28, cacheRead: 0.008 },
  "glm-4": { input: 1.1, output: 4.5 },
  "glm": { input: 1.1, output: 4.5 },
};

/** 规范化模型名：去 provider 前缀、小写、点转连字符、去日期/后缀噪声。 */
function normalize(model: string): string {
  let m = model.trim().toLowerCase();
  if (m.includes("/")) m = m.slice(m.lastIndexOf("/") + 1);
  m = m
    .replace(/[._]/g, "-")
    .replace(/-\d{6,8}$/, "") // 去掉末尾日期戳
    .replace(/-(latest|preview|stable|thinking|high|low|medium|fast)$/g, "");
  return m;
}

/** 家族键规范化（点转连字符）后按长度降序，保证最长匹配优先。 */
const NORMALIZED_FAMILY: { key: string; rates: ModelRates }[] = Object.entries(FAMILY_RATES)
  .map(([key, rates]) => ({ key: key.replace(/[._]/g, "-"), rates }))
  .sort((a, b) => b.key.length - a.key.length);

export interface PricingHit {
  rates: ModelRates;
  /** 是否命中内置家族价（false 表示走了 default 兜底）。 */
  matched: boolean;
}

/** 查询某模型的费率：家族最长匹配，未命中返回 default。 */
export function lookupRates(model: string | undefined): PricingHit {
  if (!model) return { rates: DEFAULT_RATES, matched: false };
  const norm = normalize(model);
  for (const { key, rates } of NORMALIZED_FAMILY) {
    if (norm === key || norm.includes(key)) {
      return { rates, matched: true };
    }
  }
  return { rates: DEFAULT_RATES, matched: false };
}

export interface CostInput {
  model?: string;
  inputTokens: number;
  outputTokens: number;
  cachedTokens?: number;
}

/**
 * 估算单条用量的费用（USD）。
 *
 * 各采集源对「input 是否已含缓存」口径不一：Claude/Codex 的 input 含缓存，
 * Qoder/OpenCode 的 input 已扣除缓存。用启发式统一：input >= cached 时按
 * (input-cached) 计费输入、cached 单独按缓存价，否则 input 已是净输入直接计费。
 */
export function computeCostUsd(row: CostInput): number {
  const { rates } = lookupRates(row.model);
  const cached = Math.max(0, row.cachedTokens ?? 0);
  const input = Math.max(0, row.inputTokens);
  const output = Math.max(0, row.outputTokens);
  const billedInput = input >= cached ? input - cached : input;
  const usd =
    (billedInput * rates.input +
      cached * (rates.cacheRead ?? rates.input) +
      output * rates.output) /
    1_000_000;
  return Math.round(usd * 1e6) / 1e6;
}

/** 判断模型是否命中内置定价（用于 UI 区分「预估价」标注）。 */
export function isModelPriced(model: string | undefined): boolean {
  return lookupRates(model).matched;
}
