<script setup lang="ts">
import { computed, ref } from "vue";
import type { ProjectUsage } from "@/types/usage";
import { formatTokens, formatUsd } from "@/utils/format";
import ToolLogo from "@/components/ToolLogo.vue";

const props = defineProps<{ projects: ProjectUsage[]; rangeLabel: string }>();
const emit = defineEmits<{ (e: "sync"): void }>();

const showUnknown = ref(false);
const expanded = ref(false);
const TOP_N = 8;

const isUnknown = (row: ProjectUsage): boolean => row.project === "unknown";

const filtered = computed(() =>
  showUnknown.value ? props.projects : props.projects.filter((row) => !isUnknown(row)),
);
const totalTokens = computed(() => filtered.value.reduce((sum, row) => sum + row.tokens, 0));
const totalCost = computed(() => filtered.value.reduce((sum, row) => sum + row.costUsd, 0));
const cacheHitRate = computed(() => {
  const input = filtered.value.reduce((sum, row) => sum + row.input, 0);
  const cached = filtered.value.reduce((sum, row) => sum + row.cached, 0);
  const cacheWrite = filtered.value.reduce((sum, row) => sum + row.cacheWrite, 0);
  const denom = input + cached + cacheWrite;
  return denom > 0 ? Math.round((cached / denom) * 1000) / 10 : 0;
});
const hasUnknown = computed(() => props.projects.some(isUnknown));
const rows = computed(() => {
  const list = filtered.value.map((row) => ({
    ...row,
    pct: totalTokens.value > 0 ? Math.round((row.tokens / totalTokens.value) * 1000) / 10 : 0,
  }));
  return expanded.value ? list : list.slice(0, TOP_N);
});
const hiddenCount = computed(() => Math.max(0, filtered.value.length - TOP_N));

function modelLabel(model: string): string {
  return model.length > 22 ? `${model.slice(0, 20)}…` : model;
}
</script>

<template>
  <article class="project-panel">
    <header class="pp-head">
      <div>
        <h2>项目分布</h2>
        <p>{{ rangeLabel }} · {{ filtered.length }} 个项目 · 仅本地统计</p>
      </div>
      <label v-if="hasUnknown" class="pp-toggle">
        <input v-model="showUnknown" type="checkbox" />
        显示未知项目
      </label>
    </header>

    <div v-if="filtered.length" class="pp-summary">
      <span>总 Token <strong>{{ formatTokens(totalTokens) }}</strong></span>
      <span>预估费用 <strong>{{ formatUsd(totalCost) }}</strong></span>
      <span>缓存命中率 <strong>{{ cacheHitRate }}%</strong></span>
    </div>

    <div v-if="rows.length" class="pp-list">
      <div v-for="row in rows" :key="row.project" class="pp-row">
        <div class="pp-main">
          <div class="pp-name">
            <span class="pp-dot" :class="{ unknown: isUnknown(row) }" />
            <strong>{{ isUnknown(row) ? "未知项目" : row.project }}</strong>
            <span class="pp-pct">{{ row.pct }}%</span>
          </div>
          <div class="pp-metrics">
            <span>{{ formatTokens(row.tokens) }} Token</span>
            <span>输入 {{ formatTokens(row.input) }}</span>
            <span>输出 {{ formatTokens(row.output) }}</span>
            <span>缓存 {{ formatTokens(row.cached + row.cacheWrite) }}</span>
            <span>命中 {{ row.cacheHitRate }}%</span>
            <span>{{ formatUsd(row.costUsd) }}</span>
          </div>
          <div class="pp-bar">
            <span :style="{ width: `${Math.min(100, row.pct)}%` }" />
          </div>
        </div>
        <div class="pp-models">
          <span v-for="model in row.models.slice(0, 3)" :key="`${model.platform}:${model.model}`" class="pp-chip">
            <ToolLogo :platform="model.platform" :size="12" />
            {{ modelLabel(model.model) }}
            <em>{{ model.pct }}%</em>
          </span>
          <span v-if="row.models.length > 3" class="pp-more">+{{ row.models.length - 3 }}</span>
        </div>
      </div>
    </div>

    <div v-else class="pp-empty">
      <p>所选周期内没有项目用量</p>
      <button type="button" class="pp-sync" @click="emit('sync')">同步当前范围</button>
    </div>

    <p v-if="filtered.length <= 1" class="pp-hint">
      历史项目缺失？
      <button type="button" @click="emit('sync')">按当前范围同步</button>
    </p>

    <button
      v-if="hiddenCount > 0 || expanded"
      type="button"
      class="pp-expand"
      @click="expanded = !expanded"
    >
      {{ expanded ? "收起" : `展开其余 ${hiddenCount} 个项目` }}
    </button>
  </article>
</template>

<style scoped>
.project-panel {
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 18px 20px;
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  background: var(--surface);
}
.pp-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}
.pp-head h2 {
  margin: 0;
  color: var(--text);
  font-size: 14px;
  font-weight: 680;
}
.pp-head p {
  margin: 4px 0 0;
  color: var(--text-subtle);
  font-size: 11.5px;
}
.pp-toggle {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  color: var(--text-muted);
  font-size: 11.5px;
  white-space: nowrap;
}
.pp-summary {
  display: flex;
  flex-wrap: wrap;
  gap: 18px;
  padding: 10px 12px;
  border-radius: var(--r-md);
  background: var(--surface-soft, color-mix(in srgb, var(--surface) 88%, var(--border)));
  color: var(--text-muted);
  font-size: 11.5px;
}
.pp-summary strong {
  margin-left: 4px;
  color: var(--text);
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
}
.pp-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.pp-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--border);
}
.pp-row:last-child {
  padding-bottom: 0;
  border-bottom: 0;
}
.pp-main {
  min-width: 0;
  flex: 1;
}
.pp-name {
  display: flex;
  align-items: center;
  gap: 7px;
  color: var(--text);
  font-size: 12.5px;
}
.pp-name strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.pp-dot {
  width: 7px;
  height: 7px;
  flex: 0 0 auto;
  border-radius: 50%;
  background: var(--accent);
}
.pp-dot.unknown {
  background: var(--text-subtle);
}
.pp-pct {
  margin-left: auto;
  color: var(--text-muted);
  font-family: var(--font-mono);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
}
.pp-metrics {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  margin-top: 5px;
  color: var(--text-subtle);
  font-size: 11px;
}
.pp-bar {
  height: 4px;
  margin-top: 7px;
  overflow: hidden;
  border-radius: 999px;
  background: var(--track);
}
.pp-bar span {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: var(--accent);
}
.pp-models {
  display: flex;
  max-width: 46%;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 5px;
}
.pp-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 6px;
  border: 1px solid var(--border);
  border-radius: 5px;
  color: var(--text-muted);
  font-size: 10px;
  white-space: nowrap;
}
.pp-chip em {
  color: var(--text-subtle);
  font-style: normal;
}
.pp-more {
  align-self: center;
  color: var(--text-subtle);
  font-size: 10px;
}
.pp-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  padding: 18px 0;
  color: var(--text-subtle);
  font-size: 12px;
  text-align: center;
}
.pp-empty p {
  margin: 0;
}
.pp-sync {
  padding: 5px 12px;
  border: 1px solid var(--border);
  border-radius: var(--r-md);
  background: var(--surface-2);
  color: var(--text);
  font-size: 11.5px;
  cursor: pointer;
}
.pp-sync:hover {
  border-color: var(--border-strong);
}
.pp-hint {
  margin: 0;
  color: var(--text-subtle);
  font-size: 11px;
}
.pp-hint button {
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--accent);
  font-size: 11px;
  cursor: pointer;
}
.pp-expand {
  align-self: flex-start;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--accent);
  font-size: 11.5px;
  cursor: pointer;
}
@media (max-width: 760px) {
  .pp-row {
    flex-direction: column;
  }
  .pp-models {
    max-width: 100%;
    justify-content: flex-start;
  }
}
</style>
