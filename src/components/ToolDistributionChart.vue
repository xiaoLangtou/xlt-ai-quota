<script setup lang="ts">
import { computed, ref } from "vue";
import type { ToolUsage } from "@/types/usage";
import { formatTokens } from "@/utils/format";
import { useTheme } from "@/composables/useTheme";
import { VChart } from "@/echarts";
import ToolLogo from "@/components/ToolLogo.vue";

const props = defineProps<{ tools: ToolUsage[]; title?: string }>();
const { resolved } = useTheme();

const TOOL_LABEL: Record<string, string> = {
  ark: "火山方舟",
  codex: "Codex",
  claude: "Claude Code",
  kiro: "Kiro",
  kimi: "Kimi",
  qoder: "Qoder",
  "opencode-go": "OpenCode Go",
  gemini: "Gemini CLI",
  copilot: "GitHub Copilot",
  deepseek: "DeepSeek",
};
const TOOL_BRAND: Record<string, [string, string]> = {
  ark: ["--brand-ark", "#ef8c4a"],
  codex: ["--brand-codex", "#4b8ef0"],
  claude: ["--brand-claude", "#cf7350"],
  kiro: ["--brand-kiro", "#8b7ff0"],
  kimi: ["--brand-open", "#17b8a6"],
  qoder: ["--brand-qoder", "#e5697a"],
  "opencode-go": ["--brand-open", "#17b8a6"],
  gemini: ["--brand-gemini", "#4989f5"],
  copilot: ["--brand-copilot", "#6e7681"],
  deepseek: ["--brand-deepseek", "#4d6bfe"],
};

function cssVar(name: string, fallback: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
}
function label(platform: string): string {
  return TOOL_LABEL[platform] ?? platform;
}
function color(platform: string): string {
  const [varName, fallback] = TOOL_BRAND[platform] ?? ["--accent", "#5457e6"];
  return cssVar(varName, fallback);
}

const items = computed(() => props.tools.filter((t) => t.total > 0));
const totalTokens = computed(() => items.value.reduce((sum, t) => sum + t.total, 0));

const hovered = ref<number | null>(null);
const hoveredItem = computed(() =>
  hovered.value == null ? null : items.value[hovered.value] ?? null,
);

function onOver(params: { dataIndex?: number }): void {
  if (typeof params.dataIndex === "number") hovered.value = params.dataIndex;
}
function onOut(): void {
  hovered.value = null;
}

const option = computed(() => {
  void resolved.value; // 主题切换重算
  const surface = cssVar("--surface", "#fff");
  return {
    tooltip: { show: false },
    series: [
      {
        type: "pie",
        radius: ["58%", "86%"],
        center: ["50%", "50%"],
        startAngle: 90,
        avoidLabelOverlap: false,
        itemStyle: { borderColor: surface, borderWidth: 2, borderRadius: 6 },
        label: { show: false },
        labelLine: { show: false },
        emphasis: {
          scale: true,
          scaleSize: 8,
          focus: "self",
          itemStyle: { shadowBlur: 14, shadowColor: "rgba(0, 0, 0, 0.28)" },
        },
        blur: { itemStyle: { opacity: 0.3 } },
        data: items.value.map((t) => ({
          name: label(t.platform),
          value: t.total,
          itemStyle: { color: color(t.platform) },
        })),
      },
    ],
  };
});
</script>

<template>
  <article class="dist-card">
    <div class="dist-head">
      <h3>{{ title ?? "工具分布" }}</h3>
      <p>按 AI 工具查看 Token 占比 · 悬停查看明细</p>
    </div>
    <div class="dist-body">
      <template v-if="totalTokens > 0">
        <VChart
          class="dist-chart"
          :option="option"
          autoresize
          @mouseover="onOver"
          @mouseout="onOut"
          @globalout="onOut"
        />
        <div class="dist-center">
          <template v-if="hoveredItem">
            <ToolLogo :platform="hoveredItem.platform" :size="24" />
            <strong class="dist-name">{{ label(hoveredItem.platform) }}</strong>
            <span class="dist-meta">{{ formatTokens(hoveredItem.total) }} · {{ hoveredItem.pct }}%</span>
          </template>
          <template v-else>
            <span class="dist-label">Tokens</span>
            <strong class="dist-total">{{ formatTokens(totalTokens) }}</strong>
            <span class="dist-meta">{{ items.length }} 个工具</span>
          </template>
        </div>
      </template>
      <div v-else class="dist-empty">暂无用量数据</div>
    </div>
  </article>
</template>

<style scoped>
.dist-card {
  min-width: 0;
  padding: 19px 20px 16px;
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  background: var(--surface);
  box-shadow: var(--shadow-card);
}
.dist-head h3 {
  margin: 0;
  color: var(--text);
  font-size: 15px;
  font-weight: 650;
}
.dist-head p {
  margin: 6px 0 0;
  color: var(--text-muted);
  font-size: 12px;
}
.dist-body {
  position: relative;
  width: 100%;
  height: 248px;
  margin-top: 10px;
}
.dist-chart {
  width: 100%;
  height: 248px;
}
.dist-center {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 4px;
  text-align: center;
  pointer-events: none;
}
.dist-label {
  color: var(--text-subtle);
  font-family: var(--font-mono);
  font-size: 10px;
  letter-spacing: 0.6px;
}
.dist-total {
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 24px;
  font-weight: 650;
  letter-spacing: -0.6px;
}
.dist-name {
  color: var(--text);
  font-size: 14px;
  font-weight: 650;
}
.dist-meta {
  color: var(--text-muted);
  font-family: var(--font-mono);
  font-size: 11px;
}
.dist-empty {
  display: grid;
  place-items: center;
  width: 100%;
  height: 100%;
  color: var(--text-subtle);
  font-size: 12px;
}
</style>
