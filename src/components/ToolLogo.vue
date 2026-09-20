<script setup lang="ts">
import { computed } from "vue";
import { Boxes } from "lucide-vue-next";
import arkLogo from "@lobehub/icons-static-svg/icons/bytedance-color.svg";
import antGroupLogo from "@lobehub/icons-static-svg/icons/antgroup-color.svg";
import claudeLogo from "@lobehub/icons-static-svg/icons/claudecode-color.svg";
import codexLogo from "@lobehub/icons-static-svg/icons/codex-color.svg";
import copilotLogo from "@lobehub/icons-static-svg/icons/copilot-color.svg";
import cursorLogo from "@lobehub/icons-static-svg/icons/cursor.svg";
import geminiLogo from "@lobehub/icons-static-svg/icons/gemini-color.svg";
import kiroLogo from "@lobehub/icons-static-svg/icons/kiro-color.svg";
import kimiLogo from "@lobehub/icons-static-svg/icons/kimi.svg";
import midjourneyLogo from "@lobehub/icons-static-svg/icons/midjourney.svg";
import moonshotLogo from "@lobehub/icons-static-svg/icons/moonshot.svg";
import openCodeLogo from "@lobehub/icons-static-svg/icons/opencode.svg";
import openAiLogo from "@lobehub/icons-static-svg/icons/openai.svg";
import qoderLogo from "@lobehub/icons-static-svg/icons/qoder-color.svg";
import volcengineLogo from "@lobehub/icons-static-svg/icons/volcengine-color.svg";

const props = withDefaults(
  defineProps<{
    platform: string;
    size?: number;
  }>(),
  { size: 18 },
);

const LOGOS: Record<string, { src: string; monochrome: boolean }> = {
  antgroup: { src: antGroupLogo, monochrome: false },
  "蚂蚁数字科技": { src: antGroupLogo, monochrome: false },
  ark: { src: volcengineLogo, monochrome: false },
  "火山方舟": { src: volcengineLogo, monochrome: false },
  bytedance: { src: arkLogo, monochrome: false },
  chatgpt: { src: openAiLogo, monochrome: true },
  claude: { src: claudeLogo, monochrome: false },
  "claude-code": { src: claudeLogo, monochrome: false },
  codex: { src: codexLogo, monochrome: false },
  copilot: { src: copilotLogo, monochrome: false },
  cursor: { src: cursorLogo, monochrome: true },
  gemini: { src: geminiLogo, monochrome: false },
  "github-copilot": { src: copilotLogo, monochrome: false },
  kiro: { src: kiroLogo, monochrome: false },
  kimi: { src: kimiLogo, monochrome: true },
  moonshot: { src: moonshotLogo, monochrome: true },
  midjourney: { src: midjourneyLogo, monochrome: true },
  openai: { src: openAiLogo, monochrome: true },
  opencode: { src: openCodeLogo, monochrome: true },
  "opencode-go": { src: openCodeLogo, monochrome: true },
  qoder: { src: qoderLogo, monochrome: false },
};

const logo = computed(() => LOGOS[props.platform.trim().toLowerCase().replace(/\s+/g, "-")]);
</script>

<template>
  <img
    v-if="logo"
    class="tool-logo"
    :class="{ monochrome: logo.monochrome }"
    :src="logo.src"
    :width="size"
    :height="size"
    :style="{ width: `${size}px`, height: `${size}px` }"
    alt=""
    aria-hidden="true"
  />
  <Boxes
    v-else
    class="tool-logo default-logo"
    :size="size"
    :stroke-width="1.8"
    aria-hidden="true"
  />
</template>

<style scoped>
.tool-logo {
  display: block;
  max-width: none;
  flex: 0 0 auto;
  object-fit: contain;
}

.default-logo {
  color: var(--text-muted);
}
</style>
