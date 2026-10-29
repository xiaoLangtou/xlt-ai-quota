import { ref } from "vue";

export interface TranslatePageRequest { text: string; from: string; to: string }
const pendingRequest = ref<TranslatePageRequest | null>(null);

/** 独立面板通过主窗口事件交接文本及语言方向。 */
export function useTranslateRequest() {
  return {
    pendingRequest,
    request(value: TranslatePageRequest) { pendingRequest.value = value; },
    take(): TranslatePageRequest | null {
      const value = pendingRequest.value;
      pendingRequest.value = null;
      return value;
    },
  };
}
