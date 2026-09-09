<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { daysUntil, useSubscriptions } from "@/composables/useSubscriptions";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { Select } from "@/components/ui/select";
import { Input } from "@/components/ui/input";
import { DatePicker } from "@/components/ui/date-picker";
import { MonthPicker } from "@/components/ui/month-picker";
import { Table, TableBody, TableCell, TableFooter, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Menu } from "@/components/ui/menu";
import {
  BILL_CATEGORY_LABEL,
  BILL_CATEGORY_OPTIONS,
  BILL_PAYMENT_LABEL,
  BILL_PAYMENT_OPTIONS,
  BILL_SOURCE_LABEL,
  type AiSubscription,
  type BillCategory,
  type BillEntry,
  type BillPaymentMethod,
  type BillSource,
  type SubscriptionBillingCycle,
} from "@/types/usage";

const props = defineProps<{ open?: boolean; embedded?: boolean }>();
const emit = defineEmits<{ (e: "update:open", value: boolean): void }>();

const COMMON_PROVIDERS = [
  "ChatGPT",
  "Claude",
  "Kiro",
  "Gemini",
  "Cursor",
  "GitHub Copilot",
  "Midjourney",
  "OpenCode",
  "Qoder",
] as const;
const CUSTOM_PROVIDER = "__custom__";
// reka-ui 的 SelectItem 不允许空字符串值，用哨兵值表示「不关联订阅」。
const NO_SUBSCRIPTION = "__none__";

type FormState = {
  providerChoice: string;
  customProvider: string;
  planName: string;
  account: string;
  price: number | null;
  currency: "CNY" | "USD";
  billingCycle: SubscriptionBillingCycle;
  nextBillingDate: string;
  category: BillCategory;
  paymentMethod: BillPaymentMethod;
  note: string;
};

type BillFormState = {
  date: string;
  subscriptionId: string;
  service: string;
  category: BillCategory;
  amount: number | null;
  currency: "CNY" | "USD";
  paymentMethod: BillPaymentMethod;
  source: BillSource;
  note: string;
};

const {
  subscriptions,
  usdToCnyRate,
  billsForMonth,
  monthTotalCny,
  addBill,
  updateBill,
  removeBill,
  save,
  toggle,
  remove,
  setUsdToCnyRate,
} = useSubscriptions();

const activeView = ref<"subscriptions" | "bills">("subscriptions");
const exchangeRate = ref(usdToCnyRate.value);

// ---- 账单流水筛选 ----
const selectedMonth = ref(new Date().toISOString().slice(0, 7));
const selectedCategory = ref<BillCategory | "all">("all");
const categoryFilterOptions = [
  { value: "all", label: "全部分类" },
  ...BILL_CATEGORY_OPTIONS,
];
const sourceOptions = [
  { value: "manual", label: "手动记账" },
  { value: "usage_settlement", label: "用量结算" },
];
const monthBills = computed(() => billsForMonth(selectedMonth.value, selectedCategory.value));
const monthTotal = computed(() => monthTotalCny(selectedMonth.value, selectedCategory.value));
const isCurrentMonth = computed(() => selectedMonth.value === new Date().toISOString().slice(0, 7));
const monthFooterLabel = computed(() => {
  const [year, m] = selectedMonth.value.split("-").map(Number);
  return `${year} 年 ${m} 月合计${isCurrentMonth.value ? "（截至今日）" : ""}`;
});

// ---- 订阅表单 ----
const editingId = ref<string | null>(null);
const editingStatus = ref<AiSubscription["status"]>("active");
const showForm = ref(false);
const formError = ref("");

function emptyForm(): FormState {
  return {
    providerChoice: "ChatGPT",
    customProvider: "",
    planName: "",
    account: "",
    price: null,
    currency: "CNY",
    billingCycle: "monthly",
    nextBillingDate: "",
    category: "model_subscription",
    paymentMethod: "credit_card",
    note: "",
  };
}

const form = reactive<FormState>(emptyForm());
const isCustomProvider = computed(() => form.providerChoice === CUSTOM_PROVIDER);
const providerValue = computed(() =>
  isCustomProvider.value ? form.customProvider.trim() : form.providerChoice,
);
const editing = computed(() => editingId.value != null);

function resetForm(): void {
  Object.assign(form, emptyForm());
  editingId.value = null;
  editingStatus.value = "active";
  formError.value = "";
}

function openCreate(): void {
  resetForm();
  showForm.value = true;
}

function openEdit(item: AiSubscription): void {
  Object.assign(form, {
    providerChoice: COMMON_PROVIDERS.includes(item.provider as (typeof COMMON_PROVIDERS)[number])
      ? item.provider
      : CUSTOM_PROVIDER,
    customProvider: COMMON_PROVIDERS.includes(item.provider as (typeof COMMON_PROVIDERS)[number])
      ? ""
      : item.provider,
    planName: item.planName,
    account: item.account,
    price: item.price,
    currency: item.currency,
    billingCycle: item.billingCycle,
    nextBillingDate: item.nextBillingDate,
    category: item.category ?? "model_subscription",
    paymentMethod: item.paymentMethod ?? "credit_card",
    note: item.note,
  });
  editingId.value = item.id;
  editingStatus.value = item.status;
  formError.value = "";
  showForm.value = true;
}

function submit(): void {
  if (!providerValue.value || !form.planName.trim() || !form.account.trim() || !form.nextBillingDate) {
    formError.value = "请填写服务商、套餐、账号与下次扣费日期。";
    return;
  }
  if (form.price == null || !Number.isFinite(form.price) || form.price < 0) {
    formError.value = "请输入有效的订阅价格。";
    return;
  }
  save({
    provider: providerValue.value,
    planName: form.planName.trim(),
    account: form.account.trim(),
    price: form.price,
    currency: form.currency,
    billingCycle: form.billingCycle,
    nextBillingDate: form.nextBillingDate,
    status: editingStatus.value,
    category: form.category,
    paymentMethod: form.paymentMethod,
    note: form.note.trim(),
  }, editingId.value ?? undefined);
  showForm.value = false;
  resetForm();
}

// ---- 账单表单（手动记账 / 编辑） ----
const showBillForm = ref(false);
const editingBillId = ref<string | null>(null);
const billFormError = ref("");
const editingBill = computed(() => editingBillId.value != null);

function emptyBillForm(): BillFormState {
  return {
    date: new Date().toISOString().slice(0, 10),
    subscriptionId: NO_SUBSCRIPTION,
    service: "",
    category: "model_subscription",
    amount: null,
    currency: "CNY",
    paymentMethod: "alipay",
    source: "manual",
    note: "",
  };
}
const billForm = reactive<BillFormState>(emptyBillForm());
const subscriptionLinkOptions = computed(() => [
  { value: NO_SUBSCRIPTION, label: "不关联（手动记账）" },
  ...subscriptions.value.map((item) => ({
    value: item.id,
    label: `${item.provider} · ${item.planName}`,
  })),
]);

// 关联订阅后自动带出服务/分类/支付方式（仅新增时）。
watch(
  () => billForm.subscriptionId,
  (id) => {
    if (editingBillId.value || !id || id === NO_SUBSCRIPTION) return;
    const sub = subscriptions.value.find((item) => item.id === id);
    if (!sub) return;
    billForm.service = sub.provider;
    billForm.category = sub.category ?? "model_subscription";
    billForm.paymentMethod = sub.paymentMethod ?? billForm.paymentMethod;
    billForm.currency = sub.currency;
    billForm.amount = sub.price;
  },
);

function resetBillForm(): void {
  Object.assign(billForm, emptyBillForm());
  editingBillId.value = null;
  billFormError.value = "";
}

function openBillCreate(): void {
  resetBillForm();
  billForm.date = `${selectedMonth.value}-${new Date().toISOString().slice(8, 10)}`;
  showBillForm.value = true;
}

function openBillEdit(bill: BillEntry): void {
  Object.assign(billForm, {
    date: bill.date,
    subscriptionId: bill.subscriptionId ?? NO_SUBSCRIPTION,
    service: bill.service,
    category: bill.category,
    amount: bill.amount,
    currency: bill.currency,
    paymentMethod: bill.paymentMethod,
    source: bill.source,
    note: bill.note,
  });
  editingBillId.value = bill.id;
  billFormError.value = "";
  showBillForm.value = true;
}

function submitBill(): void {
  if (!billForm.date || !billForm.service.trim()) {
    billFormError.value = "请填写日期与服务名称。";
    return;
  }
  if (billForm.amount == null || !Number.isFinite(billForm.amount) || billForm.amount < 0) {
    billFormError.value = "请输入有效的金额。";
    return;
  }
  const draft = {
    date: billForm.date,
    subscriptionId:
      billForm.subscriptionId && billForm.subscriptionId !== NO_SUBSCRIPTION
        ? billForm.subscriptionId
        : undefined,
    service: billForm.service.trim(),
    category: billForm.category,
    amount: billForm.amount,
    currency: billForm.currency,
    paymentMethod: billForm.paymentMethod,
    source: billForm.source,
    note: billForm.note.trim(),
  };
  if (editingBillId.value) updateBill(editingBillId.value, draft);
  else addBill(draft);
  showBillForm.value = false;
  resetBillForm();
}

function confirmRemoveBill(bill: BillEntry): void {
  if (window.confirm(`删除「${bill.date} · ${bill.service}」这条账单？`)) {
    removeBill(bill.id);
  }
}

// ---- 通用 ----
function close(): void {
  emit("update:open", false);
}

function formatMoney(value: number, currency: "CNY" | "USD"): string {
  return new Intl.NumberFormat("zh-CN", {
    style: "currency",
    currency,
    maximumFractionDigits: 2,
  }).format(value);
}

function saveExchangeRate(): void {
  setUsdToCnyRate(exchangeRate.value);
  exchangeRate.value = usdToCnyRate.value;
}

function billingLabel(item: AiSubscription): string {
  return item.billingCycle === "yearly" ? "年付" : "月付";
}

function dueLabel(date: string): string {
  const days = daysUntil(date);
  if (days < 0) return `已逾期 ${Math.abs(days)} 天`;
  if (days === 0) return "今日扣费";
  if (days === 1) return "明日扣费";
  return `${days} 天后扣费`;
}

function subscriptionStatus(item: AiSubscription): "active" | "soon" | "overdue" | "paused" {
  if (item.status === "paused") return "paused";
  const days = daysUntil(item.nextBillingDate);
  if (days < 0) return "overdue";
  if (days <= 7) return "soon";
  return "active";
}

function subscriptionStatusLabel(item: AiSubscription): string {
  const status = subscriptionStatus(item);
  if (status === "paused") return "已暂停";
  if (status === "overdue") return "已逾期";
  if (status === "soon") return "即将续费";
  return "生效中";
}

function confirmRemove(item: AiSubscription): void {
  if (window.confirm(`删除「${item.provider} · ${item.planName}」订阅记录？`)) {
    remove(item.id);
  }
}

function onSubAction(item: AiSubscription, action: string): void {
  if (action === "toggle") toggle(item.id, item.status === "active" ? "paused" : "active");
  else if (action === "remove") confirmRemove(item);
}

watch(
  () => props.open,
  (open) => {
    if (!open) {
      showForm.value = false;
      showBillForm.value = false;
      resetForm();
      resetBillForm();
    }
  },
);

watch(usdToCnyRate, (value) => {
  exchangeRate.value = value;
});
</script>

<template>
  <Dialog :open="Boolean(open)" :static="embedded" content-class="subscription-dialog" @update:open="close">
        <section v-if="embedded || open" class="panel" :class="{ embedded }" aria-label="AI 订阅与账单管理">
          <header class="panel-head">
            <div>
              <span class="eyebrow">SUBSCRIPTIONS &amp; BILLING</span>
              <h2>订阅与账单</h2>
              <p>管理 AI 服务订阅，并按月查看自动生成 + 手动记账的账单流水。</p>
            </div>
            <Button v-if="!embedded" variant="ghost" size="icon" aria-label="关闭" @click="close">×</Button>
          </header>

          <div class="manager-workspace">
            <Tabs v-model="activeView" class="workspace-tabs">
              <div class="ws-bar">
                <TabsList aria-label="订阅工作区">
                  <TabsTrigger value="subscriptions">订阅</TabsTrigger>
                  <TabsTrigger value="bills">账单流水</TabsTrigger>
                </TabsList>
                <div class="ws-actions">
                  <template v-if="activeView === 'subscriptions'">
                    <label class="toolbar-rate"><span>USD 汇率</span><b>1 USD = ¥</b><Input v-model="exchangeRate" type="number" min="0.01" step="0.01" @change="saveExchangeRate" /></label>
                    <Button @click="openCreate">新增订阅</Button>
                  </template>
                  <template v-else>
                    <MonthPicker v-model="selectedMonth" aria-label="选择账单月份" />
                    <Select v-model="selectedCategory" :options="categoryFilterOptions" aria-label="按分类筛选" />
                    <Button variant="outline" @click="openBillCreate">＋ 手动记账</Button>
                  </template>
                </div>
              </div>

              <TabsContent value="subscriptions" class="directory" aria-label="我的订阅">
              <div v-if="subscriptions.length" class="table-wrap"><Table class="workspace-table"><colgroup><col class="col-service"><col class="col-account"><col class="col-amount"><col class="col-cycle"><col class="col-renewal"><col class="col-status"><col class="col-actions"></colgroup><TableHeader><TableRow><TableHead>服务</TableHead><TableHead>账号</TableHead><TableHead class="num">金额</TableHead><TableHead>周期</TableHead><TableHead>下次续费</TableHead><TableHead>状态</TableHead><TableHead class="action-head">操作</TableHead></TableRow></TableHeader><TableBody><TableRow v-for="item in subscriptions" :key="item.id"><TableCell><div class="service-cell"><div class="provider-mark">{{ item.provider.slice(0, 1).toUpperCase() }}</div><div><strong>{{ item.provider }}</strong><small>{{ item.planName }}</small></div></div></TableCell><TableCell class="muted-cell" :title="item.account">{{ item.account }}</TableCell><TableCell class="num money-cell">{{ formatMoney(item.price, item.currency) }}</TableCell><TableCell>{{ billingLabel(item) }}</TableCell><TableCell><span>{{ item.nextBillingDate }}</span><small class="due-copy">{{ dueLabel(item.nextBillingDate) }}</small></TableCell><TableCell><span class="status-pill" :class="subscriptionStatus(item)">{{ subscriptionStatusLabel(item) }}</span></TableCell><TableCell><div class="row-actions"><Button variant="ghost" size="sm" @click="openEdit(item)">编辑</Button><Menu :items="[{ label: item.status === 'active' ? '暂停订阅' : '恢复订阅', value: 'toggle' }, { label: '删除订阅', value: 'remove', danger: true }]" aria-label="更多订阅操作" @select="(v) => onSubAction(item, v)" /></div></TableCell></TableRow></TableBody></Table></div>
              <div v-else class="empty-state"><div>＋</div><strong>还没有订阅记录</strong><p>从常用服务商开始，记录你的套餐和续费日期。</p><Button @click="openCreate">新增第一条订阅</Button></div>
              </TabsContent>

              <TabsContent value="bills" class="bill-workspace" aria-label="账单流水">
                <div v-if="monthBills.length" class="table-wrap">
                  <Table class="workspace-table ledger-table">
                    <colgroup><col class="col-l-date"><col class="col-l-service"><col class="col-l-cat"><col class="col-l-pay"><col class="col-l-src"><col class="col-l-amount"><col class="col-l-actions"></colgroup>
                    <TableHeader><TableRow><TableHead>日期</TableHead><TableHead>服务</TableHead><TableHead>分类</TableHead><TableHead>支付方式</TableHead><TableHead>来源</TableHead><TableHead class="num">金额</TableHead><TableHead class="action-head">操作</TableHead></TableRow></TableHeader>
                    <TableBody>
                      <TableRow v-for="bill in monthBills" :key="bill.id">
                        <TableCell class="date-cell">{{ bill.date.slice(5) }}</TableCell>
                        <TableCell><div class="service-cell"><div class="provider-mark">{{ bill.service.slice(0, 1).toUpperCase() }}</div><div><strong>{{ bill.service }}</strong><small v-if="bill.note">{{ bill.note }}</small></div></div></TableCell>
                        <TableCell><span class="cat-tag">{{ BILL_CATEGORY_LABEL[bill.category] }}</span></TableCell>
                        <TableCell class="muted-cell">{{ BILL_PAYMENT_LABEL[bill.paymentMethod] }}</TableCell>
                        <TableCell><span class="src-text" :class="bill.source">{{ BILL_SOURCE_LABEL[bill.source] }}</span></TableCell>
                        <TableCell class="num money-cell">{{ formatMoney(bill.amount, bill.currency) }}<small v-if="bill.currency === 'USD'">≈ {{ formatMoney(bill.amount * usdToCnyRate, 'CNY') }}</small></TableCell>
                        <TableCell><div class="row-actions"><Button variant="ghost" size="sm" @click="openBillEdit(bill)">编辑</Button><Menu :items="[{ label: '删除账单', value: 'remove', danger: true }]" aria-label="更多账单操作" @select="() => confirmRemoveBill(bill)" /></div></TableCell>
                      </TableRow>
                    </TableBody>
                    <TableFooter><TableRow><TableCell colspan="5">{{ monthFooterLabel }}</TableCell><TableCell class="num money-cell">{{ formatMoney(monthTotal, "CNY") }}</TableCell><TableCell /></TableRow></TableFooter>
                  </Table>
                </div>
                <div v-else class="bill-empty">该月暂无账单 · 点「手动记账」新增，或等待订阅到期自动生成。</div>
              </TabsContent>
            </Tabs>
          </div>

          <Dialog :open="showForm" @update:open="showForm = $event">
            <form class="editor" @submit.prevent="submit">
              <div class="editor-head">
                <h3>{{ editing ? "编辑订阅" : "新增订阅" }}</h3>
                <Button variant="ghost" size="sm" type="button" @click="showForm = false">收起</Button>
              </div>
              <div class="fields">
                <label>
                  <span>服务商</span>
                  <Select v-model="form.providerChoice" :options="[...COMMON_PROVIDERS.map((provider) => ({ value: provider, label: provider })), { value: CUSTOM_PROVIDER, label: '自定义服务商' }]" />
                </label>
                <label v-if="isCustomProvider">
                  <span>自定义服务商名称</span>
                  <Input v-model.trim="form.customProvider" placeholder="例如 Perplexity" maxlength="50" />
                </label>
                <label>
                  <span>套餐名称</span>
                  <Input v-model.trim="form.planName" placeholder="例如 Plus、Pro、Team" maxlength="80" />
                </label>
                <label>
                  <span>账号</span>
                  <Input v-model.trim="form.account" placeholder="邮箱或账号备注" maxlength="120" />
                </label>
                <label>
                  <span>订阅价格</span>
                  <Input v-model.number="form.price" type="number" min="0" step="0.01" placeholder="0.00" />
                </label>
                <label>
                  <span>币种</span>
                  <Select v-model="form.currency" :options="[{ value: 'CNY', label: 'CNY（人民币）' }, { value: 'USD', label: 'USD（美元）' }]" />
                </label>
                <label>
                  <span>计费周期</span>
                  <Select v-model="form.billingCycle" :options="[{ value: 'monthly', label: '月付' }, { value: 'yearly', label: '年付' }]" />
                </label>
                <label>
                  <span>下次扣费日期</span>
                  <DatePicker v-model="form.nextBillingDate" placeholder="选择下次扣费日期" />
                </label>
                <label>
                  <span>账单分类</span>
                  <Select v-model="form.category" :options="BILL_CATEGORY_OPTIONS" />
                </label>
                <label>
                  <span>支付方式</span>
                  <Select v-model="form.paymentMethod" :options="BILL_PAYMENT_OPTIONS" />
                </label>
                <label class="wide">
                  <span>备注（可选）</span>
                  <Input v-model.trim="form.note" placeholder="例如家庭组、自动续费等" maxlength="160" />
                </label>
              </div>
              <p v-if="formError" class="form-error">{{ formError }}</p>
              <footer class="editor-foot">
                <Button variant="outline" type="button" @click="showForm = false">取消</Button>
                <Button type="submit">{{ editing ? "保存修改" : "保存订阅" }}</Button>
              </footer>
            </form>
          </Dialog>

          <Dialog :open="showBillForm" @update:open="showBillForm = $event">
            <form class="editor" @submit.prevent="submitBill">
              <div class="editor-head">
                <h3>{{ editingBill ? "编辑账单" : "手动记账" }}</h3>
                <Button variant="ghost" size="sm" type="button" @click="showBillForm = false">收起</Button>
              </div>
              <div class="fields">
                <label>
                  <span>日期</span>
                  <DatePicker v-model="billForm.date" placeholder="选择账单日期" />
                </label>
                <label>
                  <span>关联订阅（可选）</span>
                  <Select v-model="billForm.subscriptionId" :options="subscriptionLinkOptions" />
                </label>
                <label>
                  <span>服务</span>
                  <Input v-model.trim="billForm.service" placeholder="例如 OpenAI API、Cursor" maxlength="80" />
                </label>
                <label>
                  <span>分类</span>
                  <Select v-model="billForm.category" :options="BILL_CATEGORY_OPTIONS" />
                </label>
                <label>
                  <span>金额</span>
                  <Input v-model.number="billForm.amount" type="number" min="0" step="0.01" placeholder="0.00" />
                </label>
                <label>
                  <span>币种</span>
                  <Select v-model="billForm.currency" :options="[{ value: 'CNY', label: 'CNY（人民币）' }, { value: 'USD', label: 'USD（美元）' }]" />
                </label>
                <label>
                  <span>支付方式</span>
                  <Select v-model="billForm.paymentMethod" :options="BILL_PAYMENT_OPTIONS" />
                </label>
                <label>
                  <span>来源</span>
                  <Select v-model="billForm.source" :options="sourceOptions" />
                </label>
                <label class="wide">
                  <span>备注（可选）</span>
                  <Input v-model.trim="billForm.note" placeholder="例如充值、按量结算说明等" maxlength="160" />
                </label>
              </div>
              <p v-if="billFormError" class="form-error">{{ billFormError }}</p>
              <footer class="editor-foot">
                <Button variant="outline" type="button" @click="showBillForm = false">取消</Button>
                <Button type="submit">{{ editingBill ? "保存修改" : "保存账单" }}</Button>
              </footer>
            </form>
          </Dialog>
        </section>
  </Dialog>
</template>

<style scoped>
/* 订阅与账单工作台：一套 token 化视觉系统，随主题自适应。 */
.panel {
  position: relative;
  width: min(1120px, calc(100vw - 32px));
  max-height: calc(100vh - 32px);
  overflow: auto;
  padding: 28px;
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  background: var(--surface);
  box-shadow: var(--shadow-pop);
}
.panel.embedded {
  width: 100%;
  max-width: 1320px;
  max-height: none;
  margin: 0 auto;
  padding: 0;
  overflow: visible;
  border: 0;
  border-radius: 0;
  background: transparent;
  box-shadow: none;
}
.panel-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 20px;
  margin-bottom: 22px;
}
.eyebrow {
  color: var(--text-subtle);
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 1.5px;
}
.panel-head h2 {
  margin: 5px 0 0;
  font-size: 22px;
  font-weight: 650;
}
.panel-head p {
  margin: 6px 0 0;
  color: var(--text-muted);
  font-size: 13px;
}

.manager-workspace {
  width: 100%;
}
.workspace-tabs {
  gap: 0;
}
.ws-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  flex-wrap: wrap;
  margin: 2px 0 18px;
  padding-bottom: 16px;
  border-bottom: 1px solid var(--border);
}
.ws-actions {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}
.ws-actions :deep(.cn-month-picker-trigger) {
  width: 150px;
  height: 34px;
}
.ws-actions :deep(.cn-select-trigger) {
  width: 128px;
  height: 34px;
}
.toolbar-rate {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--text-muted);
  font-size: 12px;
}
.toolbar-rate > span {
  display: none;
}
.toolbar-rate b {
  font-size: 11px;
  white-space: nowrap;
}
.toolbar-rate :deep(.cn-input) {
  width: 76px;
  height: 30px;
  text-align: center;
  font-family: var(--font-mono);
}

.directory,
.bill-workspace {
  width: 100%;
}
.table-wrap {
  width: 100%;
  overflow-x: auto;
}
.empty-state,
.bill-empty {
  display: grid;
  min-height: 220px;
  place-content: center;
  border: 1px dashed var(--border-strong);
  border-radius: var(--r-md);
  color: var(--text-muted);
  text-align: center;
}
.empty-state > div {
  margin-bottom: 8px;
  color: var(--accent);
  font-size: 28px;
}
.empty-state strong {
  color: var(--text);
  font-size: 14px;
}
.empty-state p {
  margin: 6px 0 16px;
  font-size: 13px;
}

:deep(.workspace-table) {
  width: 100%;
  min-width: 980px;
  table-layout: fixed;
  border-collapse: collapse;
  color: var(--text);
  font-size: 13px;
}
:deep(.workspace-table .col-service) { width: 23%; }
:deep(.workspace-table .col-account) { width: 15%; }
:deep(.workspace-table .col-amount) { width: 210px; }
:deep(.workspace-table .col-cycle) { width: 78px; }
:deep(.workspace-table .col-renewal) { width: 158px; }
:deep(.workspace-table .col-status) { width: 112px; }
:deep(.workspace-table .col-actions) { width: 150px; }
:deep(.workspace-table th) {
  padding: 0 14px 11px 0;
  border-bottom: 1px solid var(--border);
  color: var(--text-muted);
  font-size: 11.5px;
  font-weight: 600;
  text-align: left;
  white-space: nowrap;
}
:deep(.workspace-table td) {
  padding: 14px 14px 14px 0;
  overflow: hidden;
  border-bottom: 1px solid var(--border);
  vertical-align: middle;
}
:deep(.workspace-table tbody tr:hover) {
  background: var(--surface-2);
}
:deep(.workspace-table th.num),
:deep(.workspace-table td.num) {
  text-align: right;
}
:deep(.workspace-table .action-head) {
  padding-left: 8px;
}
:deep(.workspace-table tfoot td) {
  padding: 13px 14px 13px 0;
  border-top: 1px solid var(--border);
  border-bottom: 1px solid var(--border);
  color: var(--text);
  font-weight: 650;
}

/* 账单台账列宽 */
:deep(.ledger-table) {
  min-width: 860px;
}
:deep(.ledger-table .col-l-date) { width: 78px; }
:deep(.ledger-table .col-l-service) { width: auto; }
:deep(.ledger-table .col-l-cat) { width: 118px; }
:deep(.ledger-table .col-l-pay) { width: 104px; }
:deep(.ledger-table .col-l-src) { width: 110px; }
:deep(.ledger-table .col-l-amount) { width: 168px; }
:deep(.ledger-table .col-l-actions) { width: 116px; }

.service-cell {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}
.provider-mark {
  display: grid;
  width: 30px;
  height: 30px;
  flex: 0 0 30px;
  place-items: center;
  border-radius: var(--r-sm);
  background: var(--accent-weak);
  color: var(--accent);
  font-family: var(--font-mono);
  font-size: 13px;
  font-weight: 700;
}
.service-cell strong,
.service-cell small,
.money-cell small,
.due-copy {
  display: block;
}
.service-cell strong {
  color: var(--text);
  font-size: 13px;
}
.service-cell small,
.muted-cell,
.money-cell small,
.due-copy {
  margin-top: 3px;
  overflow: hidden;
  color: var(--text-muted);
  font-size: 11.5px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.money-cell,
.date-cell {
  font-family: var(--font-mono);
  white-space: nowrap;
}
.money-cell {
  overflow: hidden;
  text-overflow: ellipsis;
  font-variant-numeric: tabular-nums;
}
.date-cell {
  color: var(--text-muted);
}

.cat-tag {
  display: inline-block;
  padding: 2px 8px;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: var(--surface-2);
  color: var(--text-muted);
  font-size: 11px;
  white-space: nowrap;
}
.src-text {
  color: var(--text-muted);
  font-size: 12px;
}
.src-text.manual {
  color: var(--accent);
}
.src-text.usage_settlement {
  color: var(--u-warn);
}

.status-pill {
  display: inline-block;
  padding: 3px 8px;
  border-radius: 999px;
  font-size: 11.5px;
  font-weight: 550;
  white-space: nowrap;
}
.status-pill.active {
  background: color-mix(in srgb, var(--u-ok) 16%, transparent);
  color: var(--u-ok);
}
.status-pill.soon {
  background: color-mix(in srgb, var(--u-warn) 16%, transparent);
  color: var(--u-warn);
}
.status-pill.overdue {
  background: color-mix(in srgb, var(--u-crit) 16%, transparent);
  color: var(--u-crit);
}
.status-pill.paused {
  background: var(--surface-2);
  color: var(--text-muted);
}

.row-actions {
  display: flex;
  gap: 2px;
  white-space: nowrap;
}
.row-actions :deep(.cn-button) {
  height: 1.7rem;
  padding: 0 0.35rem;
  color: var(--text-muted);
  font-size: 12px;
}
.row-actions :deep(.cn-button:hover) {
  color: var(--text);
  background: transparent;
}
.row-actions :deep(.delete-action) {
  color: var(--u-crit);
}

.editor {
  padding: 24px;
  background: var(--surface);
}
.editor-head,
.editor-foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}
.editor h3 {
  margin: 0;
  font-size: 18px;
  font-weight: 650;
}
.fields {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 14px;
  margin-top: 18px;
}
.fields label {
  min-width: 0;
}
.fields label > span {
  display: block;
  margin-bottom: 6px;
  color: var(--text-muted);
  font-size: 12px;
  font-weight: 600;
}
.wide {
  grid-column: 1 / -1;
}
.form-error {
  color: var(--u-crit);
  font-size: 12px;
}
.editor-foot {
  justify-content: flex-end;
  margin-top: 20px;
}

@media (max-width: 760px) {
  .panel {
    padding: 20px;
  }
  .panel.embedded {
    padding: 0;
  }
  .ws-bar {
    align-items: stretch;
    flex-direction: column;
  }
  .ws-actions {
    width: 100%;
  }
  .ws-actions :deep(.cn-month-picker-trigger),
  .ws-actions :deep(.cn-select-trigger) {
    flex: 1;
    width: auto;
  }
  .ws-actions > :deep(.cn-button) {
    flex: 1;
  }
  .fields {
    grid-template-columns: 1fr;
  }
  .wide {
    grid-column: auto;
  }
}
</style>
