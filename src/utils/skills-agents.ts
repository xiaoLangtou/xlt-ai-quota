/**
 * Agent 品牌样式。
 * - 品牌 logo 使用 public/agent-icons 下的本地图标（不依赖 iconify 运行时/网络），保证离线可渲染。
 * - 找不到对应图标的用首字母彩色头像兜底。
 */

export interface AgentStyle {
  /** 本地图标 URL（存在则渲染品牌 logo，否则用字母头像） */
  icon?: string;
  /** 品牌主色（字母头像底色） */
  color: string;
  /** 展示名 */
  label: string;
}

/** public/agent-icons 的访问前缀（随 Vite base 变化） */
const ICON_BASE = `${import.meta.env.BASE_URL}agent-icons/`;

/** 已知 Agent → 本地图标文件名（位于 public/agent-icons） */
const AGENT_ICON_FILE: Record<string, string> = {
  Qoder: "qoder.svg",
  Claude: "claude_code.svg",
  "Claude Code": "claude_code.svg",
  Codex: "codex.svg",
  Cursor: "cursor.png",
  Gemini: "gemini_cli.svg",
  "Gemini CLI": "gemini_cli.svg",
  Windsurf: "windsurf.svg",
  Trae: "trae.svg",
  Augment: "augment.svg",
  Cline: "cline.png",
  Continue: "continue.png",
  Kiro: "kiro.svg",
  Roo: "roo_code.svg",
  OpenCode: "opencode.png",
  CodeBuddy: "codebuddy.svg",
  通义灵码: "qwen_code.png",
};

/** 已知 Agent → 品牌样式（key 为 config.rs 中 detect_agent 产出的展示名） */
const AGENT_STYLES: Record<string, AgentStyle> = {
  Qoder: { color: "#22c55e", label: "Qoder" },
  Claude: { color: "#D97757", label: "Claude" },
  "Claude Code": { color: "#D97757", label: "Claude Code" },
  Codex: { color: "#000000", label: "Codex" },
  Cursor: { color: "#000000", label: "Cursor" },
  Gemini: { color: "#1C69FF", label: "Gemini" },
  "Gemini CLI": { color: "#1C69FF", label: "Gemini CLI" },
  Windsurf: { color: "#09B6A2", label: "Windsurf" },
  Codeium: { color: "#09B6A2", label: "Codeium" },
  Augment: { color: "#7c3aed", label: "Augment" },
  Agents: { color: "#64748b", label: "Agents" },
  Cline: { color: "#3b82f6", label: "Cline" },
  Continue: { color: "#0ea5e9", label: "Continue" },
  Kiro: { color: "#8b5cf6", label: "Kiro" },
  Aider: { color: "#ef4444", label: "Aider" },
  Roo: { color: "#f97316", label: "Roo" },
  OpenCode: { color: "#0d9488", label: "OpenCode" },
  CodeBuddy: { color: "#0052D9", label: "CodeBuddy" },
  Trae: { color: "#ec4899", label: "Trae" },
  通义灵码: { color: "#615CED", label: "通义灵码" },
  MarsCode: { color: "#325AB4", label: "MarsCode" },
};

/** 由字符串稳定生成一个 HSL 颜色，作为未知 Agent 的兜底底色 */
function colorFromString(value: string): string {
  let hash = 0;
  for (let index = 0; index < value.length; index += 1) {
    hash = (hash * 31 + value.charCodeAt(index)) | 0;
  }
  const hue = Math.abs(hash) % 360;
  return `hsl(${hue} 55% 45%)`;
}

/** 取 Agent 的品牌样式；已知则附带本地图标，未知则用字母 + 派生色兜底 */
export function agentStyle(agent?: string): AgentStyle {
  if (agent && AGENT_STYLES[agent]) {
    const base = AGENT_STYLES[agent];
    const file = AGENT_ICON_FILE[agent];
    return file ? { ...base, icon: ICON_BASE + file } : base;
  }
  const label = agent?.trim() || "未知";
  return { color: colorFromString(label), label };
}

/** 取用于字母头像的首字符（大写） */
export function agentInitial(agent?: string): string {
  const label = agentStyle(agent).label;
  return (label.slice(0, 1) || "?").toUpperCase();
}
