import { invoke } from "@tauri-apps/api/core";
import type { TranslateConfig, TranslateOutcome, TranslateShortcutStatus } from "@/types/translate";

export const translateService = {
  getConfig(): Promise<TranslateConfig> {
    return invoke<TranslateConfig>("translate_config_get");
  },

  saveConfig(config: TranslateConfig): Promise<TranslateConfig> {
    return invoke<TranslateConfig>("translate_config_save", { config });
  },

  /** 按空行分段后逐段翻译，返回值与原文段落一一对应。 */
  run(text: string, from: string, to: string, requestId = crypto.randomUUID()): Promise<TranslateOutcome> {
    return invoke<TranslateOutcome>("translate_run", { text, from, to, requestId });
  },

  cancel(requestId: string): Promise<boolean> {
    return invoke<boolean>("translate_cancel", { requestId });
  },

  retry(text: string, index: number, from: string, to: string, requestId: string, expectedConfig: string): Promise<TranslateOutcome> {
    return invoke<TranslateOutcome>("translate_retry", { text, index, from, to, requestId, expectedConfig });
  },

  /** 用固定短句验证应用 ID / 密钥；传草稿时只测试、不落盘。 */
  test(config?: TranslateConfig): Promise<string> {
    return invoke<string>("translate_test", { config: config ?? null });
  },

  showPanel(): Promise<void> {
    return invoke<void>("translate_show_panel");
  },

  hidePanel(): Promise<void> {
    return invoke<void>("translate_hide_panel");
  },

  /** 面板快捷键是否真的注册到了系统（被其他应用占用时为 false）。 */
  shortcutStatus(): Promise<TranslateShortcutStatus> {
    return invoke<TranslateShortcutStatus>("translate_shortcut_status");
  },

  /** 录制后立即生效：只改快捷键并重新注册，不影响其他未保存的改动。 */
  setShortcut(shortcut: string): Promise<TranslateShortcutStatus> {
    return invoke<TranslateShortcutStatus>("translate_set_shortcut", { shortcut });
  },

  /** 翻译页切换内置服务：立即持久化，后续请求按新引擎走。 */
  setEngine(engine: string): Promise<TranslateConfig> {
    return invoke<TranslateConfig>("translate_set_engine", { engine });
  },
};
