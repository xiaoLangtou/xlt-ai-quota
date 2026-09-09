import type {
  Platform,
  QuotaSnapshot,
  SyncInfo,
  TokenDailyUsage,
} from "@/types/usage";

// 原型中的每日柱高比例（最近 7 天），用于分配 7 天总量
const DAY_WEIGHTS = [91, 27, 67, 5, 38, 47, 3] as const;
const PLATFORM_SPLIT: { platform: string; ratio: number }[] = [
  { platform: "ark", ratio: 0.68 },
  { platform: "codex", ratio: 0.16 },
  { platform: "kiro", ratio: 0.16 },
];
const INPUT_RATIO = 0.78;

const TOTAL_7D = 110_700_000;
const INPUT_7D = 86_300_000;
const OUTPUT_7D = TOTAL_7D - INPUT_7D;

// 上一周期总量：使 7 天 delta 显示为 ↓12.6%（prev = total / (1 - 0.126)）
const TOTAL_PREV_7D = Math.round(TOTAL_7D / (1 - 0.126));

// 更早的 16 天权重（填充 30 天趋势），按上一周期单权重比例缩放
const OLD_WEIGHTS = [
  65, 72, 58, 80, 50, 68, 75, 62, 70, 55, 78, 60, 82, 66, 54, 70,
] as const;

function pad2(n: number): string {
  return String(n).padStart(2, "0");
}

function ymd(d: Date): string {
  return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;
}

function isoOffset(daysFromNow: number, hours = 0, minutes = 0): string {
  const d = new Date();
  d.setDate(d.getDate() + daysFromNow);
  d.setHours(d.getHours() + hours, d.getMinutes() + minutes, 0, 0);
  return d.toISOString();
}

export function buildSeedQuotas(collectedAt: string): QuotaSnapshot[] {
  const q = (
    platform: Platform,
    accountName: string,
    metric: QuotaSnapshot["metric"],
    used: number,
    limit: number,
    unit: QuotaSnapshot["unit"],
    resetsAt?: string,
  ): QuotaSnapshot => ({ platform, accountName, metric, used, limit, unit, resetsAt, collectedAt });

  return [
    // Codex Plus
    q("codex", "ChatGPT Plus", "five_hour", 72, 100, "percent", isoOffset(0, 1, 42)),
    q("codex", "ChatGPT Plus", "weekly", 38, 100, "percent", nextMonday08()),
    // 火山方舟 企业版
    q("ark", "ARK 企业版", "five_hour", 54, 100, "percent", isoOffset(0, 3, 11)),
    q("ark", "ARK 企业版", "weekly", 83, 100, "percent", isoOffset(2, 0, 0)),
    // Kiro Pro（Credits）
    q("kiro", "Kiro Pro", "credits", 760, 2000, "credits", isoOffset(9, 0, 0)),
    // Qoder Pro（Credits）
    q("qoder", "Qoder Pro", "credits", 480, 1600, "credits", isoOffset(12, 0, 0)),
    q("qoder", "Qoder Pro", "addon_credits", 147, 2000, "credits", isoOffset(12, 0, 0)),
    // OpenCode Go 个人版
    q("opencode-go", "OpenCode Go 个人版", "five_hour", 21, 100, "percent", isoOffset(0, 4, 7)),
    q("opencode-go", "OpenCode Go 个人版", "weekly", 64, 100, "percent", isoOffset(5, 0, 0)),
  ];
}

function nextMonday08(): string {
  const d = new Date();
  const day = d.getDay(); // 0=Sun..6=Sat
  const daysUntilMonday = ((8 - day) % 7) || 7;
  const monday = new Date(d);
  monday.setDate(d.getDate() + daysUntilMonday);
  monday.setHours(8, 0, 0, 0);
  return monday.toISOString();
}

export function buildSeedTokens(collectedAt: string): TokenDailyUsage[] {
  const today = new Date();
  today.setHours(0, 0, 0, 0);

  // 构建最近 30 天的"日总量"数组，索引 0 = 今天
  // - 最近 7 天：按原型柱高比例分配，总量严格 = TOTAL_7D
  // - 上一周期 7 天（8..14 天前）：总量 = TOTAL_PREV_7D，使 7 天 delta = ↓12.6%
  // - 更早 16 天：按上一周期单权重比例缩放，填充 30 天趋势
  const last7Sum = DAY_WEIGHTS.reduce((a, b) => a + b, 0);
  const prevWeights = [120, 40, 90, 30, 50, 60, 20];
  const prevSum = prevWeights.reduce((a, b) => a + b, 0);
  const prevUnit = TOTAL_PREV_7D / prevSum;

  const dayTotals: number[] = [];
  // 索引 0..6 = 最近 7 天（0 = 今天）
  // DAY_WEIGHTS 按时间顺序排列（首位 = 最旧的一天，末位 = 今天），故反向取
  for (let i = 0; i < DAY_WEIGHTS.length; i++) {
    const w = DAY_WEIGHTS[DAY_WEIGHTS.length - 1 - i];
    dayTotals.push(Math.round((w / last7Sum) * TOTAL_7D));
  }
  // 索引 7..13 = 上一周期，调整最后一日吸收取整误差
  let prevAcc = 0;
  for (let i = 0; i < prevWeights.length - 1; i++) {
    const t = Math.round(prevWeights[i] * prevUnit);
    dayTotals.push(t);
    prevAcc += t;
  }
  dayTotals.push(TOTAL_PREV_7D - prevAcc);
  // 索引 14..29 = 更早 16 天
  for (let i = 0; i < OLD_WEIGHTS.length; i++) {
    dayTotals.push(Math.round(OLD_WEIGHTS[i] * prevUnit));
  }

  const records: TokenDailyUsage[] = [];
  for (let i = 0; i < dayTotals.length; i++) {
    const day = new Date(today);
    day.setDate(today.getDate() - i);
    const date = ymd(day);
    const dayTotal = dayTotals[i];
    // 平台拆分；最后一平台吸收取整误差，保证当日三平台之和 = dayTotal
    let acc = 0;
    for (let p = 0; p < PLATFORM_SPLIT.length; p++) {
      const { platform } = PLATFORM_SPLIT[p];
      const platformTotal =
        p < PLATFORM_SPLIT.length - 1
          ? Math.round(dayTotal * PLATFORM_SPLIT[p].ratio)
          : dayTotal - acc;
      acc += platformTotal;
      const input = Math.round(platformTotal * INPUT_RATIO);
      const output = platformTotal - input;
      records.push({
        platform,
        date,
        inputTokens: input,
        outputTokens: output,
        requestCount: Math.round(platformTotal / 4200),
        collectedAt,
      });
    }
  }

  return records;
}

export function buildSeedSyncInfo(): SyncInfo {
  const twoMinAgo = new Date(Date.now() - 2 * 60 * 1000).toISOString();
  return { lastSyncAt: twoMinAgo, syncing: false };
}

export const SEED_INPUT_7D = INPUT_7D;
export const SEED_OUTPUT_7D = OUTPUT_7D;
