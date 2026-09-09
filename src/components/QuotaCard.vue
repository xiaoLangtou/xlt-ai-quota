<script setup lang="ts">
import type { PlatformQuotaView, QuotaWindowView } from "@/types/usage";

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
  <article
    class="quota"
    :class="{
      'plan-rows-two': (view.credits && !view.credits.addOn) || view.windows.length === 2,
      'plan-rows-three': Boolean(view.credits?.addOn),
    }"
  >
    <div class="platform">
      <span class="brand">
        <i class="platform-dot" :class="view.logoClass" aria-hidden="true" />
        {{ view.name }}
      </span>
      <span class="tag">{{ view.planTag }}</span>
    </div>

    <div v-if="view.credits" class="plan-meters credit-plans">
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
  flex-direction: column;
  height: 100%;
  min-height: 196px;
  padding: 18px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  box-shadow: var(--shadow-card);
  transition: border-color 0.15s ease;
}
.quota:hover {
  border-color: var(--border-strong);
}
.platform {
  display: flex;
  align-items: center;
  gap: 12px;
  justify-content: space-between;
  margin-bottom: 16px;
  font-size: 15px;
  font-weight: 600;
}
.brand {
  display: flex;
  gap: 9px;
  align-items: center;
  color: var(--text);
}
.platform-dot {
  width: 9px;
  height: 9px;
  border-radius: 50%;
  background: var(--text-subtle);
  box-shadow: 0 0 0 3px color-mix(in srgb, currentColor 0%, transparent);
}
.platform-dot.codex {
  background: var(--brand-codex);
}
.platform-dot.ark {
  background: var(--brand-ark);
}
.platform-dot.kiro {
  background: var(--brand-kiro);
}
.platform-dot.qoder {
  background: var(--brand-qoder);
}
.platform-dot.open {
  background: var(--brand-open);
}
.tag {
  margin-left: auto;
  padding: 3px 9px;
  border-radius: 999px;
  background: var(--surface-2);
  color: var(--text-muted);
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 500;
}

.plan-meters {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.plan-meter {
  display: flex;
  flex-direction: column;
  justify-content: center;
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: var(--r-md);
  background: var(--surface-2);
}
.plan-meter.is-exhausted {
  border-color: color-mix(in srgb, var(--u-crit) 40%, var(--border));
  background: color-mix(in srgb, var(--u-crit) 8%, var(--surface-2));
}
.plan-meter-header {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}
.plan-meter-label {
  flex: 0 0 auto;
  color: var(--text);
  font-size: 12px;
  font-weight: 600;
}
.plan-meter-reset {
  min-width: 0;
  overflow: hidden;
  color: var(--text-subtle);
  font-family: var(--font-mono);
  font-size: 10px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.plan-meter-header strong {
  margin-left: auto;
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 15px;
  line-height: 1;
  letter-spacing: -0.5px;
  font-variant-numeric: tabular-nums;
}
.credit-total .plan-meter-header strong {
  color: var(--brand-kiro);
  font-size: 22px;
  letter-spacing: -1px;
}
.credit-addon .plan-meter-header strong {
  color: var(--brand-kiro);
}
.plan-meter-header strong small {
  margin-left: 1px;
  color: var(--text-subtle);
  font-size: 10px;
  letter-spacing: 0;
}
.usage-state {
  margin-left: auto;
  padding: 3px 7px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--u-crit) 16%, transparent);
  color: var(--u-crit);
  font-size: 10px;
  font-weight: 600;
}
.plan-meter .meter-top {
  display: flex;
  justify-content: space-between;
  gap: 8px;
  color: var(--text-muted);
  font-size: 11px;
  line-height: 1.25;
}
.plan-meter .meter-top span {
  min-width: 0;
  white-space: nowrap;
}
.plan-meter .meter-top span:last-child {
  overflow: hidden;
  text-align: right;
  text-overflow: ellipsis;
  font-family: var(--font-mono);
}
.plan-meter-value {
  margin-top: 7px;
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 16px;
  font-weight: 650;
  line-height: 1;
  letter-spacing: -0.5px;
}
.plan-meter-value small {
  margin-left: 1px;
  color: var(--text-subtle);
  font-size: 10px;
  letter-spacing: 0;
}

/* 利用率热力进度条：轨道 + 圆角实心填充，颜色由 tone 决定 */
.bar {
  height: 6px;
  margin-top: 8px;
  border-radius: 999px;
  background: var(--surface-3);
  overflow: hidden;
}
.plan-meter:not(.credit-total) > .bar {
  margin-top: 6px;
}
.bar span {
  display: block;
  height: 100%;
  min-width: 3px;
  border-radius: 999px;
  background: var(--accent);
  transition: width 0.4s ease;
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
  margin: 0;
  color: var(--text-subtle);
  font-size: 12px;
}
.account-label {
  display: block;
  color: var(--text);
  font-weight: 600;
  font-size: 13px;
}
.account-note {
  display: block;
  margin-top: 4px;
}

@media (max-width: 640px) {
  .quota {
    min-height: 0;
    padding: 16px;
  }
  .platform {
    margin-bottom: 14px;
  }
}
</style>
