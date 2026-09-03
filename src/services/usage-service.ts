import { settings } from "@/config/settings";
import { createStorage } from "@/storage/web-storage";
import type { UsageStorage } from "@/storage/storage";
import {
  type Platform,
  PLATFORM_META,
  type PlatformQuotaView,
  type QuotaMetric,
  type QuotaSnapshot,
  type RangePreset,
  type SyncInfo,
  type TokenSummary,
  type TrendPoint,
  type PlatformDailyPoint,
  type QuotaWindowView,
} from "@/types/usage";
import { ArkConnector } from "@/connectors/ark";
import { OpenAIApiConnector } from "@/connectors/openai-api";
import { CodexConnector } from "@/connectors/codex";
import { KiroConnector } from "@/connectors/kiro";
import { OpenCodeConnector } from "@/connectors/opencode";
import { ClaudeConnector } from "@/connectors/claude";
import {
  ConnectorError,
  type Connector,
  type QuotaConnector,
  type TokenConnector,
} from "@/connectors/types";

interface Range {
  start: string; // YYYY-MM-DD
  end: string; // YYYY-MM-DD
}

const WINDOW_SPEC: Record<
  Platform,
  { metric: QuotaMetric; label: string; tone: QuotaWindowView["tone"] }[]
> = {
  ark: [], // 动态：按快照实际周期渲染（coding-plan 为 session/weekly/monthly，agent-plan 为 5h/weekly/monthly）
  codex: [], // 动态：five_hour + weekly（来自 /wham/usage）
  kiro: [{ metric: "credits", label: "本周期已消耗", tone: "purple" }],
  "opencode-go": [
    { metric: "five_hour", label: "5 小时额度", tone: "green" },
    { metric: "weekly", label: "周额度", tone: "green" },
  ],
};

/** 各周期的展示元数据与规范排序；ark 卡片按此顺序取前 2 个可用周期 */
const METRIC_META: Record<
  QuotaMetric,
  { label: string; tone: QuotaWindowView["tone"]; order: number }
> = {
  session: { label: "Session 额度", tone: "blue", order: 0 },
  five_hour: { label: "5 小时额度", tone: "blue", order: 1 },
  weekly: { label: "周额度", tone: "blue", order: 2 },
  monthly: { label: "月额度", tone: "blue", order: 3 },
  credits: { label: "本周期已消耗", tone: "purple", order: 4 },
  account: { label: "账户", tone: "blue", order: 5 },
};

/** 火山方舟套餐页使用的周期名称，与控制台保持一致。 */
const ARK_METRIC_META: Record<
  "session" | "five_hour" | "weekly" | "monthly",
  (typeof METRIC_META)[QuotaMetric]
> = {
  session: { label: "当前会话", tone: "blue", order: 0 },
  five_hour: { label: "当前会话", tone: "blue", order: 1 },
  weekly: { label: "近 1 周", tone: "blue", order: 2 },
  monthly: { label: "近 1 月", tone: "blue", order: 3 },
};

export class UsageService {
  private readonly storage: UsageStorage;
  private readonly quotaConnectors: (Connector & QuotaConnector)[];
  private readonly tokenConnectors: (Connector & TokenConnector)[];

  /** 数据版本：变更时清空本地旧数据（含历史种子默认值） */
  static readonly DATA_VERSION = "v2-no-seed";

  constructor(storage: UsageStorage = createStorage()) {
    this.storage = storage;
    const ark = new ArkConnector();
    const openai = new OpenAIApiConnector();
    const codex = new CodexConnector();
    const kiro = new KiroConnector();
    const opencode = new OpenCodeConnector();
    const claude = new ClaudeConnector();
    // Kiro 仅 Credits；Codex、OpenCode 与 Claude Code 的 Token 来自本机会话记录。
    this.quotaConnectors = [ark, codex, kiro];
    this.tokenConnectors = [ark, openai, codex, opencode, claude];
  }

  /** 数据版本变更时清空旧数据（含历史种子）；不再注入任何默认值 */
  ensureSeeded(): void {
    this.storage.resetIfVersionChanged(UsageService.DATA_VERSION);
  }

  // ---- 读：订阅额度 ----

  getPlatformQuotaViews(): PlatformQuotaView[] {
    const all = this.storage.listQuotas();
    const order: Platform[] = ["codex", "ark", "kiro"];
    return order.map((platform) => this.buildPlatformView(platform, all));
  }

  private buildPlatformView(
    platform: Platform,
    all: QuotaSnapshot[],
  ): PlatformQuotaView {
    const meta = PLATFORM_META[platform];
    const latest = this.latestByMetric(platform, all);
    const collectedAt =
      latest[0]?.collectedAt ?? new Date(0).toISOString();
    const view: PlatformQuotaView = {
      platform,
      name: meta.name,
      planTag: PLAN_TAG[platform],
      logoChar: meta.logoChar,
      logoClass: meta.logoClass,
      windows: [],
      collectedAt,
    };

    if (platform === "ark") {
      view.planTag = arkPlanTag(latest[0]?.accountName);
    }

    if (platform === "codex") {
      const snap = latest[0];
      if (snap?.accountName) view.planTag = snap.accountName;
    }

    if (platform === "kiro") {
      const c = latest.find((q) => q.metric === "credits");
      if (c) {
        view.planTag = c.accountName || PLAN_TAG.kiro;
        view.credits = {
          remaining: Math.max(0, c.limit - c.used),
          total: c.limit,
          refreshIn: formatRelative(c.resetsAt, "刷新"),
        };
      }
    }

    // 方舟展示会话、周、月三个套餐周期；Codex 维持两个核心周期。
    const specs =
      platform === "ark" || platform === "codex"
        ? latest
            .map((s) => s.metric)
            .filter((m) => m !== "account" && m !== "credits")
            .sort((a, b) => METRIC_META[a].order - METRIC_META[b].order)
            .slice(0, platform === "ark" ? 3 : 2)
            .map((metric) => ({
              metric,
              ...(platform === "ark" ? ARK_METRIC_META[metric] : METRIC_META[metric]),
            }))
        : WINDOW_SPEC[platform];

    for (const spec of specs) {
      const snap = latest.find((q) => q.metric === spec.metric);
      if (!snap) continue;
      const rawUsedPct =
        snap.unit === "percent"
          ? snap.used
          : snap.limit > 0
            ? (snap.used / snap.limit) * 100
            : 0;
      // Credits 的用量常远低于 1%，保留一位小数避免显示为 0%。
      const usedPct =
        spec.metric === "credits"
          ? Math.round(rawUsedPct * 10) / 10
          : Math.round(rawUsedPct);
      const warn = spec.metric !== "credits" && usedPct >= 80;
      view.windows.push({
        metric: spec.metric,
        label: spec.label,
        usedPct,
        usedText:
          snap.unit === "percent"
            ? `${usedPct}%`
            : `${formatNum(snap.used)} / ${formatNum(snap.limit)}`,
        resetsAt: snap.resetsAt,
        resetsIn: formatRelative(snap.resetsAt, "重置"),
        tone: warn && spec.tone !== "purple" ? "orange" : spec.tone,
      });
    }
    return view;
  }

  private latestByMetric(
    platform: Platform,
    all: QuotaSnapshot[],
  ): QuotaSnapshot[] {
    return all
      .filter((q) => q.platform === platform)
      .sort((a, b) => (a.collectedAt < b.collectedAt ? 1 : -1))
      .filter((q, i, arr) => arr.findIndex((x) => x.metric === q.metric) === i);
  }

  // ---- 读：Token ----

  getRange(preset: RangePreset): Range {
    const end = new Date();
    end.setHours(0, 0, 0, 0);
    const days = preset === "today" ? 0 : preset === "7d" ? 6 : 29;
    const start = new Date(end);
    start.setDate(start.getDate() - days);
    return { start: ymd(start), end: ymd(end) };
  }

  getTokenSummary(preset: RangePreset): TokenSummary {
    const range = this.getRange(preset);
    const rows = this.storage.listTokensByRange(range.start, range.end);
    const total = sum(rows, (r) => r.inputTokens + r.outputTokens);
    const input = sum(rows, (r) => r.inputTokens);
    const output = sum(rows, (r) => r.outputTokens);
    const cached = sum(rows, (r) => r.cachedTokens ?? 0);

    // 上一周期对比
    const prev = this.prevRange(range, preset);
    const prevRows = this.storage.listTokensByRange(prev.start, prev.end);
    const prevTotal = sum(prevRows, (r) => r.inputTokens + r.outputTokens);
    const deltaPct =
      prevTotal > 0 ? Math.round(((total - prevTotal) / prevTotal) * 1000) / 10 : undefined;

    return { total, input, output, cached: cached || undefined, deltaPct };
  }

  getTokenTrend(preset: RangePreset): TrendPoint[] {
    const range = this.getRange(preset);
    const rows = this.storage.listTokensByRange(range.start, range.end);
    const byDate = new Map<string, TrendPoint>();
    for (const r of rows) {
      const cur =
        byDate.get(r.date) ??
        ({ date: r.date, total: 0, input: 0, output: 0 } satisfies TrendPoint);
      cur.input += r.inputTokens;
      cur.output += r.outputTokens;
      cur.total += r.inputTokens + r.outputTokens;
      byDate.set(r.date, cur);
    }
    return fillMissingDays(range, byDate);
  }

  getPlatformDaily(preset: RangePreset): PlatformDailyPoint[] {
    const range = this.getRange(preset);
    const rows = this.storage.listTokensByRange(range.start, range.end);
    const byDate = new Map<string, PlatformDailyPoint>();
    for (const r of rows) {
      const cur =
        byDate.get(r.date) ??
        ({ date: r.date, byPlatform: {} } satisfies PlatformDailyPoint);
      cur.byPlatform[r.platform] =
        (cur.byPlatform[r.platform] ?? 0) + r.inputTokens + r.outputTokens;
      byDate.set(r.date, cur);
    }
    return [...byDate.values()].sort((a, b) => (a.date < b.date ? -1 : 1));
  }

  private prevRange(range: Range, preset: RangePreset): Range {
    if (preset === "today") {
      const start = new Date(range.start);
      start.setDate(start.getDate() - 1);
      return { start: ymd(start), end: ymd(start) };
    }
    const days = preset === "7d" ? 7 : 30;
    const end = new Date(range.start);
    end.setDate(end.getDate() - 1);
    const start = new Date(end);
    start.setDate(start.getDate() - (days - 1));
    return { start: ymd(start), end: ymd(end) };
  }

  // ---- 读：同步状态 ----

  getSyncInfo(): SyncInfo {
    return this.storage.getSyncInfo();
  }

  /** 已配置的 connector 列表（供设置面板展示） */
  getConnectorStatus(): { id: string; configured: boolean }[] {
    const seen = new Set<string>();
    const out: { id: string; configured: boolean }[] = [];
    for (const c of [...this.quotaConnectors, ...this.tokenConnectors]) {
      if (seen.has(c.id)) continue;
      seen.add(c.id);
      out.push({ id: c.id, configured: c.isConfigured() });
    }
    return out;
  }

  // ---- 写：同步 ----

  /** 菜单栏后台启动只拉取订阅额度，避免启动时扫描完整 Token 历史。 */
  async syncQuotas(onProgress?: () => void): Promise<void> {
    const errors: string[] = [];
    let successful = 0;
    const collectedAt = new Date().toISOString();
    const waitForConnector = <T>(connectorId: string, operation: Promise<T>): Promise<T> =>
      new Promise((resolve, reject) => {
        const timer = window.setTimeout(() => {
          reject(new Error(`${connectorId} 超过 25 秒未完成`));
        }, 25_000);
        operation.then(
          (value) => {
            window.clearTimeout(timer);
            resolve(value);
          },
          (error) => {
            window.clearTimeout(timer);
            reject(error);
          },
        );
      });

    await Promise.all(
      this.quotaConnectors
        .filter((connector) => connector.isConfigured())
        .map(async (connector) => {
          try {
            const snapshots = await waitForConnector(connector.id, connector.fetchQuotas());
            if (snapshots.length) this.storage.saveQuotas(snapshots);
            successful += 1;
          } catch (error) {
            errors.push(`${connector.id}: ${msg(error)}`);
          } finally {
            onProgress?.();
          }
        }),
    );

    const previous = this.storage.getSyncInfo();
    this.storage.saveSyncInfo({
      lastSyncAt: successful ? collectedAt : previous.lastSyncAt,
      syncing: false,
      outcome: errors.length ? (successful ? "partial" : "failed") : "success",
      error: errors.length
        ? `${successful ? "部分连接器同步失败" : "没有连接器同步成功"}：${errors.join("; ")}`
        : undefined,
    });
  }

  /** 运行所有已配置的 connector，持久化结果并更新同步时间 */
  async syncAll(onProgress?: () => void): Promise<void> {
    const errors: string[] = [];
    let successful = 0;
    const collectedAt = new Date().toISOString();
    const waitForConnector = <T>(connectorId: string, operation: Promise<T>): Promise<T> =>
      new Promise((resolve, reject) => {
        const timer = window.setTimeout(() => {
          reject(new Error(`${connectorId} 超过 25 秒未完成`));
        }, 25_000);
        operation.then(
          (value) => {
            window.clearTimeout(timer);
            resolve(value);
          },
          (error) => {
            window.clearTimeout(timer);
            reject(error);
          },
        );
      });
    const reportProgress = () => {
      try {
        onProgress?.();
      } catch {
        // 界面刷新失败不应中断其他平台同步。
      }
    };

    const quotaTasks = this.quotaConnectors
      .filter((c) => c.isConfigured())
      .map(async (c) => {
        try {
          const snaps = await waitForConnector(c.id, c.fetchQuotas());
          if (snaps.length) this.storage.saveQuotas(snaps);
          successful += 1;
        } catch (e) {
          errors.push(`${c.id}: ${msg(e)}`);
        } finally {
          reportProgress();
        }
      });

    const tokenRange = this.getRange("30d");
    const tokenTasks = this.tokenConnectors
      .filter((c) => c.isConfigured())
      .map(async (c) => {
        try {
          const rows = await waitForConnector(c.id, c.fetchTokens(tokenRange));
          // 已成功取得的空数组代表该平台当前没有历史 Token，仍应覆盖旧数据。
          this.storage.replaceTokensForPlatform(
            c.platformId(),
            tokenRange.start,
            tokenRange.end,
            rows,
          );
          successful += 1;
        } catch (e) {
          errors.push(`${c.id}: ${msg(e)}`);
        } finally {
          reportProgress();
        }
      });

    await Promise.all([...quotaTasks, ...tokenTasks]);

    const previous = this.storage.getSyncInfo();
    this.storage.saveSyncInfo({
      lastSyncAt: successful ? collectedAt : previous.lastSyncAt,
      syncing: false,
      outcome: errors.length ? (successful ? "partial" : "failed") : "success",
      error: errors.length
        ? `${successful ? "部分连接器同步失败" : "没有连接器同步成功"}：${errors.join("; ")}`
        : undefined,
    });
  }

  setSyncing(flag: boolean): void {
    this.storage.saveSyncInfo({ ...this.storage.getSyncInfo(), syncing: flag });
  }

  // ---- 设置 ----

  getArkConfigView(): { baseUrl: string } {
    return settings.getArkConfig();
  }
  getOpenAIConfigView(): { adminKey: string; orgId: string; baseUrl: string } {
    return settings.getOpenAIConfig();
  }

  saveArkBaseUrl(baseUrl: string): void {
    settings.saveArkBaseUrl(baseUrl);
  }
  saveOpenAIConfig(patch: {
    adminKey?: string;
    orgId?: string;
    baseUrl?: string;
  }): void {
    settings.saveOpenAIConfig(patch);
  }
}

const PLAN_TAG: Record<Platform, string> = {
  codex: "Plus",
  ark: "企业版",
  kiro: "Pro",
  "opencode-go": "个人版",
};

/** 由 arkcli 的 product 字段推导 ark 卡片套餐标签 */
function arkPlanTag(accountName: string | undefined): string {
  const p = accountName ?? "";
  const team = p.includes("-team");
  if (p.includes("coding-plan")) return team ? "Coding Plan 团队" : "Coding Plan";
  if (p.includes("agent-plan")) return team ? "Agent Plan 团队" : "Agent Plan";
  return "企业版";
}

function sum<T>(arr: T[], pick: (x: T) => number): number {
  return arr.reduce((a, x) => a + pick(x), 0);
}

function ymd(d: Date): string {
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

function fillMissingDays(range: Range, byDate: Map<string, TrendPoint>): TrendPoint[] {
  const out: TrendPoint[] = [];
  const cursor = new Date(range.start);
  const end = new Date(range.end);
  while (cursor <= end) {
    const key = ymd(cursor);
    out.push(
      byDate.get(key) ?? { date: key, total: 0, input: 0, output: 0 },
    );
    cursor.setDate(cursor.getDate() + 1);
  }
  return out;
}

function formatNum(n: number): string {
  return n.toLocaleString("en-US");
}

function formatRelative(iso: string | undefined, verb: string): string | undefined {
  if (!iso) return undefined;
  const target = new Date(iso).getTime();
  const now = Date.now();
  let diff = target - now;
  if (diff <= 0) return `即将${verb}`;
  const totalMin = Math.round(diff / 60000);
  if (totalMin < 60) return `${totalMin} 分钟后${verb}`;
  if (totalMin < 60 * 24) {
    const h = Math.floor(totalMin / 60);
    const m = totalMin % 60;
    return m ? `${h} 小时 ${m} 分钟后${verb}` : `${h} 小时后${verb}`;
  }
  if (totalMin < 60 * 24 * 7) {
    return `${Math.round(totalMin / (60 * 24))} 天后${verb}`;
  }
  const d = new Date(iso);
  const weekday = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"][d.getDay()];
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  return `${weekday} ${hh}:${mm} ${verb}`;
}

function msg(e: unknown): string {
  if (e instanceof ConnectorError && e.cause !== undefined) {
    return `${e.message}（${msg(e.cause)}）`;
  }
  return e instanceof Error ? e.message : String(e);
}

export const usageService = new UsageService();
