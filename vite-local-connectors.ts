import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import vm from "node:vm";
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

function collectCodexTokenStats(start: string, end: string): LocalTokenStats[] {
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
        addStats(
          stats,
          date,
          numeric(usage.input_tokens),
          numeric(usage.output_tokens),
          numeric(usage.cached_input_tokens) + numeric(usage.cache_write_input_tokens),
        );
      });
    }
  }
  return statsRows(stats);
}

function collectClaudeTokenStats(start: string, end: string): LocalTokenStats[] {
  const stats = new Map<string, LocalTokenStats>();
  for (const file of listJsonlFiles(join(homedir(), ".claude", "projects"))) {
    readJsonLines(file, (event) => {
      const message = isRecord(event.message) ? event.message : undefined;
      const usage = message && isRecord(message.usage) ? message.usage : undefined;
      const date = localDate(event.timestamp);
      if (event.type !== "assistant" || message?.role !== "assistant" || !usage || !date || date < start || date > end) return;
      const cached = numeric(usage.cache_creation_input_tokens) + numeric(usage.cache_read_input_tokens);
      // Claude 将缓存 Token 独立于 input_tokens 返回；合并后才是完整输入量。
      addStats(stats, date, numeric(usage.input_tokens) + cached, numeric(usage.output_tokens), cached);
    });
  }
  return statsRows(stats);
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
