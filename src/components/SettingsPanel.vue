<script setup lang="ts">
import { ref, watch } from "vue";
import { httpGet } from "@/connectors/types";
import { useUsageDashboard } from "@/composables/useUsageDashboard";
import { Dialog } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import ToolLogo from "@/components/ToolLogo.vue";

type ArkStatusPayload = {
  logged_in?: boolean;
  ok?: boolean;
  control_plane_auth?: { status?: string; reason?: unknown };
  volc_sso?: { expired?: boolean };
  active_profile?: { name?: string; type?: string };
  profiles_summary?: { is_default?: boolean; display_name?: string }[];
  error?: { message?: unknown };
};

const props = defineProps<{ open?: boolean; embedded?: boolean }>();
const emit = defineEmits<{
  (e: "update:open", value: boolean): void;
  (e: "synced"): void;
}>();

const { loadSettings, saveSettings } = useUsageDashboard();

const arkBase = ref("");
const arkStatus = ref<{ checking: boolean; ok: boolean | null; message: string }>({
  checking: false,
  ok: null,
  message: "",
});
const saved = ref(false);

/** 除火山方舟外的本机 CLI 连接器：登录态由各自 CLI 维护，同步时自动汇总。 */
const CONNECTORS = [
  { key: "codex", name: "Codex", desc: "读取 ~/.codex/sessions 的逐请求 Token", badge: "自动采集", tone: "ok" },
  { key: "claude", name: "Claude Code", desc: "读取 ~/.claude/projects 的逐请求 Token", badge: "自动采集", tone: "ok" },
  { key: "opencode", name: "OpenCode", desc: "读取本机 OpenCode 数据库的逐请求 Token", badge: "自动采集", tone: "ok" },
  { key: "kiro", name: "Kiro CLI", desc: "官方 Credits + 本地会话 Token（estimateTokens 估算）", badge: "额度+估算", tone: "ok" },
  { key: "qoder", name: "Qoder", desc: "套餐 Credits + 本地 SQLite 会话 Token（真实计数）", badge: "额度+Token", tone: "ok" },
] as const;

function fillForm() {
  const cfg = loadSettings();
  arkBase.value = cfg.ark.baseUrl;
}

async function checkArkStatus() {
  arkStatus.value = { checking: true, ok: null, message: "检测中…" };
  try {
    const data = (await httpGet({
      baseUrl: "/api/ark",
      path: "/status",
    })) as ArkStatusPayload;
    const cp = data?.control_plane_auth;
    const loggedIn = data?.logged_in === true || data?.ok === true;
    const cpOk = cp?.status === "ok";
    const ssoExpired = data?.volc_sso?.expired === true;
    const display =
      data?.profiles_summary?.find?.((p: { is_default?: boolean }) => p.is_default)?.display_name ||
      data?.active_profile?.name ||
      data?.active_profile?.type;
    if (loggedIn && cpOk && !ssoExpired) {
      arkStatus.value = {
        checking: false,
        ok: true,
        message: display ? `已登录 · ${display}` : "已登录",
      };
    } else {
      const reason =
        data?.error?.message || cp?.reason || (ssoExpired ? "SSO 已过期" : "未登录");
      arkStatus.value = {
        checking: false,
        ok: false,
        message: String(reason),
      };
    }
  } catch (e) {
    arkStatus.value = {
      checking: false,
      ok: false,
      message: "无法检测本机 arkcli 状态",
    };
    void e;
  }
}

watch(
  () => props.open,
  (v) => {
    if (v) {
      saved.value = false;
      fillForm();
      checkArkStatus();
    }
  },
  { immediate: true },
);

function close() {
  emit("update:open", false);
}

function persist(andSync: boolean) {
  saveSettings({ ark: { baseUrl: arkBase.value } });
  fillForm();
  if (andSync) {
    emit("synced");
    close();
  } else {
    saved.value = true;
  }
}
</script>

<template>
  <Dialog :open="Boolean(open)" :static="embedded" @update:open="close">
    <div v-if="embedded || open" :class="{ 'embedded-root': embedded }">
      <div class="panel" :class="{ embedded }">
        <header class="panel-head">
          <div v-if="embedded">
            <h2>数据源</h2>
            <p>均为本机 CLI，登录态只保存在本机；点击同步统一汇总</p>
          </div>
          <div v-else>
            <span class="eyebrow">CONNECTORS</span>
            <h2>连接与设置</h2>
            <p>数据源均为本机 CLI，登录态只保存在本机；点击同步统一汇总。</p>
          </div>
          <Button v-if="!embedded" variant="ghost" size="icon" aria-label="关闭" @click="close">×</Button>
        </header>

        <div class="connector-list">
          <!-- 火山方舟：需登录，展示动态状态 -->
          <div class="connector-row">
            <span class="conn-mark"><ToolLogo platform="ark" :size="22" /></span>
            <div class="conn-info">
              <strong>火山方舟</strong>
              <small>套餐额度 + Token 用量 · 需 Volc SSO 登录</small>
            </div>
            <div class="conn-side">
              <span class="conn-badge" :class="arkStatus.ok ? 'ok' : arkStatus.checking ? 'neutral' : 'off'">
                {{ arkStatus.checking ? "检测中" : arkStatus.ok ? "已登录" : "未登录" }}
              </span>
              <Button variant="ghost" size="sm" :disabled="arkStatus.checking" @click="checkArkStatus">重新检测</Button>
            </div>
          </div>
          <div v-if="!arkStatus.ok && !arkStatus.checking" class="connector-note">
            <p>
              Ark 用量需 Volc 签名（AK/SK 或 SSO），<b>不能</b>用 <code>ark-*</code> Bearer Key。
              请在终端登录后再同步：
            </p>
            <pre class="cmd">arkcli auth login volc-sso</pre>
            <p v-if="arkStatus.message" class="conn-msg">{{ arkStatus.message }}</p>
          </div>

          <!-- 其他本机 CLI：自动采集，无需登录配置 -->
          <div v-for="c in CONNECTORS" :key="c.key" class="connector-row">
            <span class="conn-mark"><ToolLogo :platform="c.key" :size="22" /></span>
            <div class="conn-info">
              <strong>{{ c.name }}</strong>
              <small>{{ c.desc }}</small>
            </div>
            <div class="conn-side">
              <span class="conn-badge" :class="c.tone">{{ c.badge }}</span>
            </div>
          </div>
        </div>

        <footer class="panel-foot">
          <span class="foot-hint">
            <template v-if="saved">已保存</template>
            <template v-else>WebStorm ACP 只是启动这些 CLI 的入口，不单独保存 Token。</template>
          </span>
          <div class="actions">
            <Button v-if="!embedded" variant="ghost" @click="close">关闭</Button>
            <Button @click="persist(true)">立即同步</Button>
          </div>
        </footer>
      </div>
    </div>
  </Dialog>
</template>

<style scoped>
.panel {
  width: min(560px, 100%);
  max-height: 88vh;
  overflow: auto;
  background: var(--surface);
  border-radius: var(--r-xl);
  box-shadow: var(--shadow-pop);
  padding: 22px 24px 18px;
}
.embedded-root {
  width: 100%;
}
.panel.embedded {
  width: 100%;
  max-width: 1320px;
  max-height: none;
  margin: 0 auto;
  padding: 0;
  border: 0;
  border-radius: 0;
  background: transparent;
  box-shadow: none;
}
.panel-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 20px;
  margin-bottom: 20px;
  padding-bottom: 18px;
  border-bottom: 1px solid var(--border);
}
.eyebrow {
  color: var(--text-subtle);
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 1.5px;
}
.panel-head h2 {
  margin: 5px 0 0;
  font-size: 22px;
  font-weight: 650;
}
.panel-head p {
  margin: 6px 0 0;
  color: var(--text-muted);
  font-size: 13px;
}

.connector-list {
  display: grid;
  gap: 10px;
}
.connector-row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 14px 16px;
  border: 1px solid var(--border);
  border-radius: var(--r-md);
  background: var(--surface);
}
.conn-mark {
  display: grid;
  width: 34px;
  height: 34px;
  flex: 0 0 34px;
  place-items: center;
  border-radius: var(--r-sm);
  background: var(--surface-2);
}
.conn-info {
  min-width: 0;
  flex: 1;
}
.conn-info strong {
  display: block;
  color: var(--text);
  font-size: 14px;
  font-weight: 600;
}
.conn-info small {
  display: block;
  margin-top: 3px;
  color: var(--text-muted);
  font-size: 12px;
}
.conn-side {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 0 0 auto;
}
.conn-badge {
  padding: 3px 9px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 550;
  white-space: nowrap;
}
.conn-badge.ok {
  background: color-mix(in srgb, var(--u-ok) 16%, transparent);
  color: var(--u-ok);
}
.conn-badge.neutral {
  background: var(--surface-2);
  color: var(--text-muted);
}
.conn-badge.off {
  background: color-mix(in srgb, var(--u-warn) 16%, transparent);
  color: var(--u-warn);
}
.connector-note {
  margin: -2px 0 2px;
  padding: 14px 16px;
  border: 1px dashed var(--border-strong);
  border-radius: var(--r-md);
  background: var(--surface-2);
}
.connector-note p {
  margin: 0;
  color: var(--text-muted);
  font-size: 12px;
  line-height: 1.6;
}
.connector-note code {
  padding: 1px 5px;
  border-radius: 4px;
  background: var(--surface-3);
  font-family: var(--font-mono);
  font-size: 11px;
}
.cmd {
  margin: 10px 0;
  padding: 9px 12px;
  border: 1px solid var(--border);
  border-radius: var(--r-sm);
  background: var(--surface-3);
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 12px;
  overflow-x: auto;
}
.conn-msg {
  color: var(--u-warn);
}
.panel-foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  margin-top: 18px;
  padding-top: 16px;
  border-top: 1px solid var(--border);
}
.foot-hint {
  color: var(--text-subtle);
  font-size: 12px;
}
.actions {
  display: flex;
  gap: 8px;
  flex: 0 0 auto;
}

@media (max-width: 640px) {
  .connector-row {
    flex-wrap: wrap;
  }
  .conn-side {
    width: 100%;
    justify-content: flex-start;
    padding-left: 46px;
  }
}

/* Glass prototype */
.panel.embedded {
  width: 100%;
  max-width: none;
  padding: 0 0 6px;
  border: 1px solid var(--glass-border);
  border-radius: 20px;
  background: var(--glass-fill);
  box-shadow: var(--glass-shadow);
  backdrop-filter: blur(18px) saturate(180%);
  -webkit-backdrop-filter: blur(18px) saturate(180%);
}
.panel.embedded .panel-head { align-items: baseline; margin: 0; padding: 21px 26px 16px; border: 0; }
.panel.embedded .panel-head h2 { margin: 0 0 3px; font-size: 16.5px; font-weight: 700; }
.panel.embedded .panel-head p { margin: 0; color: var(--text-subtle); font-size: 12.5px; }
.panel.embedded .connector-list { display: block; }
.panel.embedded .connector-row { gap: 14px; padding: 16px 26px; border: 0; border-top: 1px solid var(--border); border-radius: 0; background: transparent; }
.panel.embedded .connector-row:first-child { border-top: 0; }
.panel.embedded .conn-mark { width: 37px; height: 37px; flex-basis: 37px; border-radius: 12px; font-family: "Manrope", sans-serif; }
.panel.embedded .conn-info strong { margin-bottom: 2px; font-size: 14px; font-weight: 700; }
.panel.embedded .conn-info small { margin: 0; color: var(--text-subtle); font-size: 12px; }
.panel.embedded .conn-badge { padding: 5px 11px; border-radius: 20px; font-size: 11px; font-weight: 600; }
.panel.embedded .conn-badge.ok { background: rgba(47, 190, 143, 0.15); color: #1e9a76; }
.panel.embedded .conn-badge.off { background: rgba(242, 146, 74, 0.15); color: #c46a1f; }
.panel.embedded .connector-note { margin: 0 26px 18px; padding: 14px 17px; border: 1px solid rgba(242, 146, 74, 0.24); border-radius: 15px; background: rgba(242, 146, 74, 0.08); }
.panel.embedded .connector-note p { color: #8a5623; }
.panel.embedded .connector-note code { background: var(--surface-3); }
.panel.embedded .cmd { margin: 10px 0 0; padding: 10px 15px; border: 0; border-radius: 10px; background: rgba(27, 32, 54, 0.9); color: #eaf0ff; font-size: 12.5px; }
.panel.embedded .panel-foot { margin: 0; padding: 14px 26px 13px; border: 0; }
.panel.embedded .foot-hint { color: var(--text-subtle); font-size: 11.5px; }
.panel.embedded .actions { display: none; }
</style>
