<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { useOilMonitor } from "@/composables/useOilMonitor";
import { settings } from "@/config/settings";
import type { OilForecastDirection, OilGrade } from "@/types/oil";

const emit = defineEmits<{ (event: "configure"): void }>();
const { snapshot, loading, error, configured, sync } = useOilMonitor();

const directionMeta: Record<OilForecastDirection, { label: string; symbol: string }> = {
  up: { label: "预计上调", symbol: "↑" },
  down: { label: "预计下调", symbol: "↓" },
  unchanged: { label: "预计搁浅", symbol: "—" },
};

const forecastMeta = computed(() => snapshot.value
  ? directionMeta[snapshot.value.forecast.direction]
  : directionMeta.unchanged);

const officialMeta = computed(() => snapshot.value
  ? directionMeta[snapshot.value.officialAdjustment.direction]
  : directionMeta.unchanged);

const nextAdjustmentText = computed(() => {
  const date = snapshot.value?.forecast.nextAdjustmentDate;
  if (!date) return "";
  const target = new Date(`${date}T00:00:00+08:00`).getTime() + 24 * 60 * 60_000;
  const diff = target - Date.now();
  if (diff <= 0) return "等待正式公告";
  const totalHours = Math.ceil(diff / 3_600_000);
  const days = Math.floor(totalHours / 24);
  const hours = totalHours % 24;
  return days ? `${days} 天 ${hours} 小时后` : `${hours} 小时后`;
});

/** 主展示标号来自「设置 → 油价监控 → 油品标号」。 */
const primaryGrade = ref<OilGrade>(settings.getOilConfig().grade);

function syncGrade(): void {
  primaryGrade.value = settings.getOilConfig().grade;
}

onMounted(() => window.addEventListener("xlt:oil-config-changed", syncGrade));
onUnmounted(() => window.removeEventListener("xlt:oil-config-changed", syncGrade));

const gasoline = computed(() =>
  snapshot.value?.prices.find((item) => item.grade === primaryGrade.value)
  ?? snapshot.value?.prices.find((item) => item.name.includes(String(primaryGrade.value)))
  ?? snapshot.value?.prices[0] ?? null);

const otherGrades = computed(() =>
  snapshot.value ? snapshot.value.prices.filter((item) => item !== gasoline.value) : []);

const forecastTone = computed(() => {
  if (!snapshot.value) return "";
  if (snapshot.value.forecast.direction === "up") return "crit";
  if (snapshot.value.forecast.direction === "down") return "ok";
  return "";
});

/** 预测调价幅度以 ±0.30 元/升 为满刻度。 */
const forecastPct = computed(() => {
  const change = Math.abs(snapshot.value?.forecast.estimatedChangePerLiter ?? 0);
  return Math.min(100, Math.round((change / 0.3) * 100));
});

function signed(value: number): string {
  return `${value > 0 ? "+" : ""}${value.toLocaleString("zh-CN")}`;
}

function dateText(value: string): string {
  return new Intl.DateTimeFormat("zh-CN", { month: "numeric", day: "numeric" }).format(new Date(`${value}T12:00:00+08:00`));
}
</script>

<template>
  <section class="oil-card" aria-label="国内油价监控">
    <header class="oil-head">
      <h2>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"
          stroke-linejoin="round">
          <path d="M12 2v4M5 9a7 7 0 0 1 14 0v9a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2z" />
          <path d="M5 13h14" />
        </svg>
        油价监控
      </h2>
      <button v-if="configured" class="refresh" type="button" :disabled="loading" @click="sync()">
        {{ loading ? "同步中…" : "同步" }}
      </button>
    </header>

    <div v-if="!configured" class="oil-empty">
      <p>选择省份后开始监控 92 / 95 / 98 号汽油与 0 号柴油。</p>
      <button type="button" @click="emit('configure')">前往设置</button>
    </div>

    <div v-else-if="!snapshot" class="oil-empty">
      <p>{{ error ? "油价同步失败" : "正在获取油价" }}：{{ error || "正在读取省级指导价。" }}</p>
      <button v-if="error" type="button" @click="sync()">重试</button>
    </div>

    <template v-else>
      <p class="oil-sub">
        {{ gasoline?.name ?? "92#" }} 汽油 · {{ snapshot.province }} · 下一调价窗口 {{ dateText(snapshot.forecast.nextAdjustmentDate) }} 24时
      </p>
      <div class="oil-price-row">
        <span class="oil-price">¥{{ gasoline ? gasoline.price.toFixed(2) : "--" }}</span>
        <span class="oil-unit">/ 升</span>
        <span class="oil-forecast" :class="forecastTone">
          {{ forecastMeta.symbol }} {{ forecastMeta.label }}
          {{ signed(snapshot.forecast.estimatedChangePerLiter) }}
        </span>
      </div>
      <div class="oil-meter" :class="forecastTone">
        <i :style="{ width: `${forecastPct}%` }" />
      </div>
      <p class="oil-note">
        {{ nextAdjustmentText }} · 预测幅度 {{ signed(snapshot.forecast.estimatedChangePerTon) }} 元/吨 · {{ snapshot.forecast.confidence }}置信度
      </p>

      <ul v-if="otherGrades.length" class="oil-grades">
        <li v-for="item in otherGrades" :key="item.grade">
          <span>{{ item.name }}</span>
          <strong>¥{{ item.price.toFixed(2) }}</strong>
        </li>
      </ul>

      <footer class="oil-official">
        <span class="mark" :class="snapshot.officialAdjustment.direction">{{ officialMeta.symbol }}</span>
        <a :href="snapshot.officialAdjustment.sourceUrl" target="_blank" rel="noreferrer">
          上次发改委公告 · {{ snapshot.officialAdjustment.title }}
        </a>
      </footer>
    </template>
  </section>
</template>

<style scoped>
.oil-card {
  display: flex;
  min-width: 0;
  flex-direction: column;
  padding: 16px 18px;
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  background: var(--surface);
  box-shadow: var(--shadow-card);
}

.oil-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.oil-head h2 {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 0;
  color: var(--text);
  font-size: 13.5px;
  font-weight: 700;
}

.oil-head h2 svg {
  width: 15px;
  height: 15px;
  color: var(--u-warn);
}

.refresh {
  flex: none;
  padding: 4px 9px;
  border: 0;
  border-radius: var(--r-sm);
  background: transparent;
  color: var(--text-muted);
  font: inherit;
  font-size: 12px;
  cursor: pointer;
}

.refresh:hover {
  background: var(--surface-2);
  color: var(--text);
}

.refresh:disabled {
  cursor: wait;
  opacity: 0.6;
}

.oil-sub {
  margin: 4px 0 0;
  color: var(--text-subtle);
  font-size: 11.5px;
}

.oil-price-row {
  display: flex;
  align-items: baseline;
  gap: 10px;
  margin-top: 12px;
}

.oil-price {
  color: var(--text);
  font-size: 24px;
  font-weight: 750;
  letter-spacing: -0.02em;
  font-variant-numeric: tabular-nums;
}

.oil-unit {
  color: var(--text-muted);
  font-size: 12px;
}

.oil-forecast {
  margin-left: auto;
  padding: 2px 8px;
  border: 1px solid color-mix(in srgb, var(--u-crit) 30%, var(--border));
  border-radius: 999px;
  background: color-mix(in srgb, var(--u-crit) 8%, transparent);
  color: var(--u-crit);
  font-size: 11px;
  font-weight: 650;
  white-space: nowrap;
}

.oil-forecast.ok {
  border-color: color-mix(in srgb, var(--u-ok) 30%, var(--border));
  background: color-mix(in srgb, var(--u-ok) 8%, transparent);
  color: var(--u-ok);
}

.oil-meter {
  height: 6px;
  margin-top: 12px;
  overflow: hidden;
  border-radius: 999px;
  background: var(--surface-3);
}

.oil-meter i {
  display: block;
  height: 100%;
  border-radius: 999px;
  background: var(--u-warn);
}

.oil-meter.ok i {
  background: var(--u-ok);
}

.oil-note {
  margin: 8px 0 0;
  color: var(--text-subtle);
  font-size: 11.5px;
}

.oil-grades {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 8px;
  margin: 13px 0 0;
  padding: 0;
  list-style: none;
}

.oil-grades li {
  padding: 7px 10px;
  border-radius: var(--r-sm);
  background: var(--surface-2);
}

.oil-grades span {
  display: block;
  color: var(--text-subtle);
  font-size: 10.5px;
}

.oil-grades strong {
  display: block;
  margin-top: 2px;
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 13px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
}

.oil-official {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 8px;
  margin-top: 12px;
  padding-top: 11px;
  border-top: 1px dashed var(--border);
}

.oil-official .mark {
  display: grid;
  width: 20px;
  height: 20px;
  flex: none;
  place-items: center;
  border-radius: 50%;
  background: var(--surface-3);
  color: var(--text-muted);
  font-size: 11px;
  font-weight: 800;
}

.oil-official .mark.up {
  color: var(--u-crit);
}

.oil-official .mark.down {
  color: var(--u-ok);
}

.oil-official a {
  overflow: hidden;
  color: var(--text-muted);
  font-size: 10.5px;
  text-decoration: none;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.oil-official a:hover {
  color: var(--accent);
}

.oil-empty {
  margin-top: 12px;
}

.oil-empty p {
  margin: 0;
  color: var(--text-muted);
  font-size: 12px;
  line-height: 1.6;
}

.oil-empty button {
  margin-top: 10px;
  padding: 6px 12px;
  border: 1px solid var(--border);
  border-radius: var(--r-sm);
  background: var(--surface-2);
  color: var(--text);
  font: inherit;
  font-size: 12px;
  cursor: pointer;
}

.oil-empty button:hover {
  border-color: var(--accent);
  color: var(--accent);
}

@media (max-width: 1180px) {
  .oil-grades {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }
}
</style>
