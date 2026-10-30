import { settings } from "@/config/settings";
import type { Platform, QuotaBalance, QuotaSnapshot, TokenDailyUsage } from "@/types/usage";
import {
  type Connector,
  type QuotaConnector,
  type TokenConnector,
  ConnectorError,
  httpGet,
} from "./types";

/**
 * DeepSeek Connector
 *
 * 额度（费用中心）：官方 `GET https://api.deepseek.com/user/balance`，
 * 返回账户总余额 / 赠金 / 充值余额（CNY 或 USD）。需 API Key，仅本机保存；
 * 未配置 Key 时不展示余额卡片，也不报错。
 *
 * Token（分析）：本机 DeepSeek Harness（DSH）客户端会话
 * `~/.dsh/sessions` 下的 `session*.jsonl.zstd`：`assistant/message` 事件内
 * `data.usage`（inputTokens/outputTokens/cacheReadTokens）为模型返回的真实用量；
 * 旧版会话无 usage 时用 DeepSeek 分词器估算。dev 由 /api/deepseek/* 中间件转发，
 * 桌面端由 Rust 侧对应实现处理。
 */
export class DeepSeekConnector implements Connector, QuotaConnector, TokenConnector {
  readonly id = "deepseek";
  private readonly baseUrl = "/api/deepseek";

  platformId(): string {
    return "deepseek";
  }

  isConfigured(): boolean {
    return true;
  }

  async fetchQuotas(): Promise<QuotaSnapshot[]> {
    const { apiKey } = settings.getDeepseekConfig();
    if (!apiKey) return [];
    const collectedAt = new Date().toISOString();
    const payload = (await httpGet({
      baseUrl: this.baseUrl,
      path: "/balance",
      query: { apiKey },
    }).catch((e: unknown) => {
      throw new ConnectorError(this.id, "拉取 DeepSeek 余额失败", e);
    })) as DeepSeekBalancePayload;

    const balance = pickBalance(payload);
    if (!balance) {
      throw new ConnectorError(this.id, "响应缺少 balance_infos 余额数据");
    }

    return [
      {
        platform: "deepseek" satisfies Platform,
        accountName: balance.currency,
        metric: "balance",
        used: 0,
        limit: balance.total,
        unit: "currency",
        collectedAt,
        balance,
      },
    ];
  }

  async fetchTokens(range: { start: string; end: string; tz?: string }): Promise<TokenDailyUsage[]> {
    const collectedAt = new Date().toISOString();
    const payload = (await httpGet({
      baseUrl: this.baseUrl,
      path: "/stats",
      query: range,
    }).catch((e: unknown) => {
      throw new ConnectorError(this.id, "拉取 DeepSeek Token 用量失败", e);
    })) as DeepSeekTokenRow[];

    const rows = Array.isArray(payload) ? payload : [];
    return rows.map((row) => ({
      platform: "deepseek",
      date: row.d,
      project: row.project || undefined,
      model: row.model || undefined,
      inputTokens: num(row.inp),
      outputTokens: num(row.outp),
      cachedTokens: row.cache != null ? num(row.cache) : undefined,
      cacheWriteTokens: row.cacheWrite != null ? num(row.cacheWrite) : undefined,
      reasoningTokens: row.reasoning != null ? num(row.reasoning) : undefined,
      requestCount: row.requests != null ? num(row.requests) : undefined,
      collectedAt,
    }));
  }
}

interface DeepSeekTokenRow {
  d: string;
  project?: string;
  model?: string;
  inp?: number | string;
  outp?: number | string;
  cache?: number | string;
  cacheWrite?: number | string;
  reasoning?: number | string;
  requests?: number | string;
}

interface DeepSeekBalanceInfo {
  currency?: string;
  total_balance?: string | number;
  granted_balance?: string | number;
  topped_up_balance?: string | number;
}

interface DeepSeekBalancePayload {
  is_available?: boolean;
  balance_infos?: DeepSeekBalanceInfo[];
  error?: string;
}

/** 优先取 USD 余额，其次 CNY；两者都无则取首条。 */
function pickBalance(payload: DeepSeekBalancePayload | undefined): QuotaBalance | null {
  const infos = payload?.balance_infos;
  if (!Array.isArray(infos) || infos.length === 0) return null;
  const info =
    infos.find((i) => i?.currency === "USD") ?? infos.find((i) => i?.currency === "CNY") ?? infos[0];
  const currency = info.currency === "USD" ? "USD" : "CNY";
  const total = num(info.total_balance);
  return {
    total,
    currency,
    granted: num(info.granted_balance),
    toppedUp: num(info.topped_up_balance),
    available: payload?.is_available !== false,
  };
}

function num(value: number | string | undefined): number {
  if (value == null) return 0;
  const parsed = typeof value === "number" ? value : Number.parseFloat(value);
  return Number.isFinite(parsed) ? parsed : 0;
}
