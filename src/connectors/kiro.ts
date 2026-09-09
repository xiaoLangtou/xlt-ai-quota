import type { Platform, QuotaSnapshot, TokenDailyUsage } from "@/types/usage";
import {
  type Connector,
  type QuotaConnector,
  type TokenConnector,
  ConnectorError,
  httpGet,
} from "./types";

/**
 * Kiro Connector
 *
 * 数据来源：本机 `kiro-cli chat "/usage" --no-interactive`（dev 由
 * `vite-local-connectors.ts` 的 /api/kiro/usage 中间件转发）。
 * 解析其文本输出（Credits / 重置日 / 套餐标签）。
 *
 * 注意：`/usage` 是 kiro-cli chat 内的 slash 命令，需登录态。
 *
 * Token 用量：Kiro CLI 的本地会话（`~/.kiro/sessions/cli/*.jsonl`）不记录真实
 * token 数，故由 /api/kiro/stats 中间件读取会话文本、用 estimateTokens 估算后按天聚合。
 */
export class KiroConnector implements Connector, QuotaConnector, TokenConnector {
  readonly id = "kiro";
  private readonly baseUrl = "/api/kiro";

  platformId(): string {
    return "kiro";
  }

  isConfigured(): boolean {
    return true;
  }

  async fetchQuotas(): Promise<QuotaSnapshot[]> {
    const collectedAt = new Date().toISOString();
    const payload = (await httpGet({
      baseUrl: this.baseUrl,
      path: "/usage",
    }).catch((e: unknown) => {
      throw new ConnectorError(this.id, "拉取 Kiro Credits 失败", e);
    })) as KiroUsagePayload;

    if (payload?.error) {
      throw new ConnectorError(this.id, String(payload.error));
    }
    if (payload.used == null || payload.total == null) {
      throw new ConnectorError(this.id, "未解析到 Credits 数据");
    }

    return [
      {
        platform: "kiro" satisfies Platform,
        accountName: payload.planTag ?? "Kiro",
        metric: "credits",
        used: payload.used,
        limit: payload.total,
        unit: "credits",
        resetsAt: payload.resetsAt,
        collectedAt,
      },
    ];
  }

  async fetchTokens(range: { start: string; end: string }): Promise<TokenDailyUsage[]> {
    const collectedAt = new Date().toISOString();
    const payload = (await httpGet({
      baseUrl: this.baseUrl,
      path: "/stats",
      query: range,
    }).catch((e: unknown) => {
      throw new ConnectorError(this.id, "拉取 Kiro Token 用量失败", e);
    })) as KiroTokenRow[];

    const rows = Array.isArray(payload) ? payload : [];
    return rows.map((row) => ({
      platform: "kiro",
      date: row.d,
      inputTokens: num(row.inp),
      outputTokens: num(row.outp),
      cachedTokens: row.cache != null ? num(row.cache) : undefined,
      requestCount: row.requests != null ? num(row.requests) : undefined,
      collectedAt,
    }));
  }
}

interface KiroTokenRow {
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

interface KiroUsagePayload {
  error?: string;
  used?: number;
  total?: number;
  planTag?: string;
  resetsAt?: string;
}
