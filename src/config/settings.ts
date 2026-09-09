import { createStorage } from "@/storage/web-storage";

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

};
