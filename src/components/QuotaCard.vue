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
</script>

<template>
  <article class="quota">
    <div class="platform">
      <span class="brand">
        <ToolLogo :platform="view.platform" :size="19" />
        {{ view.name }}
      </span>
      <span class="tag">{{ view.planTag }}</span>
    </div>

    <div v-if="view.credits" class="plan-meters">
      <div class="plan-meter credit-total">
        <div class="plan-meter-header">
          <span class="plan-meter-label">总额度</span>
          <span class="plan-meter-reset">Credits</span>
          <strong>{{ view.credits.total.toLocaleString() }}</strong>
        </div>
      </div>
      <div v-if="view.windows.length" class="plan-meter">
        <div class="meter-top">
          <span>已使用 {{ usedCredits(view).toLocaleString() }}</span>
          <span>{{ view.credits.expiresIn ?? compactReset(view.windows[0].resetsIn) }}</span>
        </div>
        <div class="bar">
          <span
            :class="view.windows[0].tone"
            :style="{ width: `${view.windows[0].usedPct}%` }"
          />
        </div>
        <div class="plan-meter-value">{{ view.windows[0].usedPct }}<small>%</small></div>
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

    <div v-if="!view.credits && view.windows.length === 0" class="empty">
      <template v-if="view.account">
        <span class="account-label">{{ view.account.label }}</span>
        <span class="account-note">已登录 · 额度详情需浏览器连接器</span>
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
  padding: 17px;
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  background: var(--surface);
  box-shadow: var(--shadow-card);
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

.plan-meter {
  display: flex;
  flex-direction: column;
  justify-content: center;
  padding: 10px 11px;
  border: 1px solid var(--border);
  border-radius: var(--r-md);
  background: var(--surface-2);
}

.plan-meter.is-exhausted {
  border-color: color-mix(in srgb, var(--u-crit) 36%, var(--border));
  background: color-mix(in srgb, var(--u-crit) 7%, var(--surface-2));
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
