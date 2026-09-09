import { createStorage } from "@/storage/web-storage";
import type { BillEntry } from "@/types/usage";

const storage = createStorage();

export type BillDraft = Omit<BillEntry, "id" | "createdAt" | "updatedAt">;

/** 账单流水的纯 CRUD：账单只记录真实发生的花费（手动记账 / 用量结算），不做任何自动生成。 */
export const billService = {
  list(): BillEntry[] {
    return storage.listBills().slice().sort((a, b) => b.date.localeCompare(a.date));
  },

  add(draft: BillDraft): BillEntry {
    const now = new Date().toISOString();
    const entry: BillEntry = {
      ...draft,
      id: crypto.randomUUID(),
      createdAt: now,
      updatedAt: now,
    };
    storage.saveBills([...storage.listBills(), entry]);
    return entry;
  },

  update(id: string, patch: Partial<BillDraft>): void {
    const now = new Date().toISOString();
    storage.saveBills(
      storage.listBills().map((item) =>
        item.id === id ? { ...item, ...patch, updatedAt: now } : item,
      ),
    );
  },

  remove(id: string): void {
    storage.saveBills(storage.listBills().filter((item) => item.id !== id));
  },
};
