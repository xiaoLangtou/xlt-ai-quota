import type { Platform, QuotaSnapshot, TokenDailyUsage } from "@/types/usage";
import {
  type Connector,
  type QuotaConnector,
  type TokenConnector,
  ConnectorError,
  httpGet,
} from "./types";

/**
 * Kimi Connector（Kimi Code 会员额度）
 *
 * 数据来源：`https://api.kimi.com/coding/v1/usages`（global 区域为 api.kimi.ai），
 * OAuth token 取自 `~/.kimi-code/credentials/kimi-code.json`（dev 由 /api/kimi/usage
 * 中间件转发，Node 端负责过期刷新与回写）。
 * 返回 5 小时滚动窗口、月 Code 额度与月总额度的 used_ratio + reset_time。
 */
export class KimiConnector implements Connector, QuotaConnector, TokenConnector {
  readonly id = "kimi";
  private readonly baseUrl = "/api/kimi";

  platformId(): string {
    return "kimi";
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
      throw new ConnectorError(this.id, "拉取 Kimi 额度失败", e);
    })) as KimiUsagePayload;

    if (payload?.error) {
      throw new ConnectorError(this.id, String(payload.error));
    }

    const accountName = payload.planTag || "Kimi";
    const snaps: QuotaSnapshot[] = [];
    if (payload.fiveHour) {
      snaps.push(this.toSnapshot(accountName, "five_hour", payload.fiveHour, collectedAt));
    }
    if (payload.monthlyCode) {
      snaps.push(this.toSnapshot(accountName, "monthly_code", payload.monthlyCode, collectedAt));
    }
    if (payload.monthlyTotal) {
      snaps.push(this.toSnapshot(accountName, "monthly", payload.monthlyTotal, collectedAt));
    }
    if (snaps.length === 0) {
      throw new ConnectorError(this.id, "响应缺少额度数据");
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
      throw new ConnectorError(this.id, "拉取 Kimi Token 用量失败", e);
    })) as KimiTokenRow[];

    const rows = Array.isArray(payload) ? payload : [];
    return rows.map((row) => ({
      platform: "kimi",
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
      platform: "kimi" satisfies Platform,
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

interface KimiTokenRow {
  d: string;
  model?: string;
  inp?: number | string;
  outp?: number | string;
  cache?: number | string;
  requests?: number | string;
}

function num(value: number | string | undefined): number {
  return value == null ? 0 : Number(value) || 0;
}

interface KimiWindow {
  usedPercent: number;
  resetsAt: string;
}

interface KimiUsagePayload {
  error?: string;
  planTag?: string;
  fiveHour?: KimiWindow;
  monthlyCode?: KimiWindow;
  monthlyTotal?: KimiWindow;
}
