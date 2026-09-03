import { invoke } from "@tauri-apps/api/core";
import type { QuotaSnapshot, TokenDailyUsage } from "@/types/usage";

export interface Connector {
  readonly id: string;
  /** 该 connector 产出的数据归属平台（用于同步时覆盖该平台旧数据） */
  platformId(): string;
  /** 是否已配置可用凭证（未配置时 service 跳过该 connector） */
  isConfigured(): boolean;
}

export interface QuotaConnector extends Connector {
  fetchQuotas(): Promise<QuotaSnapshot[]>;
}

export interface TokenConnector extends Connector {
  fetchTokens(range: { start: string; end: string }): Promise<TokenDailyUsage[]>;
}

export class ConnectorError extends Error {
  constructor(
    public readonly connectorId: string,
    message: string,
    public readonly cause?: unknown,
  ) {
    super(message);
    this.name = "ConnectorError";
  }
}

interface HttpGetOptions {
  baseUrl: string;
  path: string;
  headers?: Record<string, string>;
  query?: Record<string, string | undefined>;
}

/** Tauri 注入该标记；浏览器开发模式继续使用 Vite 本地中间件与代理。 */
export function isTauriDesktop(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** 简单 GET，桌面包交由受限的 Tauri command 执行，浏览器模式维持 HTTP 请求。 */
export async function httpGet(opts: HttpGetOptions): Promise<unknown> {
  if (isTauriDesktop()) {
    try {
      return await invoke<unknown>("connector_get", {
        request: {
          baseUrl: opts.baseUrl,
          path: opts.path,
          headers: opts.headers ?? {},
          query: opts.query ?? {},
        },
      });
    } catch (err) {
      throw new Error(`桌面端本地连接器失败: ${String(err)}`);
    }
  }

  let search = "";
  if (opts.query) {
    const params = new URLSearchParams();
    for (const [k, v] of Object.entries(opts.query)) {
      if (v !== undefined && v !== "") params.set(k, v);
    }
    const s = params.toString();
    if (s) search = `?${s}`;
  }
  const url = `${opts.baseUrl.replace(/\/$/, "")}${opts.path}${search}`;
  let resp: Response;
  try {
    resp = await fetch(url, { headers: opts.headers });
  } catch (err) {
    throw new Error(`网络请求失败: ${String(err)}`);
  }
  const text = await resp.text();
  if (!resp.ok) {
    const snippet = text.slice(0, 300);
    throw new Error(`HTTP ${resp.status}: ${snippet}`);
  }
  try {
    return JSON.parse(text);
  } catch {
    throw new Error(`响应不是合法 JSON: ${text.slice(0, 300)}`);
  }
}
