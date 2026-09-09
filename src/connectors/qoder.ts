import type { Platform, QuotaSnapshot, TokenDailyUsage } from "@/types/usage";
import {
  type Connector,
  type QuotaConnector,
  type TokenConnector,
  ConnectorError,
  httpGet,
} from "./types";

/**
 * Qoder Connector
 *
 * 数据来源：官方 Qoder Agent SDK 的 getUsageInfo()。SDK 复用本机 qodercli
 * 登录态，读取用量面板的套餐 Credits、加购 Credits 与套餐到期时间。
 * 开发期由 Vite 本地中间件调用 SDK，浏览器不会接触登录凭证。
 *
 * Token 用量：Qoder 桌面端把每条 assistant 消息的真实 token 存在本地 SQLite
 * （SharedClientCache/.../local.db 的 chat_message.token_info），由 /api/qoder/stats
 * 中间件读取并按天聚合（真实计数，非估算）。
 */
export class QoderConnector implements Connector, QuotaConnector, TokenConnector {
  readonly id = "qoder";
  private readonly baseUrl = "/api/qoder";

  platformId(): string {
    return "qoder";
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
      throw new ConnectorError(this.id, "拉取 Qoder Credits 失败", e);
    })) as QoderUsagePayload;

    if (payload.error) throw new ConnectorError(this.id, payload.error);
    if (payload.used == null || payload.total == null) {
      throw new ConnectorError(this.id, "未读取到套餐 Credits");
    }

    const snapshots: QuotaSnapshot[] = [{
      platform: "qoder" satisfies Platform,
      accountName: payload.planTag ?? "Qoder",
      metric: "credits",
      used: payload.used,
      limit: payload.total,
      unit: "credits",
      resetsAt: payload.expiresAt,
      collectedAt,
    }];
    if (payload.addOnUsed != null && payload.addOnTotal != null) {
      snapshots.push({
        platform: "qoder" satisfies Platform,
        accountName: payload.planTag ?? "Qoder",
        metric: "addon_credits",
        used: payload.addOnUsed,
        limit: payload.addOnTotal,
        unit: "credits",
        resetsAt: payload.expiresAt,
        collectedAt,
      });
    }
    return snapshots;
  }

  async fetchTokens(range: { start: string; end: string }): Promise<TokenDailyUsage[]> {
    const collectedAt = new Date().toISOString();
    const payload = (await httpGet({
      baseUrl: this.baseUrl,
      path: "/stats",
      query: range,
    }).catch((e: unknown) => {
      throw new ConnectorError(this.id, "拉取 Qoder Token 用量失败", e);
    })) as QoderTokenRow[];

    const rows = Array.isArray(payload) ? payload : [];
    return rows.map((row) => ({
      platform: "qoder",
      date: row.d,
      inputTokens: num(row.inp),
      outputTokens: num(row.outp),
      cachedTokens: row.cache != null ? num(row.cache) : undefined,
      requestCount: row.requests != null ? num(row.requests) : undefined,
      collectedAt,
    }));
  }
}

interface QoderTokenRow {
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

interface QoderUsagePayload {
  error?: string;
  used?: number;
  total?: number;
  remaining?: number;
  planTag?: string;
  expiresAt?: string;
  addOnUsed?: number;
  addOnTotal?: number;
}
