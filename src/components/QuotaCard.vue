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
  <article class="quota">
    <div class="platform">
      <span class="brand">
        <i class="platform-dot" :class="view.logoClass" aria-hidden="true" />
        {{ view.name }}
      </span>
      <span class="tag">{{ view.planTag }}</span>
    </div>

    <div v-if="view.credits" class="credit">
      <div class="credit-heading">
        <span>总额度</span>
        <strong>{{ view.credits.total.toLocaleString() }}</strong>
      </div>
      <div v-if="view.windows.length" class="credit-meter">
        <div class="meter-top">
          <span>已使用 {{ usedCredits(view).toLocaleString() }}</span>
          <span>{{ compactReset(view.windows[0].resetsIn) }}</span>
        </div>
        <div class="bar">
          <span
            :class="view.windows[0].tone"
            :style="{ width: `${view.windows[0].usedPct}%` }"
          />
        </div>
        <div class="meter-value">{{ view.windows[0].usedPct }}<small>%</small></div>
      </div>
    </div>

    <div v-else-if="view.windows.length" class="meters">
      <div
        v-for="w in view.windows"
        :key="w.metric"
        class="meter"
      >
        <div class="meter-top">
          <span>{{ compactLabel(w) }}</span>
          <span>{{ compactReset(w.resetsIn) }}</span>
        </div>
        <div class="bar">
          <span
            :class="w.tone"
            :style="{ width: `${w.usedPct}%` }"
          />
        </div>
        <div class="meter-value">{{ w.usedPct }}<small>%</small></div>
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
  padding: 20px;
  background: var(--card);
  border: 1px solid var(--line);
  border-radius: 16px;
  box-shadow: 0 7px 18px rgba(35, 44, 70, 0.025);
}
.platform {
  display: flex;
  align-items: center;
  gap: 12px;
  justify-content: space-between;
  margin-bottom: 17px;
  font-size: 15px;
  font-weight: 650;
}
.brand {
  display: flex;
  gap: 10px;
  align-items: center;
}
.platform-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
}
.platform-dot.codex {
  background: #3186e5;
}
.platform-dot.ark {
  background: #c57310;
}
.platform-dot.kiro {
  background: #7d70df;
}
.platform-dot.open {
  background: #1ca781;
}
.tag {
  margin-left: auto;
  padding: 4px 8px;
  border-radius: 999px;
  background: #f3f6fa;
  color: #637692;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 11px;
  font-weight: 500;
}
.meters {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 18px;
}
.meter-top {
  display: flex;
  justify-content: space-between;
  gap: 6px;
  color: #617490;
  font-size: 11px;
}
.meter-top span {
  min-width: 0;
  white-space: nowrap;
}
.meter-top span:last-child {
  text-align: right;
  white-space: nowrap;
  font-size: 11px;
}
.bar {
  height: 11px;
  margin-top: 9px;
  background: repeating-linear-gradient(90deg, #dfe6f0 0 4px, transparent 4px 9px);
}
.bar span {
  display: block;
  height: 100%;
  background: repeating-linear-gradient(90deg, currentColor 0 4px, transparent 4px 9px);
}
.bar span.blue {
  color: #3186e5;
}
.bar span.orange {
  color: #c56c07;
}
.bar span.purple {
  color: #7d70df;
}
.bar span.green {
  color: #1ca781;
}
.meter-value {
  margin-top: 12px;
  color: #111a2d;
  font-size: 25px;
  font-weight: 650;
  line-height: 1;
  letter-spacing: -1.4px;
}
.meter-value small {
  margin-left: 2px;
  color: #62738e;
  font-size: 12px;
  letter-spacing: 0;
}
.credit {
  display: grid;
  grid-template-columns: minmax(0, 0.9fr) minmax(0, 1.1fr);
  gap: 18px;
  align-items: end;
  color: #617490;
}
.credit-heading {
  display: grid;
  gap: 5px;
  font-size: 12px;
}
.credit strong {
  color: #7d70df;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 23px;
  font-weight: 600;
  letter-spacing: -1px;
}
.credit-meter {
  margin-top: 0;
}
.credit-meter .meter-value {
  color: #111a2d;
}
.credit-meter .meter-value {
  margin-top: 10px;
}
.empty {
  margin: 0;
  color: #b6bac4;
  font-size: 12px;
}
.account-label {
  display: block;
  color: #4a5260;
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
    margin-bottom: 17px;
  }
  .meters {
    grid-template-columns: 1fr;
    gap: 28px;
  }
}
</style>
