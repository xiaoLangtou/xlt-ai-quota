import ampLogo from "@lobehub/icons-static-svg/icons/amp.svg";
import antigravityLogo from "@lobehub/icons-static-svg/icons/antigravity-color.svg";
import arkLogo from "@lobehub/icons-static-svg/icons/bytedance-color.svg";
import antGroupLogo from "@lobehub/icons-static-svg/icons/antgroup-color.svg";
import claudeLogo from "@lobehub/icons-static-svg/icons/claudecode-color.svg";
import clineLogo from "@lobehub/icons-static-svg/icons/cline.svg";
import codebuddyLogo from "@lobehub/icons-static-svg/icons/codebuddy-color.svg";
import codexLogo from "@lobehub/icons-static-svg/icons/codex-color.svg";
import copilotLogo from "@lobehub/icons-static-svg/icons/copilot-color.svg";
import cursorLogo from "@lobehub/icons-static-svg/icons/cursor.svg";
import deepSeekLogo from "@lobehub/icons-static-svg/icons/deepseek-color.svg";
import geminiLogo from "@lobehub/icons-static-svg/icons/gemini-color.svg";
import gooseLogo from "@lobehub/icons-static-svg/icons/goose.svg";
import kiroLogo from "@lobehub/icons-static-svg/icons/kiro-color.svg";
import kiloLogo from "@lobehub/icons-static-svg/icons/kilocode.svg";
import kimiLogo from "@lobehub/icons-static-svg/icons/kimi.svg";
import mimoLogo from "@lobehub/icons-static-svg/icons/xiaomimimo.svg";
import minimaxLogo from "@lobehub/icons-static-svg/icons/minimax-color.svg";
import midjourneyLogo from "@lobehub/icons-static-svg/icons/midjourney.svg";
import moonshotLogo from "@lobehub/icons-static-svg/icons/moonshot.svg";
import openClawLogo from "@lobehub/icons-static-svg/icons/openclaw-color.svg";
import openCodeLogo from "@lobehub/icons-static-svg/icons/opencode.svg";
import openAiLogo from "@lobehub/icons-static-svg/icons/openai.svg";
import qoderLogo from "@lobehub/icons-static-svg/icons/qoder-color.svg";
import qwenLogo from "@lobehub/icons-static-svg/icons/qwen-color.svg";
import rooLogo from "@lobehub/icons-static-svg/icons/roocode.svg";
import traeLogo from "@lobehub/icons-static-svg/icons/trae-color.svg";
import volcengineLogo from "@lobehub/icons-static-svg/icons/volcengine-color.svg";

export interface ToolLogoAsset {
  src: string;
  /** 单色图标（currentColor），暗色下由全局样式反色 */
  monochrome: boolean;
}

export const TOOL_LOGO_ASSETS: Record<string, ToolLogoAsset> = {
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
  deepseek: { src: deepSeekLogo, monochrome: false },
  "deep-seek": { src: deepSeekLogo, monochrome: false },
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
  amp: { src: ampLogo, monochrome: true },
  antigravity: { src: antigravityLogo, monochrome: false },
  cline: { src: clineLogo, monochrome: true },
  codebuddy: { src: codebuddyLogo, monochrome: false },
  goose: { src: gooseLogo, monochrome: true },
  kilocode: { src: kiloLogo, monochrome: true },
  "kilo-code": { src: kiloLogo, monochrome: true },
  mimo: { src: mimoLogo, monochrome: true },
  minimax: { src: minimaxLogo, monochrome: false },
  openclaw: { src: openClawLogo, monochrome: false },
  qwen: { src: qwenLogo, monochrome: false },
  "qwen-code": { src: qwenLogo, monochrome: false },
  roocode: { src: rooLogo, monochrome: true },
  "roo-code": { src: rooLogo, monochrome: true },
  trae: { src: traeLogo, monochrome: false },
};

export function resolveToolLogo(platform: string): ToolLogoAsset | undefined {
  return TOOL_LOGO_ASSETS[platform.trim().toLowerCase().replace(/\s+/g, "-")];
}
