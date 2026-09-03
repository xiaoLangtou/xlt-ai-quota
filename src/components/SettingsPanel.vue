<script setup lang="ts">
import { ref, watch } from "vue";
import { httpGet } from "@/connectors/types";
import { useUsageDashboard } from "@/composables/useUsageDashboard";

type ArkStatusPayload = {
  logged_in?: boolean;
  ok?: boolean;
  control_plane_auth?: { status?: string; reason?: unknown };
  volc_sso?: { expired?: boolean };
  active_profile?: { name?: string; type?: string };
  profiles_summary?: { is_default?: boolean; display_name?: string }[];
  error?: { message?: unknown };
};

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{
  (e: "update:open", value: boolean): void;
  (e: "synced"): void;
}>();

const { loadSettings, saveSettings } = useUsageDashboard();

const arkBase = ref("");
const openaiKey = ref("");
const openaiOrg = ref("");
const openaiBase = ref("");
const showOpenai = ref(false);
const arkStatus = ref<{ checking: boolean; ok: boolean | null; message: string }>({
  checking: false,
  ok: null,
  message: "",
});
const connectors = ref<{ id: string; configured: boolean }[]>([]);
const saved = ref(false);

function fillForm() {
  const cfg = loadSettings();
  arkBase.value = cfg.ark.baseUrl;
  openaiKey.value = cfg.openai.adminKey;
  openaiOrg.value = cfg.openai.orgId;
  openaiBase.value = cfg.openai.baseUrl;
  connectors.value = cfg.connectors;
}

async function checkArkStatus() {
  arkStatus.value = { checking: true, ok: null, message: "检测中…" };
  try {
    const data = (await httpGet({
      baseUrl: "/api/ark",
      path: "/status",
    })) as ArkStatusPayload;
    // arkcli auth status --format json 的真实结构：
    // { logged_in, control_plane_auth:{status,reason}, volc_sso:{expired}, active_profile, profiles_summary }
    // 旧版裸命令会返回 { ok:false, error:{message} }，一并兼容
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
  saveSettings({
    ark: { baseUrl: arkBase.value },
    openai: {
      adminKey: openaiKey.value,
      orgId: openaiOrg.value,
      baseUrl: openaiBase.value,
    },
  });
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
  <Teleport to="body">
    <Transition name="fade">
      <div v-if="open" class="overlay" @click.self="close">
        <div class="panel">
          <header class="panel-head">
            <h2>设置</h2>
            <button class="close" @click="close">×</button>
          </header>

          <section class="block">
            <div class="block-title">
              <span>火山方舟</span>
              <span class="badge" :class="arkStatus.ok ? 'on' : 'off'">
                {{ arkStatus.checking ? "检测中" : arkStatus.ok ? "arkcli 已登录" : "未登录" }}
              </span>
            </div>
            <p class="status-msg" :class="{ err: !arkStatus.ok && !arkStatus.checking }">
              {{ arkStatus.message }}
            </p>
            <p class="hint">
              Ark 用量需 Volc 签名（AK/SK 或 SSO），<b>不能</b>用 <code>ark-*</code> Bearer Key。
              应用调用本机 <code>arkcli</code>，请在终端执行登录：
            </p>
            <pre class="cmd">arkcli auth login volc-sso</pre>
            <button class="btn ghost" @click="checkArkStatus" :disabled="arkStatus.checking">
              重新检测
            </button>
          </section>

          <section class="block">
            <div class="block-title">
              <span>OpenAI API</span>
              <span
                class="badge"
                :class="connectors.find((c) => c.id === 'openai-api')?.configured ? 'on' : ''"
              >
                {{ connectors.find((c) => c.id === "openai-api")?.configured ? "已配置" : "未配置" }}
              </span>
            </div>
            <label class="field">
              <span>Admin Key（组织级）</span>
              <div class="secret">
                <input
                  :type="showOpenai ? 'text' : 'password'"
                  v-model="openaiKey"
                  placeholder="sk-admin-..."
                  autocomplete="off"
                />
                <button type="button" @click="showOpenai = !showOpenai">
                  {{ showOpenai ? "隐藏" : "显示" }}
                </button>
              </div>
            </label>
            <label class="field">
              <span>Organization ID（可选）</span>
              <input v-model="openaiOrg" placeholder="org-..." />
            </label>
            <label class="field">
              <span>Base URL（可选）</span>
              <input v-model="openaiBase" placeholder="/proxy-openai" />
            </label>
            <p class="hint">仅 Token 用量。ChatGPT/Codex 订阅额度属第三阶段。</p>
          </section>

          <footer class="panel-foot">
            <span v-if="saved" class="saved">已保存</span>
            <span v-else />
            <div class="actions">
              <button class="btn ghost" @click="close">取消</button>
              <button class="btn" @click="persist(false)">保存</button>
              <button class="btn primary" @click="persist(true)">保存并同步</button>
            </div>
          </footer>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.overlay {
  position: fixed;
  inset: 0;
  background: rgba(20, 24, 35, 0.42);
  display: grid;
  place-items: center;
  z-index: 50;
  padding: 24px;
}
.panel {
  width: min(560px, 100%);
  max-height: 88vh;
  overflow: auto;
  background: var(--card);
  border-radius: 20px;
  box-shadow: 0 24px 60px rgba(20, 24, 35, 0.22);
  padding: 22px 24px 18px;
}
.panel-head {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 18px;
}
.panel-head h2 {
  margin: 0;
  font-size: 18px;
}
.close {
  border: 0;
  background: transparent;
  font-size: 22px;
  color: var(--muted);
  cursor: pointer;
  line-height: 1;
}
.block {
  padding: 16px 0;
  border-top: 1px solid var(--line);
}
.block:first-of-type {
  border-top: 0;
}
.block-title {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-weight: 700;
  font-size: 14px;
  margin-bottom: 12px;
}
.badge {
  padding: 3px 9px;
  border-radius: 99px;
  background: #f1f3f6;
  color: #7b8190;
  font-size: 11px;
  font-weight: 600;
}
.badge.on {
  background: #e3f4ec;
  color: var(--green);
}
.badge.off {
  background: #fdece6;
  color: #d65745;
}
.status-msg {
  margin: 0 0 10px;
  color: var(--muted);
  font-size: 12px;
}
.status-msg.err {
  color: #d65745;
}
.hint {
  margin: 0 0 8px;
  color: #969ba6;
  font-size: 11px;
  line-height: 1.6;
}
.hint code {
  background: #f1f3f6;
  padding: 1px 5px;
  border-radius: 4px;
  font-size: 11px;
}
.cmd {
  margin: 0 0 10px;
  padding: 9px 12px;
  background: #262b34;
  color: #e6e8ee;
  border-radius: 8px;
  font-size: 12px;
  overflow-x: auto;
}
.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-bottom: 12px;
}
.field > span {
  color: var(--muted);
  font-size: 12px;
}
.field input {
  padding: 9px 12px;
  border: 1px solid var(--line);
  border-radius: 10px;
  font-size: 13px;
  outline: none;
}
.field input:focus {
  border-color: var(--blue);
}
.secret {
  display: flex;
  gap: 8px;
}
.secret input {
  flex: 1;
}
.secret button {
  padding: 0 12px;
  border: 1px solid var(--line);
  border-radius: 10px;
  background: #f6f7f9;
  color: var(--muted);
  font-size: 12px;
  cursor: pointer;
}
.panel-foot {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-top: 8px;
  padding-top: 14px;
  border-top: 1px solid var(--line);
}
.saved {
  color: var(--green);
  font-size: 12px;
}
.actions {
  display: flex;
  gap: 8px;
}
.btn {
  padding: 8px 16px;
  border: 1px solid var(--line);
  border-radius: 10px;
  background: var(--card);
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
}
.btn.primary {
  background: var(--blue);
  border-color: var(--blue);
  color: white;
}
.btn.ghost {
  border-color: transparent;
  color: var(--muted);
}
.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.15s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
