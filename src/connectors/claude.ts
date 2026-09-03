import type { TokenDailyUsage } from "@/types/usage";
import {
  type Connector,
  ConnectorError,
  type TokenConnector,
  httpGet,
} from "./types";

/**
 * Claude Code Token Connector
 *
 * 数据来源：本机 `~/.claude/projects` 下的 JSONL 会话记录中的 assistant `message.usage`。
 * usage 中的缓存创建和命中 Token 一并计入输入 Token，`cachedTokens` 保留其细分。
 */
export class ClaudeConnector implements Connector, TokenConnector {
  readonly id = "claude";
  private readonly baseUrl = "/api/claude";

  platformId(): string {
    return "claude";
  }

  isConfigured(): boolean {
    return true;
  }

  async fetchTokens(range: { start: string; end: string }): Promise<TokenDailyUsage[]> {
    const collectedAt = new Date().toISOString();
    const payload = (await httpGet({
      baseUrl: this.baseUrl,
      path: "/stats",
      query: range,
    }).catch((e: unknown) => {
      throw new ConnectorError(this.id, "拉取 Claude Code Token 用量失败", e);
    })) as ClaudeRow[];

    const rows = Array.isArray(payload) ? payload : [];
    return rows.map((row) => ({
      platform: "claude",
      date: row.d,
      inputTokens: num(row.inp),
      outputTokens: num(row.outp),
      cachedTokens: row.cache != null ? num(row.cache) : undefined,
      requestCount: row.requests != null ? num(row.requests) : undefined,
      collectedAt,
    }));
  }
}

interface ClaudeRow {
  d: string;
  inp?: number | string;
  outp?: number | string;
  cache?: number | string;
  requests?: number | string;
}

function num(value: number | string | undefined): number {
  if (value == null) return 0;
  return typeof value === "string" ? Number.parseInt(value, 10) || 0 : value;
}
