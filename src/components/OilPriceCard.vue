<script setup lang="ts">
import { computed } from "vue";
import { useOilMonitor } from "@/composables/useOilMonitor";
import type { OilForecastDirection } from "@/types/oil";

const emit = defineEmits<{ (event: "configure"): void }>();
const { snapshot, loading, error, configured, sync } = useOilMonitor();

const directionMeta: Record<OilForecastDirection, { label: string; symbol: string }> = {
  up: { label: "预计上涨", symbol: "↑" },
  down: { label: "预计下跌", symbol: "↓" },
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

function signed(value: number): string {
  return `${value > 0 ? "+" : ""}${value.toLocaleString("zh-CN")}`;
}

function dateText(value: string): string {
  return new Intl.DateTimeFormat("zh-CN", { month: "long", day: "numeric" }).format(new Date(`${value}T12:00:00+08:00`));
}
</script>

<template>
  <section class="oil-card" aria-label="国内油价监控">
    <header class="oil-head">
      <div>
        <div class="oil-kicker">FUEL MONITOR</div>
        <h2>国内油价</h2>
      </div>
      <button v-if="configured" class="refresh" type="button" :disabled="loading" @click="sync()">
        {{ loading ? "同步中…" : "同步油价" }}
      </button>
    </header>

    <div v-if="!configured" class="oil-empty">
      <div>
        <strong>选择省份后开始监控</strong>
        <p>同步 92 / 95 / 98 号汽油与 0 号柴油，并跟踪下一调价窗口。</p>
      </div>
      <button type="button" @click="emit('configure')">前往设置</button>
    </div>

    <div v-else-if="!snapshot" class="oil-empty">
      <div>
        <strong>{{ error ? "油价同步失败" : "正在获取油价" }}</strong>
        <p>{{ error || "正在读取省级指导价与国家发改委公告。" }}</p>
      </div>
      <button v-if="error" type="button" @click="sync()">重试</button>
    </div>

    <template v-else>
      <div class="oil-main">
        <div class="price-panel">
          <div class="province-line">
            <strong>{{ snapshot.province }}</strong>
            <span>指导价 · {{ snapshot.priceDate }} · 元/升</span>
          </div>
          <div class="price-grid">
            <div v-for="item in snapshot.prices" :key="item.grade" class="price-item">
              <span>{{ item.name }}</span>
              <strong>¥{{ item.price.toFixed(2) }}</strong>
            </div>
          </div>
        </div>

        <div class="forecast-panel" :class="snapshot.forecast.direction">
          <div class="forecast-top">
            <span class="forecast-label">下一调价窗口 · {{ dateText(snapshot.forecast.nextAdjustmentDate) }} 24时</span>
            <span class="confidence">{{ snapshot.forecast.confidence }}置信度 · 预测</span>
          </div>
          <strong class="forecast-direction">
            {{ forecastMeta.symbol }} {{ forecastMeta.label }}
          </strong>
          <div class="forecast-values">
            <span>{{ signed(snapshot.forecast.estimatedChangePerTon) }} 元/吨</span>
            <span>{{ signed(snapshot.forecast.estimatedChangePerLiter) }} 元/升</span>
          </div>
          <div class="countdown">{{ nextAdjustmentText }}</div>
        </div>
      </div>

      <footer class="official">
        <div class="official-mark" :class="snapshot.officialAdjustment.direction">
          {{ officialMeta.symbol }}
        </div>
        <div class="official-copy">
          <span>最近一次国家发改委正式公告 · {{ snapshot.officialAdjustment.publishedAt }}</span>
          <a :href="snapshot.officialAdjustment.sourceUrl" target="_blank" rel="noreferrer">
            {{ snapshot.officialAdjustment.title }}
          </a>
        </div>
        <div v-if="snapshot.officialAdjustment.gasolineChangePerTon != null" class="official-delta">
          汽油 {{ signed(snapshot.officialAdjustment.gasolineChangePerTon) }}
          <small>元/吨</small>
        </div>
        <span class="provider">
          <a :href="snapshot.priceSourceUrl" target="_blank" rel="noreferrer">价格来源</a>
          · 预测：极数本源
        </span>
      </footer>
      <p v-if="error" class="oil-error">本次自动同步失败，当前显示 {{ snapshot.collectedAt.slice(0, 16).replace('T', ' ') }} 的数据：{{ error }}</p>
    </template>
  </section>
</template>

<style scoped>
.oil-card {
  overflow: hidden;
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  background: var(--surface);
  box-shadow: var(--shadow-card);
}
.oil-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 18px 12px;
}
.oil-kicker {
  color: var(--text-subtle);
  font-family: var(--font-mono);
  font-size: 9px;
  font-weight: 700;
  letter-spacing: 1.4px;
}
.oil-head h2 {
  margin: 3px 0 0;
  color: var(--text);
  font-size: 16px;
}
.refresh,
.oil-empty button {
  min-height: 32px;
  padding: 0 12px;
  border: 1px solid var(--border);
  border-radius: var(--r-sm);
  background: var(--surface-2);
  color: var(--text);
  font: inherit;
  font-size: 12px;
  cursor: pointer;
}
.refresh:disabled { cursor: wait; opacity: 0.6; }
.oil-main {
  display: grid;
  grid-template-columns: minmax(0, 1.5fr) minmax(260px, 0.75fr);
  gap: 12px;
  padding: 0 18px 16px;
}
.price-panel,
.forecast-panel {
  border: 1px solid var(--border);
  border-radius: var(--r-md);
  background: var(--surface-2);
}
.price-panel { padding: 15px; }
.province-line {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 12px;
}
.province-line strong { color: var(--text); font-size: 14px; }
.province-line span { color: var(--text-subtle); font-size: 11px; }
.price-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 9px;
  margin-top: 12px;
}


.price-item {
  padding: 11px 12px;
  border-radius: var(--r-sm);
  background: var(--surface);
}
.price-item span {
  display: block;
  color: var(--text-muted);
  font-size: 11px;
}
.price-item strong {
  display: block;
  margin-top: 5px;
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 18px;
}
.forecast-panel { padding: 14px 15px; }
.forecast-panel.up { border-color: color-mix(in srgb, var(--u-crit) 30%, var(--border)); }
.forecast-panel.down { border-color: color-mix(in srgb, var(--u-ok) 30%, var(--border)); }
.forecast-top {
  display: flex;
  justify-content: space-between;
  gap: 8px;
  color: var(--text-subtle);
  font-size: 10px;
}
.confidence {
  flex: 0 0 auto;
  color: var(--u-warn);
}
.forecast-direction {
  display: block;
  margin-top: 11px;
  color: var(--text);
  font-size: 20px;
}
.up .forecast-direction { color: var(--u-crit); }
.down .forecast-direction { color: var(--u-ok); }
.forecast-values {
  display: flex;
  gap: 12px;
  margin-top: 8px;
  color: var(--text-muted);
  font-family: var(--font-mono);
  font-size: 11px;
}
.countdown {
  margin-top: 12px;
  color: var(--text-subtle);
  font-size: 11px;
}
.official {
  display: flex;
  align-items: center;
  gap: 11px;
  padding: 12px 18px;
  border-top: 1px solid var(--border);
}
.official-mark {
  display: grid;
  width: 30px;
  height: 30px;
  flex: 0 0 30px;
  place-items: center;
  border-radius: 50%;
  background: var(--surface-2);
  color: var(--text-muted);
  font-weight: 800;
}
.official-mark.up { color: var(--u-crit); }
.official-mark.down { color: var(--u-ok); }
.official-copy { min-width: 0; flex: 1; }
.official-copy span {
  display: block;
  color: var(--text-subtle);
  font-size: 10px;
}
.official-copy a {
  display: block;
  margin-top: 3px;
  overflow: hidden;
  color: var(--text);
  font-size: 12px;
  text-decoration: none;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.official-copy a:hover { color: var(--accent); }
.provider a {
  color: inherit;
  text-decoration: none;
}
.provider a:hover { color: var(--accent); }
.official-delta {
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 12px;
  font-weight: 700;
}
.official-delta small { color: var(--text-subtle); font-size: 9px; }
.provider { color: var(--text-subtle); font-size: 9px; white-space: nowrap; }
.oil-empty {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 18px;
  margin: 0 18px 16px;
  padding: 16px;
  border: 1px dashed var(--border-strong);
  border-radius: var(--r-md);
  background: var(--surface-2);
}
.oil-empty strong { color: var(--text); font-size: 13px; }
.oil-empty p { margin: 4px 0 0; color: var(--text-muted); font-size: 12px; }
.oil-error {
  margin: 0;
  padding: 9px 18px;
  border-top: 1px solid color-mix(in srgb, var(--u-warn) 24%, var(--border));
  background: color-mix(in srgb, var(--u-warn) 8%, transparent);
  color: var(--u-warn);
  font-size: 11px;
}
@media (max-width: 900px) {
  .oil-main { grid-template-columns: 1fr; }
  .price-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
}
</style>
