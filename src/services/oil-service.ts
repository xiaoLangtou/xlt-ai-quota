import { OilPriceConnector } from "@/connectors/oil-price";
import { settings } from "@/config/settings";
import type { OilMonitorSnapshot, OilPriceChange } from "@/types/oil";

const STORAGE_KEY = "xlt.oil-monitor";

export class OilService {
  private readonly connector = new OilPriceConnector();

  getSnapshot(): OilMonitorSnapshot | null {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    const snapshot = JSON.parse(raw) as OilMonitorSnapshot;
    const province = settings.getOilConfig().province;
    return province && snapshot.province === province && snapshot.priceDate && snapshot.priceSourceUrl
      ? snapshot
      : null;
  }

  isConfigured(): boolean {
    return Boolean(settings.getOilConfig().province);
  }

  async sync(): Promise<{
    snapshot: OilMonitorSnapshot;
    priceChanges: OilPriceChange[];
    officialChanged: boolean;
  }> {
    const config = settings.getOilConfig();
    if (!config.province) throw new Error("请先在连接与设置中选择油价监控省份");
    const previous = this.getSnapshot();
    const snapshot = await this.connector.fetch(config);
    const priceChanges = previous?.province === snapshot.province
      ? snapshot.prices.flatMap((item) => {
          const old = previous.prices.find((candidate) => candidate.grade === item.grade);
          if (!old || old.price === item.price) return [];
          return [{
            grade: item.grade,
            name: item.name,
            previous: old.price,
            current: item.price,
            delta: Math.round((item.price - old.price) * 100) / 100,
          }];
        })
      : [];
    const officialChanged = Boolean(
      previous && previous.officialAdjustment.sourceUrl !== snapshot.officialAdjustment.sourceUrl,
    );
    localStorage.setItem(STORAGE_KEY, JSON.stringify(snapshot));
    return { snapshot, priceChanges, officialChanged };
  }
}

export const oilService = new OilService();
