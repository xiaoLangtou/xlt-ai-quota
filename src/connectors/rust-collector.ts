import { invoke } from "@tauri-apps/api/core";
import type { QueueBucket, TokenDailyUsage } from "@/types/usage";
import { aggregateBucketsToDaily, dayBoundsUtc } from "@/utils/usage-time";
import { isTauriDesktop, type Connector, type TokenConnector } from "./types";

interface CollectResult {
  source: string;
  buckets: QueueBucket[];
  eventsParsed: number;
  filesProcessed: number;
  skipped: boolean;
  error?: string | null;
}

/**
 * Rust 侧增量采集器：游标、去重和落盘都在 `~/.xlt-workbench` 完成。
 * 前端只负责触发一次采集并重新加载用量桶。
 */
export class RustCollectorConnector implements Connector, TokenConnector {
  readonly selfPersisting = true;

  constructor(
    readonly id: string,
    readonly source: string,
  ) {}

  platformId(): string {
    return this.source;
  }

  isConfigured(): boolean {
    return isTauriDesktop();
  }

  async fetchBuckets(range: { start: string; end: string }): Promise<QueueBucket[]> {
    const result = await invoke<CollectResult>("usage_collect", {
      source: this.source,
      since: dayBoundsUtc(range.start).start,
      until: dayBoundsUtc(range.end).end,
    });
    return result.buckets ?? [];
  }

  async fetchTokens(range: { start: string; end: string; tz?: string }): Promise<TokenDailyUsage[]> {
    const buckets = await this.fetchBuckets(range);
    return aggregateBucketsToDaily(buckets, range.tz);
  }
}
