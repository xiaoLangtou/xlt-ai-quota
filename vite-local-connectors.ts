import { execFile } from "node:child_process";
import { promisify } from "node:util";
import {
  copyFileSync,
  existsSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join } from "node:path";
import vm from "node:vm";
import { qodercliAuth, query } from "@qoder-ai/qoder-agent-sdk";
import { estimateTokens } from "./src/utils/token-estimate";
import type { Plugin } from "vite";
import type { ServerResponse } from "node:http";

const execFileAsync = promisify(execFile);
const ARKCLI = process.env.ARKCLI_BIN || "arkcli";
const OPENCODE = process.env.OPENCODE_BIN || "opencode";
const KIROCLI = process.env.KIROCLI_BIN || "kiro-cli";
const CODEX = process.env.CODEX_BIN || "codex";
const TIMEOUT_MS = 90_000;
const ARK_PLAN_TIMEOUT_MS = 20_000;
const ARK_STATS_TIMEOUT_MS = 15_000;
const MAX_BUFFER = 16 * 1024 * 1024;

interface RunResult {
  stdout: string;
  stderr: string;
}

async function run(
  bin: string,
  args: string[],
  timeout = TIMEOUT_MS,
): Promise<RunResult> {
  return execFileAsync(bin, args, {
    timeout,
    maxBuffer: MAX_BUFFER,
    env: process.env,
  });
}

/**
 * 当前请求的时区（IANA 名）。由各 /stats 端点从 ?tz= 读取后设置，
 * 供 localDate / localDateHour / todayStr 分桶。应用全局只用一个时区，
 * 故并发请求携带的 tz 一致，模块级变量无竞态风险。空=跟随系统。
 */
let activeTz: string | undefined;

/** 用配置时区拆出年/月/日/时；无 activeTz 或解析失败返回 null（回退系统本地）。 */
function tzDateParts(ms: number): { y: string; m: string; d: string; h: string } | null {
  if (!activeTz) return null;
  try {
    const parts = new Intl.DateTimeFormat("en-US", {
      timeZone: activeTz,
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      hour12: false,
    }).formatToParts(new Date(ms));
    const g = (t: string) => parts.find((p) => p.type === t)?.value ?? "";
    let h = g("hour");
    if (h === "24") h = "00"; // Intl 可能返回 24 表示午夜
    return { y: g("year"), m: g("month"), d: g("day"), h };
  } catch {
    return null;
  }
}

/** 配置时区相对 UTC 的分钟偏移（供 sqlite 分桶）；无法解析返回 null。 */
function tzOffsetMinutes(): number | null {
  if (!activeTz) return null;
  try {
    const name = new Intl.DateTimeFormat("en-US", {
      timeZone: activeTz,
      timeZoneName: "longOffset",
    })
      .formatToParts(new Date())
      .find((p) => p.type === "timeZoneName")?.value ?? "";
    const m = /GMT([+-])(\d{2}):(\d{2})/.exec(name);
    if (!m) return name.includes("GMT") ? 0 : null; // 纯 "GMT" = UTC
    const sign = m[1] === "-" ? -1 : 1;
    return sign * (Number(m[2]) * 60 + Number(m[3]));
  } catch {
    return null;
  }
}

/** OpenCode 的 sqlite 分桶修饰符：有配置时区用分钟偏移平移，否则 'localtime'。 */
function tzSqliteModifier(): string {
  const off = tzOffsetMinutes();
  if (off == null) return "'localtime'";
  return `'${off >= 0 ? "+" : "-"}${Math.abs(off)} minutes'`;
}

function todayStr(): string {
  const parts = tzDateParts(Date.now());
  if (parts) return `${parts.y}-${parts.m}-${parts.d}`;
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

const DATE_RE = /^\d{4}-\d{2}-\d{2}$/;

interface LocalTokenStats {
  d: string;
  model: string;
  inp: number;
  outp: number;
  cache: number;
  requests: number;
}

function statKey(date: string, model: string): string {
  return `${date}\u0000${model}`;
}

type JsonRecord = Record<string, unknown>;

function isRecord(value: unknown): value is JsonRecord {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function numeric(value: unknown): number {
  const parsed = typeof value === "number" ? value : Number(value);
  return Number.isFinite(parsed) ? parsed : 0;
}

function modelName(value: unknown, fallback: string): string {
  return typeof value === "string" && value.trim() ? value.trim() : fallback;
}

function modelFromEvent(event: JsonRecord): string | undefined {
  const payload = isRecord(event.payload) ? event.payload : undefined;
  const candidates = [event.model, payload?.model, payload?.model_id, payload?.modelId];
  return candidates.find((value): value is string => typeof value === "string" && Boolean(value.trim()));
}

function localDate(value: unknown): string | undefined {
  if (typeof value !== "string" && typeof value !== "number") return undefined;
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return undefined;
  const parts = tzDateParts(date.getTime());
  if (parts) return `${parts.y}-${parts.m}-${parts.d}`;
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}

/** 本地「日期 小时」桶键，如 "2026-09-09 14"；用于「今天按小时」聚合。 */
function localDateHour(value: unknown): string | undefined {
  if (typeof value !== "string" && typeof value !== "number") return undefined;
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return undefined;
  const parts = tzDateParts(date.getTime());
  if (parts) return `${parts.y}-${parts.m}-${parts.d} ${parts.h}`;
  const p = (n: number) => String(n).padStart(2, "0");
  return `${date.getFullYear()}-${p(date.getMonth() + 1)}-${p(date.getDate())} ${p(date.getHours())}`;
}

function listJsonlFiles(root: string): string[] {
  if (!existsSync(root)) return [];
  const files: string[] = [];
  const visit = (dir: string) => {
    let entries: ReturnType<typeof readdirSync>;
    try {
      entries = readdirSync(dir, { withFileTypes: true });
    } catch {
      return;
    }
    for (const entry of entries) {
      const path = join(dir, entry.name);
      if (entry.isDirectory()) visit(path);
      else if (entry.isFile() && entry.name.endsWith(".jsonl")) files.push(path);
    }
  };
  visit(root);
  return files;
}

function readJsonLines(file: string, onRecord: (record: JsonRecord) => void): void {
  let text: string;
  try {
    text = readFileSync(file, "utf8");
  } catch {
    return;
  }
  for (const line of text.split("\n")) {
    if (!line.trim()) continue;
    try {
      const parsed: unknown = JSON.parse(line);
      if (isRecord(parsed)) onRecord(parsed);
    } catch {
      // 单条损坏记录不应阻断其他本地会话的聚合。
    }
  }
}

function addStats(
  stats: Map<string, LocalTokenStats>,
  date: string,
  model: string,
  input: number,
  output: number,
  cached: number,
): void {
  if (input === 0 && output === 0 && cached === 0) return;
  const key = statKey(date, model);
  const current = stats.get(key) ?? {
    d: date,
    model,
    inp: 0,
    outp: 0,
    cache: 0,
    requests: 0,
  };
  current.inp += input;
  current.outp += output;
  current.cache += cached;
  current.requests += 1;
  stats.set(key, current);
}

function statsRows(stats: Map<string, LocalTokenStats>): LocalTokenStats[] {
  return [...stats.values()].sort((a, b) => a.d.localeCompare(b.d) || a.model.localeCompare(b.model));
}

function datesInRange(start: string, end: string): string[] {
  const dates: string[] = [];
  const cursor = new Date(`${start}T00:00:00`);
  const last = new Date(`${end}T00:00:00`);
  while (cursor <= last) {
    dates.push(`${cursor.getFullYear()}-${String(cursor.getMonth() + 1).padStart(2, "0")}-${String(cursor.getDate()).padStart(2, "0")}`);
    cursor.setDate(cursor.getDate() + 1);
  }
  return dates;
}

function collectCodexTokenStats(start: string, end: string, byHour = false): LocalTokenStats[] {
  const sessions = join(homedir(), ".codex", "sessions");
  const stats = new Map<string, LocalTokenStats>();
  for (const date of datesInRange(start, end)) {
    const [year, month, day] = date.split("-");
    for (const file of listJsonlFiles(join(sessions, year, month, day))) {
      // Codex 把模型写在 session_meta / turn_context，token_count 继承最近一次上下文模型。
      let currentModel = "codex-unknown";
      readJsonLines(file, (event) => {
        const eventModel = modelFromEvent(event);
        if (eventModel) currentModel = eventModel;
        const payload = isRecord(event.payload) ? event.payload : undefined;
        const info = payload && isRecord(payload.info) ? payload.info : undefined;
        const usage = info && isRecord(info.last_token_usage) ? info.last_token_usage : undefined;
        const eventDate = localDate(event.timestamp);
        if (event.type !== "event_msg" || payload?.type !== "token_count" || !usage || eventDate !== date) return;
        const key = byHour ? (localDateHour(event.timestamp) ?? date) : date;
        addStats(
          stats,
          key,
          currentModel,
          numeric(usage.input_tokens),
          numeric(usage.output_tokens),
          numeric(usage.cached_input_tokens) + numeric(usage.cache_write_input_tokens),
        );
      });
    }
  }
  return statsRows(stats);
}

/** 文件最后修改日期（本地）；用于跳过明显早于窗口的文件，降低高频刷新开销。 */
function fileMtimeDate(file: string): string | undefined {
  try {
    return localDate(statSync(file).mtimeMs);
  } catch {
    return undefined;
  }
}

function collectClaudeTokenStats(start: string, end: string, byHour = false): LocalTokenStats[] {
  const stats = new Map<string, LocalTokenStats>();
  for (const file of listJsonlFiles(join(homedir(), ".claude", "projects"))) {
    // 文件在窗口起点之前就没再改动过，其内容不可能落在 [start,end]，直接跳过。
    const mtimeDate = fileMtimeDate(file);
    if (mtimeDate && mtimeDate < start) continue;
    readJsonLines(file, (event) => {
      const message = isRecord(event.message) ? event.message : undefined;
      const usage = message && isRecord(message.usage) ? message.usage : undefined;
      const date = localDate(event.timestamp);
      if (event.type !== "assistant" || message?.role !== "assistant" || !usage || !date || date < start || date > end) return;
      const cached = numeric(usage.cache_creation_input_tokens) + numeric(usage.cache_read_input_tokens);
      const key = byHour ? (localDateHour(event.timestamp) ?? date) : date;
      // Claude 将缓存 Token 独立于 input_tokens 返回；合并后才是完整输入量。
      addStats(
        stats,
        key,
        modelName(message?.model, "claude-code"),
        numeric(usage.input_tokens) + cached,
        numeric(usage.output_tokens),
        cached,
      );
    });
  }
  return statsRows(stats);
}

// ---- Gemini CLI 本地会话 Token（~/.gemini/tmp/<hash>/chats/*.json[l]，真实计数）----
// Gemini 在会话文件里写「累计」token 快照（usageMetadata / tokens），故按相邻
// assistant 快照做增量差分，再按本地日期聚合。参考 juejin-usage gemini parser。

interface GeminiTotals {
  input: number;
  cached: number;
  output: number;
  total: number;
}

function normalizeGeminiTokens(msg: JsonRecord): GeminiTotals | null {
  const tokens = isRecord(msg.tokens) ? msg.tokens : undefined;
  if (tokens) {
    const input = Math.max(0, numeric(tokens.input));
    const cached = Math.max(0, numeric(tokens.cached));
    const output = Math.max(0, numeric(tokens.output)) + Math.max(0, numeric(tokens.tool));
    const thoughts = Math.max(0, numeric(tokens.thoughts));
    const total = Math.max(numeric(tokens.total), input + cached + output + thoughts);
    if (total === 0) return null;
    return { input, cached, output: output + thoughts, total };
  }
  const u = isRecord(msg.usageMetadata) ? msg.usageMetadata : isRecord(msg.usage) ? msg.usage : undefined;
  if (!u) return null;
  const cached = Math.max(0, numeric(u.cachedContentTokenCount));
  const thoughts = Math.max(0, numeric(u.thoughtsTokenCount));
  const prompt = Math.max(0, numeric(u.promptTokenCount ?? u.input_tokens));
  const candidates = Math.max(0, numeric(u.candidatesTokenCount ?? u.output_tokens));
  const input = Math.max(0, prompt - cached);
  const output = Math.max(0, candidates - thoughts) + thoughts;
  const total = input + cached + output;
  if (total === 0) return null;
  return { input, cached, output, total };
}

/** 累计快照差分：cur - prev，任一为空或回退（新会话）时取 cur。 */
function diffGeminiTotals(cur: GeminiTotals, prev: GeminiTotals | null): GeminiTotals | null {
  if (!prev) return cur;
  if (cur.total < prev.total) return cur; // 计数回退视为新会话
  const delta = {
    input: Math.max(0, cur.input - prev.input),
    cached: Math.max(0, cur.cached - prev.cached),
    output: Math.max(0, cur.output - prev.output),
    total: Math.max(0, cur.total - prev.total),
  };
  return delta.total === 0 ? null : delta;
}

function collectGeminiTokenStats(start: string, end: string, byHour = false): LocalTokenStats[] {
  const root = join(homedir(), ".gemini", "tmp");
  const stats = new Map<string, LocalTokenStats>();
  if (!existsSync(root)) return [];
  let hashes: ReturnType<typeof readdirSync>;
  try {
    hashes = readdirSync(root, { withFileTypes: true });
  } catch {
    return [];
  }
  for (const h of hashes) {
    if (!h.isDirectory()) continue;
    const chatsDir = join(root, h.name, "chats");
    for (const filePath of listChatFiles(chatsDir)) {
      const mtimeDate = fileMtimeDate(filePath);
      if (mtimeDate && mtimeDate < start) continue;
      let text: string;
      try {
        text = readFileSync(filePath, "utf8");
      } catch {
        continue;
      }
      const messages = parseGeminiMessages(filePath, text);
      let prev: GeminiTotals | null = null;
      let lastModel = "gemini";
      for (const msg of messages) {
        const role = msg.type ?? msg.role;
        const isAssistant = role === "gemini" || role === "model" || role === "assistant";
        if (typeof msg.model === "string" && msg.model) lastModel = msg.model;
        if (!isAssistant) continue;
        const cur = normalizeGeminiTokens(msg);
        if (!cur) continue;
        const delta = diffGeminiTotals(cur, prev);
        prev = cur;
        if (!delta) continue;
        const ts = msg.timestamp ?? msg.createTime;
        const dateOnly = localDate(ts);
        if (!dateOnly || dateOnly < start || dateOnly > end) continue;
        const key = byHour ? (localDateHour(ts) ?? dateOnly) : dateOnly;
        // input 含缓存（与 Claude 口径一致，供成本启发式扣减）。
        addStats(stats, key, lastModel, delta.input + delta.cached, delta.output, delta.cached);
      }
    }
  }
  return statsRows(stats);
}

/** 列出 chats 目录下的 .json / .jsonl（最多两级）。 */
function listChatFiles(dir: string): string[] {
  if (!existsSync(dir)) return [];
  const out: string[] = [];
  const visit = (d: string, depth: number) => {
    if (depth > 2) return;
    let entries: ReturnType<typeof readdirSync>;
    try {
      entries = readdirSync(d, { withFileTypes: true });
    } catch {
      return;
    }
    for (const e of entries) {
      const p = join(d, e.name);
      if (e.isDirectory()) visit(p, depth + 1);
      else if (e.name.endsWith(".json") || e.name.endsWith(".jsonl")) out.push(p);
    }
  };
  visit(dir, 0);
  return out;
}

function parseGeminiMessages(filePath: string, raw: string): JsonRecord[] {
  if (filePath.endsWith(".jsonl")) {
    const out: JsonRecord[] = [];
    for (const line of raw.split("\n")) {
      if (!line.trim()) continue;
      try {
        const o: unknown = JSON.parse(line);
        if (isRecord(o) && (typeof o.type === "string" || typeof o.role === "string")) out.push(o);
      } catch {
        // 跳过损坏行
      }
    }
    return out;
  }
  try {
    const data: unknown = JSON.parse(raw);
    if (!isRecord(data)) return [];
    const msgs = data.messages ?? data.history;
    return Array.isArray(msgs) ? (msgs.filter(isRecord) as JsonRecord[]) : [];
  } catch {
    return [];
  }
}

// ---- GitHub Copilot CLI 本地会话 Token（~/.copilot/session-state/<id>/events.jsonl，真实计数）----
// session.shutdown 事件的 data.modelMetrics[model].usage 记录每模型真实 token。
// 参考 juejin-usage copilot parser。

function collectCopilotTokenStats(start: string, end: string, byHour = false): LocalTokenStats[] {
  const stats = new Map<string, LocalTokenStats>();
  const root = join(homedir(), ".copilot", "session-state");
  if (!existsSync(root)) return [];
  let sessions: ReturnType<typeof readdirSync>;
  try {
    sessions = readdirSync(root, { withFileTypes: true });
  } catch {
    return [];
  }
  for (const s of sessions) {
    if (!s.isDirectory()) continue;
    const filePath = join(root, s.name, "events.jsonl");
    if (!existsSync(filePath)) continue;
    const mtimeDate = fileMtimeDate(filePath);
    if (mtimeDate && mtimeDate < start) continue;
    readJsonLines(filePath, (obj) => {
      if (obj.type !== "session.shutdown") return;
      const ts = typeof obj.timestamp === "string" ? obj.timestamp : undefined;
      const dateOnly = localDate(ts);
      if (!dateOnly || dateOnly < start || dateOnly > end) return;
      const key = byHour ? (localDateHour(ts) ?? dateOnly) : dateOnly;
      const data = isRecord(obj.data) ? obj.data : undefined;
      const metrics = data && isRecord(data.modelMetrics) ? data.modelMetrics : undefined;
      if (!metrics) return;
      for (const [model, m] of Object.entries(metrics)) {
        const usage = isRecord(m) && isRecord(m.usage) ? m.usage : undefined;
        if (!usage) continue;
        const totalInput = Math.max(0, numeric(usage.inputTokens));
        const cacheRead = Math.max(0, numeric(usage.cacheReadTokens));
        const output = Math.max(0, numeric(usage.outputTokens));
        // inputTokens 含缓存，保持 input 含缓存口径。
        addStats(stats, key, model || "copilot", totalInput, output, cacheRead);
      }
    });
  }
  return statsRows(stats);
}

// ---- Kiro CLI 本地会话 Token 估算（~/.kiro/sessions/cli/*.jsonl）----
// Kiro CLI 不在会话里记录真实 token 数，参考 juejin-usage 的做法：
// 逐轮累计 Prompt/ToolResults 为输入、AssistantMessage 为输出，文本量用
// estimateTokens（中文友好）估算，再按本地日期聚合。

const KIRO_NON_TEXT_KEYS = new Set([
  "signature",
  "redactedContent",
  "toolUseId",
  "modelId",
  "message_id",
  "format",
  "id",
]);

/** 递归累计文本 token（跳过非文本字段）；图片单独计固定量。 */
function estTokensText(value: unknown, model: string): number {
  if (typeof value === "string") return estimateTokens(value, model);
  if (Array.isArray(value)) {
    let n = 0;
    for (const v of value) n += estTokensText(v, model);
    return n;
  }
  if (isRecord(value)) {
    let n = 0;
    for (const [k, v] of Object.entries(value)) {
      if (KIRO_NON_TEXT_KEYS.has(k)) continue;
      n += estTokensText(v, model);
    }
    return n;
  }
  return 0;
}

/** 规范化 Kiro 模型名，仅用于为 estimateTokens 选择编码。 */
function canonicalizeKiroModel(raw: unknown): string {
  if (typeof raw !== "string") return "kiro-cli-agent";
  let name = raw.trim().toLowerCase();
  if (!name || name === "auto") return "kiro-cli-agent";
  name = name.replace(
    /^(?:arn:aws:bedrock:[^:]*:[^:]*:(?:foundation-model\/)?|anthropic\.|openai\.|aws\.)/,
    "",
  );
  name = name
    .replace(/:\d+$/, "")
    .replace(/-\d{8}-v\d+$/i, "")
    .replace(/-v\d+$/i, "")
    .replace(/-\d{8}$/, "")
    .replace(/\.v\d+$/i, "");
  return name || "kiro-cli-agent";
}

function loadKiroSessionModel(dir: string, sessionId: string): string {
  try {
    const d = JSON.parse(readFileSync(join(dir, `${sessionId}.json`), "utf8")) as {
      session_state?: { rts_model_state?: { model_info?: { model_id?: string } } };
    };
    return canonicalizeKiroModel(d.session_state?.rts_model_state?.model_info?.model_id ?? null);
  } catch {
    return "kiro-cli-agent";
  }
}

function bumpStats(
  stats: Map<string, LocalTokenStats>,
  date: string,
  model: string,
  input: number,
  output: number,
  reqInc: number,
): void {
  if (input === 0 && output === 0 && reqInc === 0) return;
  const key = statKey(date, model);
  const cur = stats.get(key) ?? { d: date, model, inp: 0, outp: 0, cache: 0, requests: 0 };
  cur.inp += input;
  cur.outp += output;
  cur.requests += reqInc;
  stats.set(key, cur);
}

/** Kiro CLI：sessions/cli/*.jsonl（Prompt/AssistantMessage 事件，按文本估算）。 */
function collectKiroCliStats(
  dir: string,
  start: string,
  end: string,
  stats: Map<string, LocalTokenStats>,
  byHour = false,
): void {
  if (!existsSync(dir)) return;
  let names: string[];
  try {
    names = readdirSync(dir).filter((f) => f.endsWith(".jsonl"));
  } catch {
    return;
  }

  for (const name of names) {
    const filePath = join(dir, name);
    const sessionId = name.slice(0, -".jsonl".length);
    const fallbackDate = fileMtimeDate(filePath);
    if (fallbackDate && fallbackDate < start) continue;
    const model = loadKiroSessionModel(dir, sessionId);

    const events: JsonRecord[] = [];
    readJsonLines(filePath, (rec) => events.push(rec));

    let curTs: number | null = null;
    let pendingInput = 0;
    for (const ev of events) {
      const data = isRecord(ev.data) ? ev.data : undefined;
      if (!data) continue;
      const content = Array.isArray(data.content) ? data.content : [];

      if (ev.kind === "Prompt") {
        const meta = isRecord(data.meta) ? data.meta : undefined;
        const ts = meta && typeof meta.timestamp === "number" ? meta.timestamp : undefined;
        if (ts && ts > 0) curTs = ts * 1000;
        for (const item of content) {
          const row = isRecord(item) ? item : undefined;
          pendingInput += row?.kind === "image" ? 1600 : estTokensText(row?.data, model);
        }
      } else if (ev.kind === "ToolResults") {
        for (const item of content) {
          pendingInput += estTokensText(isRecord(item) ? item.data : undefined, model);
        }
      } else if (ev.kind === "AssistantMessage") {
        let output = 0;
        for (const item of content) {
          const row = isRecord(item) ? item : undefined;
          const cd = row?.data;
          if (row?.kind === "thinking" && isRecord(cd)) output += estTokensText(cd.text, model);
          else output += estTokensText(cd, model);
        }
        if (pendingInput > 0 || output > 0) {
          const dateOnly = (curTs != null ? localDate(curTs) : undefined) ?? fallbackDate;
          if (dateOnly && dateOnly >= start && dateOnly <= end) {
            const key = byHour
              ? ((curTs != null ? localDateHour(curTs) : undefined) ?? `${dateOnly} 00`)
              : dateOnly;
            addStats(stats, key, model, pendingInput, output, 0);
          }
        }
        pendingInput = 0;
      } else if (ev.kind === "Compaction") {
        pendingInput = 0;
      }
    }
  }
}

/** 列出 Kiro IDE 工作区会话文件：sessions 下各 workspace 的 sess_<id>/messages.jsonl */
function listKiroIdeMessageFiles(sessionsRoot: string): string[] {
  const out: string[] = [];
  let workspaces: ReturnType<typeof readdirSync>;
  try {
    workspaces = readdirSync(sessionsRoot, { withFileTypes: true });
  } catch {
    return out;
  }
  for (const ws of workspaces) {
    if (!ws.isDirectory() || ws.name === "cli") continue;
    const wsDir = join(sessionsRoot, ws.name);
    let sessions: ReturnType<typeof readdirSync>;
    try {
      sessions = readdirSync(wsDir, { withFileTypes: true });
    } catch {
      continue;
    }
    for (const s of sessions) {
      if (!s.isDirectory() || !s.name.startsWith("sess_")) continue;
      const mf = join(wsDir, s.name, "messages.jsonl");
      if (existsSync(mf)) out.push(mf);
    }
  }
  return out;
}

/**
 * Kiro IDE：sessions 下各 workspace 的 sess_<id>/messages.jsonl。
 * 每行 {timestamp(ISO), payload:{type, content/args}}；IDE 只记 credits 不记 token，
 * 故按文本估算：user/tool_result/tool_call → 输入，assistant → 输出。
 */
function collectKiroIdeStats(
  sessionsRoot: string,
  start: string,
  end: string,
  stats: Map<string, LocalTokenStats>,
  byHour = false,
): void {
  for (const filePath of listKiroIdeMessageFiles(sessionsRoot)) {
    const mtimeDate = fileMtimeDate(filePath);
    if (mtimeDate && mtimeDate < start) continue;
    readJsonLines(filePath, (rec) => {
      const ts = typeof rec.timestamp === "string" ? rec.timestamp : undefined;
      if (!ts) return;
      const dateOnly = localDate(ts);
      if (!dateOnly || dateOnly < start || dateOnly > end) return; // 先按日期裁剪，避免无谓估算
      const key = byHour ? (localDateHour(ts) ?? dateOnly) : dateOnly;
      const payload = isRecord(rec.payload) ? rec.payload : undefined;
      if (!payload) return;
      const model = "kiro-ide";
      switch (payload.type) {
        case "user":
        case "tool_result":
          bumpStats(stats, key, model, estTokensText(payload.content, model), 0, 0);
          break;
        case "tool_call":
          bumpStats(stats, key, model, estTokensText(payload.args, model), 0, 0);
          break;
        case "assistant":
          bumpStats(stats, key, model, 0, estTokensText(payload.content, model), 1);
          break;
        default:
          break;
      }
    });
  }
}

function collectKiroTokenStats(start: string, end: string, byHour = false): LocalTokenStats[] {
  const sessionsRoot = join(homedir(), ".kiro", "sessions");
  const stats = new Map<string, LocalTokenStats>();
  collectKiroCliStats(join(sessionsRoot, "cli"), start, end, stats, byHour); // Kiro CLI
  collectKiroIdeStats(sessionsRoot, start, end, stats, byHour); // Kiro IDE 工作区会话
  return statsRows(stats);
}

// ---- Kimi Code 本地会话 Token（~/.kimi-code/sessions/**/wire.jsonl，真实计数）----
// usage.record 事件按次记录 {inputOther, output, inputCacheRead, inputCacheCreation}（model 前缀
// 如 kimi-code/k3）。旧版 kimi-cli（~/.kimi/sessions/<wd>/<sess>/wire.jsonl）用 StatusUpdate 事件，
// token_usage 为 snake_case、时间戳为秒。迁移后两套目录可能并存：kimi-code 有数据时跳过 legacy，
// 避免迁移重复计入（与 juejin-usage 的 kimi parser 口径一致）。

/** 递归收集 dir 下所有 wire.jsonl。 */
function listWireFiles(dir: string): string[] {
  return listJsonlFiles(dir).filter((f) => f.endsWith("/wire.jsonl"));
}

/** "kimi-code/k3" → "k3"。 */
function kimiModelName(raw: unknown, fallback: string): string {
  if (typeof raw !== "string" || !raw.trim()) return fallback;
  const name = raw.trim();
  return name.includes("/") ? name.split("/").pop() || fallback : name;
}

function collectKimiCodeStats(
  sessionsRoot: string,
  start: string,
  end: string,
  stats: Map<string, LocalTokenStats>,
  byHour: boolean,
): void {
  for (const filePath of listWireFiles(sessionsRoot)) {
    const mtimeDate = fileMtimeDate(filePath);
    if (mtimeDate && mtimeDate < start) continue;
    readJsonLines(filePath, (rec) => {
      if (rec.type !== "usage.record") return;
      const usage = isRecord(rec.usage) ? rec.usage : undefined;
      if (!usage) return;
      const input = numeric(usage.inputOther);
      const output = numeric(usage.output);
      const cached = numeric(usage.inputCacheRead) + numeric(usage.inputCacheCreation);
      if (input === 0 && output === 0 && cached === 0) return;
      const dateOnly = localDate(rec.time);
      if (!dateOnly || dateOnly < start || dateOnly > end) return;
      const key = byHour ? (localDateHour(rec.time) ?? dateOnly) : dateOnly;
      addStats(stats, key, kimiModelName(rec.model, "kimi-for-coding"), input + cached, output, cached);
    });
  }
}

function collectKimiLegacyStats(
  sessionsRoot: string,
  start: string,
  end: string,
  stats: Map<string, LocalTokenStats>,
  byHour: boolean,
): void {
  for (const filePath of listWireFiles(sessionsRoot)) {
    const mtimeDate = fileMtimeDate(filePath);
    if (mtimeDate && mtimeDate < start) continue;
    let currentModel = "kimi-for-coding";
    readJsonLines(filePath, (rec) => {
      const message = isRecord(rec.message) ? rec.message : undefined;
      if (message?.type !== "StatusUpdate") return;
      const payload = isRecord(message.payload) ? message.payload : undefined;
      if (!payload) return;
      if (typeof payload.model === "string" && payload.model) currentModel = payload.model;
      const usage = isRecord(payload.token_usage) ? payload.token_usage : undefined;
      if (!usage) return;
      const input = numeric(usage.input_other);
      const output = numeric(usage.output);
      const cached = numeric(usage.input_cache_read) + numeric(usage.input_cache_creation);
      if (input === 0 && output === 0 && cached === 0) return;
      // legacy 时间戳为 epoch 秒
      const ts = numeric(rec.timestamp ?? payload.timestamp) * 1000;
      if (!ts) return;
      const dateOnly = localDate(ts);
      if (!dateOnly || dateOnly < start || dateOnly > end) return;
      const key = byHour ? (localDateHour(ts) ?? dateOnly) : dateOnly;
      addStats(stats, key, currentModel, input + cached, output, cached);
    });
  }
}

function collectKimiTokenStats(start: string, end: string, byHour = false): LocalTokenStats[] {
  const stats = new Map<string, LocalTokenStats>();
  const codeRoot = join(homedir(), ".kimi-code", "sessions");
  if (listWireFiles(codeRoot).length > 0) {
    collectKimiCodeStats(codeRoot, start, end, stats, byHour);
  } else {
    collectKimiLegacyStats(join(homedir(), ".kimi", "sessions"), start, end, stats, byHour);
  }
  return statsRows(stats);
}

// ---- Qoder IDE 本地 SQLite Token（chat_message.token_info，真实计数）----
// Qoder 桌面端把每条 assistant 消息的真实 token 存在 SharedClientCache 的 local.db，
// token_info = {prompt_tokens, completion_tokens, cached_tokens}，gmt_create 为毫秒。

function appSupportBase(): string {
  if (process.platform === "darwin") return join(homedir(), "Library", "Application Support");
  if (process.platform === "win32") {
    return process.env.APPDATA?.trim() || join(homedir(), "AppData", "Roaming");
  }
  return process.env.XDG_CONFIG_HOME?.trim() || join(homedir(), ".config");
}

function qoderDbPaths(): string[] {
  return ["Qoder", "QoderCN"].map((name) =>
    join(appSupportBase(), name, "SharedClientCache", "cache", "db", "local.db"),
  );
}

function qoderModel(modelInfo: unknown, fallback: string): string {
  if (typeof modelInfo !== "string") return fallback;
  try {
    const parsed: unknown = JSON.parse(modelInfo);
    if (!isRecord(parsed)) return fallback;
    return modelName(parsed.model_key ?? parsed.model_id ?? parsed.model, fallback);
  } catch {
    return fallback;
  }
}

function isLockError(e: unknown): boolean {
  const msg = String(
    (e as { stderr?: unknown })?.stderr ?? (e as { message?: unknown })?.message ?? e,
  );
  return /database is locked|SQLITE_BUSY|resource busy|being used by another process|unable to open database file|readonly/i.test(
    msg,
  );
}

/** 查询 SQLite（sqlite3 -json）；库被占用锁定时，快照复制后再查。 */
async function querySqliteJson(dbPath: string, sql: string): Promise<JsonRecord[]> {
  const parse = (out: string): JsonRecord[] => {
    const trimmed = out.trim();
    if (!trimmed || trimmed === "[]") return [];
    return JSON.parse(trimmed) as JsonRecord[];
  };
  try {
    const { stdout } = await run("sqlite3", ["-json", dbPath, sql], 45_000);
    return parse(stdout);
  } catch (e) {
    if (!isLockError(e)) throw e;
    const dir = mkdtempSync(join(tmpdir(), "xlt-qoder-"));
    const snap = join(dir, "local.db");
    try {
      copyFileSync(dbPath, snap);
      for (const suffix of ["-wal", "-shm"]) {
        const companion = `${dbPath}${suffix}`;
        if (existsSync(companion)) copyFileSync(companion, `${snap}${suffix}`);
      }
      const { stdout } = await run("sqlite3", ["-json", snap, sql], 45_000);
      return parse(stdout);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  }
}

async function collectQoderTokenStats(
  start: string,
  end: string,
  byHour = false,
): Promise<LocalTokenStats[]> {
  const stats = new Map<string, LocalTokenStats>();
  // 用秒级下界裁剪（同时兼容毫秒/秒时间戳），精确的 [start,end] 仍由下方按本地日期过滤。
  const lowerBoundSec = Math.floor(new Date(`${start}T00:00:00`).getTime() / 1000);
  const sql =
    "SELECT token_info, model_info, gmt_create FROM chat_message " +
    "WHERE role='assistant' AND token_info IS NOT NULL AND length(token_info) > 2 " +
    `AND gmt_create >= ${lowerBoundSec}`;

  for (const dbPath of qoderDbPaths()) {
    if (!existsSync(dbPath)) continue;
    let rows: JsonRecord[];
    try {
      rows = await querySqliteJson(dbPath, sql);
    } catch {
      continue;
    }
    for (const row of rows) {
      const raw = typeof row.token_info === "string" ? row.token_info : null;
      if (!raw) continue;
      let info: JsonRecord;
      try {
        const parsed: unknown = JSON.parse(raw);
        if (!isRecord(parsed)) continue;
        info = parsed;
      } catch {
        continue;
      }
      const prompt = Math.max(0, numeric(info.prompt_tokens));
      const completion = Math.max(0, numeric(info.completion_tokens));
      const cached = Math.max(0, numeric(info.cached_tokens));
      const input = Math.max(0, prompt - cached); // 未命中缓存的输入
      const gmt = numeric(row.gmt_create);
      if (gmt <= 0) continue;
      const ms = gmt < 1e12 ? gmt * 1000 : gmt; // 容忍秒级
      const date = localDate(ms);
      if (!date || date < start || date > end) continue;
      const key = byHour ? (localDateHour(ms) ?? date) : date;
      addStats(stats, key, qoderModel(row.model_info, "qoder-ide"), input, completion, cached);
    }
  }

  // Qoder CLI transcript（~/.qoder/projects），credits-only 无真实 token，按文本估算。
  collectQoderCliTokenStats(start, end, stats, byHour);
  return statsRows(stats);
}

/**
 * Qoder CLI：~/.qoder/projects 下的 anthropic 风格 transcript（message.role + content[]）。
 * usage 只记 credits、token 全为 0，故按文本估算：user → 输入，assistant → 输出。
 */
function collectQoderCliTokenStats(
  start: string,
  end: string,
  stats: Map<string, LocalTokenStats>,
  byHour = false,
): void {
  const root = join(homedir(), ".qoder", "projects");
  if (!existsSync(root)) return;
  for (const filePath of listJsonlFiles(root)) {
    const mtimeDate = fileMtimeDate(filePath);
    if (mtimeDate && mtimeDate < start) continue;
    readJsonLines(filePath, (rec) => {
      const message = isRecord(rec.message) ? rec.message : undefined;
      if (!message) return;
      const role = message.role;
      if (role !== "user" && role !== "assistant") return;
      const dateOnly = localDate(rec.timestamp);
      if (!dateOnly || dateOnly < start || dateOnly > end) return;
      const key = byHour ? (localDateHour(rec.timestamp) ?? dateOnly) : dateOnly;
      const model = modelName(message.model, "qoder-cli");
      if (role === "user") {
        bumpStats(stats, key, model, estTokensText(message.content, model), 0, 0);
      } else {
        bumpStats(stats, key, model, 0, estTokensText(message.content, model), 1);
      }
    });
  }
}

/** OpenCode 今日按小时（改 SQL 用 strftime 分组到小时）。 */
async function collectOpenCodeHourly(today: string): Promise<LocalTokenStats[]> {
  try {
    const tzMod = tzSqliteModifier();
    const sql =
      `SELECT strftime('%Y-%m-%d %H', time_created/1000,'unixepoch',${tzMod}) AS d, ` +
      "COALESCE(json_extract(data,'$.modelID'),json_extract(data,'$.model'),json_extract(data,'$.modelId'),'opencode-unknown') AS model, " +
      "sum(json_extract(data,'$.tokens.input')) AS inp, " +
      "sum(json_extract(data,'$.tokens.output')) AS outp, " +
      "sum(COALESCE(json_extract(data,'$.tokens.cache.read'),0)) AS cache " +
      "FROM message " +
      "WHERE json_extract(data,'$.role')='assistant' " +
      "AND json_extract(data,'$.tokens.input') IS NOT NULL " +
      `AND date(time_created/1000,'unixepoch',${tzMod})='${today}' ` +
      "GROUP BY d, model ORDER BY d, model";
    const { stdout } = await run(OPENCODE, ["db", sql, "--format", "json"]);
    const rows = JSON.parse(stdout || "[]") as JsonRecord[];
    return rows.map((r) => ({
      d: String(r.d),
      model: modelName(r.model, "opencode-unknown"),
      inp: numeric(r.inp),
      outp: numeric(r.outp),
      cache: numeric(r.cache),
      requests: 0,
    }));
  } catch {
    return [];
  }
}

/** 汇总今天所有本地源的「按小时 × 平台」用量（供分析页「今天」时间轴使用）。 */
async function collectTodayHourly(): Promise<
  { platform: string; hour: number; model: string; input: number; output: number }[]
> {
  const today = todayStr();
  const out: { platform: string; hour: number; model: string; input: number; output: number }[] = [];
  const tag = (platform: string, rows: LocalTokenStats[]): void => {
    for (const r of rows) {
      const hourPart = r.d.split(" ")[1];
      const hour = hourPart != null ? Number(hourPart) : Number.NaN;
      if (Number.isNaN(hour)) continue;
      out.push({ platform, hour, model: r.model, input: r.inp, output: r.outp });
    }
  };
  tag("codex", collectCodexTokenStats(today, today, true));
  tag("claude", collectClaudeTokenStats(today, today, true));
  tag("kiro", collectKiroTokenStats(today, today, true));
  tag("qoder", await collectQoderTokenStats(today, today, true));
  tag("opencode-go", await collectOpenCodeHourly(today));
  tag("gemini", collectGeminiTokenStats(today, today, true));
  tag("copilot", collectCopilotTokenStats(today, today, true));
  tag("kimi", collectKimiTokenStats(today, today, true));
  return out;
}

function sendJson(res: ServerResponse, status: number, body: unknown): void {
  res.statusCode = status;
  res.setHeader("Content-Type", "application/json; charset=utf-8");
  res.end(JSON.stringify(body));
}

function summarizeErr(e: unknown): string {
  if (e && typeof e === "object" && "stderr" in e) {
    const stderr = String((e as { stderr?: unknown }).stderr ?? "");
    const msg = String((e as { message?: unknown }).message ?? "");
    return (stderr || msg).slice(0, 600);
  }
  return String(e).slice(0, 600);
}

const OIL_API_URL = "https://v1.apizero.cn/api/oil-price-forecast";
const OIL_PRICE_SOURCE_URL = "https://www.chajiage.com/youjia";
const NDRC_NEWS_URL = "https://www.ndrc.gov.cn/xwdt/xwfb/wap_index.html";
const OIL_PROVINCE_SLUGS: Record<string, string> = {
  "北京": "beijing", "天津": "tianjin", "河北": "hebei", "山西": "shanxi",
  "内蒙古": "neimenggu", "辽宁": "liaoning", "吉林": "jilin", "黑龙江": "heilongjiang",
  "上海": "shanghai", "江苏": "jiangsu", "浙江": "zhejiang", "安徽": "anhui",
  "福建": "fujian", "江西": "jiangxi", "山东": "shandong", "河南": "henan",
  "湖北": "hubei", "湖南": "hunan", "广东": "guangdong", "广西": "guangxi",
  "海南": "hainan", "重庆": "chongqing", "四川": "sichuan", "贵州": "guizhou",
  "云南": "yunnan", "西藏": "xizang", "陕西": "shannxi", "甘肃": "gansu",
  "青海": "qinghai", "宁夏": "ningxia", "新疆": "xinjiang",
};

async function fetchOilForecast(province: string, apiKey: string): Promise<unknown> {
  const target = new URL(OIL_API_URL);
  target.searchParams.set("action", "forecast");
  target.searchParams.set("province", province);
  target.searchParams.set("year", String(new Date().getFullYear()));
  const headers: Record<string, string> = { Accept: "application/json" };
  if (apiKey) headers.Authorization = `Bearer ${apiKey}`;
  const response = await fetch(target, { headers, signal: AbortSignal.timeout(15_000) });
  const body = await response.text();
  if (!response.ok) throw new Error(`油价数据源 HTTP ${response.status}: ${body.slice(0, 240)}`);
  return JSON.parse(body);
}

async function fetchCurrentOilPrices(province: string): Promise<JsonRecord> {
  const slug = OIL_PROVINCE_SLUGS[province];
  if (!slug) throw new Error("暂不支持该省份的今日油价");
  const sourceUrl = `${OIL_PRICE_SOURCE_URL}/${slug}.html`;
  const response = await fetch(sourceUrl, {
    headers: { "User-Agent": "Mozilla/5.0 xlt-workbench/0.1 (+local oil monitor)" },
    signal: AbortSignal.timeout(15_000),
  });
  if (!response.ok) throw new Error(`今日油价数据源 HTTP ${response.status}`);
  const plain = (await response.text())
    .replace(/<script[\s\S]*?<\/script>/gi, " ")
    .replace(/<style[\s\S]*?<\/style>/gi, " ")
    .replace(/<[^>]+>/g, " ")
    .replace(/&nbsp;|&#160;|&yen;/gi, " ")
    .replace(/\s+/g, " ");
  const prices = /(\d{4})年(\d{2})月(\d{2})日，?[^。]{0,40}?汽油、柴油每升最新价格为：?\s*92号汽油\s*为\s*(\d+(?:\.\d+)?)元，?\s*95号汽油\s*为\s*(\d+(?:\.\d+)?)元，?\s*98号汽油\s*为\s*(\d+(?:\.\d+)?)元，?\s*0号柴油\s*为\s*(\d+(?:\.\d+)?)元/.exec(plain);
  if (!prices) throw new Error("今日油价页面缺少 92/95/98 号汽油或 0 号柴油价格");
  return {
    code: 0,
    msg: "成功",
    data: {
      province,
      price_date: `${prices[1]}-${prices[2]}-${prices[3]}`,
      source_url: sourceUrl,
      prices: {
        "92号汽油": Number(prices[4]),
        "95号汽油": Number(prices[5]),
        "98号汽油": Number(prices[6]),
        "0号柴油": Number(prices[7]),
      },
    },
  };
}

async function fetchOfficialOilAdjustment(): Promise<JsonRecord> {
  const listResponse = await fetch(NDRC_NEWS_URL, {
    headers: { "User-Agent": "xlt-workbench/0.1 (+local oil monitor)" },
    signal: AbortSignal.timeout(15_000),
  });
  if (!listResponse.ok) throw new Error(`国家发改委新闻列表 HTTP ${listResponse.status}`);
  const listHtml = await listResponse.text();
  const entry = /<li><a href="([^"]+)"[^>]*>([^<]*成品油价格[^<]*)<\/a><span>([^<]+)<\/span>/.exec(listHtml);
  if (!entry) throw new Error("国家发改委新闻列表中未找到成品油调价公告");

  const sourceUrl = new URL(entry[1], NDRC_NEWS_URL).toString();
  const title = entry[2].trim();
  const publishedAt = entry[3].trim().replaceAll("/", "-");
  const articleResponse = await fetch(sourceUrl, {
    headers: { "User-Agent": "xlt-workbench/0.1 (+local oil monitor)" },
    signal: AbortSignal.timeout(15_000),
  });
  if (!articleResponse.ok) throw new Error(`国家发改委调价公告 HTTP ${articleResponse.status}`);
  const plain = (await articleResponse.text())
    .replace(/<script[\s\S]*?<\/script>/gi, " ")
    .replace(/<style[\s\S]*?<\/style>/gi, " ")
    .replace(/<[^>]+>/g, " ")
    .replace(/&nbsp;|&#160;/g, " ")
    .replace(/\s+/g, " ");

  const actual = /调控后实际(上调|下调)\s*(\d+)元[、，]\s*(\d+)元/.exec(plain);
  const normal = /汽、柴油[^。]{0,80}?价格每吨分别(?:应)?(上调|下调)\s*(\d+)元[、，]\s*(\d+)元/.exec(plain);
  const amounts = actual ?? normal;
  const direction = title.includes("不作调整") || plain.includes("不作调整")
    ? "unchanged"
    : amounts?.[1] === "下调"
      ? "down"
      : amounts?.[1] === "上调"
        ? "up"
        : undefined;
  if (!direction) throw new Error("无法识别国家发改委公告中的调价方向");

  const effectiveTime = new Date(`${publishedAt}T00:00:00+08:00`).getTime() + 24 * 60 * 60_000;
  const sign = direction === "down" ? -1 : 1;
  return {
    title,
    publishedAt,
    effectiveAt: new Date(effectiveTime).toISOString(),
    direction,
    gasolineChangePerTon: amounts ? sign * Number(amounts[2]) : undefined,
    dieselChangePerTon: amounts ? sign * Number(amounts[3]) : undefined,
    sourceUrl,
  };
}

/**
 * 开发期通过本机 CLI 拉取用量：
 * - arkcli：火山方舟套餐额度 + Token 用量（SSO→签名）
 * - opencode db：OpenCode 本地 SQLite 的逐日 Token 用量（usage 字段聚合）
 * - kiro-cli /usage：Kiro Credits + 重置日（chat slash 命令，文本解析）
 * 浏览器无法执行进程 / Volc 签名，故在 Vite dev server 内 spawn。
 * 生产期（Tauri）改走 Rust 侧 shell 或自实现签名。
 */
export function localConnectorsDevPlugin(): Plugin {
  return {
    name: "xlt-local-connectors",
    configureServer(server) {
      server.middlewares.use(async (req, res, next) => {
        const raw = req.url ?? "";
        const url = new URL(raw, "http://localhost");
        const p = url.pathname;
        // 分桶时区：由前端按用量偏好传入 ?tz=IANA；空则跟随系统本地时区。
        activeTz = url.searchParams.get("tz")?.trim() || undefined;

        // ---- arkcli（火山方舟）----
        if (p.startsWith("/api/ark/")) {
          try {
            if (p === "/api/ark/status") {
              const { stdout } = await run(
                ARKCLI,
                ["auth", "status", "--format", "json"],
                ARK_PLAN_TIMEOUT_MS,
              );
              return sendJson(res, 200, JSON.parse(stdout || "{}"));
            }
            if (p === "/api/ark/plan") {
              const { stdout } = await run(
                ARKCLI,
                ["usage", "plan", "--format", "json"],
                ARK_PLAN_TIMEOUT_MS,
              );
              return sendJson(res, 200, JSON.parse(stdout || "{}"));
            }
            if (p === "/api/ark/stats") {
              const start = url.searchParams.get("start") ?? todayStr();
              const end = url.searchParams.get("end") ?? todayStr();
              if (!DATE_RE.test(start) || !DATE_RE.test(end)) {
                return sendJson(res, 400, { error: "start/end must be YYYY-MM-DD" });
              }
              // 依次尝试 endpoint -> apikey -> 账号全量；首个非空即返回
              const attempts: string[][] = [
                ["usage", "stats", "--start", start, "--end", end, "--mine", "--format", "json"],
                ["usage", "stats", "--start", start, "--end", end, "--mine", "--mine-by", "apikey", "--format", "json"],
                ["usage", "stats", "--start", start, "--end", end, "--format", "json"],
              ];
              let lastValid: unknown = null;
              let lastErr = "";
              for (const args of attempts) {
                try {
                  const { stdout } = await run(ARKCLI, args, ARK_STATS_TIMEOUT_MS);
                  const data = JSON.parse(stdout || "{}");
                  if (Number(data?.data_count ?? 0) > 0) return sendJson(res, 200, data);
                  lastValid = data;
                } catch (e) {
                  lastErr = summarizeErr(e);
                }
              }
              if (lastValid) return sendJson(res, 200, lastValid);
              return sendJson(res, 502, {
                error: "arkcli usage stats 失败: " + (lastErr || "未知错误"),
                hint: "若提示未登录，请在终端运行 `arkcli auth login volc-sso`",
              });
            }
          } catch (e) {
            return sendJson(res, 502, {
              error: "arkcli 调用失败: " + summarizeErr(e),
              hint: "若提示未登录，请在终端运行 `arkcli auth login volc-sso`",
            });
          }
        }

        // ---- opencode 本地 SQLite（Token 用量）----
        if (p === "/api/opencode/stats") {
          try {
            const tzMod = tzSqliteModifier();
            const sql =
              `SELECT date(time_created/1000,'unixepoch',${tzMod}) AS d, ` +
              "COALESCE(json_extract(data,'$.modelID'),json_extract(data,'$.model'),json_extract(data,'$.modelId'),'opencode-unknown') AS model, " +
              "sum(json_extract(data,'$.tokens.input')) AS inp, " +
              "sum(json_extract(data,'$.tokens.output')) AS outp, " +
              "sum(COALESCE(json_extract(data,'$.tokens.cache.read'),0)) AS cache " +
              "FROM message " +
              "WHERE json_extract(data,'$.role')='assistant' " +
              "AND json_extract(data,'$.tokens.input') IS NOT NULL " +
              "GROUP BY d, model ORDER BY d, model";
            const { stdout } = await run(OPENCODE, ["db", sql, "--format", "json"]);
            return sendJson(res, 200, JSON.parse(stdout || "[]"));
          } catch (e) {
            return sendJson(res, 502, {
              error: "opencode db 查询失败: " + summarizeErr(e),
              hint: "确认 opencode 已安装且有历史会话",
            });
          }
        }

        // ---- opencode.ai 网页订阅额度（需 auth cookie + workspace id）----
        if (p === "/api/opencode/quota") {
          try {
            const cookie = url.searchParams.get("cookie") || "";
            const workspace = url.searchParams.get("workspace") || "";
            if (!cookie || !workspace) {
              return sendJson(res, 400, { error: "需要 cookie 与 workspace 参数" });
            }
            const data = await fetchOpenCodeWebQuota(cookie, workspace);
            return sendJson(res, 200, data);
          } catch (e) {
            return sendJson(res, 502, {
              error: "opencode.ai 额度抓取失败: " + summarizeErr(e),
              hint: "cookie 可能过期，请重新登录 opencode.ai 并更新设置",
            });
          }
        }

        // ---- Kiro CLI 本地会话 Token（~/.kiro/sessions/cli，estimateTokens 估算）----
        if (p === "/api/kiro/stats") {
          const start = url.searchParams.get("start") ?? todayStr();
          const end = url.searchParams.get("end") ?? todayStr();
          if (!DATE_RE.test(start) || !DATE_RE.test(end) || start > end) {
            return sendJson(res, 400, { error: "start/end must be ordered YYYY-MM-DD dates" });
          }
          try {
            return sendJson(res, 200, collectKiroTokenStats(start, end));
          } catch (e) {
            return sendJson(res, 502, { error: "Kiro 本地 Token 聚合失败: " + summarizeErr(e) });
          }
        }

        // ---- kiro-cli /usage（Credits）----
        if (p === "/api/kiro/usage") {
          try {
            // kiro-cli 在无 TTY 时把 /usage 输出到 stderr，故合并解析
            const { stdout, stderr } = await run(KIROCLI, [
              "chat",
              "/usage",
              "--no-interactive",
            ]);
            return sendJson(res, 200, parseKiroUsage(`${stdout}\n${stderr}`));
          } catch (e) {
            return sendJson(res, 502, {
              error: "kiro-cli /usage 失败: " + summarizeErr(e),
              hint: "若提示未登录，请在终端运行 `kiro-cli login`",
            });
          }
        }

        // ---- Qoder 本地会话 Token（IDE 本地 SQLite 的 token_info，真实计数）----
        if (p === "/api/qoder/stats") {
          const start = url.searchParams.get("start") ?? todayStr();
          const end = url.searchParams.get("end") ?? todayStr();
          if (!DATE_RE.test(start) || !DATE_RE.test(end) || start > end) {
            return sendJson(res, 400, { error: "start/end must be ordered YYYY-MM-DD dates" });
          }
          try {
            return sendJson(res, 200, await collectQoderTokenStats(start, end));
          } catch (e) {
            return sendJson(res, 502, { error: "Qoder 本地 Token 聚合失败: " + summarizeErr(e) });
          }
        }

        // ---- Qoder Agent SDK（套餐 Credits）----
        if (p === "/api/qoder/usage") {
          try {
            return sendJson(res, 200, await fetchQoderUsage());
          } catch (e) {
            return sendJson(res, 502, {
              error: "Qoder Credits 查询失败: " + summarizeErr(e),
              hint: "请先在终端运行 qodercli login 完成登录",
            });
          }
        }

        // ---- codex 本地会话 Token（~/.codex/sessions）----
        if (p === "/api/codex/stats") {
          const start = url.searchParams.get("start") ?? todayStr();
          const end = url.searchParams.get("end") ?? todayStr();
          if (!DATE_RE.test(start) || !DATE_RE.test(end) || start > end) {
            return sendJson(res, 400, { error: "start/end must be ordered YYYY-MM-DD dates" });
          }
          try {
            return sendJson(res, 200, collectCodexTokenStats(start, end));
          } catch (e) {
            return sendJson(res, 502, { error: "Codex 本地 Token 聚合失败: " + summarizeErr(e) });
          }
        }

        // ---- Claude Code 本地会话 Token（~/.claude/projects）----
        if (p === "/api/claude/stats") {
          const start = url.searchParams.get("start") ?? todayStr();
          const end = url.searchParams.get("end") ?? todayStr();
          if (!DATE_RE.test(start) || !DATE_RE.test(end) || start > end) {
            return sendJson(res, 400, { error: "start/end must be ordered YYYY-MM-DD dates" });
          }
          try {
            return sendJson(res, 200, collectClaudeTokenStats(start, end));
          } catch (e) {
            return sendJson(res, 502, { error: "Claude Code 本地 Token 聚合失败: " + summarizeErr(e) });
          }
        }

        // ---- Gemini CLI 本地会话 Token（~/.gemini/tmp/<hash>/chats）----
        if (p === "/api/gemini/stats") {
          const start = url.searchParams.get("start") ?? todayStr();
          const end = url.searchParams.get("end") ?? todayStr();
          if (!DATE_RE.test(start) || !DATE_RE.test(end) || start > end) {
            return sendJson(res, 400, { error: "start/end must be ordered YYYY-MM-DD dates" });
          }
          try {
            return sendJson(res, 200, collectGeminiTokenStats(start, end));
          } catch (e) {
            return sendJson(res, 502, { error: "Gemini 本地 Token 聚合失败: " + summarizeErr(e) });
          }
        }

        // ---- GitHub Copilot CLI 本地会话 Token（~/.copilot/session-state）----
        if (p === "/api/copilot/stats") {
          const start = url.searchParams.get("start") ?? todayStr();
          const end = url.searchParams.get("end") ?? todayStr();
          if (!DATE_RE.test(start) || !DATE_RE.test(end) || start > end) {
            return sendJson(res, 400, { error: "start/end must be ordered YYYY-MM-DD dates" });
          }
          try {
            return sendJson(res, 200, collectCopilotTokenStats(start, end));
          } catch (e) {
            return sendJson(res, 502, { error: "Copilot 本地 Token 聚合失败: " + summarizeErr(e) });
          }
        }

        // ---- Kimi Code 本地会话 Token（~/.kimi-code/sessions 的 wire.jsonl，真实计数）----
        if (p === "/api/kimi/stats") {
          const start = url.searchParams.get("start") ?? todayStr();
          const end = url.searchParams.get("end") ?? todayStr();
          if (!DATE_RE.test(start) || !DATE_RE.test(end) || start > end) {
            return sendJson(res, 400, { error: "start/end must be ordered YYYY-MM-DD dates" });
          }
          try {
            return sendJson(res, 200, collectKimiTokenStats(start, end));
          } catch (e) {
            return sendJson(res, 502, { error: "Kimi 本地 Token 聚合失败: " + summarizeErr(e) });
          }
        }

        // ---- 今日按小时 × 平台（分析页「今天」时间轴）----
        if (p === "/api/oil/price" || p === "/api/oil/forecast") {
          const province = url.searchParams.get("province")?.trim() ?? "";
          if (!province || province.length > 12) {
            return sendJson(res, 400, { error: "需要有效的省份名称" });
          }
          try {
            const data = p.endsWith("/price")
              ? await fetchCurrentOilPrices(province)
              : await fetchOilForecast(province, url.searchParams.get("apiKey")?.trim() ?? "");
            return sendJson(res, 200, data);
          } catch (e) {
            return sendJson(res, 502, { error: "国内油价同步失败: " + summarizeErr(e) });
          }
        }

        if (p === "/api/oil/adjustment") {
          try {
            return sendJson(res, 200, await fetchOfficialOilAdjustment());
          } catch (e) {
            return sendJson(res, 502, { error: "国家发改委调价公告同步失败: " + summarizeErr(e) });
          }
        }

        // ---- 今日按小时 × 平台（分析页「今天」时间轴）----
        if (p === "/api/today-hourly") {
          try {
            return sendJson(res, 200, await collectTodayHourly());
          } catch (e) {
            return sendJson(res, 502, { error: "今日按小时聚合失败: " + summarizeErr(e) });
          }
        }

        // ---- codex 额度（wham/usage，token 在 ~/.codex/auth.json）----
        if (p === "/api/codex/usage") {
          try {
            const data = await fetchCodexUsage();
            return sendJson(res, 200, data);
          } catch (e) {
            return sendJson(res, 502, {
              error: "codex 额度查询失败: " + summarizeErr(e),
              hint: "若未登录，请在终端运行 `codex login`",
            });
          }
        }

        // ---- Kimi Code 会员额度（/coding/v1/usages，token 在 ~/.kimi-code/credentials）----
        if (p === "/api/kimi/usage") {
          try {
            const data = await fetchKimiUsage();
            return sendJson(res, 200, data);
          } catch (e) {
            return sendJson(res, 502, {
              error: "kimi 额度查询失败: " + summarizeErr(e),
              hint: "若未登录，请在终端运行 `kimi login`",
            });
          }
        }

        // ---- codex 登录状态（保留，供设置面板展示）----
        if (p === "/api/codex/status") {
          try {
            // codex 在无 TTY 时把 login status 输出到 stderr，故合并解析
            const { stdout, stderr } = await run(CODEX, ["login", "status"]);
            return sendJson(res, 200, parseCodexStatus(`${stdout}\n${stderr}`));
          } catch (e) {
            return sendJson(res, 502, {
              error: "codex login status 失败: " + summarizeErr(e),
              hint: "若未登录，请在终端运行 `codex login`",
            });
          }
        }

        return next();
      });
    },
  };
}

async function fetchQoderUsage(): Promise<{
  used: number;
  total: number;
  remaining: number;
  planTag?: string;
  expiresAt?: string;
  addOnUsed?: number;
  addOnTotal?: number;
}> {
  let releaseInput: (() => void) | undefined;
  async function* idlePrompt(): AsyncGenerator<never> {
    await new Promise<void>((resolve) => {
      releaseInput = resolve;
    });
  }

  const q = query({
    prompt: idlePrompt(),
    options: {
      auth: qodercliAuth(),
      cwd: process.cwd(),
    },
  });
  try {
    await q.initializationResult();
    const usage = await q.getUsageInfo();
    const quota = usage?.userQuota;
    if (!quota || !Number.isFinite(quota.used) || !Number.isFinite(quota.total)) {
      throw new Error("Qoder 未返回套餐 Credits，请确认已登录且 CLI 为最新版本");
    }
    return {
      used: quota.used,
      total: quota.total,
      remaining: Number.isFinite(quota.remaining) ? quota.remaining : quota.total - quota.used,
      planTag: formatQoderPlan(usage.userType),
      expiresAt:
        typeof usage.expiresAt === "number"
          ? new Date(usage.expiresAt).toISOString()
          : undefined,
      addOnUsed: Number.isFinite(usage.addOnQuota?.used) ? usage.addOnQuota?.used : undefined,
      addOnTotal: Number.isFinite(usage.addOnQuota?.total) ? usage.addOnQuota?.total : undefined,
    };
  } finally {
    releaseInput?.();
    await q.close();
  }
}

function formatQoderPlan(userType: string | undefined): string | undefined {
  const plans: Record<string, string> = {
    personal_free: "Free",
    personal_professional: "Pro",
    personal_teams: "Teams",
  };
  return userType ? (plans[userType] ?? userType) : undefined;
}

/** 解析 kiro-cli /usage 的文本输出为结构化 Credits 数据 */
function parseKiroUsage(stdout: string): {
  used?: number;
  total?: number;
  planTag?: string;
  resetsAt?: string;
  error?: string;
} {
  const strip = stdout.replace(/\x1b\[[0-?]*[ -/]*[@-~]/g, "");
  const credits = strip.match(
    /Credits\s*(?:\(\s*)?([\d,.]+)\s*(?:of|used\s*\/)\s*([\d,.]+)\s*(?:covered\s+in\s+(?:the\s+)?plan)?\)?/i,
  );
  const resets = strip.match(/resets on\s+(\d{4}-\d{2}-\d{2})/i);
  const tag = strip.match(/Estimated Usage[^|]*\|[^|]*\|\s*([^\n]+)/);
  if (!credits) return { error: "未匹配到 Credits 行" };
  const used = Number.parseFloat(credits[1].replace(/,/g, ""));
  const total = Number.parseFloat(credits[2].replace(/,/g, ""));
  const resetsAt = resets ? `${resets[1]}T00:00:00+08:00` : undefined;
  const planTag = tag ? tag[1].trim() : undefined;
  return { used, total, planTag, resetsAt };
}

/**
 * 解析 codex login status 文本 + ~/.codex/auth.json 的 id_token JWT，
 * 得到登录状态 / 邮箱 / 套餐类型。5h、weekly 额度仅在 TUI /status 可见，CLI 无法脚本化获取。
 */
function parseCodexStatus(stdout: string): {
  loggedIn: boolean;
  email?: string;
  planType?: string;
  planTag?: string;
  subscriptionActiveUntil?: string;
} {
  const loggedIn = /Logged in/i.test(stdout);
  let email: string | undefined;
  let planType: string | undefined;
  let subscriptionActiveUntil: string | undefined;
  try {
    const authPath = join(homedir(), ".codex", "auth.json");
    const auth = JSON.parse(readFileSync(authPath, "utf8"));
    const idToken: string | undefined = auth?.tokens?.id_token;
    if (idToken) {
      const payload = idToken.split(".")[1];
      const claims = JSON.parse(
        Buffer.from(payload + "=".repeat((4 - (payload.length % 4)) % 4), "base64url").toString("utf8"),
      );
      email = claims?.email;
      const authClaim = claims?.["https://api.openai.com/auth"];
      planType = authClaim?.chatgpt_plan_type;
      subscriptionActiveUntil = authClaim?.chatgpt_subscription_active_until;
    }
  } catch {
    // auth.json 读取/解析失败时只回退到登录文本
  }
  const planTag = planType ? planType.charAt(0).toUpperCase() + planType.slice(1) : undefined;
  return { loggedIn, email, planType, planTag, subscriptionActiveUntil };
}

/**
 * 调用 ChatGPT 官方 /wham/usage 拉取 Codex 真实额度（5h / weekly）。
 * token 取自 ~/.codex/auth.json。用 curl -sk 走系统代理 + 跳过企业 MITM 证书，
 * 不用 Node fetch（Node fetch 默认不走 HTTP(S)_PROXY 环境变量，且 TLS 会被拦截）。
 */
async function fetchCodexUsage(): Promise<{
  email?: string;
  planType?: string;
  planTag?: string;
  primary?: { usedPercent: number; windowSeconds: number; resetsAt: string };
  secondary?: { usedPercent: number; windowSeconds: number; resetsAt: string };
}> {
  const authPath = join(homedir(), ".codex", "auth.json");
  const auth = JSON.parse(readFileSync(authPath, "utf8"));
  const accessToken: string | undefined = auth?.tokens?.access_token;
  const idToken: string | undefined = auth?.tokens?.id_token;
  if (!accessToken) throw new Error("auth.json 无 access_token（未登录？）");

  // 从 id_token JWT 解析 email + account_id
  let email: string | undefined;
  let accountId: string | undefined;
  let planType: string | undefined;
  if (idToken) {
    const payload = idToken.split(".")[1];
    const claims = JSON.parse(
      Buffer.from(payload + "=".repeat((4 - (payload.length % 4)) % 4), "base64url").toString("utf8"),
    );
    email = claims?.email;
    accountId = claims?.["https://api.openai.com/auth"]?.chatgpt_account_id;
    planType = claims?.["https://api.openai.com/auth"]?.chatgpt_plan_type;
  }

  const args = [
    "-sk",
    "--max-time", "20",
    "-H", `Authorization: Bearer ${accessToken}`,
    "-H", "User-Agent: OpenCode-Status-Plugin/1.0",
  ];
  if (accountId) args.push("-H", `ChatGPT-Account-Id: ${accountId}`);
  args.push("https://chatgpt.com/backend-api/wham/usage");

  const { stdout } = await run("curl", args);
  let data: { rate_limit?: { primary_window?: unknown; secondary_window?: unknown } };
  try {
    data = JSON.parse(stdout);
  } catch {
    throw new Error("响应不是 JSON: " + stdout.slice(0, 200));
  }

  const win = (w: unknown) => {
    if (!w || typeof w !== "object") return undefined;
    const o = w as { used_percent?: number; limit_window_seconds?: number; reset_at?: number };
    if (o.used_percent == null || o.reset_at == null) return undefined;
    return {
      usedPercent: Math.round(o.used_percent),
      windowSeconds: o.limit_window_seconds ?? 0,
      resetsAt: new Date(o.reset_at * 1000).toISOString(),
    };
  };
  return {
    email,
    planType,
    planTag: planType ? planType.charAt(0).toUpperCase() + planType.slice(1) : undefined,
    primary: win(data?.rate_limit?.primary_window),
    secondary: win(data?.rate_limit?.secondary_window),
  };
}

// ---- Kimi Code 会员额度 ----
// token 取自 ~/.kimi-code/credentials/kimi-code.json（旧版 kimi-cli 迁移目录 ~/.kimi 作回退）。
// access_token 900 秒过期；过期时用 refresh_token 走设备流同款刷新端点换新，
// 并把轮换后的新凭据原子写回原文件（服务端会轮换 refresh_token，不写回会导致 CLI 掉登录）。
// 区域由 ~/.kimi-code/region 决定：mainland-cn → *.kimi.com，global → *.kimi.ai。

const KIMI_CLIENT_ID = "17e5f671-d194-4dfb-9706-5516cb48c098";

interface KimiCredentials {
  access_token: string;
  refresh_token: string;
  expires_at: number;
  expires_in?: number;
  token_type?: string;
  scope?: string;
}

function kimiCredentialsPath(): string {
  for (const dir of [".kimi-code", ".kimi"]) {
    const path = join(homedir(), dir, "credentials", "kimi-code.json");
    if (existsSync(path)) return path;
  }
  return join(homedir(), ".kimi-code", "credentials", "kimi-code.json");
}

function kimiHosts(): { api: string; auth: string } {
  let region = "mainland-cn";
  for (const dir of [".kimi-code", ".kimi"]) {
    try {
      const value = readFileSync(join(homedir(), dir, "region"), "utf8").trim();
      if (value) {
        region = value;
        break;
      }
    } catch {
      // region 文件缺失时按 mainland-cn 处理
    }
  }
  return region === "global"
    ? { api: "https://api.kimi.ai/coding/v1", auth: "https://auth.kimi.ai" }
    : { api: "https://api.kimi.com/coding/v1", auth: "https://auth.kimi.com" };
}

async function kimiCurl(args: string[]): Promise<string> {
  const { stdout } = await run("curl", ["-sk", "--max-time", "20", ...args]);
  return stdout;
}

/** 刷新过期 token 并原子写回凭据文件（保留 0600 权限）。 */
async function refreshKimiToken(
  path: string,
  credentials: KimiCredentials,
  authHost: string,
): Promise<KimiCredentials> {
  const body = new URLSearchParams({
    grant_type: "refresh_token",
    refresh_token: credentials.refresh_token,
    client_id: KIMI_CLIENT_ID,
  });
  const stdout = await kimiCurl([
    "-X", "POST",
    "-H", "Content-Type: application/x-www-form-urlencoded",
    "-d", body.toString(),
    `${authHost}/api/oauth/token`,
  ]);
  let data: JsonRecord;
  try {
    data = JSON.parse(stdout) as JsonRecord;
  } catch {
    throw new Error("Kimi token 刷新响应不是 JSON: " + stdout.slice(0, 200));
  }
  if (typeof data.access_token !== "string" || !data.access_token) {
    throw new Error("Kimi token 刷新失败（可能已登出，请重新运行 `kimi login`）");
  }
  const expiresIn = numeric(data.expires_in) || 900;
  const next: KimiCredentials = {
    ...credentials,
    access_token: data.access_token,
    refresh_token:
      typeof data.refresh_token === "string" && data.refresh_token
        ? data.refresh_token
        : credentials.refresh_token,
    expires_in: expiresIn,
    expires_at: Date.now() / 1000 + expiresIn,
  };
  const tmp = `${path}.tmp-${process.pid}`;
  writeFileSync(tmp, JSON.stringify(next, null, 2), { mode: 0o600 });
  renameSync(tmp, path);
  return next;
}

async function kimiAccessToken(): Promise<{ token: string; api: string }> {
  const path = kimiCredentialsPath();
  let credentials: KimiCredentials;
  try {
    credentials = JSON.parse(readFileSync(path, "utf8")) as KimiCredentials;
  } catch {
    throw new Error("未找到 Kimi 凭据（未登录？）");
  }
  if (!credentials.access_token) throw new Error("Kimi 凭据缺少 access_token（未登录？）");
  const { api, auth } = kimiHosts();
  // 预留 30 秒余量，避免请求途中过期
  if (numeric(credentials.expires_at) < Date.now() / 1000 + 30) {
    if (!credentials.refresh_token) throw new Error("Kimi token 已过期且无 refresh_token");
    credentials = await refreshKimiToken(path, credentials, auth);
  }
  return { token: credentials.access_token, api };
}

async function kimiGetJson(api: string, path: string, token: string): Promise<JsonRecord> {
  const stdout = await kimiCurl([
    "-H", `Authorization: Bearer ${token}`,
    `${api}${path}`,
  ]);
  try {
    return JSON.parse(stdout) as JsonRecord;
  } catch {
    throw new Error(`Kimi ${path} 响应不是 JSON: ` + stdout.slice(0, 200));
  }
}

/**
 * 拉取 Kimi Code 会员额度：5h 滚动窗口 + 月 Code 额度 + 月总额度（used_ratio 0-1）。
 * /me 仅用于取会员等级标签，失败时不阻断额度返回。
 */
async function fetchKimiUsage(): Promise<{
  planTag?: string;
  fiveHour?: { usedPercent: number; resetsAt: string };
  monthlyCode?: { usedPercent: number; resetsAt: string };
  monthlyTotal?: { usedPercent: number; resetsAt: string };
}> {
  const { token, api } = await kimiAccessToken();
  const usagesPayload = await kimiGetJson(api, "/usages", token);
  const usages = isRecord(usagesPayload.usages) ? usagesPayload.usages : {};

  const win = (key: string) => {
    const w = usages[key];
    if (!isRecord(w)) return undefined;
    const ratio = numeric(w.used_ratio);
    const resetTime = typeof w.reset_time === "string" ? w.reset_time : undefined;
    if (!resetTime) return undefined;
    return {
      usedPercent: Math.round(ratio * 1000) / 10,
      resetsAt: new Date(resetTime).toISOString(),
    };
  };

  let planTag: string | undefined;
  try {
    const me = await kimiGetJson(api, "/me", token);
    if (typeof me.user_level_name === "string" && me.user_level_name.trim()) {
      planTag = me.user_level_name.trim();
    }
  } catch {
    // 会员等级获取失败不影响额度展示
  }

  const fiveHour = win("limit_5h");
  const monthlyCode = win("limit_month_code");
  const monthlyTotal = win("limit_month_total");
  if (!fiveHour && !monthlyCode && !monthlyTotal) {
    throw new Error("Kimi /usages 响应缺少额度数据");
  }
  return { planTag, fiveHour, monthlyCode, monthlyTotal };
}
