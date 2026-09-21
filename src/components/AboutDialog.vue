<script setup lang="ts">
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
  <UModal
    :open="open"
    title="AI 用量看板"
    description="本地优先，统一展示各平台订阅额度与 Token 用量"
    @update:open="emit('update:open', $event)"
  >
    <template #body>
      <div class="space-y-5">
        <div class="flex items-center gap-3 text-xs text-muted">
          <UBadge color="neutral" variant="subtle" class="font-mono">v{{ APP_VERSION }}</UBadge>
          <span>数据只保存在本机，不上传云端</span>
        </div>

        <section>
          <h3 class="mb-2 text-[11px] font-semibold uppercase tracking-wider text-dimmed">支持的工具</h3>
          <div class="grid grid-cols-1 sm:grid-cols-2 gap-2.5">
            <div
              v-for="tool in TOOLS"
              :key="tool.key"
              class="flex items-start gap-2.5 rounded-xl border border-default bg-elevated/40 p-3"
            >
              <ToolLogo :platform="tool.key" :size="22" />
              <div class="min-w-0">
                <strong class="block text-[13px] font-semibold text-highlighted">{{ tool.name }}</strong>
                <small class="mt-0.5 block text-[11.5px] text-muted">{{ tool.hint }}</small>
                <code
                  v-if="tool.login"
                  class="mt-1.5 inline-block break-all rounded bg-elevated px-1.5 py-0.5 font-mono text-[11px] text-highlighted"
                >{{ tool.login }}</code>
              </div>
            </div>
          </div>
        </section>

        <p class="text-xs leading-relaxed text-dimmed">
          各工具登录态只保存在本机 CLI；应用不要求配置第三方 API Key。首次使用点击左下角「同步」从本机 CLI 拉取真实用量。
        </p>
      </div>
    </template>

    <template #footer>
      <div class="flex justify-end w-full">
        <UButton @click="close">知道了</UButton>
      </div>
    </template>
  </UModal>
</template>
