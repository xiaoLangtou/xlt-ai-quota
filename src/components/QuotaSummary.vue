<script setup lang="ts">
import { computed } from "vue";
import { Button } from "@/components/ui/button";
import ToolLogo from "@/components/ToolLogo.vue";
import type { PlatformQuotaView } from "@/types/usage";

const props = defineProps<{ views: PlatformQuotaView[] }>();
const emit = defineEmits<{ (e: "navigate"): void }>();
const platformCount = computed(() => new Set(props.views.map((view) => view.platform)).size);

type Tone = "ok" | "warn" | "crit";
interface Row {
  id: string;
  platform: string;
  name: string;
  planTag: string;
  pct: number | null;
  tone: Tone;
  note: string | null;
  exhausted: boolean;
}

const rows = computed<Row[]>(() =>
  props.views.map((v) => {
    let pct: number | null = null;
    if (v.credits) {
      pct =
        v.credits.total > 0
          ? Math.round(((v.credits.total - v.credits.remaining) / v.credits.total) * 100)
          : 0;
    } else if (v.windows.length) {
      // 取利用率最高的窗口作为该平台的代表值（最吃紧的那一项）。
      const w = v.windows.reduce((a, b) => (b.usedPct > a.usedPct ? b : a));
      pct = w.usedPct;
    }
    const exhausted = pct != null && pct >= 100;
    const tone: Tone = pct == null ? "ok" : pct >= 100 ? "crit" : pct >= 80 ? "warn" : "ok";
    const note = pct == null ? (v.account ? "已登录 · 详情见连接器" : "暂无数据") : null;
    return {
      id: v.id,
      platform: v.platform,
      name: v.name,
      planTag: v.planTag,
      pct,
      tone,
      note,
      exhausted,
    };
  }),
);
</script>

<template>
  <article class="qsummary">
    <div class="qsummary-head">
      <div>
        <h2>配额摘要</h2>
        <p>按平台聚合的额度使用率 · {{ platformCount }} 个平台</p>
      </div>
      <Button class="qsummary-link" type="button" variant="ghost" size="sm" @click="emit('navigate')">
        详情 →
      </Button>
    </div>
    <ul class="qsummary-rows">
      <li v-for="r in rows" :key="r.id" class="qrow">
        <span class="qname">
          <ToolLogo :platform="r.platform" :size="15" />
          {{ r.name }}
        </span>
        <template v-if="r.pct != null">
          <div class="qbar">
            <span :class="r.tone" :style="{ width: `${Math.min(100, r.pct)}%` }" />
          </div>
          <span class="qval" :class="r.tone">{{ r.exhausted ? "耗尽" : `${r.pct}%` }}</span>
        </template>
        <template v-else>
          <span class="qnote">{{ r.note }}</span>
          <span class="qval muted">—</span>
        </template>
      </li>
    </ul>
  </article>
</template>

<style scoped>
.qsummary {
  display: flex;
  min-width: 0;
  flex-direction: column;
  padding: 16px 18px;
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  background: var(--surface);
  box-shadow: var(--shadow-card);
}

.qsummary-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
}

.qsummary-head h2 {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 0;
  color: var(--text);
  font-size: 13.5px;
  font-weight: 700;
}

.qsummary-head p {
  margin: 2px 0 0;
  color: var(--text-subtle);
  font-size: 11.5px;
}

.qsummary-link {
  flex: 0 0 auto;
  height: 24px;
  padding: 0 6px;
  color: var(--accent);
  font-size: 11.5px;
}

.qsummary-rows {
  margin: 10px 0 0;
  padding: 0;
  list-style: none;
}

.qrow {
  display: grid;
  grid-template-columns: minmax(76px, 92px) minmax(60px, 1fr) 40px;
  align-items: center;
  gap: 9px;
  padding: 6px 0;
  font-size: 12.5px;
}

.qname {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 7px;
  overflow: hidden;
  color: var(--text);
  font-weight: 550;
  white-space: nowrap;
}

.qbar {
  height: 6px;
  overflow: hidden;
  border-radius: 999px;
  background: var(--surface-3);
}

.qbar span {
  display: block;
  height: 100%;
  min-width: 3px;
  border-radius: inherit;
  transition: width 0.3s ease;
}

.qbar span.ok {
  background: var(--accent);
}

.qbar span.warn {
  background: var(--u-warn);
}

.qbar span.crit {
  background: var(--u-crit);
}

.qval {
  color: var(--text-muted);
  font-size: 11.5px;
  font-weight: 650;
  text-align: right;
  font-variant-numeric: tabular-nums;
}

.qval.warn {
  color: var(--u-warn);
}

.qval.crit {
  color: var(--u-crit);
}

.qval.muted,
.qnote {
  color: var(--text-subtle);
}

.qnote {
  overflow: hidden;
  font-size: 11px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
