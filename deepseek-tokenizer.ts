import { readFileSync } from "node:fs";
import { gunzipSync } from "node:zlib";
import { fileURLToPath } from "node:url";

/**
 * DeepSeek 分词器（离线 Token 估算）。
 *
 * 数据来自官方 `deepseek_v4_tokenizer`（tokenizer.json，BPE，ByteLevel，128k 词表），
 * 以 gzip 形式内置于 assets/deepseek-tokenizer.json.gz。仅用于 DSH 客户端旧版会话
 * 没有 `usage` 时的离线估算；有 `usage` 时以模型返回的真实计数为准。
 *
 * 本模块依赖 node:fs/zlib，只允许被 Node 侧（Vite dev 中间件）导入，不进入前端包。
 */

interface BpeModel {
  vocab: Map<string, number>;
  ranks: Map<string, number>;
}

let cache: BpeModel | null = null;
let disabled = false;

function bytesToUnicode(): Map<number, string> {
  const bs: number[] = [];
  for (let i = 33; i <= 126; i += 1) bs.push(i);
  for (let i = 161; i <= 172; i += 1) bs.push(i);
  for (let i = 174; i <= 255; i += 1) bs.push(i);
  const cs = bs.slice();
  let n = 0;
  for (let b = 0; b < 256; b += 1) {
    if (!bs.includes(b)) {
      bs.push(b);
      cs.push(256 + n);
      n += 1;
    }
  }
  const map = new Map<number, string>();
  for (let i = 0; i < bs.length; i += 1) map.set(bs[i], String.fromCodePoint(cs[i]));
  return map;
}

const BYTE_TO_UNICODE = bytesToUnicode();

const RE_NUMBER = /\p{N}{1,3}/gu;
const RE_CJK = /[\u4e00-\u9fa5\u3040-\u309f\u30a0-\u30ff]+/gu;
const RE_WORD =
  /[!"#$%&'()*+,\-./:;<=>?@[\\\]^_`{|}~][A-Za-z]+|[^\r\n\p{L}\p{P}\p{S}]?[\p{L}\p{M}]+| ?[\p{P}\p{S}]+[\r\n]*|\s*[\r\n]+|\s+(?!\S)|\s+/gu;

function load(): BpeModel | null {
  if (cache) return cache;
  if (disabled) return null;
  try {
    const path = fileURLToPath(new URL("./assets/deepseek-tokenizer.json.gz", import.meta.url));
    const parsed = JSON.parse(gunzipSync(readFileSync(path)).toString("utf8")) as {
      model?: { vocab?: Record<string, number>; merges?: string[] };
    };
    const vocab = new Map<string, number>();
    for (const [token, id] of Object.entries(parsed.model?.vocab ?? {})) vocab.set(token, id);
    const ranks = new Map<string, number>();
    const merges = parsed.model?.merges ?? [];
    for (let i = 0; i < merges.length; i += 1) {
      const entry = merges[i];
      const sep = entry.indexOf(" ");
      if (sep > 0) ranks.set(entry, i);
    }
    if (vocab.size === 0) throw new Error("Empty vocab");
    cache = { vocab, ranks };
    return cache;
  } catch {
    disabled = true;
    return null;
  }
}

/** 按「Isolated」语义做一次 Split：命中片段与未命中片段都保留为独立元素。 */
function splitIsolated(text: string, re: RegExp): string[] {
  const out: string[] = [];
  let last = 0;
  re.lastIndex = 0;
  let match: RegExpExecArray | null;
  while ((match = re.exec(text)) !== null) {
    if (match.index > last) out.push(text.slice(last, match.index));
    out.push(match[0]);
    last = match.index + match[0].length;
    if (match[0].length === 0) re.lastIndex += 1;
  }
  if (last < text.length) out.push(text.slice(last));
  return out;
}

function preTokenize(text: string): string[] {
  let pieces = splitIsolated(text, RE_NUMBER);
  pieces = pieces.flatMap((piece) => splitIsolated(piece, RE_CJK));
  pieces = pieces.flatMap((piece) => splitIsolated(piece, RE_WORD));
  return pieces;
}

function byteLevel(text: string): string {
  let out = "";
  for (const byte of Buffer.from(text, "utf8")) out += BYTE_TO_UNICODE.get(byte);
  return out;
}

/** 对单个 ByteLevel 词做 BPE 合并；每轮合并当前最优（rank 最小）的一处相邻对。 */
function bpe(token: string, ranks: Map<string, number>): string[] {
  let word = [...token];
  while (word.length > 1) {
    let bestRank = Infinity;
    let bestIndex = -1;
    for (let i = 0; i < word.length - 1; i += 1) {
      const rank = ranks.get(`${word[i]} ${word[i + 1]}`);
      if (rank !== undefined && rank < bestRank) {
        bestRank = rank;
        bestIndex = i;
      }
    }
    if (bestIndex < 0) break;
    word = [
      ...word.slice(0, bestIndex),
      word[bestIndex] + word[bestIndex + 1],
      ...word.slice(bestIndex + 2),
    ];
  }
  return word;
}

/**
 * 用 DeepSeek BPE 计算文本 token 数；词表不可用或出现未登录词时返回 null，
 * 由调用方回退到启发式估算。
 */
export function deepseekEncode(text: string): number[] | null {
  const model = load();
  if (!model) return null;
  const ids: number[] = [];
  try {
    for (const piece of preTokenize(text)) {
      for (const word of bpe(byteLevel(piece), model.ranks)) {
        const id = model.vocab.get(word);
        if (id === undefined) return null;
        ids.push(id);
      }
    }
  } catch {
    return null;
  }
  return ids;
}
