import type { TokenDailyUsage } from "@/types/usage";
import {
  type Connector,
  ConnectorError,
  type TokenConnector,
  httpGet,
} from "./types";

/**
 * Gemini CLI Token Connector
 *
 * 数据来源：本机 `~/.gemini/tmp/<hash>/chats/*.json[l]` 会话文件中 assistant 消息的
 * usageMetadata / tokens（累计快照，中间件按相邻快照差分为增量），真实计数。
 */
export class GeminiConnector implements Connector, TokenConnector {
  readonly id = "gemini";
  private readonly baseUrl = "/api/gemini";

  platformId(): string {
    return "gemini";
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
      throw new ConnectorError(this.id, "拉取 Gemini CLI Token 用量失败", e);
    })) as GeminiRow[];

    const rows = Array.isArray(payload) ? payload : [];
    return rows.map((row) => ({
      platform: "gemini",
      date: row.d,
      model: row.model || undefined,
      inputTokens: num(row.inp),
      outputTokens: num(row.outp),
      cachedTokens: row.cache != null ? num(row.cache) : undefined,
      requestCount: row.requests != null ? num(row.requests) : undefined,
      collectedAt,
    }));
  }
}

interface GeminiRow {
  d: string;
  model?: string;
  inp?: number | string;
  outp?: number | string;
  cache?: number | string;
  requests?: number | string;
}

function num(value: number | string | undefined): number {
  if (value == null) return 0;
  return typeof value === "string" ? Number.parseInt(value, 10) || 0 : value;
}
