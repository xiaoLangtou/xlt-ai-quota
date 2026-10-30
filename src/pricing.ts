// 模型定价与费用估算（纯前端、无副作用）。
//
// 查找顺序与 juejin-usage 对齐：
//   exact → sourceAlias → alias → fuzzy → contained → 家族兜底 → default
// 费率四档：input / output / cache_read / cache_write（USD / 1M token）。
// 内置底表来自 docs/juejin-usage 的 pricing.json；运行时可通过 Tauri 拉远程覆盖层。

import pricingJson from "@/data/pricing.json";

export interface ModelRates {
  /** 输入 token 单价（USD / 1M） */
  input: number;
  /** 输出 token 单价（USD / 1M） */
  output: number;
  /** 缓存命中读取单价（USD / 1M） */
  cacheRead?: number;
  /** 缓存写入单价（USD / 1M） */
  cacheWrite?: number;
}

/** 未知模型的兜底费率（对齐 juejin default）。 */
export const DEFAULT_RATES: ModelRates = {
  input: 1,
  output: 5,
  cacheRead: 0.1,
  cacheWrite: 1.25,
};

interface RawRates {
  input?: number;
  output?: number;
  cache_read?: number;
  cache_write?: number;
}

interface PricingTable {
  exact: Record<string, RawRates>;
  alias?: Record<string, string>;
  fuzzy?: { match: string; ref: string }[];
  sourceAlias?: Record<string, Record<string, string>>;
  default?: RawRates;
}

const BUILTIN = pricingJson as unknown as PricingTable;
let activeTable: PricingTable = BUILTIN;

function toRates(raw: RawRates | undefined): ModelRates {
  return {
    input: raw?.input ?? DEFAULT_RATES.input,
    output: raw?.output ?? DEFAULT_RATES.output,
    cacheRead: raw?.cache_read,
    cacheWrite: raw?.cache_write,
  };
}

/** 拉取远程覆盖层后重新加载合并表；失败时静默保留当前表。 */
export async function refreshPricingOverlay(): Promise<void> {
  const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  if (!isTauri) return;
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("usage_refresh_pricing");
    await loadPricingOverlay();
  } catch {
    // 网络不可用时继续使用内置表。
  }
}

/** 从 Tauri 读取「内置 + 远程覆盖层」合并后的定价表；非桌面环境保持内置表。 */
export async function loadPricingOverlay(): Promise<void> {
  const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  if (!isTauri) return;
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    const table = await invoke<PricingTable>("usage_pricing_table");
    if (table?.exact && Object.keys(table.exact).length > 0) {
      activeTable = table;
      exactIndexCache = null;
    }
  } catch {
    // 拉取失败时保留内置表，不影响看板。
  }
}

/** 家族级内置费率（精确表未命中时的兜底）。 */
const FAMILY_RATES: Record<string, ModelRates> = {
  "claude-opus": { input: 5, output: 25, cacheRead: 0.5, cacheWrite: 6.25 },
  "claude-sonnet": { input: 3, output: 15, cacheRead: 0.3, cacheWrite: 3.75 },
  "claude-haiku": { input: 0.8, output: 4, cacheRead: 0.08, cacheWrite: 1 },
  "claude-3-5-sonnet": { input: 3, output: 15, cacheRead: 0.3, cacheWrite: 3.75 },
  "claude-3-5-haiku": { input: 0.8, output: 4, cacheRead: 0.08, cacheWrite: 1 },
  "claude-3-opus": { input: 15, output: 75, cacheRead: 1.5, cacheWrite: 18.75 },
  "gpt-5": { input: 1.25, output: 10, cacheRead: 0.125, cacheWrite: 1.25 },
  "gpt-4.1": { input: 2, output: 8, cacheRead: 0.5, cacheWrite: 2 },
  "gpt-4o": { input: 2.5, output: 10, cacheRead: 1.25, cacheWrite: 2.5 },
  o3: { input: 2, output: 8, cacheRead: 0.5, cacheWrite: 2 },
  o1: { input: 15, output: 60, cacheRead: 7.5, cacheWrite: 15 },
  "gemini-2.5-pro": { input: 1.25, output: 10, cacheRead: 0.125, cacheWrite: 1.25 },
  "gemini-2.5-flash": { input: 0.3, output: 2.5, cacheRead: 0.03, cacheWrite: 0.3 },
  "gemini-2.0-flash": { input: 0.1, output: 0.4, cacheRead: 0.025, cacheWrite: 0.1 },
  "gemini-1.5-pro": { input: 3.5, output: 10.5, cacheRead: 0.35, cacheWrite: 3.5 },
  gemini: { input: 1.25, output: 10, cacheRead: 0.125, cacheWrite: 1.25 },
  "deepseek-reasoner": { input: 0.14, output: 0.28, cacheRead: 0.0028, cacheWrite: 0.14 },
  "deepseek-chat": { input: 0.14, output: 0.28, cacheRead: 0.0028, cacheWrite: 0.14 },
  deepseek: { input: 0.14, output: 0.28, cacheRead: 0.0028, cacheWrite: 0.14 },
  qwen: { input: 0.05, output: 0.4, cacheRead: 0.005, cacheWrite: 0.05 },
  kimi: { input: 0.6, output: 3, cacheRead: 0.1, cacheWrite: 0.6 },
  doubao: { input: 0.11, output: 0.28, cacheRead: 0.008, cacheWrite: 0.11 },
  "glm-4": { input: 1.1, output: 4.5, cacheRead: 0.11, cacheWrite: 1.1 },
  glm: { input: 1.1, output: 4.5, cacheRead: 0.11, cacheWrite: 1.1 },
};

const SUFFIX_STRIP = [
  /-xhigh-fast$/,
  /-high-fast$/,
  /-medium-fast$/,
  /-low-fast$/,
  /-xhigh$/,
  /-high$/,
  /-medium$/,
  /-low$/,
  /-fast$/,
];

function stripReasoningSuffix(model: string): string {
  for (const pattern of SUFFIX_STRIP) {
    if (pattern.test(model)) return model.replace(pattern, "");
  }
  return model;
}

function normalizeClaudeModel(model: string): string {
  const base = model.includes("/") ? model.slice(model.lastIndexOf("/") + 1) : model;
  let normalized = base
    .trim()
    .toLowerCase()
    .replace(/\([^)]*\)/g, " ")
    .replace(/[^a-z0-9.]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .replace(/-{2,}/g, "-");
  if (/^claude-(sonnet|opus|haiku)-\d+\.\d+/.test(normalized)) {
    normalized = normalized.replace(/^(claude-(?:sonnet|opus|haiku)-\d+)\.(\d+)/, "$1-$2");
  }
  return normalized;
}

/** 规范化模型名：去 provider 前缀、小写、点转连字符、去日期/后缀噪声。 */
function normalize(model: string, source?: string): string {
  const base = stripReasoningSuffix(model.trim());
  if (source === "claude") return normalizeClaudeModel(base);
  let value = base.toLowerCase();
  if (value.includes("/")) value = value.slice(value.lastIndexOf("/") + 1);
  return value
    .replace(/[._]/g, "-")
    .replace(/-\d{6,8}$/, "")
    .replace(/-(latest|preview|stable|thinking|high|low|medium|fast)$/g, "");
}

/** 表键规范化：统一点/下划线为连字符，避免 gemini-2.5 与 gemini-2-5 互不匹配。 */
function normalizeKey(value: string): string {
  return value.trim().toLowerCase().replace(/[._]/g, "-");
}

function dotRestored(model: string): string | null {
  const restored = model.replace(/(\d+)-(\d+)/g, "$1.$2");
  return restored === model ? null : restored;
}

let exactIndexCache: Map<string, RawRates> | null = null;

function exactIndex(): Map<string, RawRates> {
  if (exactIndexCache) return exactIndexCache;
  const index = new Map<string, RawRates>();
  for (const [key, rates] of Object.entries(activeTable.exact)) {
    index.set(normalizeKey(key), rates);
    const last = key.slice(key.lastIndexOf("/") + 1);
    if (!index.has(normalizeKey(last))) index.set(normalizeKey(last), rates);
  }
  exactIndexCache = index;
  return index;
}

function exactLookup(key: string): RawRates | null {
  const index = exactIndex();
  const last = key.slice(key.lastIndexOf("/") + 1);
  return index.get(normalizeKey(key)) ?? index.get(normalizeKey(last)) ?? null;
}

function containedLookup(key: string): RawRates | null {
  const index = exactIndex();
  const needle = normalizeKey(key);
  const keys = [...index.keys()].sort((a, b) => b.length - a.length);
  const match = keys.find((name) => needle.includes(name));
  return match ? index.get(match) ?? null : null;
}

function aliasLookup(
  aliases: Record<string, string> | undefined,
  key: string,
): string | null {
  if (!aliases) return null;
  if (aliases[key]) return aliases[key];
  const needle = normalizeKey(key);
  for (const [name, ref] of Object.entries(aliases)) {
    if (normalizeKey(name) === needle) return ref;
  }
  return null;
}

function resolveRef(ref: string): ModelRates | null {
  const exact = exactLookup(ref);
  if (exact) return toRates(exact);
  const contained = containedLookup(ref);
  return contained ? toRates(contained) : null;
}

/** 家族键规范化（点转连字符）后按长度降序，保证最长匹配优先。 */
const NORMALIZED_FAMILY = Object.entries(FAMILY_RATES)
  .map(([key, rates]) => ({ key: key.replace(/[._]/g, "-"), rates }))
  .sort((a, b) => b.key.length - a.key.length);

export interface PricingHit {
  rates: ModelRates;
  /** 是否命中内置/覆盖表（false 表示走了 default 兜底）。 */
  matched: boolean;
  /** 命中的表名：exact / sourceAlias / alias / fuzzy / contained / family / default */
  via: string;
}

/** 查询某模型的费率。 */
export function lookupRates(model: string | undefined, source?: string): PricingHit {
  if (!model) return { rates: DEFAULT_RATES, matched: false, via: "default" };
  const normalized = normalize(model, source);
  const candidates = [normalized];
  const restored = dotRestored(normalized);
  if (restored) candidates.push(restored);

  for (const candidate of candidates) {
    const exact = exactLookup(candidate);
    if (exact) return { rates: toRates(exact), matched: true, via: "exact" };
  }

  if (source) {
    const aliases = activeTable.sourceAlias?.[source];
    for (const candidate of candidates) {
      const ref = aliasLookup(aliases, candidate);
      if (!ref) continue;
      const rates = resolveRef(ref);
      if (rates) return { rates, matched: true, via: "sourceAlias" };
    }
  }

  for (const candidate of candidates) {
    const ref = aliasLookup(activeTable.alias, candidate);
    if (!ref) continue;
    const rates = resolveRef(ref);
    if (rates) return { rates, matched: true, via: "alias" };
  }

  for (const candidate of candidates) {
    const needle = normalizeKey(candidate);
    for (const rule of activeTable.fuzzy ?? []) {
      if (!needle.includes(normalizeKey(rule.match))) continue;
      const rates = resolveRef(rule.ref);
      if (rates) return { rates, matched: true, via: "fuzzy" };
    }
  }

  for (const candidate of candidates) {
    const contained = containedLookup(candidate);
    if (contained) return { rates: toRates(contained), matched: true, via: "contained" };
  }

  for (const { key, rates } of NORMALIZED_FAMILY) {
    if (normalized === key || normalized.includes(key)) {
      return { rates, matched: true, via: "family" };
    }
  }

  return {
    rates: toRates(activeTable.default),
    matched: false,
    via: "default",
  };
}

export interface CostInput {
  model?: string;
  source?: string;
  inputTokens: number;
  outputTokens: number;
  /** 缓存命中读取 token（Claude cache_read 等） */
  cachedTokens?: number;
  /** 缓存写入 token（Claude cache_creation 等） */
  cacheWriteTokens?: number;
  /** 推理 token；Codex / Every Code 的 output 已包含推理，避免重复计费 */
  reasoningTokens?: number;
}

/**
 * 缓存读取兜底单价：优先取表内 cacheRead；缺失时按输入价 10% 折算，
 * 不能回退成输入全价——否则长上下文工具的巨量缓存读取会按输入价计费。
 */
function cacheReadRate(rates: ModelRates): number {
  return rates.cacheRead ?? rates.input * 0.1;
}

/** 缓存写入兜底单价：优先取表内 cacheWrite；缺失时按输入价 1.25 倍。 */
function cacheWriteRate(rates: ModelRates): number {
  return rates.cacheWrite ?? rates.input * 1.25;
}

/**
 * 估算单条用量的费用（USD）。
 *
 * 与 juejin 对齐：`inputTokens` 视为**净输入**（不含缓存），缓存读/写与推理分开计费。
 * Codex / Every Code 的 output 已包含推理 token，不再重复计推理费。
 */
export function computeCostUsd(row: CostInput): number {
  const { rates } = lookupRates(row.model, row.source);
  const input = Math.max(0, row.inputTokens);
  const output = Math.max(0, row.outputTokens);
  const cached = Math.max(0, row.cachedTokens ?? 0);
  const cacheWrite = Math.max(0, row.cacheWriteTokens ?? 0);
  const reasoning = Math.max(0, row.reasoningTokens ?? 0);
  const reasoningIncluded =
    row.source === "codex" || row.source === "every-code";
  const usd =
    (input * rates.input +
      output * rates.output +
      cached * cacheReadRate(rates) +
      cacheWrite * cacheWriteRate(rates) +
      (reasoningIncluded ? 0 : reasoning * rates.output)) /
    1_000_000;
  return Math.round(usd * 1e9) / 1e9;
}

/** 判断模型是否命中定价表（用于 UI 区分「预估价」标注）。 */
export function isModelPriced(model: string | undefined, source?: string): boolean {
  return lookupRates(model, source).matched;
}
