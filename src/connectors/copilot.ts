import type { TokenDailyUsage } from "@/types/usage";
import {
  type Connector,
  ConnectorError,
  type TokenConnector,
  httpGet,
} from "./types";

/**
 * GitHub Copilot CLI Token Connector
 *
 * 数据来源：本机 `~/.copilot/session-state/<id>/events.jsonl` 的 session.shutdown
 * 事件 data.modelMetrics[model].usage（每模型真实 token），由中间件按天聚合。
 */
export class CopilotConnector implements Connector, TokenConnector {
  readonly id = "copilot";
  private readonly baseUrl = "/api/copilot";

  platformId(): string {
    return "copilot";
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
      throw new ConnectorError(this.id, "拉取 GitHub Copilot Token 用量失败", e);
    })) as CopilotRow[];

    const rows = Array.isArray(payload) ? payload : [];
    return rows.map((row) => ({
      platform: "copilot",
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

interface CopilotRow {
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
