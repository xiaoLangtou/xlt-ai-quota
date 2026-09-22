import type { InstanceStatus, McpPackage, McpServer, McpTransport, Runtime } from "@/types/mcp";

/** 拆分命令行参数：支持单 / 双引号。 */
export function splitArgs(text: string): string[] {
  const tokens: string[] = [];
  let current = "";
  let quote: string | null = null;
  let escaped = false;
  let started = false;
  for (const character of text) {
    if (escaped) {
      current += character;
      escaped = false;
      started = true;
      continue;
    }
    if (character === "\\" && quote !== "'") {
      escaped = true;
      continue;
    }
    if ((character === '"' || character === "'") && quote === character) {
      quote = null;
      continue;
    }
    if ((character === '"' || character === "'") && quote === null) {
      quote = character;
      started = true;
      continue;
    }
    if (/\s/.test(character) && quote === null) {
      if (started) {
        tokens.push(current);
        current = "";
        started = false;
      }
      continue;
    }
    current += character;
    started = true;
  }
  if (started) tokens.push(current);
  return tokens;
}

/** 解析 `KEY=VALUE`（每行一条）为对象。 */
export function parseKeyValues(text: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const line of text.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const index = trimmed.indexOf("=");
    if (index <= 0) continue;
    out[trimmed.slice(0, index).trim()] = trimmed.slice(index + 1).trim();
  }
  return out;
}

/** 把对象格式化为 `KEY=VALUE` 列表。 */
export function formatKeyValues(map?: Record<string, string>): string {
  if (!map) return "";
  return Object.entries(map)
    .map(([key, value]) => `${key}=${value}`)
    .join("\n");
}

export interface StatusMeta {
  label: string;
  color: "success" | "warning" | "error" | "neutral" | "primary";
  icon: string;
}

export function statusMeta(status: InstanceStatus): StatusMeta {
  switch (status) {
    case "ok":
      return { label: "正常", color: "success", icon: "i-lucide-circle-check" };
    case "disabled":
      return { label: "已停用", color: "neutral", icon: "i-lucide-circle-pause" };
    case "config_error":
      return { label: "配置错误", color: "error", icon: "i-lucide-circle-alert" };
    case "command_missing":
      return { label: "命令不存在", color: "error", icon: "i-lucide-terminal-x" };
    default:
      return { label: "未检测", color: "warning", icon: "i-lucide-circle-help" };
  }
}

export function transportLabel(transport: McpTransport): string {
  switch (transport) {
    case "stdio":
      return "stdio";
    case "http":
      return "HTTP";
    case "sse":
      return "SSE";
    default:
      return transport;
  }
}

export function sourceLabel(source: string): string {
  switch (source) {
    case "builtin":
      return "内置精选";
    case "registry":
      return "官方 Registry";
    case "gitcode":
      return "GitCode";
    case "custom":
      return "自定义源";
    default:
      return source;
  }
}

export function formatStars(value?: number): string {
  if (value === undefined || value === null) return "—";
  if (value >= 1000) return `${(value / 1000).toFixed(1)}k`;
  return String(value);
}

export function formatDate(value?: string): string {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleDateString("zh-CN", { year: "numeric", month: "2-digit", day: "2-digit" });
}

/** 展示服务的主命令 / URL。 */
export function serverTargetText(server: {
  transport: McpTransport;
  command?: string;
  args: string[];
  url?: string;
}): string {
  if (server.transport === "stdio") {
    return [server.command, ...server.args].filter(Boolean).join(" ");
  }
  return server.url ?? "";
}

/** 风险等级标签。 */
export function packageRiskLabel(entry: McpPackage): string {
  if (entry.source === "builtin") return "内置精选";
  if (entry.verified) return "官方已验证";
  if (entry.source === "custom") return "自定义源";
  return "社区";
}

/** 把库条目名规范为可写入配置的服务名。 */
export function sanitizeServerName(name: string): string {
  const cleaned = name
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return cleaned || "mcp-server";
}

/** 由选定运行方式 + 环境变量构造服务定义。 */
export function serverFromRuntime(
  packageName: string,
  runtime: Runtime,
  env: Record<string, string>,
): McpServer {
  const name = sanitizeServerName(packageName);
  if (runtime.type === "remote" || (!runtime.command && runtime.url)) {
    return {
      name,
      transport: "http",
      args: [],
      env: {},
      url: runtime.url ?? "",
      headers: {},
      extra: {},
    };
  }
  return {
    name,
    transport: "stdio",
    command: runtime.command ?? runtime.type,
    args: runtime.args ?? [],
    env,
    headers: {},
    extra: {},
  };
}
