import type { Platform, QuotaMetric, QuotaSnapshot } from "@/types/usage";
import {
  type Connector,
  type QuotaConnector,
  ConnectorError,
  httpGet,
} from "./types";

interface DevinLimit {
  id: string;
  label?: string;
  usedPercent?: number;
  resetsAt?: string | null;
}

interface DevinUsagePayload {
  planTag?: string;
  tier?: string;
  plan?: string;
  email?: string;
  membership?: string;
  limits?: DevinLimit[];
  message?: string;
  error?: string;
}

/**
 * Devin 套餐连接器。
 *
 * 本机 `devin auth status` 能给出 Tier / Plan / 邮箱，但没有可读的额度数值；
 * 这里先落一张「套餐信息」卡，等官方提供只读 usage 接口后再补 limits。
 */
export class DevinConnector implements Connector, QuotaConnector {
  readonly id = "devin";
  private readonly baseUrl = "/api/devin";

  platformId(): string {
    return "devin";
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
      throw new ConnectorError(this.id, "拉取 Devin 套餐信息失败", e);
    })) as DevinUsagePayload;

    if (payload?.error) {
      throw new ConnectorError(this.id, String(payload.error));
    }
    const planTag = payload.planTag || payload.tier || payload.plan || "Devin";
    const limits = payload.limits ?? [];
    if (limits.length > 0) {
      return limits.map((limit) => ({
        platform: "devin" satisfies Platform,
        accountName: planTag,
        metric: limit.id as QuotaMetric,
        used: limit.usedPercent ?? 0,
        limit: 100,
        unit: "percent",
        resetsAt: limit.resetsAt ?? undefined,
        collectedAt,
      }));
    }
    return [
      {
        platform: "devin" satisfies Platform,
        accountName: planTag,
        metric: "account",
        used: 0,
        limit: 0,
        unit: "percent",
        collectedAt,
      },
    ];
  }
}
