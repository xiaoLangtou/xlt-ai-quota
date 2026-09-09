// 验证中文友好的 token 估算：真实 tokenizer + 启发式降级。
import { estimateTokens, estimateTokensHeuristic } from "@/utils/token-estimate";

let ok = true;
function assert(name: string, condition: boolean, detail = ""): void {
  console.log(`${condition ? "PASS" : "FAIL"}  ${name}${detail ? ` — ${detail}` : ""}`);
  if (!condition) ok = false;
}

// 空输入
assert("空字符串 = 0", estimateTokens("") === 0);
assert("启发式空字符串 = 0", estimateTokensHeuristic("") === 0);

// 纯中文：国网系统代码注释 / 中文 PRD 常见文本
const zh = "本次改造需要统计本地各 AI 工具的用量，并按工具与时间维度展示。";
const zhTik = estimateTokens(zh, "gpt-4o");
const zhHeur = estimateTokensHeuristic(zh);
assert("纯中文 tokenizer > 0", zhTik > 0, `tik=${zhTik}`);
assert("纯中文 启发式 > 0", zhHeur > 0, `heur=${zhHeur}`);
// 旧「字符数 ÷ 4」会严重低估：中文 token 数应远大于 length/4。
assert(
  "纯中文 优于 length/4（不低估）",
  zhTik > Math.ceil(zh.length / 4),
  `tik=${zhTik} vs len/4=${Math.ceil(zh.length / 4)}`,
);

// 纯英文
const en = "Please implement a Chinese-friendly token estimator with a heuristic fallback.";
const enTik = estimateTokens(en, "gpt-4");
const enHeur = estimateTokensHeuristic(en);
assert("纯英文 tokenizer > 0", enTik > 0, `tik=${enTik}`);
assert("纯英文 启发式 > 0", enHeur > 0, `heur=${enHeur}`);

// 中英混合
const mix = "把 estimateTokens 函数接入 middleware，覆盖 Kiro CLI 与 Claude Code 的本地会话。";
const mixTik = estimateTokens(mix, "claude-3-5-sonnet");
const mixHeur = estimateTokensHeuristic(mix);
assert("混合 tokenizer > 0", mixTik > 0, `tik=${mixTik}`);
assert("混合 启发式 > 0", mixHeur > 0, `heur=${mixHeur}`);

// 启发式与真实 tokenizer 的误差应在合理范围（0.5x ~ 2x）。
function within(estimate: number, truth: number, lo = 0.5, hi = 2): boolean {
  return estimate >= truth * lo && estimate <= truth * hi;
}
assert("纯中文 启发式误差合理", within(zhHeur, zhTik), `heur=${zhHeur} tik=${zhTik}`);
assert("纯英文 启发式误差合理", within(enHeur, enTik), `heur=${enHeur} tik=${enTik}`);
assert("混合 启发式误差合理", within(mixHeur, mixTik), `heur=${mixHeur} tik=${mixTik}`);

// 中文占比更高时，每字符 token 数应高于纯英文（体现分段估算而非统一 /4）。
const zhPerChar = zhHeur / [...zh].length;
const enPerChar = enHeur / [...en].length;
assert(
  "中文每字符 token 高于英文",
  zhPerChar > enPerChar,
  `zh/char=${zhPerChar.toFixed(3)} en/char=${enPerChar.toFixed(3)}`,
);

console.log(
  `\n样本：中文 tik=${zhTik}/heur=${zhHeur}，英文 tik=${enTik}/heur=${enHeur}，混合 tik=${mixTik}/heur=${mixHeur}`,
);
console.log("结果:", ok ? "全部通过 ✅" : "存在失败 ❌");
if (!ok) process.exit(1);
