import { settings } from "@/config/settings";
import type {
  Platform,
  QuotaMetric,
  QuotaSnapshot,
  TokenDailyUsage,
} from "@/types/usage";
import {
  type Connector,
  type QuotaConnector,
  ConnectorError,
  type TokenConnector,
  httpGet,
} from "./types";

/**
 * 火山方舟 Connector
 *
 * 实现：通过本机 arkcli 拉取（dev 期由 Vite 中间件 /api/ark/* 转发；
 * 生产期 Tauri 可改走 Rust shell 或自实现 Volc 签名）。
 *
 * 数据来源：
 * - 套餐额度：arkcli usage plan（OpenAPI GetAFPUsage / GetCodingPlanUsage 等）
 * - Token 用量：arkcli usage stats --mine（BFF 聚合管道，5-30 分钟延迟）
 *
 * 认证由 arkcli 的 SSO profile 负责，无需在应用内保存 Ark 凭证。
 */
export class ArkConnector
  implements Connector, QuotaConnector, TokenConnector
{
  readonly id = "ark";

  platformId(): string {
    return "ark";
  }

  /** arkcli 路径在 dev 期始终可用；真实鉴权在 fetch 时由 arkcli 校验 */
  isConfigured(): boolean {
    return true;
  }

  private get baseUrl(): string {
    return settings.getArkConfig().baseUrl;
  }

  async fetchQuotas(): Promise<QuotaSnapshot[]> {
    const collectedAt = new Date().toISOString();
    const payload = (await httpGet({
      baseUrl: this.baseUrl,
      path: "/plan",
    }).catch((e: unknown) => {
      throw new ConnectorError(this.id, "拉取套餐额度失败", e);
    })) as ArkPlanPayload;

    if (payload?.error) {
      throw new ConnectorError(this.id, String(payload.error));
    }

    const snaps: QuotaSnapshot[] = [];
    for (const item of payload?.items ?? []) {
      if (!item.subscribed) continue;
      const product = item.product ?? "ark";
      for (const per of item.periods ?? []) {
        const metric = toMetric(per.label);
        if (!metric) continue;
        const percent =
          typeof per.percent === "number"
            ? Math.round(per.percent)
            : per.total
              ? Math.round((num(per.used) / num(per.total)) * 100)
              : 0;
        snaps.push({
          platform: "ark" satisfies Platform,
          accountName: product,
          metric,
          used: percent,
          limit: 100,
          unit: "percent",
          resetsAt: per.reset_at,
          collectedAt,
        });
      }
    }
    return snaps;
  }

  async fetchTokens(range: { start: string; end: string }): Promise<TokenDailyUsage[]> {
    const collectedAt = new Date().toISOString();
    const payload = (await httpGet({
      baseUrl: this.baseUrl,
      path: "/stats",
      query: { start: range.start, end: range.end },
    }).catch((e: unknown) => {
      throw new ConnectorError(this.id, "拉取 Token 用量失败", e);
    })) as ArkStatsPayload;

    if (payload?.error) {
      throw new ConnectorError(this.id, String(payload.error));
    }

    const records = payload?.records ?? [];
    return records.map((r) => ({
      platform: "ark",
      date: r.Day,
      model: r.ModelName,
      inputTokens: num(r.InputTokens),
      outputTokens: num(r.OutputTokens),
      cachedTokens: r.CacheTokensHit != null ? num(r.CacheTokensHit) : undefined,
      requestCount: r.ReqCnt != null ? num(r.ReqCnt) : undefined,
      collectedAt,
    }));
  }
}

function toMetric(label: string | undefined): QuotaMetric | null {
  switch (label) {
    case "session":
      return "session";
    case "5h":
      return "five_hour";
    case "weekly":
      return "weekly";
    case "monthly":
      return "monthly";
    default:
      return null;
  }
}

function num(v: number | string | undefined): number {
  if (v == null) return 0;
  return typeof v === "string" ? Number.parseInt(v, 10) || 0 : v;
}

interface ArkPeriod {
  label?: string;
  used?: number | string;
  total?: number | string;
  percent?: number;
  reset_at?: string;
}
interface ArkPlanItem {
  product?: string;
  subscribed?: boolean;
  periods?: ArkPeriod[];
}
interface ArkPlanPayload {
  error?: string;
  items?: ArkPlanItem[];
}
interface ArkStatsRow {
  Day: string;
  InputTokens?: number | string;
  OutputTokens?: number | string;
  CacheTokensHit?: number | string;
  ReqCnt?: number | string;
  ModelName?: string;
}
interface ArkStatsPayload {
  error?: string;
  records?: ArkStatsRow[];
  data_count?: number;
}
