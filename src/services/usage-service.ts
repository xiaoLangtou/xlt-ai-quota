import { settings } from "@/config/settings";
import type { OilGrade } from "@/types/oil";
import { createUsageStorage } from "@/storage/usage-storage";
import type { UsageStorage } from "@/storage/storage";
import {
  type Platform,
  PLATFORM_META,
  type QuotaDisplayTarget,
  QUOTA_PLATFORMS,
  type PlatformQuotaView,
  type QuotaMetric,
  type QuotaSnapshot,
  type QuotaStateSnapshot,
  type QuotaWindow,
  type RangePreset,
  type SyncInfo,
  type TokenSummary,
  type TrendPoint,
  type PlatformDailyPoint,
  type ToolUsage,
  type ModelUsage,
  type HeatmapDay,
  type QuotaWindowView,
  type UsageCacheStats,
  type UsageConfig,
  type ProjectUsage,
  type ProjectModelUsage,
} from "@/types/usage";
import { flatToStateSnapshots, failureSnapshot } from "@/utils/quota";
import { localHourKey, normalizeProjectName } from "@/utils/usage-time";
import { ArkConnector } from "@/connectors/ark";
import { CodexConnector } from "@/connectors/codex";
import { KimiConnector } from "@/connectors/kimi";
import { KiroConnector } from "@/connectors/kiro";
import { QoderConnector } from "@/connectors/qoder";
import { OpenCodeConnector } from "@/connectors/opencode";
import { ClaudeConnector } from "@/connectors/claude";
import { GeminiConnector } from "@/connectors/gemini";
import { CopilotConnector } from "@/connectors/copilot";
import { DeepSeekConnector } from "@/connectors/deepseek";
import { DevinConnector } from "@/connectors/devin";
import { RustCollectorConnector } from "@/connectors/rust-collector";
import { computeCostUsd, isModelPriced, loadPricingOverlay, refreshPricingOverlay } from "@/pricing";
import {
  ConnectorError,
  type Connector,
  type QuotaConnector,
  type TokenConnector,
} from "@/connectors/types";

/** 分析页「今天」时间轴用：按小时聚合的趋势与平台分布。 */
export interface TodayHourly {
  labels: string[];
  trend: TrendPoint[];
  platform: PlatformDailyPoint[];
}

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
  qoder: [{ metric: "credits", label: "套餐已消耗", tone: "purple" }],
  kimi: [
    { metric: "five_hour", label: "5 小时额度", tone: "blue" },
    { metric: "monthly_code", label: "月 Code 额度", tone: "blue" },
    { metric: "monthly", label: "月总额度", tone: "blue" },
  ],
  "opencode-go": [
    { metric: "five_hour", label: "5 小时额度", tone: "green" },
    { metric: "weekly", label: "周额度", tone: "green" },
  ],
  deepseek: [], // 动态：账户余额走 balance 区块，不渲染进度条
  devin: [], // 套餐信息卡，额度数值待官方接口
};

/** 各周期的展示元数据与规范排序。 */
const METRIC_META: Record<
  QuotaMetric,
  { label: string; tone: QuotaWindowView["tone"]; order: number }
> = {
  daily: { label: "日额度", tone: "blue", order: 0 },
  session: { label: "Session 额度", tone: "blue", order: 0 },
  five_hour: { label: "5 小时额度", tone: "blue", order: 1 },
  weekly: { label: "周额度", tone: "blue", order: 2 },
  monthly: { label: "月额度", tone: "blue", order: 3 },
  monthly_code: { label: "月 Code 额度", tone: "blue", order: 3 },
  credits: { label: "本周期已消耗", tone: "purple", order: 4 },
  addon_credits: { label: "加购已消耗", tone: "purple", order: 5 },
  account: { label: "账户", tone: "blue", order: 6 },
  balance: { label: "账户余额", tone: "green", order: 7 },
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

  constructor(storage: UsageStorage = createUsageStorage()) {
    this.storage = storage;
    const ark = new ArkConnector();
    const codex = new CodexConnector();
    const kimi = new KimiConnector();
    const kiro = new KiroConnector();
    const qoder = new QoderConnector();
    const opencode = new OpenCodeConnector();
    const claude = new ClaudeConnector();
    const gemini = new GeminiConnector();
    const copilot = new CopilotConnector();
    const deepseek = new DeepSeekConnector();
    const devin = new DevinConnector();
    // Kiro 仅 Credits；Codex、OpenCode 与 Claude Code 的 Token 来自本机会话记录。
    this.quotaConnectors = [ark, codex, kiro, qoder, kimi, deepseek, devin];
    // Kiro CLI 本地会话经 estimateTokens 估算；Qoder 读本地 SQLite 的真实 token。
    // Gemini CLI / GitHub Copilot CLI 读本机会话文件的真实 token；Kimi 读 wire.jsonl 的 usage.record。
    // DeepSeek 读 DSH 客户端会话的 usage（余额走官方 /user/balance）。
    // 以下为 Rust 侧增量采集器（游标 + 去重 + 落盘统一在 ~/.xlt-workbench）。
    const rustCollectors = [
      new RustCollectorConnector("qwen", "qwen"),
      new RustCollectorConnector("amp", "amp"),
      new RustCollectorConnector("roocode", "roocode"),
      new RustCollectorConnector("kilocode", "kilocode"),
      new RustCollectorConnector("openclaw", "openclaw"),
      new RustCollectorConnector("cline", "cline"),
      new RustCollectorConnector("codebuddy", "codebuddy"),
      new RustCollectorConnector("workbuddy", "workbuddy"),
      new RustCollectorConnector("jetbrains-acp", "jetbrains-acp"),
    ];
    this.tokenConnectors = [ark, codex, opencode, claude, kiro, qoder, gemini, copilot, kimi, deepseek, ...rustCollectors];
  }

  /** 初始化存储（Tauri 文件存储 / 旧 localStorage 迁移）后再清空过期版本数据。 */
  async init(): Promise<void> {
    await this.storage.init?.();
    await loadPricingOverlay();
    void refreshPricingOverlay();
    this.ensureSeeded();
  }

  /** 数据版本变更时清空旧数据（含历史种子）；不再注入任何默认值 */
  ensureSeeded(): void {
    this.storage.resetIfVersionChanged(UsageService.DATA_VERSION);
  }

  // ---- 读：订阅额度 ----

  getPlatformQuotaViews(): PlatformQuotaView[] {
    const hidden = new Set(
      this.storage.getConnectorConfig().quotaDisplay?.hiddenPlatforms ?? [],
    );
    const states = this.storage.listQuotaStates?.() ?? [];
    if (states.length > 0) {
      return this.buildViewsFromStates(states, hidden);
    }
    // 旧实现（localStorage / 测试夹具）：直接从扁平读数构建。
    const all = this.storage.listQuotas();
    return QUOTA_PLATFORMS
      .filter((platform) => platform === "ark" || !hidden.has(platform))
      .flatMap((platform) => {
        if (platform !== "ark") return [this.buildPlatformView(platform, all)];
        const plans = this.latestArkPlans(all);
        return plans.length
          ? plans
              .filter((plan) => {
                const target = arkQuotaDisplayTarget(plan);
                return !target || !hidden.has(target);
              })
              .map((plan) => this.buildPlatformView(platform, all, plan))
          : [this.buildPlatformView(platform, all)];
      });
  }

  /** 新模型：每个 (channel, account) 一张卡，状态机 + limits[] + stale。 */
  private buildViewsFromStates(
    states: QuotaStateSnapshot[],
    hidden: Set<QuotaDisplayTarget>,
  ): PlatformQuotaView[] {
    const views: PlatformQuotaView[] = [];
    for (const platform of QUOTA_PLATFORMS) {
      const all = states.filter((state) => state.channel === platform);
      // 方舟按套餐分卡；若已有带 account 的套餐，忽略历史遗留的无 account 失败卡。
      const items =
        platform === "ark" && all.some((state) => state.account)
          ? all.filter((state) => state.account)
          : all;
      if (items.length === 0) {
        if (!hidden.has(platform as QuotaDisplayTarget)) {
          views.push(this.buildStateView(platform));
        }
        continue;
      }
      // 非方舟：一个平台只保留一张卡；历史多张卡合并 limits。
      if (platform !== "ark") {
        if (hidden.has(platform as QuotaDisplayTarget)) continue;
        views.push(this.buildStateView(platform, this.mergeStates(items)));
        continue;
      }
      items.sort(
        (a, b) =>
          arkPlanOrder(a.account ?? "") - arkPlanOrder(b.account ?? "") ||
          (a.account ?? "").localeCompare(b.account ?? ""),
      );
      for (const state of items) {
        const target = arkQuotaDisplayTarget(state.account ?? "");
        if (hidden.has(platform as QuotaDisplayTarget) || (target && hidden.has(target))) {
          continue;
        }
        views.push(this.buildStateView(platform, state));
      }
    }
    return views;
  }

  /** 合并同一平台的旧快照：以最新一次成功读数为准，limits 按 id 去重。 */
  private mergeStates(states: QuotaStateSnapshot[]): QuotaStateSnapshot {
    const sorted = [...states].sort((a, b) => (a.fetchedAt ?? "").localeCompare(b.fetchedAt ?? ""));
    const base = sorted[sorted.length - 1] ?? states[0];
    const limits = new Map<string, QuotaStateSnapshot["limits"][number]>();
    for (const state of sorted) {
      for (const limit of state.limits) limits.set(limit.id, limit);
    }
    return {
      ...base,
      account: undefined,
      limits: [...limits.values()],
      balance: base.balance ?? sorted.find((state) => state.balance)?.balance,
    };
  }

  private buildStateView(
    platform: Platform,
    state?: QuotaStateSnapshot,
  ): PlatformQuotaView {
    const meta = PLATFORM_META[platform];
    const limits = this.stateWindows(state);
    const view: PlatformQuotaView = {
      id: state?.account ? `${platform}:${state.account}` : platform,
      platform,
      name: meta.name,
      planTag: PLAN_TAG[platform],
      logoChar: meta.logoChar,
      logoClass: meta.logoClass,
      status: state?.status ?? "unavailable",
      stale: state?.stale ?? false,
      message: state?.message ?? null,
      windows: [],
      collectedAt: state?.fetchedAt ?? new Date(0).toISOString(),
    };

    if (platform === "ark") {
      view.planTag = arkPlanTag(state?.account ?? state?.planLabel ?? undefined);
    } else if (state?.planLabel) {
      view.planTag = state.planLabel;
    }

    const credit = limits.find((item) => item.id === "credits");
    if (credit) {
      const addon = limits.find((item) => item.id === "addon_credits");
      view.credits = {
        remaining: Math.max(0, credit.remaining ?? 0),
        total: credit.total ?? 0,
        refreshIn: platform === "kiro" ? formatRelative(credit.resetsAt ?? undefined, "刷新") : undefined,
        expiresIn: platform === "qoder" ? formatRelative(credit.resetsAt ?? undefined, "到期") : undefined,
        addOn: addon
          ? {
              remaining: Math.max(0, addon.remaining ?? 0),
              total: addon.total ?? 0,
            }
          : undefined,
      };
    }

    if (state?.balance) {
      view.balance = state.balance;
      view.planTag = state.balance.currency;
    }

    if (view.windows.length === 0 && !view.credits && !view.balance && state?.planLabel) {
      view.account = { label: state.planLabel };
    }

    view.windows = limits
      .filter(
        (item) =>
          item.id !== "credits" &&
          item.id !== "addon_credits" &&
          item.id !== "balance" &&
          item.id !== "account",
      )
      .map((item) => {
        const metric = item.id as QuotaMetric;
        const metaInfo = METRIC_META[metric] ?? {
          label: item.label,
          tone: "blue" as const,
          order: 50,
        };
        const usedPct = Math.round(item.usedPercent);
        const warn = usedPct >= 80 && metaInfo.tone !== "purple";
        return {
          metric,
          label: item.label || metaInfo.label,
          usedPct,
          usedText:
            item.total != null
              ? `${formatNum(Math.max(0, item.total - (item.remaining ?? 0)))} / ${formatNum(item.total)}`
              : `${usedPct}%`,
          resetsAt: item.resetsAt ?? undefined,
          resetsIn: formatRelative(item.resetsAt ?? undefined, "重置"),
          tone: warn ? ("orange" as const) : metaInfo.tone,
        } satisfies QuotaWindowView;
      })
      .sort((a, b) => (METRIC_META[a.metric]?.order ?? 50) - (METRIC_META[b.metric]?.order ?? 50));

    return view;
  }

  private stateWindows(state?: QuotaStateSnapshot): QuotaWindow[] {
    if (!state) return [];
    const map = new Map<string, QuotaWindow>();
    for (const limit of state.limits) map.set(limit.id, limit);
    const named: Array<[string, QuotaWindow | undefined]> = [
      ["five_hour", state.fiveHour],
      ["weekly", state.weekly],
      ["seven_day", state.sevenDay],
      ["primary_models", state.primaryModels],
      ["other_models", state.otherModels],
      ["plan", state.plan],
    ];
    for (const [id, window] of named) {
      if (window && !map.has(id)) {
        map.set(id, { ...window, id, label: window.label || id });
      }
    }
    return [...map.values()];
  }

  private buildPlatformView(
    platform: Platform,
    all: QuotaSnapshot[],
    accountName?: string,
  ): PlatformQuotaView {
    const meta = PLATFORM_META[platform];
    const latest = this.latestByMetric(platform, all, accountName);
    const collectedAt =
      latest[0]?.collectedAt ?? new Date(0).toISOString();
    const view: PlatformQuotaView = {
      id: accountName ? `${platform}:${accountName}` : platform,
      platform,
      name: meta.name,
      planTag: PLAN_TAG[platform],
      logoChar: meta.logoChar,
      logoClass: meta.logoClass,
      windows: [],
      collectedAt,
    };

    if (platform === "ark") {
      view.planTag = arkPlanTag(accountName ?? latest[0]?.accountName);
    }

    if (platform === "codex" || platform === "kimi") {
      const snap = latest[0];
      if (snap?.accountName) view.planTag = snap.accountName;
    }

    if (platform === "kiro" || platform === "qoder") {
      const c = latest.find((q) => q.metric === "credits");
      if (c) {
        view.planTag = c.accountName || PLAN_TAG[platform];
        const addon = latest.find((q) => q.metric === "addon_credits");
        view.credits = {
          remaining: Math.max(0, c.limit - c.used),
          total: c.limit,
          refreshIn: platform === "kiro" ? formatRelative(c.resetsAt, "刷新") : undefined,
          expiresIn: platform === "qoder" ? formatRelative(c.resetsAt, "到期") : undefined,
          addOn: addon
            ? {
                remaining: Math.max(0, addon.limit - addon.used),
                total: addon.limit,
              }
            : undefined,
        };
      }
    }

    if (platform === "deepseek") {
      const snap = latest.find((q) => q.metric === "balance");
      if (snap?.balance) {
        view.balance = snap.balance;
        view.planTag = snap.balance.currency;
      }
    }

    // 方舟展示会话、周、月三个套餐周期；Codex 维持两个核心周期。
    const specs =
      platform === "ark" || platform === "codex"
        ? latest
            .map((s) => s.metric)
            .filter((m) => m !== "account" && m !== "credits" && m !== "addon_credits" && m !== "monthly_code" && m !== "balance")
            .sort((a, b) => METRIC_META[a].order - METRIC_META[b].order)
            .slice(0, platform === "ark" ? 3 : 2)
            .map((metric) => ({
              metric,
              ...(platform === "ark"
                ? (ARK_METRIC_META[metric as keyof typeof ARK_METRIC_META] ?? METRIC_META[metric])
                : METRIC_META[metric]),
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
    accountName?: string,
  ): QuotaSnapshot[] {
    return all
      .filter((q) => q.platform === platform && (!accountName || q.accountName === accountName))
      .sort((a, b) => (a.collectedAt < b.collectedAt ? 1 : -1))
      .filter((q, i, arr) => arr.findIndex((x) => x.metric === q.metric) === i);
  }

  /** 仅使用方舟最新一次采集返回的已订阅套餐，避免已退订的历史套餐继续显示。 */
  private latestArkPlans(all: QuotaSnapshot[]): string[] {
    const snapshots = all.filter((item) => item.platform === "ark");
    const latestAt = snapshots.reduce(
      (value, item) => (item.collectedAt > value ? item.collectedAt : value),
      "",
    );
    if (!latestAt) return [];
    return [...new Set(
      snapshots
        .filter((item) => item.collectedAt === latestAt)
        .map((item) => item.accountName)
        .filter(Boolean),
    )].sort((a, b) => arkPlanOrder(a) - arkPlanOrder(b) || a.localeCompare(b));
  }

  // ---- 读：Token ----

  getRange(preset: RangePreset): Range {
    const end = new Date();
    end.setHours(0, 0, 0, 0);
    const offset = preset === "today" ? 0 : preset === "7d" ? 6 : preset === "30d" ? 29 : 89;
    const start = new Date(end);
    start.setDate(start.getDate() - offset);
    const endStr = ymd(end);
    let startStr = ymd(start);
    // 统计起始日：早于此日期的数据不纳入分析/同步。clamp 不超过 end。
    const since = settings.getPreferences().statsSince;
    if (since && since > startStr) startStr = since > endStr ? endStr : since;
    return { start: startStr, end: endStr };
  }

  getTokenSummary(preset: RangePreset): TokenSummary {
    const range = this.getRange(preset);
    const rows = this.storage.listTokensByRange(range.start, range.end);
    const total = sum(rows, totalTokensOf);
    const input = sum(rows, (r) => r.inputTokens);
    const output = sum(rows, (r) => r.outputTokens);
    const cached = sum(rows, (r) => r.cachedTokens ?? 0);
    const cacheWrite = sum(rows, (r) => r.cacheWriteTokens ?? 0);
    const reasoning = sum(rows, (r) => r.reasoningTokens ?? 0);
    const requests = sum(rows, (r) => r.requestCount ?? 0);
    const costUsd = sum(rows, (r) => costOf(r));
    const cacheDenom = input + cached + cacheWrite;

    // 上一周期对比（各指标独立）
    const prev = this.prevRange(range, preset);
    const prevRows = this.storage.listTokensByRange(prev.start, prev.end);
    const prevTotal = sum(prevRows, totalTokensOf);
    const prevInput = sum(prevRows, (r) => r.inputTokens);
    const prevOutput = sum(prevRows, (r) => r.outputTokens);
    const prevReq = sum(prevRows, (r) => r.requestCount ?? 0);
    const prevCost = sum(prevRows, (r) => costOf(r));

    return {
      total,
      input,
      output,
      cached: cached || undefined,
      cacheWrite: cacheWrite || undefined,
      reasoning: reasoning || undefined,
      cacheHitRate: cacheDenom > 0 ? Math.round((cached / cacheDenom) * 1000) / 10 : undefined,
      requests,
      costUsd,
      deltaPct: deltaPct(total, prevTotal),
      inputDeltaPct: deltaPct(input, prevInput),
      outputDeltaPct: deltaPct(output, prevOutput),
      requestsDeltaPct: deltaPct(requests, prevReq),
      costDeltaPct: deltaPct(costUsd, prevCost),
    };
  }

  getTokenTrend(preset: RangePreset): TrendPoint[] {
    const range = this.getRange(preset);
    const rows = this.storage.listTokensByRange(range.start, range.end);
    const byDate = new Map<string, TrendPoint>();
    for (const r of rows) {
      const cur =
        byDate.get(r.date) ??
        ({ date: r.date, total: 0, input: 0, output: 0, requests: 0, costUsd: 0 } satisfies TrendPoint);
      cur.input += r.inputTokens;
      cur.output += r.outputTokens;
      cur.total += totalTokensOf(r);
      cur.requests += r.requestCount ?? 0;
      cur.costUsd += costOf(r);
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
        (cur.byPlatform[r.platform] ?? 0) + totalTokensOf(r);
      byDate.set(r.date, cur);
    }
    return [...byDate.values()].sort((a, b) => (a.date < b.date ? -1 : 1));
  }

  /** 按 AI 工具（平台）聚合所选周期的用量，用于「按工具统计」。 */
  getToolBreakdown(preset: RangePreset): ToolUsage[] {
    const range = this.getRange(preset);
    const rows = this.storage.listTokensByRange(range.start, range.end);
    const byTool = new Map<string, ToolUsage>();
    let grand = 0;
    for (const r of rows) {
      const cur =
        byTool.get(r.platform) ??
        ({ platform: r.platform, total: 0, input: 0, output: 0, requests: 0, costUsd: 0, pct: 0 } satisfies ToolUsage);
      cur.input += r.inputTokens;
      cur.output += r.outputTokens;
      cur.total += totalTokensOf(r);
      cur.requests += r.requestCount ?? 0;
      cur.costUsd += costOf(r);
      byTool.set(r.platform, cur);
      grand += totalTokensOf(r);
    }
    return [...byTool.values()]
      .map((t) => ({ ...t, pct: grand > 0 ? Math.round((t.total / grand) * 1000) / 10 : 0 }))
      .sort((a, b) => b.total - a.total);
  }

  /** 按模型聚合所选周期的用量，用于「按模型统计」。无模型信息的行归入 "未知模型"。 */
  getModelBreakdown(preset: RangePreset): ModelUsage[] {
    const range = this.getRange(preset);
    const rows = this.storage.listTokensByRange(range.start, range.end);
    // 每个模型再按平台记 Token，用于挑选主要归属平台。
    const byModel = new Map<
      string,
      ModelUsage & { platformTotals: Map<string, number> }
    >();
    let grand = 0;
    for (const r of rows) {
      const model = r.model && r.model.trim() ? r.model : "未知模型";
      const tokens = totalTokensOf(r);
      const cur =
        byModel.get(model) ??
        {
          model,
          platform: r.platform,
          total: 0,
          input: 0,
          output: 0,
          cached: 0,
          requests: 0,
          costUsd: 0,
          priced: isModelPriced(r.model, r.platform),
          pct: 0,
          platformTotals: new Map<string, number>(),
        };
      cur.input += r.inputTokens;
      cur.output += r.outputTokens;
      cur.cached += r.cachedTokens ?? 0;
      cur.total += tokens;
      cur.requests += r.requestCount ?? 0;
      cur.costUsd += costOf(r);
      cur.platformTotals.set(r.platform, (cur.platformTotals.get(r.platform) ?? 0) + tokens);
      byModel.set(model, cur);
      grand += tokens;
    }
    return [...byModel.values()]
      .map(({ platformTotals, ...m }) => {
        // 主要归属平台 = 该模型下 Token 最多的平台。
        let topPlatform = m.platform;
        let topTokens = -1;
        for (const [platform, tokens] of platformTotals) {
          if (tokens > topTokens) {
            topTokens = tokens;
            topPlatform = platform;
          }
        }
        return {
          ...m,
          platform: topPlatform,
          pct: grand > 0 ? Math.round((m.total / grand) * 1000) / 10 : 0,
        } satisfies ModelUsage;
      })
      .sort((a, b) => b.total - a.total);
  }

  /** 按项目聚合所选周期的用量（项目名取工作目录最后一段）。 */
  getProjectBreakdown(preset: RangePreset): ProjectUsage[] {
    const range = this.getRange(preset);
    const rows = this.storage.listTokensByRange(range.start, range.end);
    type Acc = ProjectUsage & { modelTotals: Map<string, ProjectModelUsage> };
    const byProject = new Map<string, Acc>();
    let grand = 0;
    for (const r of rows) {
      const project = normalizeProjectName(r.project);
      const tokens = totalTokensOf(r);
      const cost = costOf(r);
      const cur =
        byProject.get(project) ??
        ({
          project,
          tokens: 0,
          input: 0,
          output: 0,
          cached: 0,
          cacheWrite: 0,
          reasoning: 0,
          requests: 0,
          costUsd: 0,
          cacheHitRate: 0,
          pct: 0,
          models: [],
          modelTotals: new Map<string, ProjectModelUsage>(),
        } satisfies Acc);
      cur.tokens += tokens;
      cur.input += r.inputTokens;
      cur.output += r.outputTokens;
      cur.cached += r.cachedTokens ?? 0;
      cur.cacheWrite += r.cacheWriteTokens ?? 0;
      cur.reasoning += r.reasoningTokens ?? 0;
      cur.requests += r.requestCount ?? 0;
      cur.costUsd += cost;

      const modelKey = `${r.platform}\u0000${r.model ?? ""}`;
      const model =
        cur.modelTotals.get(modelKey) ??
        ({
          model: r.model?.trim() || "未知模型",
          platform: r.platform,
          tokens: 0,
          costUsd: 0,
          pct: 0,
        } satisfies ProjectModelUsage);
      model.tokens += tokens;
      model.costUsd += cost;
      cur.modelTotals.set(modelKey, model);
      byProject.set(project, cur);
      grand += tokens;
    }

    return [...byProject.values()]
      .map(({ modelTotals, ...project }) => {
        const denom = project.input + project.cached + project.cacheWrite;
        return {
          ...project,
          cacheHitRate:
            denom > 0 ? Math.round((project.cached / denom) * 1000) / 10 : 0,
          pct: grand > 0 ? Math.round((project.tokens / grand) * 1000) / 10 : 0,
          models: [...modelTotals.values()]
            .map((model) => ({
              ...model,
              pct:
                project.tokens > 0
                  ? Math.round((model.tokens / project.tokens) * 1000) / 10
                  : 0,
            }))
            .sort((a, b) => b.tokens - a.tokens),
        } satisfies ProjectUsage;
      })
      .sort((a, b) => b.tokens - a.tokens);
  }

  /** 近一年贡献热力图：按周日对齐，缺失日期补 0，并保留工具/模型明细。 */
  getDailyHeatmap(weeks = 52): HeatmapDay[] {
    const end = new Date();
    end.setHours(0, 0, 0, 0);
    const start = new Date(end);
    // 回退到 (weeks-1) 周前那一周的周日，保证首列从周日开始。
    start.setDate(end.getDate() - end.getDay() - (weeks - 1) * 7);
    const rows = this.storage.listTokensByRange(ymd(start), ymd(end));
    const byDate = new Map<string, { total: number; details: Map<string, HeatmapDay["details"][number]> }>();
    for (const r of rows) {
      const tokens = totalTokensOf(r);
      const day = byDate.get(r.date) ?? { total: 0, details: new Map() };
      const detailKey = `${r.platform}\u0000${r.model ?? ""}`;
      const detail = day.details.get(detailKey) ?? {
        platform: r.platform,
        model: r.model,
        total: 0,
      };
      day.total += tokens;
      detail.total += tokens;
      day.details.set(detailKey, detail);
      byDate.set(r.date, day);
    }
    const out: HeatmapDay[] = [];
    const cursor = new Date(start);
    while (cursor <= end) {
      const key = ymd(cursor);
      const day = byDate.get(key);
      out.push({
        date: key,
        total: day?.total ?? 0,
        details: [...(day?.details.values() ?? [])].sort((a, b) => b.total - a.total),
      });
      cursor.setDate(cursor.getDate() + 1);
    }
    return out;
  }

  /** 本地最早一条 Token 记录的日期（YYYY-MM-DD）；无数据返回 null。 */
  getEarliestTokenDate(): string | null {
    let earliest: string | null = null;
    for (const record of this.storage.listTokens()) {
      if (!record.date) continue;
      if (earliest === null || record.date < earliest) earliest = record.date;
    }
    return earliest;
  }

  /**
   * 今日按小时的趋势与平台分布（0 点到当前小时，缺失补 0）。
   * 走 /api/today-hourly，直连本地会话按小时聚合，不落日粒度存储。
   */
  async getTodayHourly(): Promise<TodayHourly> {
    const maxHour = new Date().getHours();
    const range = this.getRange("today");
    const timezone = settings.getPreferences().timezone;
    const buckets = this.storage.listBucketsByRange?.(range.start, range.end) ?? [];
    const rows = buckets.map((bucket) => ({
      platform: bucket.source,
      hour: Number(localHourKey(bucket.hourStart, timezone).slice(11, 13)),
      input: bucket.inputTokens,
      output: bucket.outputTokens,
      cached: bucket.cachedInputTokens,
      cacheWrite: bucket.cacheCreationInputTokens,
      reasoning: bucket.reasoningOutputTokens,
    }));
    const labels: string[] = [];
    const trend: TrendPoint[] = [];
    const platform: PlatformDailyPoint[] = [];
    for (let h = 0; h <= maxHour; h += 1) {
      const label = `${String(h).padStart(2, "0")}:00`;
      labels.push(label);
      const hourRows = rows.filter((r) => r.hour === h);
      const input = hourRows.reduce((s, r) => s + r.input, 0);
      const output = hourRows.reduce((s, r) => s + r.output, 0);
      const total = hourRows.reduce(
        (s, r) => s + r.input + r.output + r.cached + r.cacheWrite + r.reasoning,
        0,
      );
      trend.push({ date: label, total, input, output, requests: 0, costUsd: 0 });
      const byPlatform: Record<string, number> = {};
      for (const r of hourRows) {
        byPlatform[r.platform] =
          (byPlatform[r.platform] ?? 0) + r.input + r.output + r.cached + r.cacheWrite + r.reasoning;
      }
      platform.push({ date: label, byPlatform });
    }
    return { labels, trend, platform };
  }

  private prevRange(range: Range, preset: RangePreset): Range {
    if (preset === "today") {
      const start = new Date(range.start);
      start.setDate(start.getDate() - 1);
      return { start: ymd(start), end: ymd(start) };
    }
    const days = preset === "7d" ? 7 : preset === "30d" ? 30 : 90;
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
            await this.saveQuotaReadings(connector.platformId() as Platform, snapshots);
            successful += 1;
          } catch (error) {
            await this.saveQuotaFailure(connector.platformId() as Platform, error);
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
  async syncAll(onProgress?: () => void, preset: RangePreset = "30d"): Promise<void> {
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
          await this.saveQuotaReadings(c.platformId() as Platform, snaps);
          successful += 1;
        } catch (e) {
          await this.saveQuotaFailure(c.platformId() as Platform, e);
          errors.push(`${c.id}: ${msg(e)}`);
        } finally {
          reportProgress();
        }
      });

    const tokenRange = { ...this.getRange(preset), tz: settings.getPreferences().timezone };
    let reloadNeeded = false;
    const tokenTasks = this.tokenConnectors
      .filter((c) => c.isConfigured())
      .map(async (c) => {
        try {
          if (c.selfPersisting && c.fetchBuckets && this.storage.reloadBuckets) {
            // Rust 采集器自行落盘并推进游标，最后统一重新加载一次。
            await waitForConnector(c.id, c.fetchBuckets(tokenRange));
            reloadNeeded = true;
          } else {
            const rows = await waitForConnector(c.id, c.fetchTokens(tokenRange));
            // 已成功取得的空数组代表该平台当前没有历史 Token，仍应覆盖旧数据。
            await this.storage.replaceTokensForPlatform(
              c.platformId(),
              tokenRange.start,
              tokenRange.end,
              rows,
            );
          }
          successful += 1;
        } catch (e) {
          errors.push(`${c.id}: ${msg(e)}`);
        } finally {
          reportProgress();
        }
      });

    await Promise.all([...quotaTasks, ...tokenTasks]);
    if (reloadNeeded) await this.storage.reloadBuckets?.();

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

  private async saveQuotaReadings(
    platform: Platform,
    snapshots: QuotaSnapshot[],
  ): Promise<void> {
    if (this.storage.saveQuotaStates) {
      await this.storage.saveQuotaStates(flatToStateSnapshots(platform, snapshots));
      return;
    }
    if (snapshots.length) await this.storage.saveQuotas(snapshots);
  }

  private async saveQuotaFailure(platform: Platform, error: unknown): Promise<void> {
    if (this.storage.saveQuotaStates) {
      await this.storage.saveQuotaStates([failureSnapshot(platform, error)]);
    }
  }

  setSyncing(flag: boolean): void {
    this.storage.saveSyncInfo({ ...this.storage.getSyncInfo(), syncing: flag });
  }

  /**
   * 轻量「今天」刷新：仅重拉本地会话类 Token 连接器（排除 arkcli 网络调用），
   * 只覆盖今天这一天的数据，供定时/聚焦自动刷新使用，不改动同步状态与 loading。
   */
  async syncTodayTokens(): Promise<void> {
    const range = { ...this.getRange("today"), tz: settings.getPreferences().timezone };
    const local = this.tokenConnectors.filter((c) => c.id !== "ark" && c.isConfigured());
    let reloadNeeded = false;
    await Promise.all(
      local.map(async (c) => {
        try {
          if (c.selfPersisting && c.fetchBuckets && this.storage.reloadBuckets) {
            await Promise.race([
              c.fetchBuckets(range),
              new Promise<never>((_, reject) =>
                window.setTimeout(() => reject(new Error(`${c.id} 刷新超时`)), 15_000),
              ),
            ]);
            reloadNeeded = true;
            return;
          }
          const rows = await Promise.race([
            c.fetchTokens(range),
            new Promise<never>((_, reject) =>
              window.setTimeout(() => reject(new Error(`${c.id} 刷新超时`)), 15_000),
            ),
          ]);
          await this.storage.replaceTokensForPlatform(c.platformId(), range.start, range.end, rows);
        } catch {
          // 单个连接器刷新失败不影响其他平台，也不打断自动刷新。
        }
      }),
    );
    if (reloadNeeded) await this.storage.reloadBuckets?.();
  }

  // ---- 设置 ----

  getArkConfigView(): { baseUrl: string } {
    return settings.getArkConfig();
  }
  saveArkBaseUrl(baseUrl: string): void {
    settings.saveArkBaseUrl(baseUrl);
  }
  getDeepseekConfigView(): { apiKey: string } {
    return settings.getDeepseekConfig();
  }
  saveDeepseekApiKey(apiKey: string): void {
    settings.saveDeepseekApiKey(apiKey);
  }
  getOilConfigView(): { province: string; apiKey: string; grade: OilGrade } {
    return settings.getOilConfig();
  }
  saveOilConfig(config: { province: string; apiKey: string; grade?: OilGrade }): void {
    settings.saveOilConfig(config);
  }
  getQuotaDisplayConfigView(): { hiddenPlatforms: QuotaDisplayTarget[] } {
    return settings.getQuotaDisplayConfig();
  }
  saveQuotaDisplayConfig(config: { hiddenPlatforms: QuotaDisplayTarget[] }): void {
    settings.saveQuotaDisplayConfig(config);
  }

  async reloadQuotas(): Promise<void> {
    await this.storage.reloadQuotas?.();
  }

  getCacheStats(): UsageCacheStats | null {
    return this.storage.cacheStats?.() ?? null;
  }

  async cleanupCache(retentionDays?: number): Promise<UsageCacheStats | null> {
    return (await this.storage.cleanup?.(retentionDays)) ?? null;
  }

  async clearUsageCache(): Promise<UsageCacheStats | null> {
    return (await this.storage.clearCache?.()) ?? null;
  }

  getUsageConfig(): UsageConfig | null {
    return this.storage.getUsageConfig?.() ?? null;
  }

  saveUsageConfig(patch: Partial<UsageConfig>): void {
    this.storage.saveUsageConfig?.(patch);
  }

  getPreferencesView(): { timezone: string; statsSince: string | null } {
    return settings.getPreferences();
  }
  getSystemTimezone(): string {
    return settings.systemTimezone();
  }
  savePreferences(patch: { timezone?: string; statsSince?: string | null }): void {
    settings.savePreferences(patch);
  }
}

const PLAN_TAG: Record<Platform, string> = {
  codex: "Plus",
  ark: "企业版",
  kiro: "Pro",
  qoder: "Pro",
  kimi: "会员",
  "opencode-go": "个人版",
  deepseek: "余额",
  devin: "Devin",
};

/** 由 arkcli 的 product 字段推导 ark 卡片套餐标签 */
function arkPlanTag(accountName: string | undefined): string {
  const p = accountName ?? "";
  const team = p.includes("-team");
  if (p.includes("coding-plan")) return team ? "Coding Plan 团队" : "Coding Plan";
  if (p.includes("agent-plan")) return team ? "Agent Plan 团队" : "Agent Plan";
  return "企业版";
}

function arkPlanOrder(accountName: string): number {
  if (accountName.includes("coding-plan")) return 0;
  if (accountName.includes("agent-plan")) return 1;
  return 2;
}

function arkQuotaDisplayTarget(accountName: string): QuotaDisplayTarget | undefined {
  if (accountName.includes("coding-plan")) return "ark-coding";
  if (accountName.includes("agent-plan")) return "ark-agent";
  return undefined;
}

function sum<T>(arr: T[], pick: (x: T) => number): number {
  return arr.reduce((a, x) => a + pick(x), 0);
}

/** 单条日用量的预估费用（USD），按模型单价估算。 */
function costOf(r: {
  platform?: string;
  model?: string;
  inputTokens: number;
  outputTokens: number;
  cachedTokens?: number;
  cacheWriteTokens?: number;
  reasoningTokens?: number;
  reportedCostUsd?: number;
}): number {
  // 上游给了真实费用就直接用，避免再按模型单价估一遍。
  if (r.reportedCostUsd != null && r.reportedCostUsd > 0) return r.reportedCostUsd;
  return computeCostUsd({
    model: r.model,
    source: r.platform,
    inputTokens: r.inputTokens,
    outputTokens: r.outputTokens,
    cachedTokens: r.cachedTokens,
    cacheWriteTokens: r.cacheWriteTokens,
    reasoningTokens: r.reasoningTokens,
  });
}

/** 与 juejin 对齐：总 Token = 输入 + 输出 + 缓存读 + 缓存写 + 推理。 */
function totalTokensOf(row: {
  inputTokens: number;
  outputTokens: number;
  cachedTokens?: number;
  cacheWriteTokens?: number;
  reasoningTokens?: number;
}): number {
  return (
    row.inputTokens +
    row.outputTokens +
    (row.cachedTokens ?? 0) +
    (row.cacheWriteTokens ?? 0) +
    (row.reasoningTokens ?? 0)
  );
}

/** 与上一周期对比的百分比（保留一位小数）；上一周期为 0 时返回 undefined。 */
function deltaPct(current: number, previous: number): number | undefined {
  if (previous <= 0) return undefined;
  return Math.round(((current - previous) / previous) * 1000) / 10;
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
      byDate.get(key) ?? { date: key, total: 0, input: 0, output: 0, requests: 0, costUsd: 0 },
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
