import type { Platform, QuotaSnapshot } from "@/types/usage";
import {
  type Connector,
  type QuotaConnector,
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
 * Token 用量不在此 connector 范围（Kiro 用 Credits，不参与 Token 聚合）。
 */
export class KiroConnector implements Connector, QuotaConnector {
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
}

interface KiroUsagePayload {
  error?: string;
  used?: number;
  total?: number;
  planTag?: string;
  resetsAt?: string;
}
