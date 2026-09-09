<script setup lang="ts">
import { computed } from "vue";
import ToolLogo from "@/components/ToolLogo.vue";
import type { PlatformQuotaView } from "@/types/usage";

const props = defineProps<{ views: PlatformQuotaView[] }>();
const emit = defineEmits<{ (e: "navigate"): void }>();

type Tone = "ok" | "warn" | "crit";
interface Row {
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
        <h2>订阅额度</h2>
        <p>{{ views.length }} 个平台 · 利用率概览</p>
      </div>
      <button class="qsummary-link" type="button" @click="emit('navigate')">
        前往订阅与账单 →
      </button>
    </div>
    <ul class="qsummary-rows">
      <li v-for="r in rows" :key="r.platform" class="qrow">
        <span class="qname">
          <ToolLogo :platform="r.platform" :size="18" />
          {{ r.name }}
          <em class="qtag">{{ r.planTag }}</em>
        </span>
        <template v-if="r.pct != null">
          <div class="qbar">
            <span :class="r.tone" :style="{ width: `${Math.min(100, r.pct)}%` }" />
          </div>
          <span class="qval" :class="r.tone">{{ r.exhausted ? "已耗尽" : `${r.pct}%` }}</span>
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
  padding: 20px 22px;
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  background: var(--surface);
  box-shadow: var(--shadow-card);
}
.qsummary-head {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 6px;
}
.qsummary-head h2 {
  margin: 0;
  font-size: 16px;
  font-weight: 650;
  letter-spacing: -0.2px;
}
.qsummary-head p {
  margin: 5px 0 0;
  color: var(--text-muted);
  font-size: 12px;
}
.qsummary-link {
  flex: 0 0 auto;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--accent);
  font: inherit;
  font-size: 12px;
  cursor: pointer;
}
.qsummary-link:hover {
  text-decoration: underline;
}
.qsummary-rows {
  margin: 0;
  padding: 0;
  list-style: none;
}
.qrow {
  display: grid;
  grid-template-columns: minmax(130px, 200px) 1fr 62px;
  align-items: center;
  gap: 14px;
  padding: 12px 0;
  border-top: 1px solid var(--border);
}
.qrow:first-child {
  border-top: 0;
}
.qname {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  color: var(--text);
  font-size: 13px;
  font-weight: 550;
}
.qtag {
  overflow: hidden;
  color: var(--text-subtle);
  font-family: var(--font-mono);
  font-size: 10px;
  font-style: normal;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.qbar {
  height: 6px;
  border-radius: 999px;
  background: var(--surface-3);
  overflow: hidden;
}
.qbar span {
  display: block;
  height: 100%;
  min-width: 3px;
  border-radius: 999px;
  transition: width 0.4s ease;
}
.qbar span.ok { background: var(--u-ok); }
.qbar span.warn { background: var(--u-warn); }
.qbar span.crit { background: var(--u-crit); }
.qval {
  font-family: var(--font-mono);
  font-size: 13px;
  font-weight: 600;
  text-align: right;
  font-variant-numeric: tabular-nums;
}
.qval.ok { color: var(--text); }
.qval.warn { color: var(--u-warn); }
.qval.crit { color: var(--u-crit); }
.qval.muted { color: var(--text-subtle); }
.qnote {
  overflow: hidden;
  color: var(--text-subtle);
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
@media (max-width: 520px) {
  .qrow {
    grid-template-columns: 1fr 52px;
  }
  .qname {
    grid-column: 1 / -1;
  }
}

/* Glass prototype */
.qsummary {
  padding: 0 0 7px;
  border: 1px solid var(--glass-border);
  border-radius: 20px;
  background: var(--glass-fill);
  box-shadow: var(--glass-shadow);
  backdrop-filter: blur(18px) saturate(180%);
  -webkit-backdrop-filter: blur(18px) saturate(180%);
}
.qsummary-head {
  align-items: baseline;
  margin: 0;
  padding: 21px 26px 5px;
}
.qsummary-head h2 { margin: 0 0 3px; font-size: 16.5px; font-weight: 700; }
.qsummary-head p { margin: 0; color: var(--text-subtle); font-size: 12.5px; }
.qsummary-link { color: var(--accent); font-size: 12.5px; font-weight: 600; }
.qrow {
  grid-template-columns: 175px 1fr 66px;
  gap: 14px;
  padding: 13px 26px;
  border-top: 1px solid var(--border);
}
.qrow:first-child { border-top: 0; }
.qname { gap: 8px; color: var(--text); font-size: 14px; font-weight: 600; }
.qtag { padding: 2px 8px; border-radius: 7px; background: var(--surface-3); color: var(--text-subtle); font-family: var(--font-sans); font-size: 10.5px; font-weight: 600; }
.qbar { height: 7px; background: var(--track); }
.qval { width: 66px; color: var(--text); font-size: 13px; font-weight: 700; }
.qval.crit { color: #d8405c; }
.qnote { color: var(--text-subtle); font-size: 12px; }

@media (max-width: 640px) {
  .qrow { grid-template-columns: 1fr 52px; padding: 13px 18px; }
  .qname { grid-column: 1 / -1; }
}
</style>
