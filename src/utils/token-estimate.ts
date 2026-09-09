import { getEncoding, type Tiktoken } from "js-tiktoken";

/**
 * 中文友好的 token 估算。
 *
 * 背景：juejin-usage 等实现用「字符数 ÷ 4」估算，该比例源自英文 GPT tokenizer，
 * 在中文场景（代码注释、PRD 文档等）会显著低估。本模块：
 *  - 首选真实 tokenizer（js-tiktoken，GPT 精确 / Claude 近似）；
 *  - 无法编码时降级为「按字符类型分段」的启发式（CJK 与 ASCII 分别套比例）。
 *
 * 纯函数、无副作用：输入文本，输出整数 token 数，便于单测与替换实现。
 */

type EncodingName = "o200k_base" | "cl100k_base";

// 惰性构建并缓存编码器；构建一次即可复用（rank 表较大）。
const encoderCache = new Map<EncodingName, Tiktoken>();
let tiktokenDisabled = false;

/** 根据模型名选择编码：GPT-4o/o 系列/GPT-5 用 o200k，其余（含 Claude 近似）用 cl100k。 */
function pickEncoding(model?: string): EncodingName {
  const m = (model ?? "").toLowerCase();
  if (
    m.includes("o200k") ||
    m.includes("gpt-4o") ||
    m.includes("gpt-4.1") ||
    m.includes("gpt-5") ||
    /(^|[^a-z])o[134]([^a-z]|$)/.test(m) // o1 / o3 / o4 推理模型
  ) {
    return "o200k_base";
  }
  return "cl100k_base";
}

function getEncoder(model?: string): Tiktoken | null {
  if (tiktokenDisabled) return null;
  const name = pickEncoding(model);
  const cached = encoderCache.get(name);
  if (cached) return cached;
  try {
    const enc = getEncoding(name);
    encoderCache.set(name, enc);
    return enc;
  } catch {
    // 环境不支持（如未打包 rank 表）时，全局降级到启发式。
    tiktokenDisabled = true;
    return null;
  }
}

/** Unicode 码点是否属于 CJK 及相关（含中日韩文字、假名、全角、标点）。 */
function isCjkCodePoint(cp: number | undefined): boolean {
  if (cp == null) return false;
  return (
    (cp >= 0x3000 && cp <= 0x303f) || // CJK 符号与标点
    (cp >= 0x3040 && cp <= 0x30ff) || // 平假名 / 片假名
    (cp >= 0x3400 && cp <= 0x4dbf) || // CJK 扩展 A
    (cp >= 0x4e00 && cp <= 0x9fff) || // CJK 基本区
    (cp >= 0xac00 && cp <= 0xd7af) || // 谚文
    (cp >= 0xf900 && cp <= 0xfaff) || // CJK 兼容表意
    (cp >= 0xff00 && cp <= 0xffef) || // 全角 / 半角形式
    (cp >= 0x20000 && cp <= 0x2fa1f) // CJK 扩展 B~F
  );
}

/**
 * 降级估算：按字符类型分段后分别套比例。
 *  - CJK 字符：约 1.7 个字符 ≈ 1 token
 *  - 其余（英文/数字/符号/空白）：约 4 个字符 ≈ 1 token
 */
export function estimateTokensHeuristic(text: string): number {
  if (typeof text !== "string" || text.length === 0) return 0;
  let cjk = 0;
  let other = 0;
  for (const ch of text) {
    if (isCjkCodePoint(ch.codePointAt(0))) cjk += 1;
    else other += 1;
  }
  return Math.ceil(cjk / 1.7) + Math.ceil(other / 4);
}

/**
 * 估算文本的 token 数。优先使用真实 tokenizer，失败时降级到启发式。
 * @param text  待估算文本
 * @param model 可选模型名，用于选择编码（默认 cl100k_base）
 */
export function estimateTokens(text: string, model?: string): number {
  if (typeof text !== "string" || text.length === 0) return 0;
  const enc = getEncoder(model);
  if (enc) {
    try {
      return enc.encode(text).length;
    } catch {
      // 单条编码失败时退回启发式，不影响整体统计。
    }
  }
  return estimateTokensHeuristic(text);
}
