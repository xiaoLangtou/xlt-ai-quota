<script setup lang="ts">
import { Dialog } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import ToolLogo from "@/components/ToolLogo.vue";

defineProps<{ open: boolean }>();
const emit = defineEmits<{ (e: "update:open", value: boolean): void }>();

const APP_VERSION = "0.1.3";

// 支持的本机工具与接入方式（安装引导 / 支持工具网格）。
const TOOLS: { key: string; name: string; hint: string; login?: string }[] = [
  { key: "ark", name: "火山方舟", hint: "套餐额度 + Token 用量", login: "arkcli auth login volc-sso" },
  { key: "codex", name: "Codex", hint: "读取 ~/.codex/sessions", login: "codex login" },
  { key: "claude", name: "Claude Code", hint: "读取 ~/.claude/projects" },
  { key: "kiro", name: "Kiro", hint: "Credits + 本地会话估算", login: "kiro-cli login" },
  { key: "qoder", name: "Qoder", hint: "Credits + 本地 SQLite Token", login: "qodercli login" },
  { key: "opencode-go", name: "OpenCode", hint: "读取本机 OpenCode 数据库" },
  { key: "gemini", name: "Gemini CLI", hint: "读取 ~/.gemini/tmp 会话" },
  { key: "copilot", name: "GitHub Copilot", hint: "读取 ~/.copilot 会话" },
];

function close() {
  emit("update:open", false);
}
</script>

<template>
  <Dialog :open="open" content-class="about-dialog" @update:open="emit('update:open', $event)">
    <div class="about">
      <header class="about-head">
        <div>
          <div class="about-logo">/_</div>
          <h2>AI 用量看板</h2>
          <p>本地优先，统一展示各平台订阅额度与 Token 用量</p>
        </div>
        <Button variant="ghost" size="icon" aria-label="关闭" @click="close">×</Button>
      </header>

      <div class="about-meta">
        <span class="ver">v{{ APP_VERSION }}</span>
        <span class="dot">·</span>
        <span>数据只保存在本机，不上传云端</span>
      </div>

      <div class="about-section">
        <h3>支持的工具</h3>
        <div class="tool-grid">
          <div v-for="t in TOOLS" :key="t.key" class="tool-card">
            <ToolLogo :platform="t.key" :size="22" />
            <div class="tool-info">
              <strong>{{ t.name }}</strong>
              <small>{{ t.hint }}</small>
              <code v-if="t.login">{{ t.login }}</code>
            </div>
          </div>
        </div>
      </div>

      <p class="about-note">
        各工具登录态只保存在本机 CLI；应用不要求配置第三方 API Key。首次使用点击左下角「同步」从本机 CLI 拉取真实用量。
      </p>

      <footer class="about-foot">
        <Button @click="close">知道了</Button>
      </footer>
    </div>
  </Dialog>
</template>

<style scoped>
.about {
  width: 100%;
  min-height: 0;
  padding: 24px 26px 20px;
  border: 1px solid var(--glass-border, var(--border));
  border-radius: 22px;
  background: var(--glass-fill, var(--surface));
  box-shadow: var(--glass-shadow, var(--shadow-pop));
  backdrop-filter: blur(20px) saturate(180%);
  -webkit-backdrop-filter: blur(20px) saturate(180%);
}
.about-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}
.about-logo {
  display: grid;
  place-items: center;
  width: 42px;
  height: 42px;
  margin-bottom: 12px;
  border-radius: 13px;
  background: var(--surface);
  border: 1px solid var(--border);
  box-shadow: var(--shadow-sm);
  color: var(--text);
  font-family: var(--font-mono);
  font-weight: 700;
  font-size: 15px;
}
.about-head h2 {
  margin: 0;
  font-family: "Manrope", "PingFang SC", sans-serif;
  font-size: 21px;
  font-weight: 800;
  letter-spacing: -0.01em;
}
.about-head p {
  margin: 5px 0 0;
  color: var(--text-muted);
  font-size: 13px;
}
.about-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 14px 0 4px;
  color: var(--text-subtle);
  font-size: 12px;
}
.about-meta .ver {
  padding: 2px 9px;
  border-radius: 999px;
  background: var(--surface-3);
  color: var(--text);
  font-family: var(--font-mono);
  font-weight: 600;
}
.about-section {
  margin-top: 16px;
}
.about-section h3 {
  margin: 0 0 10px;
  color: var(--text-subtle);
  font-family: var(--font-mono);
  font-size: 10.5px;
  font-weight: 600;
  letter-spacing: 0.8px;
  text-transform: uppercase;
}
.tool-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
}
.tool-card {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  padding: 12px 13px;
  border: 1px solid var(--border);
  border-radius: 14px;
  background: color-mix(in srgb, var(--surface) 60%, transparent);
  transition: border-color 0.16s ease, background 0.16s ease;
}
.tool-card:hover {
  border-color: var(--border-strong);
  background: var(--surface);
}
.tool-info {
  min-width: 0;
}
.tool-info strong {
  display: block;
  color: var(--text);
  font-size: 13px;
  font-weight: 600;
}
.tool-info small {
  display: block;
  margin-top: 2px;
  color: var(--text-muted);
  font-size: 11.5px;
}
.tool-info code {
  display: inline-block;
  margin-top: 6px;
  padding: 2px 6px;
  border-radius: 5px;
  background: var(--surface-3);
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 11px;
  word-break: break-all;
}
.about-note {
  margin: 16px 0 0;
  color: var(--text-subtle);
  font-size: 12px;
  line-height: 1.6;
}
.about-foot {
  display: flex;
  justify-content: flex-end;
  margin-top: 18px;
  padding-top: 15px;
  border-top: 1px solid var(--glass-border, var(--border));
}
@media (max-width: 520px) {
  .tool-grid {
    grid-template-columns: 1fr;
  }
}
</style>
