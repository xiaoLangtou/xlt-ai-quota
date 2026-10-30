import type {
  Platform,
  QuotaMetric,
  QuotaSnapshot,
  QuotaStateSnapshot,
  QuotaStatus,
  QuotaWindow,
} from "@/types/usage";

const METRIC_LABEL: Record<string, string> = {
  session: "Session 额度",
  five_hour: "5 小时额度",
  weekly: "周额度",
  seven_day: "7 天额度",
  monthly: "月额度",
  monthly_code: "月 Code 额度",
  credits: "本周期已消耗",
  addon_credits: "加购已消耗",
  account: "账户",
  balance: "账户余额",
};

export function metricLabel(metric: string | undefined): string {
  return METRIC_LABEL[metric ?? ""] ?? metric ?? "额度";
}

function toWindow(row: QuotaSnapshot): QuotaWindow {
  const usedPercent =
    row.unit === "percent"
      ? row.used
      : row.limit > 0
        ? (row.used / row.limit) * 100
        : 0;
  return {
    id: row.metric,
    label: metricLabel(row.metric),
    usedPercent: Math.max(0, Math.min(100, usedPercent)),
    resetsAt: row.resetsAt ?? null,
    description: null,
    remaining: row.unit === "percent" ? Math.max(0, 100 - row.used) : Math.max(0, row.limit - row.used),
    total: row.unit === "percent" ? 100 : row.limit,
  };
}

/**
 * 把 connector 的扁平读数聚合成状态快照。
 *
 * - 方舟：按套餐（coding-plan / agent-plan）分卡；
 * - 其他平台：一个平台一张卡，`accountName` 只作为套餐标签，不参与分组。
 *   这样 Codex 的 Plus/Free、Kiro 的 Pro/Pro+、Qoder 的 Pro/personal_standard
 *   不会因为套餐名变化而堆出多张卡。
 */
export function flatToStateSnapshots(
  platform: Platform,
  rows: QuotaSnapshot[],
): QuotaStateSnapshot[] {
  if (rows.length === 0) return [];
  const groups =
    platform === "ark"
      ? groupByAccount(rows)
      : [[rows[0]?.accountName?.trim() || platform, rows] as const];

  return groups.map(([account, items]) => {
    const limits = items
      .filter((row) => row.metric !== "balance")
      .map(toWindow);
    const balanceRow = items.find((row) => row.metric === "balance");
    return {
      channel: platform,
      account: platform === "ark" ? account : undefined,
      status: "ready",
      planLabel: account,
      region: null,
      limits,
      fetchedAt: items[0]?.collectedAt ?? new Date().toISOString(),
      stale: false,
      message: null,
      balance: balanceRow?.balance,
    } satisfies QuotaStateSnapshot;
  });
}

function groupByAccount(rows: QuotaSnapshot[]): Array<readonly [string, QuotaSnapshot[]]> {
  const byAccount = new Map<string, QuotaSnapshot[]>();
  for (const row of rows) {
    const account = row.accountName?.trim() || "ark";
    byAccount.set(account, [...(byAccount.get(account) ?? []), row]);
  }
  return [...byAccount.entries()];
}

const ERROR_STATUS: { pattern: RegExp; status: QuotaStatus }[] = [
  { pattern: /not installed|未安装|ENOENT|not found/i, status: "not-installed" },
  { pattern: /not signed in|未登录|login required|请先.*登录/i, status: "not-signed-in" },
  { pattern: /expired|过期|token.*失效/i, status: "expired" },
  { pattern: /403|access denied|无权|权限/i, status: "access-denied" },
  { pattern: /custom provider|自定义.*地址|base.?url/i, status: "custom-provider" },
  { pattern: /unsupported|不支持/i, status: "unsupported-account" },
];

export function classifyQuotaError(error: unknown): QuotaStatus {
  const message = error instanceof Error ? error.message : String(error);
  return ERROR_STATUS.find((item) => item.pattern.test(message))?.status ?? "temporarily-unavailable";
}

export function failureSnapshot(platform: Platform, error: unknown): QuotaStateSnapshot {
  const message = error instanceof Error ? error.message : String(error);
  return {
    channel: platform,
    status: classifyQuotaError(error),
    limits: [],
    fetchedAt: null,
    stale: false,
    message,
  };
}

/** 旧扁平读数 → 状态快照（用于 localStorage 迁移或测试夹具）。 */
export function stateFromFlat(rows: QuotaSnapshot[]): QuotaStateSnapshot[] {
  const byPlatform = new Map<Platform, QuotaSnapshot[]>();
  for (const row of rows) {
    byPlatform.set(row.platform, [...(byPlatform.get(row.platform) ?? []), row]);
  }
  return [...byPlatform.entries()].flatMap(([platform, items]) =>
    flatToStateSnapshots(platform, items),
  );
}

export function windowMetric(window: QuotaWindow): QuotaMetric {
  return (window.id as QuotaMetric) ?? "account";
}
