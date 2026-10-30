import type { QueueBucket, TokenDailyUsage } from "@/types/usage";

/** 指定时区相对 UTC 的分钟偏移（东为正）；无时区或解析失败返回 0。 */
export function tzOffsetMinutes(timezone: string | undefined, at: Date): number {
  if (!timezone) return 0;
  try {
    const name =
      new Intl.DateTimeFormat("en-US", {
        timeZone: timezone,
        timeZoneName: "longOffset",
      })
        .formatToParts(at)
        .find((part) => part.type === "timeZoneName")?.value ?? "GMT";
    const match = /GMT([+-])(\d{2}):(\d{2})/.exec(name);
    if (!match) return 0;
    const sign = match[1] === "-" ? -1 : 1;
    return sign * (Number(match[2]) * 60 + Number(match[3]));
  } catch {
    return 0;
  }
}

/** 把 UTC 时刻换算成用户时区的 YYYY-MM-DD。 */
export function localDate(iso: string, timezone?: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso.slice(0, 10);
  try {
    const parts = new Intl.DateTimeFormat("en-CA", {
      timeZone: timezone || undefined,
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
    }).formatToParts(date);
    const get = (type: string) => parts.find((part) => part.type === type)?.value ?? "";
    return `${get("year")}-${get("month")}-${get("day")}`;
  } catch {
    return date.toISOString().slice(0, 10);
  }
}

/** 把 UTC 时刻换算成用户时区的 `YYYY-MM-DD HH`，用于小时级聚合。 */
export function localHourKey(iso: string, timezone?: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso.slice(0, 13).replace("T", " ");
  try {
    const parts = new Intl.DateTimeFormat("en-US", {
      timeZone: timezone || undefined,
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      hour12: false,
    }).formatToParts(date);
    const get = (type: string) => parts.find((part) => part.type === type)?.value ?? "";
    const hour = get("hour") === "24" ? "00" : get("hour");
    return `${get("year")}-${get("month")}-${get("day")} ${hour}`;
  } catch {
    return date.toISOString().slice(0, 13).replace("T", " ");
  }
}

/**
 * 某本地日期的「本地中午」对应的 UTC 时刻。
 * 用中午而不是零点，避免 ±12 小时时区把日期推到前一天/后一天。
 */
export function localNoonUtc(date: string, timezone?: string): string {
  const guess = new Date(`${date}T12:00:00.000Z`);
  if (Number.isNaN(guess.getTime())) return `${date}T12:00:00.000Z`;
  const offset = tzOffsetMinutes(timezone, guess);
  return new Date(guess.getTime() - offset * 60_000).toISOString();
}

/** 覆盖某本地日期全天的 UTC 区间（前后各放宽 14 小时，兼容所有时区）。 */
export function dayBoundsUtc(date: string): { start: string; end: string } {
  const base = new Date(`${date}T00:00:00.000Z`);
  const pad = 14 * 60 * 60 * 1000;
  return {
    start: new Date(base.getTime() - pad).toISOString(),
    end: new Date(base.getTime() + 24 * 60 * 60 * 1000 + pad).toISOString(),
  };
}

/** 项目名规范化：解开 percent-encoding / file URL，取路径最后一段。 */
export function normalizeProjectName(value?: string): string {
  const raw = (value ?? "").trim();
  if (!raw || raw === "unknown") return "unknown";
  let decoded = raw;
  try {
    decoded = decodeURIComponent(raw);
  } catch {
    // 保留原值
  }
  if (/^file:/i.test(decoded)) decoded = decoded.replace(/^file:\/\//i, "");
  const parts = decoded.replace(/\\/g, "/").replace(/\/+$/, "").split("/");
  return parts[parts.length - 1]?.trim() || "unknown";
}

/** 把半小时桶聚合成日粒度（platform + date + model + project）。 */
export function aggregateBucketsToDaily(
  buckets: QueueBucket[],
  timezone?: string,
): TokenDailyUsage[] {
  const byKey = new Map<string, TokenDailyUsage>();
  for (const bucket of buckets) {
    const date = localDate(bucket.hourStart, timezone);
    const model = bucket.model || "未知模型";
    const project = normalizeProjectName(bucket.project);
    const key = `${bucket.source}|${date}|${model}|${project}`;
    const current =
      byKey.get(key) ??
      ({
        platform: bucket.source,
        date,
        model,
        project,
        inputTokens: 0,
        outputTokens: 0,
        cachedTokens: 0,
        cacheWriteTokens: 0,
        reasoningTokens: 0,
        requestCount: 0,
        reportedCostUsd: 0,
        collectedAt: bucket.hourStart,
      } satisfies TokenDailyUsage);
    current.inputTokens += bucket.inputTokens;
    current.outputTokens += bucket.outputTokens;
    current.cachedTokens = (current.cachedTokens ?? 0) + bucket.cachedInputTokens;
    current.cacheWriteTokens = (current.cacheWriteTokens ?? 0) + bucket.cacheCreationInputTokens;
    current.reasoningTokens = (current.reasoningTokens ?? 0) + bucket.reasoningOutputTokens;
    current.requestCount = (current.requestCount ?? 0) + (bucket.localMetrics?.requestCount ?? 0);
    current.reportedCostUsd = (current.reportedCostUsd ?? 0) + (bucket.reportedCostUsd ?? 0);
    if (bucket.hourStart < current.collectedAt) current.collectedAt = bucket.hourStart;
    byKey.set(key, current);
  }
  return [...byKey.values()];
}

/** 日粒度记录 → 半小时桶（旧采集器只有日粒度时使用本地中午作为锚点）。 */
export function dailyToBuckets(
  rows: TokenDailyUsage[],
  timezone?: string,
): QueueBucket[] {
  return rows.map((row) => {
    const input = Math.max(0, row.inputTokens);
    const output = Math.max(0, row.outputTokens);
    const cached = Math.max(0, row.cachedTokens ?? 0);
    const cacheWrite = Math.max(0, row.cacheWriteTokens ?? 0);
    const reasoning = Math.max(0, row.reasoningTokens ?? 0);
    return {
      hourStart: localNoonUtc(row.date, timezone),
      source: row.platform,
      project: normalizeProjectName(row.project),
      model: row.model?.trim() || "unknown",
      inputTokens: input,
      outputTokens: output,
      cachedInputTokens: cached,
      cacheCreationInputTokens: cacheWrite,
      reasoningOutputTokens: reasoning,
      totalTokens: input + output + cached + cacheWrite + reasoning,
      conversationCount: 0,
      reportedCostUsd: row.reportedCostUsd,
      localMetrics: {
        requestCount: row.requestCount ?? 0,
        cacheHitTokens: cached,
        cacheMissTokens: 0,
        requestCountComplete: true,
      },
    } satisfies QueueBucket;
  });
}
