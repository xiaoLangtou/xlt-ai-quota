import { invoke } from "@tauri-apps/api/core";
import type { QuotaSnapshot, QueueBucket, TokenDailyUsage } from "@/types/usage";

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
  /** tz：可选 IANA 时区，用于服务端按该时区分桶（空=系统本地）。 */
  fetchTokens(range: { start: string; end: string; tz?: string }): Promise<TokenDailyUsage[]>;
  /** 可选：直接返回半小时桶（保留小时粒度）。 */
  fetchBuckets?(range: { start: string; end: string; tz?: string }): Promise<QueueBucket[]>;
  /** true 表示采集器已在 Rust 侧落盘，同步时只需重新加载。 */
  readonly selfPersisting?: boolean;
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
  query?: Record<string, string | undefined>;
}

/** Tauri 注入该标记；非桌面环境没有本地连接器。 */
export function isTauriDesktop(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** 本地连接器统一由 Tauri command 执行；浏览器模式没有采集能力。 */
type ConnectorMock = (opts: HttpGetOptions) => Promise<unknown>;

export async function httpGet(opts: HttpGetOptions): Promise<unknown> {
  // 测试 seam：单元冒烟在 Node 下注入本地返回，不依赖 Tauri 运行时。
  const mock = (globalThis as { __XLT_CONNECTOR_MOCK__?: ConnectorMock })
    .__XLT_CONNECTOR_MOCK__;
  if (mock) return mock(opts);
  if (!isTauriDesktop()) {
    throw new Error("本地连接器仅在桌面端可用，请使用 pnpm tauri dev 启动");
  }
  try {
    return await invoke<unknown>("connector_get", {
      request: {
        baseUrl: opts.baseUrl,
        path: opts.path,
        query: opts.query ?? {},
      },
    });
  } catch (err) {
    throw new Error(`桌面端本地连接器失败: ${String(err)}`);
  }
}
