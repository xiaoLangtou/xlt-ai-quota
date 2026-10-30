<script setup lang="ts">
import type { PlatformQuotaView, QuotaWindowView } from "@/types/usage";
import ToolLogo from "@/components/ToolLogo.vue";

defineProps<{ view: PlatformQuotaView }>();

function compactLabel(window: QuotaWindowView): string {
  if (window.metric === "five_hour") return "5h 额度";
  if (window.metric === "session") return "Session";
  return window.label;
}

function compactReset(value?: string): string | undefined {
  return value
    ?.replace(/(\d+) 小时 (\d+) 分钟后重置/, "$1h $2m 重置")
    .replace(/(\d+) 小时后重置/, "$1h 重置")
    .replace(/(\d+) 分钟后重置/, "$1m 重置")
    .replace(/(\d+) 天后重置/, "$1d 重置")
    .replace("后重置", "");
}

function usedCredits(view: PlatformQuotaView): number {
  return view.credits ? Math.max(0, view.credits.total - view.credits.remaining) : 0;
}

function creditUsedPct(view: PlatformQuotaView): number {
  const total = view.credits?.total ?? 0;
  if (total <= 0) return 0;
  return Math.min(100, Math.round((usedCredits(view) / total) * 1000) / 10);
}

function money(value: number, currency: string): string {
  const symbol = currency === "USD" ? "$" : "¥";
  return `${symbol}${value.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
}

const STATUS_TEXT: Record<string, string> = {
  "not-installed": "未检测到本地工具",
  "not-signed-in": "请先在原工具中登录",
  expired: "登录已过期，请重新登录",
  "access-denied": "当前账户无权读取额度",
  "custom-provider": "检测到自定义服务地址，已停止默认请求",
  "unsupported-account": "当前账户类型不提供订阅额度",
  "unsupported-plan": "当前套餐不提供可读取额度",
  "unsupported-version": "本地工具版本不兼容",
  "temporarily-unavailable": "暂时无法获取额度",
  unavailable: "暂无数据",
};

function statusText(status?: string): string {
  return status ? STATUS_TEXT[status] ?? "暂无数据" : "暂无数据";
}
</script>

<template>
  <article class="quota">
    <div class="platform">
      <span class="brand">
        <ToolLogo :platform="view.platform" :size="19" />
        {{ view.name }}
      </span>
      <span class="tag">{{ view.planTag }}</span>
      <span v-if="view.stale" class="tag tag-stale" title="正在展示上次成功读数">陈旧</span>
    </div>

    <div v-if="view.balance" class="plan-meters">
      <div class="plan-meter balance-total" :class="{ 'is-exhausted': !view.balance.available }">
        <div class="plan-meter-header">
          <span class="plan-meter-label">账户余额</span>
          <span class="plan-meter-reset">{{ view.balance.currency }}</span>
          <strong>{{ money(view.balance.total, view.balance.currency) }}</strong>
        </div>
        <div class="meter-top">
          <span>赠金 {{ money(view.balance.granted, view.balance.currency) }}</span>
          <span>充值 {{ money(view.balance.toppedUp, view.balance.currency) }}</span>
        </div>
      </div>
    </div>

    <div v-else-if="view.credits" class="plan-meters">
      <div class="plan-meter credit-total">
        <div class="plan-meter-header">
          <span class="plan-meter-label">总额度</span>
          <span class="plan-meter-reset">Credits</span>
          <strong>{{ view.credits.total.toLocaleString() }}</strong>
        </div>
      </div>
      <div class="plan-meter">
        <div class="meter-top">
          <span>已使用 {{ usedCredits(view).toLocaleString() }}</span>
          <span>剩余 {{ view.credits.remaining.toLocaleString() }}</span>
        </div>
        <div class="bar">
          <span class="purple" :style="{ width: `${creditUsedPct(view)}%` }" />
        </div>
        <div class="plan-meter-value">{{ creditUsedPct(view) }}<small>%</small></div>
        <div v-if="view.credits.expiresIn || view.credits.refreshIn" class="plan-meter-reset">
          {{ view.credits.expiresIn ?? view.credits.refreshIn }}
        </div>
      </div>
      <div v-if="view.credits.addOn" class="plan-meter credit-addon">
        <div class="plan-meter-header">
          <span class="plan-meter-label">加购额度</span>
          <span class="plan-meter-reset">Credits</span>
          <strong>{{ view.credits.addOn.total.toLocaleString() }}</strong>
        </div>
        <div class="meter-top">
          <span>已使用 {{ (view.credits.addOn.total - view.credits.addOn.remaining).toLocaleString() }}</span>
          <span>剩余 {{ view.credits.addOn.remaining.toLocaleString() }}</span>
        </div>
        <div class="bar">
          <span
            class="purple"
            :style="{ width: `${view.credits.addOn.total > 0 ? ((view.credits.addOn.total - view.credits.addOn.remaining) / view.credits.addOn.total) * 100 : 0}%` }"
          />
        </div>
      </div>
    </div>

    <div v-else-if="view.windows.length" class="plan-meters">
      <div
        v-for="w in view.windows"
        :key="w.metric"
        class="plan-meter"
        :class="{ 'is-exhausted': w.usedPct >= 100 }"
      >
        <div class="plan-meter-header">
          <span class="plan-meter-label">{{ compactLabel(w) }}</span>
          <span class="plan-meter-reset">{{ compactReset(w.resetsIn) }}</span>
          <span v-if="w.usedPct >= 100" class="usage-state">已耗尽</span>
          <strong v-else>{{ w.usedPct }}<small>%</small></strong>
        </div>
        <div class="bar">
          <span :class="w.tone" :style="{ width: `${Math.min(100, w.usedPct)}%` }" />
        </div>
      </div>
    </div>

    <div v-if="!view.balance && !view.credits && view.windows.length === 0" class="empty">
      <template v-if="view.account">
        <span class="account-label">{{ view.account.label }}</span>
        <span class="account-note">{{ view.message || "已登录 · 额度详情暂不可读" }}</span>
      </template>
      <template v-else-if="view.status && view.status !== 'ready'">
        <span class="account-note">{{ view.message || statusText(view.status) }}</span>
      </template>
      <template v-else>暂无数据</template>
    </div>
  </article>
</template>

<style scoped>
.quota {
  display: flex;
  height: 100%;
  min-height: 176px;
  flex-direction: column;
  padding: 17px 19px;
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  background: var(--surface);
  box-shadow: none;
  transition: border-color 0.15s ease, transform 0.15s ease;
}

.quota:hover {
  border-color: var(--border-strong);
  transform: translateY(-1px);
}

.platform {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 12px;
}

.brand {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 8px;
  color: var(--text);
  font-size: 13.5px;
  font-weight: 680;
}

.tag-stale {
  margin-left: 6px;
  border-color: color-mix(in srgb, var(--u-warn) 30%, var(--border));
  background: color-mix(in srgb, var(--u-warn) 12%, transparent);
  color: var(--u-warn);
}

.tag {
  max-width: 46%;
  overflow: hidden;
  padding: 2px 7px;
  border: 1px solid color-mix(in srgb, var(--accent) 15%, var(--border));
  border-radius: 5px;
  background: var(--accent-weak);
  color: var(--accent);
  font-size: 10px;
  font-weight: 620;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.plan-meters {
  display: flex;
  flex-direction: column;
  gap: 9px;
}

/* 额度行不再各自描边，去掉卡片内的层层边框；仅保留极淡的耗尽底色。 */
.plan-meter {
  display: flex;
  flex-direction: column;
  justify-content: center;
  padding: 4px 1px;
  border: 0;
  border-radius: var(--r-md);
  background: transparent;
}

.plan-meter.is-exhausted {
  background: color-mix(in srgb, var(--u-crit) 8%, transparent);
}

.plan-meter-header {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 6px;
}

.plan-meter-label {
  flex: 0 0 auto;
  color: var(--text-muted);
  font-size: 11.5px;
  font-weight: 550;
}

.plan-meter-reset {
  min-width: 0;
  overflow: hidden;
  color: var(--text-subtle);
  font-size: 10.5px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.plan-meter-header strong {
  margin-left: auto;
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 15px;
  font-weight: 680;
  line-height: 1;
  letter-spacing: -0.04em;
  font-variant-numeric: tabular-nums;
}

.credit-total .plan-meter-header strong {
  color: var(--accent);
  font-size: 20px;
}

.balance-total .plan-meter-header strong {
  color: var(--brand-deepseek);
  font-size: 20px;
}

.credit-addon .plan-meter-header strong {
  color: var(--brand-kiro);
}

.plan-meter-header strong small,
.plan-meter-value small {
  margin-left: 1px;
  color: var(--text-subtle);
  font-size: 10px;
  letter-spacing: 0;
}

.usage-state {
  margin-left: auto;
  padding: 2px 6px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--u-crit) 14%, transparent);
  color: var(--u-crit);
  font-size: 9.5px;
  font-weight: 650;
}

.meter-top {
  display: flex;
  justify-content: space-between;
  gap: 8px;
  color: var(--text-muted);
  font-size: 10.5px;
  line-height: 1.25;
}

.meter-top span {
  min-width: 0;
  white-space: nowrap;
}

.meter-top span:last-child {
  overflow: hidden;
  color: var(--text-subtle);
  text-align: right;
  text-overflow: ellipsis;
}

.plan-meter-value {
  margin-top: 6px;
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 15px;
  font-weight: 650;
  line-height: 1;
  font-variant-numeric: tabular-nums;
}

.bar {
  height: 6px;
  margin-top: 7px;
  overflow: hidden;
  border-radius: 999px;
  background: var(--track);
}

.bar span {
  display: block;
  height: 100%;
  min-width: 3px;
  border-radius: inherit;
  background: var(--accent);
  transition: width 0.3s ease;
}

.bar span.blue {
  background: var(--accent);
}

.bar span.orange {
  background: var(--u-warn);
}

.bar span.purple {
  background: var(--brand-kiro);
}

.bar span.green {
  background: var(--u-ok);
}

.plan-meter.is-exhausted .bar span {
  background: var(--u-crit);
}

.empty {
  margin: auto 0;
  color: var(--text-subtle);
  font-size: 11.5px;
}

.account-label,
.account-note {
  display: block;
}

.account-label {
  color: var(--text);
  font-size: 12.5px;
  font-weight: 620;
}

.account-note {
  margin-top: 4px;
}

@media (max-width: 640px) {
  .quota {
    min-height: 0;
    padding: 15px;
  }
}
</style>
