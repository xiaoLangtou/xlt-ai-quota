import type { TokenDailyUsage } from "@/types/usage";
import {
  type Connector,
  ConnectorError,
  type TokenConnector,
  httpGet,
} from "./types";

/**
 * OpenCode Go Token Connector
 *
 * 数据来源：本机 opencode 的 SQLite（`opencode db`），聚合 assistant 消息的
 * usage 字段（input/output/cache tokens），按本地日分组。
 *
 * 这对应产品方案中"请求返回的 usage 字段"数据源。
 * 订阅额度（5h/$12 等美元窗口）不在本地 DB，仍需浏览器连接器或官方 API。
 */
export class OpenCodeConnector implements Connector, TokenConnector {
  readonly id = "opencode";
  private readonly baseUrl = "/api/opencode";

  platformId(): string {
    return "opencode-go";
  }

  isConfigured(): boolean {
    return true;
  }

  async fetchTokens(range: { start: string; end: string; tz?: string }): Promise<TokenDailyUsage[]> {
    const collectedAt = new Date().toISOString();
    const payload = (await httpGet({
      baseUrl: this.baseUrl,
      path: "/stats",
      query: { tz: range.tz },
    }).catch((e: unknown) => {
      throw new ConnectorError(this.id, "拉取 OpenCode Token 用量失败", e);
    })) as OpenCodeRow[];

    const rows = Array.isArray(payload) ? payload : [];
    return rows
      .filter((r) => r.d >= range.start && r.d <= range.end)
      .map((r) => ({
        platform: "opencode-go",
        date: r.d,
        model: r.model || undefined,
        inputTokens: num(r.inp),
        outputTokens: num(r.outp),
        cachedTokens: r.cache != null ? num(r.cache) : undefined,
        collectedAt,
      }));
  }
}

interface OpenCodeRow {
  d: string;
  model?: string;
  inp?: number | string;
  outp?: number | string;
  cache?: number | string;
}

function num(v: number | string | undefined): number {
  if (v == null) return 0;
  return typeof v === "string" ? Number.parseInt(v, 10) || 0 : v;
}
