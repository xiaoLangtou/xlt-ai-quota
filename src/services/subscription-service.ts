import { createStorage } from "@/storage/web-storage";
import type { AiSubscription, SubscriptionPreferences } from "@/types/usage";

const storage = createStorage();

export type SubscriptionDraft = Omit<AiSubscription, "id" | "createdAt" | "updatedAt">;

/**
 * 计划口径的订阅月支出（CNY）：所有生效中订阅折算到每月的合计。
 * 年付 ÷ 12，USD 按汇率折算。这是「计划应付」，与账单流水里的「实际已记」相互独立。
 */
export function computeMonthlySubscriptionSpend(
  subscriptions: AiSubscription[],
  usdToCnyRate: number,
): number {
  return subscriptions
    .filter((s) => s.status === "active")
    .reduce((sum, s) => {
      const monthly = s.billingCycle === "yearly" ? s.price / 12 : s.price;
      const cny = s.currency === "USD" ? monthly * usdToCnyRate : monthly;
      return sum + cny;
    }, 0);
}

export const subscriptionService = {
  list(): AiSubscription[] {
    return storage
      .listSubscriptions()
      .sort((a, b) => a.nextBillingDate.localeCompare(b.nextBillingDate));
  },

  save(draft: SubscriptionDraft, id?: string): AiSubscription {
    const now = new Date().toISOString();
    const subscriptions = storage.listSubscriptions();
    const existing = id ? subscriptions.find((item) => item.id === id) : undefined;
    const saved: AiSubscription = {
      ...draft,
      id: existing?.id ?? crypto.randomUUID(),
      createdAt: existing?.createdAt ?? now,
      updatedAt: now,
    };
    storage.saveSubscriptions(
      existing
        ? subscriptions.map((item) => (item.id === existing.id ? saved : item))
        : [...subscriptions, saved],
    );
    return saved;
  },

  setStatus(id: string, status: AiSubscription["status"]): void {
    const updatedAt = new Date().toISOString();
    storage.saveSubscriptions(
      storage
        .listSubscriptions()
        .map((item) => (item.id === id ? { ...item, status, updatedAt } : item)),
    );
  },

  remove(id: string): void {
    // 只移除订阅本身；已产生的账单是历史花费记录，应保留在账单流水中。
    // 订阅删除后自然不会再自动生成新账单。
    storage.saveSubscriptions(storage.listSubscriptions().filter((item) => item.id !== id));
  },

  getPreferences(): SubscriptionPreferences {
    return storage.getSubscriptionPreferences();
  },

  saveUsdToCnyRate(usdToCnyRate: number): void {
    storage.saveSubscriptionPreferences({ usdToCnyRate });
  },
};
