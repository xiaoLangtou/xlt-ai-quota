<script setup lang="ts">
import { computed } from "vue";
import type { PlatformQuotaView, QuotaWindowView } from "@/types/usage";
import ToolLogo from "@/components/ToolLogo.vue";

const props = defineProps<{ view: PlatformQuotaView }>();

const BRAND: Record<string, string> = {
  codex: "var(--brand-codex)",
  ark: "var(--brand-ark)",
  kiro: "var(--brand-kiro)",
  qoder: "var(--brand-qoder)",
  kimi: "#7c5cff",
  deepseek: "var(--brand-deepseek)",
  devin: "#2f80ed",
};

const brand = computed(() => BRAND[props.view.platform] ?? "var(--accent)");
const windows = computed(() => props.view.windows);
const hero = computed(() =>
  windows.value.length
    ? windows.value.reduce((a, b) => (b.usedPct > a.usedPct ? b : a))
    : null,
);
const rest = computed(() =>
  hero.value ? windows.value.filter((item) => item !== hero.value) : [],
);

function windowLabel(window: QuotaWindowView): string {
  switch (window.metric) {
    case "five_hour":
      return "5 小时额度";
    case "session":
      return "当前会话";
    case "daily":
      return "日额度";
    case "weekly":
      return "周额度";
    case "monthly":
      return "月额度";
    case "monthly_code":
      return "月 Code 额度";
    default:
      return window.label;
  }
}

function compactReset(value?: string): string | undefined {
  if (!value) return undefined;
  return value
    .replace(/(\d+) 小时 (\d+) 分钟后重置/, "$1h $2m 后")
    .replace(/(\d+) 小时后重置/, "$1h 后")
    .replace(/(\d+) 分钟后重置/, "$1m 后")
    .replace(/(\d+) 天后重置/, "$1d 后")
    .replace("后重置", "后");
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
  "custom-provider": "检测到自定义服务地址",
  "unsupported-account": "当前账户不提供订阅额度",
  "unsupported-plan": "当前套餐不提供额度",
  "unsupported-version": "本地工具版本不兼容",
  "temporarily-unavailable": "暂时无法获取额度",
  unavailable: "暂无额度数据",
};

function statusText(status?: string): string {
  return status ? STATUS_TEXT[status] ?? "暂无额度数据" : "暂无额度数据";
}

function toneClass(pct: number): string {
  if (pct >= 100) return "critical";
  if (pct >= 80) return "warn";
  return "normal";
}
</script>

<template>
  <article
    class="quota"
    :class="{ stale: view.stale, empty: !view.balance && !view.credits && windows.length === 0 }"
    :style="{ '--brand': brand }"
  >
    <header class="q-head">
      <span class="q-brand">
        <span class="q-icon"><ToolLogo :platform="view.platform" :size="17" /></span>
        <strong>{{ view.name }}</strong>
      </span>
      <span class="q-side">
        <span class="q-plan">{{ view.planTag }}</span>
        <span v-if="view.stale" class="q-stale"><i />陈旧</span>
      </span>
    </header>

    <div v-if="view.balance" class="q-hero q-hero-balance">
      <strong class="q-hero-value balance">{{ money(view.balance.total, view.balance.currency) }}</strong>
      <span class="q-hero-label">账户余额 · {{ view.balance.currency }}</span>
      <span class="q-hero-meta">
        赠金 {{ money(view.balance.granted, view.balance.currency) }}
        <i>·</i>
        充值 {{ money(view.balance.toppedUp, view.balance.currency) }}
      </span>
    </div>

    <div v-else-if="view.credits" class="q-credits">
      <div class="q-hero">
        <strong class="q-hero-value">{{ view.credits.total.toLocaleString() }}</strong>
        <span class="q-hero-label">Credits 总额</span>
        <span class="q-hero-meta">
          已用 {{ usedCredits(view).toLocaleString() }}
          <i>·</i>
          剩余 {{ view.credits.remaining.toLocaleString() }}
          <template v-if="view.credits.expiresIn || view.credits.refreshIn">
            <i>·</i>
            {{ view.credits.expiresIn ?? view.credits.refreshIn }}
          </template>
        </span>
      </div>
      <div class="q-bar">
        <span :style="{ width: `${creditUsedPct(view)}%` }" />
      </div>
      <div v-if="view.credits.addOn" class="q-addon">
        <span>加购额度</span>
        <strong>{{ view.credits.addOn.remaining.toLocaleString() }} / {{ view.credits.addOn.total.toLocaleString() }}</strong>
      </div>
    </div>

    <template v-else-if="hero">
      <div class="q-hero">
        <strong class="q-hero-value" :class="toneClass(hero.usedPct)">
          {{ hero.usedPct }}<small>%</small>
        </strong>
        <span class="q-hero-label">{{ windowLabel(hero) }}</span>
        <span class="q-hero-meta">{{ compactReset(hero.resetsIn) ?? "—" }}</span>
      </div>
      <div class="q-bar">
        <span :class="toneClass(hero.usedPct)" :style="{ width: `${Math.min(100, Math.max(hero.usedPct, 1))}%` }" />
      </div>
      <div v-if="rest.length" class="q-rest">
        <div v-for="window in rest" :key="window.metric" class="q-rest-row">
          <span>{{ windowLabel(window) }}</span>
          <em>{{ compactReset(window.resetsIn) ?? "" }}</em>
          <strong :class="toneClass(window.usedPct)">{{ window.usedPct }}%</strong>
        </div>
      </div>
    </template>

    <div v-else class="q-empty">
      <span class="q-empty-title">{{ view.account?.label || statusText(view.status) }}</span>
      <span class="q-empty-note">
        {{ view.message || (view.account ? "已登录 · 额度详情暂不可读" : "点「同步」从本机 CLI 拉取额度") }}
      </span>
    </div>
  </article>
</template>

<style scoped>
.quota {
  position: relative;
  display: flex;
  height: 100%;
  min-height: 178px;
  flex-direction: column;
  gap: 13px;
  overflow: hidden;
  padding: 18px 19px 17px;
  border: 1px solid color-mix(in srgb, var(--border) 78%, transparent);
  border-radius: 16px;
  background:
    linear-gradient(180deg, color-mix(in srgb, var(--surface) 96%, white 4%), var(--surface));
  box-shadow:
    0 1px 1px color-mix(in srgb, var(--text) 3%, transparent),
    0 10px 28px -22px color-mix(in srgb, var(--text) 34%, transparent);
  transition: border-color 0.18s ease, transform 0.18s ease, box-shadow 0.18s ease;
}
.quota::before {
  position: absolute;
  inset: 0 0 auto;
  height: 1.5px;
  background: linear-gradient(90deg, var(--brand), transparent 68%);
  content: "";
  opacity: 0.75;
}
.quota::after {
  position: absolute;
  top: -56px;
  left: -44px;
  width: 150px;
  height: 150px;
  border-radius: 50%;
  background: radial-gradient(circle, color-mix(in srgb, var(--brand) 13%, transparent), transparent 68%);
  content: "";
  pointer-events: none;
}
.quota:hover {
  border-color: color-mix(in srgb, var(--brand) 24%, var(--border));
  box-shadow:
    0 1px 1px color-mix(in srgb, var(--text) 3%, transparent),
    0 18px 36px -24px color-mix(in srgb, var(--text) 40%, transparent);
  transform: translateY(-2px);
}
.quota.stale {
  border-color: color-mix(in srgb, var(--u-warn) 26%, var(--border));
}
.quota.empty {
  justify-content: space-between;
}

.q-head {
  position: relative;
  z-index: 1;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}
.q-brand {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 10px;
}
.q-brand strong {
  overflow: hidden;
  color: var(--text);
  font-size: 13.5px;
  font-weight: 650;
  letter-spacing: -0.01em;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.q-icon {
  display: grid;
  width: 30px;
  height: 30px;
  flex: 0 0 30px;
  place-items: center;
  border: 1px solid color-mix(in srgb, var(--brand) 18%, transparent);
  border-radius: 9px;
  background: color-mix(in srgb, var(--brand) 10%, var(--surface));
}
.q-side {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  gap: 9px;
}
.q-plan {
  color: var(--text-subtle);
  font-size: 9.5px;
  font-weight: 650;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}
.q-stale {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  color: var(--u-warn);
  font-size: 9.5px;
  font-weight: 600;
}
.q-stale i {
  width: 4px;
  height: 4px;
  border-radius: 50%;
  background: var(--u-warn);
}

.q-hero {
  position: relative;
  z-index: 1;
  display: flex;
  flex: 1;
  flex-direction: column;
  justify-content: flex-end;
  gap: 2px;
}
.q-hero-value {
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 34px;
  font-weight: 700;
  letter-spacing: -0.055em;
  line-height: 1;
  font-variant-numeric: tabular-nums;
}
.q-hero-value small {
  margin-left: 2px;
  color: var(--text-subtle);
  font-size: 14px;
  font-weight: 500;
  letter-spacing: -0.02em;
}
.q-hero-value.warn {
  color: var(--u-warn);
}
.q-hero-value.critical {
  color: var(--u-crit);
}
.q-hero-value.balance {
  color: var(--brand-deepseek);
  font-size: 30px;
}
.q-hero-label {
  color: var(--text-muted);
  font-size: 11px;
  font-weight: 550;
}
.q-hero-meta {
  color: var(--text-subtle);
  font-size: 10px;
}
.q-hero-meta i {
  margin: 0 3px;
  font-style: normal;
  opacity: 0.5;
}

.q-bar {
  position: relative;
  z-index: 1;
  height: 5px;
  overflow: hidden;
  border-radius: 999px;
  background: color-mix(in srgb, var(--text) 7%, transparent);
}
.q-bar span {
  display: block;
  height: 100%;
  min-width: 2px;
  border-radius: inherit;
  background: var(--brand);
  transition: width 0.4s cubic-bezier(0.2, 0.8, 0.2, 1);
}
.q-bar span.warn {
  background: var(--u-warn);
}
.q-bar span.critical {
  background: var(--u-crit);
}

.q-rest {
  position: relative;
  z-index: 1;
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding-top: 10px;
  border-top: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
}
.q-rest-row {
  display: flex;
  align-items: baseline;
  gap: 8px;
  color: var(--text-muted);
  font-size: 10.5px;
}
.q-rest-row em {
  overflow: hidden;
  margin-left: auto;
  color: var(--text-subtle);
  font-style: normal;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.q-rest-row strong {
  min-width: 34px;
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 650;
  text-align: right;
  font-variant-numeric: tabular-nums;
}
.q-rest-row strong.warn {
  color: var(--u-warn);
}
.q-rest-row strong.critical {
  color: var(--u-crit);
}

.q-credits {
  position: relative;
  z-index: 1;
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 9px;
}
.q-addon {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-top: 9px;
  border-top: 1px dashed color-mix(in srgb, var(--border) 80%, transparent);
  color: var(--text-subtle);
  font-size: 10.5px;
}
.q-addon strong {
  color: var(--text-muted);
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 600;
}

.q-empty {
  position: relative;
  z-index: 1;
  display: flex;
  flex: 1;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 5px;
  text-align: center;
}
.q-empty-title {
  color: var(--text);
  font-size: 12.5px;
  font-weight: 620;
}
.q-empty-note {
  max-width: 220px;
  color: var(--text-subtle);
  font-size: 10.5px;
  line-height: 1.5;
}
</style>
