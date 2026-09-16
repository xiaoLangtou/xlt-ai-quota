import type { Platform, QuotaSnapshot, TokenDailyUsage } from "@/types/usage";
import {
  type Connector,
  type QuotaConnector,
  type TokenConnector,
  ConnectorError,
  httpGet,
} from "./types";

/**
 * Codex Connector（ChatGPT /wham/usage）
 *
 * 数据来源：ChatGPT 官方 `https://chatgpt.com/backend-api/wham/usage`，
 * token 取自 `~/.codex/auth.json`（dev 由 /api/codex/usage 中间件转发，Node 端调用绕过 CORS/Cloudflare）。
 * 返回 5h（primary_window）与 weekly（secondary_window）的 used_percent + reset_at。
 */
export class CodexConnector implements Connector, QuotaConnector, TokenConnector {
  readonly id = "codex";
  private readonly baseUrl = "/api/codex";

  platformId(): string {
    return "codex";
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
      throw new ConnectorError(this.id, "拉取 Codex 额度失败", e);
    })) as CodexUsagePayload;

    if (payload?.error) {
      throw new ConnectorError(this.id, String(payload.error));
    }

    const accountName = payload.planTag || "Codex";
    const snaps: QuotaSnapshot[] = [];
    if (payload.primary) {
      snaps.push(this.toSnapshot(accountName, "five_hour", payload.primary, collectedAt));
    }
    if (payload.secondary) {
      snaps.push(this.toSnapshot(accountName, "weekly", payload.secondary, collectedAt));
    }
    if (snaps.length === 0) {
      throw new ConnectorError(this.id, "响应缺少 primary_window 额度数据");
    }
    return snaps;
  }

  async fetchTokens(range: { start: string; end: string }): Promise<TokenDailyUsage[]> {
    const collectedAt = new Date().toISOString();
    const payload = (await httpGet({
      baseUrl: this.baseUrl,
      path: "/stats",
      query: range,
    }).catch((e: unknown) => {
      throw new ConnectorError(this.id, "拉取 Codex Token 用量失败", e);
    })) as CodexTokenRow[];

    const rows = Array.isArray(payload) ? payload : [];
    return rows.map((row) => ({
      platform: "codex",
      date: row.d,
      model: row.model || undefined,
      inputTokens: num(row.inp),
      outputTokens: num(row.outp),
      cachedTokens: row.cache != null ? num(row.cache) : undefined,
      requestCount: row.requests != null ? num(row.requests) : undefined,
      collectedAt,
    }));
  }

  private toSnapshot(
    accountName: string,
    metric: QuotaSnapshot["metric"],
    w: { usedPercent: number; resetsAt: string },
    collectedAt: string,
  ): QuotaSnapshot {
    return {
      platform: "codex" satisfies Platform,
      accountName,
      metric,
      used: w.usedPercent,
      limit: 100,
      unit: "percent",
      resetsAt: w.resetsAt,
      collectedAt,
    };
  }
}

interface CodexTokenRow {
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

interface CodexWindow {
  usedPercent: number;
  windowSeconds: number;
  resetsAt: string;
}
interface CodexUsagePayload {
  error?: string;
  email?: string;
  planType?: string;
  planTag?: string;
  primary?: CodexWindow;
  secondary?: CodexWindow;
}
