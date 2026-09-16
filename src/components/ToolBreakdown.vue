<script setup lang="ts">
import type { ToolUsage } from "@/types/usage";
import { formatTokens, formatUsd } from "@/utils/format";
import ToolLogo from "@/components/ToolLogo.vue";

defineProps<{ tools: ToolUsage[]; rangeLabel: string }>();

const TOOL_LABEL: Record<string, string> = {
  ark: "火山方舟",
  codex: "Codex",
  claude: "Claude Code",
  kiro: "Kiro",
  qoder: "Qoder",
  "opencode-go": "OpenCode Go",
  gemini: "Gemini CLI",
  copilot: "GitHub Copilot",
};
const TOOL_BRAND: Record<string, string> = {
  ark: "--brand-ark",
  codex: "--brand-codex",
  claude: "--brand-claude",
  kiro: "--brand-kiro",
  qoder: "--brand-qoder",
  "opencode-go": "--brand-open",
  gemini: "--brand-gemini",
  copilot: "--brand-copilot",
};

function label(platform: string): string {
  return TOOL_LABEL[platform] ?? platform;
}
function brandVar(platform: string): string {
  return TOOL_BRAND[platform] ?? "--accent";
}
</script>

<template>
  <article class="toolbreak">
    <div class="tb-head">
      <div>
        <h3>按工具用量</h3>
        <p>{{ rangeLabel }} · 各 AI 工具的 Token 分布</p>
      </div>
    </div>

    <div v-if="tools.length" class="tb-table" role="table">
      <div class="tb-row tb-header" role="row">
        <span>工具</span>
        <span>占比</span>
        <span class="num">Token</span>
        <span class="num">费用</span>
        <span class="num">请求</span>
      </div>
      <div v-for="t in tools" :key="t.platform" class="tb-row" role="row">
        <span class="tb-name">
          <ToolLogo :platform="t.platform" :size="20" />
          {{ label(t.platform) }}
        </span>
        <span class="tb-share">
          <span class="tb-bar">
            <span :style="{ width: `${t.pct}%`, background: `var(${brandVar(t.platform)})` }" />
          </span>
          <b>{{ t.pct }}%</b>
        </span>
        <span class="num tb-total">{{ formatTokens(t.total) }}</span>
        <span class="num tb-cost">{{ formatUsd(t.costUsd) }}</span>
        <span class="num tb-req">{{ t.requests ? t.requests.toLocaleString() : "—" }}</span>
      </div>
    </div>
    <div v-else class="tb-empty">该周期暂无 Token 记录</div>
  </article>
</template>

<style scoped>
.toolbreak {
  padding: 0;
  overflow: hidden;
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  background: var(--surface);
  box-shadow: var(--shadow-card);
}
.tb-head {
  padding: 18px 20px 4px;
}
.tb-head h3 {
  margin: 0;
  color: var(--text);
  font-size: 15px;
  font-weight: 650;
}
.tb-head p {
  margin: 3px 0 0;
  color: var(--text-muted);
  font-size: 12px;
}
.tb-table {
  margin-top: 6px;
  padding: 0 20px 8px;
}
.tb-row {
  display: grid;
  grid-template-columns: minmax(120px, 1.2fr) minmax(150px, 1.6fr) 92px 92px 76px;
  align-items: center;
  gap: 14px;
  padding: 11px 0;
  border-top: 1px solid var(--border);
}
.tb-header {
  padding: 0 0 8px;
  border-top: 0;
  color: var(--text-subtle);
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.6px;
}
.tb-row .num {
  text-align: right;
}
.tb-name {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  color: var(--text);
  font-size: 13px;
  font-weight: 550;
}
.tb-share {
  display: flex;
  align-items: center;
  gap: 10px;
}
.tb-bar {
  flex: 1;
  height: 7px;
  border-radius: 999px;
  background: var(--surface-3);
  overflow: hidden;
}
.tb-bar span {
  display: block;
  height: 100%;
  min-width: 2px;
  border-radius: 999px;
}
.tb-share b {
  flex: 0 0 auto;
  width: 40px;
  color: var(--text-muted);
  font-family: var(--font-mono);
  font-size: 12px;
  font-weight: 600;
  text-align: right;
}
.tb-total {
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 14px;
  font-weight: 650;
  font-variant-numeric: tabular-nums;
}
.tb-cost {
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 13px;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}
.tb-req {
  color: var(--text-muted);
  font-family: var(--font-mono);
  font-size: 11.5px;
  font-variant-numeric: tabular-nums;
}
.tb-empty {
  display: grid;
  place-items: center;
  min-height: 0;
  padding: 36px 0;
  color: var(--text-subtle);
  font-size: 13px;
}
@media (max-width: 820px) {
  .tb-row {
    grid-template-columns: 1fr auto;
    gap: 6px 12px;
  }
  .tb-header {
    display: none;
  }
  .tb-share {
    grid-column: 1 / -1;
    order: 3;
  }
  .tb-total {
    text-align: right;
  }
  .tb-cost {
    text-align: right;
  }
  .tb-req {
    display: none;
  }
}

</style>
