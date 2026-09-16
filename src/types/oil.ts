export type OilGrade = "92" | "95" | "98" | "0";

export type OilForecastDirection = "up" | "down" | "unchanged";

export interface OilPriceItem {
  grade: OilGrade;
  name: string;
  price: number;
  unit: "元/升";
}

export interface OilForecast {
  nextAdjustmentDate: string;
  daysRemaining: number;
  direction: OilForecastDirection;
  estimatedChangePerTon: number;
  estimatedChangePerLiter: number;
  confidence: string;
  analysis: string;
  brentPrice: number;
  brentChange: number;
}

export interface OilOfficialAdjustment {
  title: string;
  publishedAt: string;
  effectiveAt: string;
  direction: OilForecastDirection;
  gasolineChangePerTon?: number;
  dieselChangePerTon?: number;
  sourceUrl: string;
}

export interface OilMonitorSnapshot {
  province: string;
  prices: OilPriceItem[];
  priceDate: string;
  priceSourceUrl: string;
  forecast: OilForecast;
  officialAdjustment: OilOfficialAdjustment;
  collectedAt: string;
}

export interface OilPriceChange {
  grade: OilGrade;
  name: string;
  previous: number;
  current: number;
  delta: number;
}

export interface OilMonitorConfig {
  province: string;
  apiKey: string;
}
