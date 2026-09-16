import { reactive } from "vue";

export type ToastTone = "success" | "error" | "warn" | "info";

export interface ToastItem {
  id: number;
  tone: ToastTone;
  message: string;
}

const state = reactive<{ items: ToastItem[] }>({ items: [] });
let seq = 0;

/** 弹出一条 toast，duration 毫秒后自动消失（error 默认更久）。 */
export function pushToast(message: string, tone: ToastTone = "info", duration?: number): number {
  const id = ++seq;
  state.items.push({ id, tone, message });
  const ttl = duration ?? (tone === "error" ? 6000 : tone === "warn" ? 5000 : 3000);
  window.setTimeout(() => dismissToast(id), ttl);
  return id;
}

export function dismissToast(id: number): void {
  const idx = state.items.findIndex((t) => t.id === id);
  if (idx >= 0) state.items.splice(idx, 1);
}

/** 供 ToastHost 订阅的响应式列表。 */
export function useToast() {
  return { items: state.items, push: pushToast, dismiss: dismissToast };
}
