import { createStorage } from "@/storage/web-storage";
import { QUOTA_DISPLAY_TARGETS, type QuotaDisplayTarget } from "@/types/usage";
import type { OilGrade } from "@/types/oil";

const OIL_GRADES: readonly OilGrade[] = ["92", "95", "98", "0"];

const storage = createStorage();

// 安全读取 Vite 环境变量：非 Vite 环境下 import.meta.env 为 undefined，回退到默认值。
const env: Record<string, string | undefined> =
  (typeof import.meta !== "undefined" ? import.meta.env : undefined) ?? {};

const DEFAULT_ARK_BASE_URL = env.VITE_ARK_BASE_URL || "/api/ark";

export const settings = {
  // Ark 走本机 arkcli（dev 由 /api/ark/* 中间件转发），无需应用侧 Key
  getArkConfig() {
    const cfg = storage.getConnectorConfig().ark ?? {};
    return { baseUrl: cfg.baseUrl || DEFAULT_ARK_BASE_URL };
  },

  saveArkBaseUrl(baseUrl: string) {
    const full = storage.getConnectorConfig();
    storage.saveConnectorConfig({ ...full, ark: { baseUrl } });
  },

  getOilConfig(): { province: string; apiKey: string; grade: OilGrade } {
    const cfg = storage.getConnectorConfig().oil ?? {};
    const grade = OIL_GRADES.includes(cfg.grade as OilGrade) ? (cfg.grade as OilGrade) : "92";
    return {
      province: cfg.province?.trim() ?? "",
      apiKey: cfg.apiKey?.trim() ?? "",
      grade,
    };
  },

  saveOilConfig(config: { province: string; apiKey: string; grade?: OilGrade }): void {
    const full = storage.getConnectorConfig();
    const province = config.province.trim();
    const apiKey = config.apiKey.trim();
    const grade = config.grade ?? full.oil?.grade;
    storage.saveConnectorConfig({
      ...full,
      oil: {
        ...(province ? { province } : {}),
        ...(apiKey ? { apiKey } : {}),
        ...(grade ? { grade } : {}),
      },
    });
  },

  getQuotaDisplayConfig(): { hiddenPlatforms: QuotaDisplayTarget[] } {
    const configured = storage.getConnectorConfig().quotaDisplay?.hiddenPlatforms ?? [];
    return {
      hiddenPlatforms: QUOTA_DISPLAY_TARGETS.filter((target) => configured.includes(target)),
    };
  },

  saveQuotaDisplayConfig(config: { hiddenPlatforms: QuotaDisplayTarget[] }): void {
    const full = storage.getConnectorConfig();
    const hiddenPlatforms = QUOTA_DISPLAY_TARGETS.filter((target) =>
      config.hiddenPlatforms.includes(target),
    );
    storage.saveConnectorConfig({
      ...full,
      quotaDisplay: { hiddenPlatforms },
    });
  },

  /** 系统时区（Intl 解析）；解析失败回退 UTC。 */
  systemTimezone(): string {
    try {
      return Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
    } catch {
      return "UTC";
    }
  },

  /** 读取用量偏好：timezone 缺省=系统时区；statsSince 缺省=null。 */
  getPreferences(): { timezone: string; statsSince: string | null } {
    // 非浏览器环境（如 Node 单测）无 localStorage，读取失败时回退默认值。
    let pref: { timezone?: string; statsSince?: string } = {};
    try {
      pref = storage.getConnectorConfig().preferences ?? {};
    } catch {
      pref = {};
    }
    return {
      timezone: pref.timezone?.trim() || this.systemTimezone(),
      statsSince: pref.statsSince?.trim() || null,
    };
  },

  savePreferences(patch: { timezone?: string; statsSince?: string | null }): void {
    const full = storage.getConnectorConfig();
    const prev = full.preferences ?? {};
    const next = { ...prev };
    if (patch.timezone !== undefined) {
      const tz = patch.timezone.trim();
      if (tz && tz !== this.systemTimezone()) next.timezone = tz;
      else delete next.timezone;
    }
    if (patch.statsSince !== undefined) {
      const s = (patch.statsSince ?? "").trim();
      if (s) next.statsSince = s;
      else delete next.statsSince;
    }
    storage.saveConnectorConfig({ ...full, preferences: next });
  },
};
