import { computed, ref } from "vue";
import {
  computeMonthlySubscriptionSpend,
  subscriptionService,
  type SubscriptionDraft,
} from "@/services/subscription-service";
import { billService, type BillDraft } from "@/services/bill-service";
import type { AiSubscription, BillCategory, BillEntry } from "@/types/usage";

const subscriptions = ref<AiSubscription[]>(subscriptionService.list());
const preferences = ref(subscriptionService.getPreferences());
const bills = ref<BillEntry[]>(billService.list());

function refreshBills(): void {
  bills.value = billService.list();
}

function refresh(): void {
  subscriptions.value = subscriptionService.list();
  preferences.value = subscriptionService.getPreferences();
  refreshBills();
}

export function daysUntil(date: string): number {
  const now = new Date();
  const today = Date.UTC(now.getFullYear(), now.getMonth(), now.getDate());
  const [year, month, day] = date.split("-").map(Number);
  return Math.ceil((Date.UTC(year, month - 1, day) - today) / 86_400_000);
}

export function useSubscriptions() {
  const active = computed(() => subscriptions.value.filter((item) => item.status === "active"));
  const monthlyUsd = computed(() => active.value
    .filter((item) => item.currency === "USD")
    .reduce((sum, item) => sum + item.price / (item.billingCycle === "yearly" ? 12 : 1), 0));
  const usdToCnyRate = computed(() => preferences.value.usdToCnyRate);
  // 计划口径：订阅月支出（与账单流水的「实际已记」相互独立）。
  const monthlyCny = computed(() =>
    computeMonthlySubscriptionSpend(subscriptions.value, usdToCnyRate.value),
  );
  const upcomingCount = computed(() => active.value.filter((item) => {
    const days = daysUntil(item.nextBillingDate);
    return days >= 0 && days <= 7;
  }).length);

  function toCny(bill: BillEntry): number {
    return bill.currency === "USD" ? bill.amount * usdToCnyRate.value : bill.amount;
  }

  function billsForMonth(month: string, category?: BillCategory | "all"): BillEntry[] {
    return bills.value
      .filter((bill) => bill.date.slice(0, 7) === month)
      .filter((bill) => !category || category === "all" || bill.category === category)
      .sort((a, b) => b.date.localeCompare(a.date));
  }

  function monthTotalCny(month: string, category?: BillCategory | "all"): number {
    return billsForMonth(month, category).reduce((sum, bill) => sum + toCny(bill), 0);
  }

  return {
    subscriptions: computed(() => subscriptions.value),
    bills: computed(() => bills.value),
    monthlyCny,
    monthlyUsd,
    usdToCnyRate,
    upcomingCount,
    toCny,
    billsForMonth,
    monthTotalCny,
    addBill(draft: BillDraft) {
      billService.add(draft);
      refreshBills();
    },
    updateBill(id: string, patch: Partial<BillDraft>) {
      billService.update(id, patch);
      refreshBills();
    },
    removeBill(id: string) {
      billService.remove(id);
      refreshBills();
    },
    save(draft: SubscriptionDraft, id?: string) {
      subscriptionService.save(draft, id);
      refresh();
    },
    toggle(id: string, status: AiSubscription["status"]) {
      subscriptionService.setStatus(id, status);
      refresh();
    },
    remove(id: string) {
      subscriptionService.remove(id);
      refresh();
    },
    setUsdToCnyRate(value: number) {
      if (!Number.isFinite(value) || value <= 0) return;
      subscriptionService.saveUsdToCnyRate(value);
      refresh();
    },
  };
}
