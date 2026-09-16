import { httpGet } from "@/connectors/types";
import type {
  OilForecast,
  OilForecastDirection,
  OilGrade,
  OilMonitorConfig,
  OilMonitorSnapshot,
  OilOfficialAdjustment,
  OilPriceItem,
} from "@/types/oil";

type UnknownRecord = Record<string, unknown>;

type OilQuery = { province: string; apiKey: string | undefined };
type OilRequest = typeof httpGet;

export async function fetchOilPayloads(
  query: OilQuery,
  request: OilRequest = httpGet,
): Promise<{ pricePayload: unknown; forecastPayload: unknown; officialPayload: unknown }> {
  const [pricePayload, forecastPayload, officialPayload] = await Promise.all([
    request({ baseUrl: "/api/oil", path: "/price", query }),
    request({ baseUrl: "/api/oil", path: "/forecast", query }),
    request({ baseUrl: "/api/oil", path: "/adjustment" }),
  ]);
  return { pricePayload, forecastPayload, officialPayload };
}

const PRICE_FIELDS: { grade: OilGrade; name: string; field: string }[] = [
  { grade: "92", name: "92号汽油", field: "92号汽油" },
  { grade: "95", name: "95号汽油", field: "95号汽油" },
  { grade: "98", name: "98号汽油", field: "98号汽油" },
  { grade: "0", name: "0号柴油", field: "0号柴油" },
];

function record(value: unknown, field: string): UnknownRecord {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`油价接口缺少 ${field}`);
  }
  return value as UnknownRecord;
}

function text(value: unknown, field: string): string {
  if (typeof value !== "string" || !value.trim()) throw new Error(`油价接口缺少 ${field}`);
  return value.trim();
}

function number(value: unknown, field: string): number {
  const parsed = typeof value === "number" ? value : Number.parseFloat(String(value ?? ""));
  if (!Number.isFinite(parsed)) throw new Error(`油价接口的 ${field} 不是有效数字`);
  return parsed;
}

function checkResponse(payload: unknown): UnknownRecord {
  const root = record(payload, "响应体");
  if (Number(root.code) !== 0) throw new Error(text(root.msg, "错误说明"));
  return record(root.data, "data");
}

export function parseOilPrices(payload: unknown): {
  province: string;
  prices: OilPriceItem[];
  priceDate: string;
  sourceUrl: string;
} {
  const data = checkResponse(payload);
  const rawPrices = record(data.prices, "prices");
  const prices = PRICE_FIELDS.map(({ grade, name, field }) => ({
    grade,
    name,
    price: number(rawPrices[field], field),
    unit: "元/升" as const,
  }));
  return {
    province: text(data.province, "province"),
    prices,
    priceDate: text(data.price_date, "price_date"),
    sourceUrl: text(data.source_url, "source_url"),
  };
}

function forecastDirection(value: unknown): OilForecastDirection {
  const direction = text(value, "prediction.direction");
  if (direction.includes("涨")) return "up";
  if (direction.includes("跌") || direction.includes("降")) return "down";
  return "unchanged";
}

export function parseOilForecast(payload: unknown): OilForecast {
  const data = checkResponse(payload);
  const prediction = record(data.prediction, "prediction");
  const crude = record(data.crude_oil, "crude_oil");
  const direction = forecastDirection(prediction.direction);
  const estimatedChangePerTon = number(prediction.estimated_change_per_ton, "estimated_change_per_ton");
  const rawLiterChange = number(prediction.estimated_change_per_liter, "estimated_change_per_liter");
  const estimatedChangePerLiter = estimatedChangePerTon < 0
    ? -Math.abs(rawLiterChange)
    : estimatedChangePerTon > 0
      ? Math.abs(rawLiterChange)
      : 0;
  return {
    nextAdjustmentDate: text(data.next_adjust_date, "next_adjust_date"),
    daysRemaining: number(data.days_remaining, "days_remaining"),
    direction,
    estimatedChangePerTon,
    estimatedChangePerLiter,
    confidence: text(prediction.confidence, "prediction.confidence"),
    analysis: text(prediction.analysis, "prediction.analysis"),
    brentPrice: number(crude.brent, "crude_oil.brent"),
    brentChange: number(crude.brent_change, "crude_oil.brent_change"),
  };
}

export function parseOfficialAdjustment(payload: unknown): OilOfficialAdjustment {
  const data = record(payload, "发改委公告");
  const direction = text(data.direction, "direction");
  if (direction !== "up" && direction !== "down" && direction !== "unchanged") {
    throw new Error("发改委公告的调价方向无效");
  }
  const gasoline = data.gasolineChangePerTon;
  const diesel = data.dieselChangePerTon;
  return {
    title: text(data.title, "title"),
    publishedAt: text(data.publishedAt, "publishedAt"),
    effectiveAt: text(data.effectiveAt, "effectiveAt"),
    direction,
    gasolineChangePerTon: gasoline == null ? undefined : number(gasoline, "gasolineChangePerTon"),
    dieselChangePerTon: diesel == null ? undefined : number(diesel, "dieselChangePerTon"),
    sourceUrl: text(data.sourceUrl, "sourceUrl"),
  };
}

export class OilPriceConnector {
  async fetch(config: OilMonitorConfig): Promise<OilMonitorSnapshot> {
    const query = { province: config.province, apiKey: config.apiKey || undefined };
    const { pricePayload, forecastPayload, officialPayload } = await fetchOilPayloads(query);
    const price = parseOilPrices(pricePayload);
    return {
      province: price.province,
      prices: price.prices,
      priceDate: price.priceDate,
      priceSourceUrl: price.sourceUrl,
      forecast: parseOilForecast(forecastPayload),
      officialAdjustment: parseOfficialAdjustment(officialPayload),
      collectedAt: new Date().toISOString(),
    };
  }
}
