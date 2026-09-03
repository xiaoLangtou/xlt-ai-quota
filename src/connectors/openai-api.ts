import { settings } from "@/config/settings";
import type { TokenDailyUsage } from "@/types/usage";
import {
  type Connector,
  ConnectorError,
  type TokenConnector,
  httpGet,
} from "./types";

/**
 * OpenAI API Connector
 *
 * 数据来源：组织级 Usage API（completions）。
 * 文档：https://developers.openai.com/api/reference/resources/admin/subresources/organization/subresources/usage
 *
 * 需要组织级 Admin Key。Codex/ChatGPT 订阅额度不在本 connector 范围（第三阶段浏览器连接器）。
 */
export class OpenAIApiConnector implements Connector, TokenConnector {
  readonly id = "openai-api";

  platformId(): string {
    return "openai";
  }

  isConfigured(): boolean {
    return Boolean(settings.getOpenAIConfig().adminKey);
  }

  private authHeaders(): Record<string, string> {
    const { adminKey, orgId } = settings.getOpenAIConfig();
    if (!adminKey) throw new ConnectorError(this.id, "未配置 OpenAI Admin Key");
    const headers: Record<string, string> = {
      Authorization: `Bearer ${adminKey}`,
    };
    if (orgId) headers["OpenAI-Organization"] = orgId;
    return headers;
  }

  async fetchTokens(range: { start: string; end: string }): Promise<TokenDailyUsage[]> {
    const { baseUrl } = settings.getOpenAIConfig();
    const collectedAt = new Date().toISOString();
    const payload = (await httpGet({
      baseUrl,
      path: "/v1/organization/usage/completions",
      headers: this.authHeaders(),
      query: {
        start_date: range.start,
        end_date: range.end,
        bucket_width: "1d",
      },
    }).catch((e: unknown) => {
      throw new ConnectorError(this.id, "拉取 OpenAI Token 用量失败", e);
    })) as { data?: OpenAIUsageRow[] };

    // 按天聚合（API 按模型分组，看板只需日级总览，但仍保留 model 维度）
    const byKey = new Map<string, TokenDailyUsage>();
    for (const row of payload.data ?? []) {
      const key = `${row.date}|${row.model ?? ""}`;
      const cur =
        byKey.get(key) ??
        ({
          platform: "openai",
          date: row.date,
          model: row.model,
          inputTokens: 0,
          outputTokens: 0,
          collectedAt,
        } satisfies TokenDailyUsage);
      cur.inputTokens += num(row.input_tokens);
      cur.outputTokens += num(row.output_tokens);
      if (row.cached_tokens != null) {
        cur.cachedTokens = (cur.cachedTokens ?? 0) + num(row.cached_tokens);
      }
      cur.requestCount = (cur.requestCount ?? 0) + num(row.num_requests);
      byKey.set(key, cur);
    }
    return [...byKey.values()];
  }
}

interface OpenAIUsageRow {
  date: string; // YYYY-MM-DD
  model?: string;
  input_tokens?: number | string;
  output_tokens?: number | string;
  cached_tokens?: number | string;
  num_requests?: number | string;
}

function num(v: number | string | undefined): number {
  if (v == null) return 0;
  return typeof v === "string" ? Number.parseInt(v, 10) || 0 : v;
}
