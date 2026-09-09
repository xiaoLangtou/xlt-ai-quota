import { execFile } from "node:child_process";
import { promisify } from "node:util";
import {
  copyFileSync,
  existsSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
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

function todayStr(): string {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

const DATE_RE = /^\d{4}-\d{2}-\d{2}$/;

interface LocalTokenStats {
  d: string;
  inp: number;
  outp: number;
  cache: number;
  requests: number;
}

type JsonRecord = Record<string, unknown>;

function isRecord(value: unknown): value is JsonRecord {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function numeric(value: unknown): number {
  const parsed = typeof value === "number" ? value : Number(value);
  return Number.isFinite(parsed) ? parsed : 0;
}

function localDate(value: unknown): string | undefined {
  if (typeof value !== "string" && typeof value !== "number") return undefined;
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return undefined;
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}

/** 本地「日期 小时」桶键，如 "2026-09-09 14"；用于「今天按小时」聚合。 */
function localDateHour(value: unknown): string | undefined {
  if (typeof value !== "string" && typeof value !== "number") return undefined;
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return undefined;
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
  input: number,
  output: number,
  cached: number,
): void {
  if (input === 0 && output === 0 && cached === 0) return;
  const current = stats.get(date) ?? { d: date, inp: 0, outp: 0, cache: 0, requests: 0 };
  current.inp += input;
  current.outp += output;
  current.cache += cached;
  current.requests += 1;
  stats.set(date, current);
}

function statsRows(stats: Map<string, LocalTokenStats>): LocalTokenStats[] {
  return [...stats.values()].sort((a, b) => a.d.localeCompare(b.d));
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
      readJsonLines(file, (event) => {
        const payload = isRecord(event.payload) ? event.payload : undefined;
        const info = payload && isRecord(payload.info) ? payload.info : undefined;
        const usage = info && isRecord(info.last_token_usage) ? info.last_token_usage : undefined;
        const eventDate = localDate(event.timestamp);
        if (event.type !== "event_msg" || payload?.type !== "token_count" || !usage || eventDate !== date) return;
        const key = byHour ? (localDateHour(event.timestamp) ?? date) : date;
        addStats(
          stats,
          key,
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
      addStats(stats, key, numeric(usage.input_tokens) + cached, numeric(usage.output_tokens), cached);
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
  input: number,
  output: number,
  reqInc: number,
): void {
  if (input === 0 && output === 0 && reqInc === 0) return;
  const cur = stats.get(date) ?? { d: date, inp: 0, outp: 0, cache: 0, requests: 0 };
  cur.inp += input;
  cur.outp += output;
  cur.requests += reqInc;
  stats.set(date, cur);
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
            addStats(stats, key, pendingInput, output, 0);
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
      switch (payload.type) {
        case "user":
        case "tool_result":
          bumpStats(stats, key, estTokensText(payload.content, "kiro-ide"), 0, 0);
          break;
        case "tool_call":
          bumpStats(stats, key, estTokensText(payload.args, "kiro-ide"), 0, 0);
          break;
        case "assistant":
          bumpStats(stats, key, 0, estTokensText(payload.content, "kiro-ide"), 1);
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
    "SELECT token_info, gmt_create FROM chat_message " +
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
      addStats(stats, key, input, completion, cached);
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
      if (role === "user") {
        bumpStats(stats, key, estTokensText(message.content, "claude"), 0, 0);
      } else {
        bumpStats(stats, key, 0, estTokensText(message.content, "claude"), 1);
      }
    });
  }
}

/** OpenCode 今日按小时（改 SQL 用 strftime 分组到小时）。 */
async function collectOpenCodeHourly(today: string): Promise<LocalTokenStats[]> {
  try {
    const sql =
      "SELECT strftime('%Y-%m-%d %H', time_created/1000,'unixepoch','localtime') AS d, " +
      "sum(json_extract(data,'$.tokens.input')) AS inp, " +
      "sum(json_extract(data,'$.tokens.output')) AS outp, " +
      "sum(COALESCE(json_extract(data,'$.tokens.cache.read'),0)) AS cache " +
      "FROM message " +
      "WHERE json_extract(data,'$.role')='assistant' " +
      "AND json_extract(data,'$.tokens.input') IS NOT NULL " +
      `AND date(time_created/1000,'unixepoch','localtime')='${today}' ` +
      "GROUP BY d ORDER BY d";
    const { stdout } = await run(OPENCODE, ["db", sql, "--format", "json"]);
    const rows = JSON.parse(stdout || "[]") as JsonRecord[];
    return rows.map((r) => ({
      d: String(r.d),
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
  { platform: string; hour: number; input: number; output: number }[]
> {
  const today = todayStr();
  const out: { platform: string; hour: number; input: number; output: number }[] = [];
  const tag = (platform: string, rows: LocalTokenStats[]): void => {
    for (const r of rows) {
      const hourPart = r.d.split(" ")[1];
      const hour = hourPart != null ? Number(hourPart) : Number.NaN;
      if (Number.isNaN(hour)) continue;
      out.push({ platform, hour, input: r.inp, output: r.outp });
    }
  };
  tag("codex", collectCodexTokenStats(today, today, true));
  tag("claude", collectClaudeTokenStats(today, today, true));
  tag("kiro", collectKiroTokenStats(today, today, true));
  tag("qoder", await collectQoderTokenStats(today, today, true));
  tag("opencode-go", await collectOpenCodeHourly(today));
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
            const sql =
              "SELECT date(time_created/1000,'unixepoch','localtime') AS d, " +
              "sum(json_extract(data,'$.tokens.input')) AS inp, " +
              "sum(json_extract(data,'$.tokens.output')) AS outp, " +
              "sum(COALESCE(json_extract(data,'$.tokens.cache.read'),0)) AS cache " +
              "FROM message " +
              "WHERE json_extract(data,'$.role')='assistant' " +
              "AND json_extract(data,'$.tokens.input') IS NOT NULL " +
              "GROUP BY d ORDER BY d";
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
