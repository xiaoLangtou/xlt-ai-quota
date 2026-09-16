<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { httpGet } from "@/connectors/types";
import { useUsageDashboard } from "@/composables/useUsageDashboard";
import { Dialog } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { DatePicker } from "@/components/ui/date-picker";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import ToolLogo from "@/components/ToolLogo.vue";
import { type QuotaDisplayTarget } from "@/types/usage";

type ArkStatusPayload = {
  logged_in?: boolean;
  ok?: boolean;
  control_plane_auth?: { status?: string; reason?: unknown };
  volc_sso?: { expired?: boolean };
  active_profile?: { name?: string; type?: string };
  profiles_summary?: { is_default?: boolean; display_name?: string }[];
  error?: { message?: unknown };
};

const props = withDefaults(defineProps<{
  open?: boolean;
  embedded?: boolean;
  syncSignal?: number;
}>(), { syncSignal: 0 });
const emit = defineEmits<{
  (e: "update:open", value: boolean): void;
  (e: "synced"): void;
}>();

const { loadSettings, saveSettings } = useUsageDashboard();

const arkBase = ref("");
const timezone = ref("");
const statsSince = ref("");
const hiddenQuotaTargets = ref<QuotaDisplayTarget[]>([]);
const oilProvince = ref("");
const oilApiKey = ref("");
const systemTz = ref("");
const arkStatus = ref<{ checking: boolean; ok: boolean | null; message: string }>({
  checking: false,
  ok: null,
  message: "",
});
const saved = ref(false);

// 常见时区选项（含跟随系统）。
const TIMEZONE_OPTIONS = [
  "Asia/Shanghai",
  "Asia/Hong_Kong",
  "Asia/Tokyo",
  "Asia/Singapore",
  "UTC",
  "America/Los_Angeles",
  "America/New_York",
  "Europe/London",
  "Europe/Paris",
];

const OIL_PROVINCES = [
  "北京", "天津", "河北", "山西", "内蒙古", "辽宁", "吉林", "黑龙江",
  "上海", "江苏", "浙江", "安徽", "福建", "江西", "山东", "河南", "湖北",
  "湖南", "广东", "广西", "海南", "重庆", "四川", "贵州", "云南", "西藏",
  "陕西", "甘肃", "青海", "宁夏", "新疆",
];
const OIL_PROVINCE_OPTIONS = OIL_PROVINCES.map((province) => ({
  value: province,
  label: province,
}));

const QUOTA_DISPLAY_OPTIONS: { value: QuotaDisplayTarget; label: string; platform: string }[] = [
  { value: "codex", label: "Codex", platform: "codex" },
  { value: "ark-coding", label: "火山方舟 · Coding Plan", platform: "ark" },
  { value: "ark-agent", label: "火山方舟 · Agent Plan", platform: "ark" },
  { value: "kiro", label: "Kiro", platform: "kiro" },
  { value: "qoder", label: "Qoder", platform: "qoder" },
];

/** 除火山方舟外的本机 CLI 连接器：登录态由各自 CLI 维护，同步时自动汇总。 */
const CONNECTORS = [
  { key: "codex", name: "Codex", desc: "读取 ~/.codex/sessions 的逐请求 Token", badge: "自动采集", tone: "ok" },
  { key: "claude", name: "Claude Code", desc: "读取 ~/.claude/projects 的逐请求 Token", badge: "自动采集", tone: "ok" },
  { key: "opencode", name: "OpenCode", desc: "读取本机 OpenCode 数据库的逐请求 Token", badge: "自动采集", tone: "ok" },
  { key: "kiro", name: "Kiro CLI", desc: "官方 Credits + 本地会话 Token（estimateTokens 估算）", badge: "额度+估算", tone: "ok" },
  { key: "qoder", name: "Qoder", desc: "套餐 Credits + 本地 SQLite 会话 Token（真实计数）", badge: "额度+Token", tone: "ok" },
  { key: "gemini", name: "Gemini CLI", desc: "读取 ~/.gemini/tmp 会话的真实 Token", badge: "自动采集", tone: "ok" },
  { key: "copilot", name: "GitHub Copilot", desc: "读取 ~/.copilot 会话的真实 Token", badge: "自动采集", tone: "ok" },
] as const;

function fillForm() {
  const cfg = loadSettings();
  arkBase.value = cfg.ark.baseUrl;
  oilProvince.value = cfg.oil.province;
  oilApiKey.value = cfg.oil.apiKey;
  systemTz.value = cfg.systemTimezone;
  timezone.value = cfg.preferences.timezone;
  statsSince.value = cfg.preferences.statsSince ?? "";
  hiddenQuotaTargets.value = [...cfg.quotaDisplay.hiddenPlatforms];
}

/** 时区下拉选项：系统时区置顶，去重后拼接常见时区。 */
const timezoneOptions = computed(() => {
  const list = [systemTz.value, ...TIMEZONE_OPTIONS];
  return [...new Set(list.filter(Boolean))].map((value) => ({
    value,
    label: value === systemTz.value ? `${value}（系统）` : value,
  }));
});

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
    if (v || props.embedded) {
      saved.value = false;
      fillForm();
      checkArkStatus();
    }
  },
  { immediate: true },
);

watch(
  () => props.syncSignal,
  (value, previous) => {
    if (props.embedded && value !== previous) persist(true);
  },
);

function close() {
  emit("update:open", false);
}

function persist(andSync: boolean) {
  saveSettings({
    ark: { baseUrl: arkBase.value },
    oil: { province: oilProvince.value, apiKey: oilApiKey.value },
    quotaDisplay: { hiddenPlatforms: hiddenQuotaTargets.value },
    preferences: { timezone: timezone.value, statsSince: statsSince.value || null },
  });
  fillForm();
  if (andSync) {
    emit("synced");
    close();
  } else {
    saved.value = true;
  }
}

function updateQuotaDisplayTarget(target: QuotaDisplayTarget, visible: boolean): void {
  hiddenQuotaTargets.value = visible
    ? hiddenQuotaTargets.value.filter((item) => item !== target)
    : [...hiddenQuotaTargets.value, target];
  saveSettings({ quotaDisplay: { hiddenPlatforms: hiddenQuotaTargets.value } });
  saved.value = true;
}

/** 保存偏好（不触发同步），供嵌入模式即时生效。 */
function savePreferencesOnly() {
  saveSettings({ preferences: { timezone: timezone.value, statsSince: statsSince.value || null } });
  fillForm();
  saved.value = true;
}

function updateTimezone(value: string): void {
  timezone.value = value;
  savePreferencesOnly();
}

function updateStatsSince(value: string): void {
  statsSince.value = value;
  savePreferencesOnly();
}
</script>

<template>
  <Dialog :open="Boolean(open)" :static="embedded" @update:open="close">
    <div v-if="embedded || open" :class="{ 'embedded-root': embedded }">
      <div class="panel" :class="{ embedded }">
        <header class="panel-head">
          <div v-if="embedded">
            <h2>数据源</h2>
            <p>本机 CLI 与公开数据接口统一汇总，配置只保存在本机</p>
          </div>
          <div v-else>
            <span class="eyebrow">CONNECTORS</span>
            <h2>连接与设置</h2>
            <p>本机 CLI 与公开数据接口统一汇总，登录态和配置只保存在本机。</p>
          </div>
          <Button v-if="!embedded" variant="ghost" size="icon" aria-label="关闭" @click="close">×</Button>
        </header>

        <div class="pref-section quota-display-section">
          <div class="pref-head">
            <strong>套餐额度展示</strong>
            <small>只展示你正在订阅或需要关注的平台</small>
          </div>
          <div class="quota-display-grid">
            <label v-for="option in QUOTA_DISPLAY_OPTIONS" :key="option.value" class="quota-display-option">
              <Checkbox
                :model-value="!hiddenQuotaTargets.includes(option.value)"
                @update:model-value="updateQuotaDisplayTarget(option.value, $event === true)"
              />
              <ToolLogo :platform="option.platform" :size="18" />
              <span>{{ option.label }}</span>
            </label>
          </div>
          <p class="pref-note">隐藏后仅不在套餐额度区域显示，已采集的数据不会删除。</p>
        </div>

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

        <div class="pref-section">
          <div class="pref-head">
            <strong>国内油价监控</strong>
            <small>省级最高零售指导价、下一调价窗口与国家发改委正式公告</small>
          </div>
          <div class="pref-grid">
            <label class="pref-field">
              <span>监控省份</span>
              <Select v-model="oilProvince" :options="OIL_PROVINCE_OPTIONS" placeholder="请选择省份" />
            </label>
            <label class="pref-field oil-key-field">
              <span>极数本源预测 API KEY（可选）</span>
              <Input v-model="oilApiKey" type="password" autocomplete="off" placeholder="匿名额度不足时填写" />
            </label>
          </div>
          <p class="pref-note">今日指导价与预测分开同步；加油站实际挂牌价可能浮动，正式调价以国家发改委公告为准。</p>
        </div>

        <div class="pref-section">
          <div class="pref-head">
            <strong>统计偏好</strong>
            <small>影响用量分析的分桶时区与统计起始日</small>
          </div>
          <div class="pref-grid">
            <label class="pref-field">
              <span>时区</span>
              <Select :model-value="timezone" :options="timezoneOptions" @update:model-value="updateTimezone" />
            </label>
            <label class="pref-field">
              <span>统计起始日</span>
              <DatePicker :model-value="statsSince" placeholder="选择起始日期" @update:model-value="updateStatsSince" />
            </label>
            <Button
              v-if="statsSince"
              type="button"
              variant="ghost"
              size="sm"
              @click="statsSince = ''; savePreferencesOnly()"
            >
              清除起始日
            </Button>
          </div>
          <p class="pref-note">时区 / 起始日调整后，点击「立即同步」按新设置重新采集本机用量。</p>
        </div>

        <footer class="panel-foot">
          <span class="foot-hint">
            <template v-if="saved">已保存</template>
            <template v-else>连接配置只保存在本机，不写入项目文件。</template>
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
  max-width: none;
  max-height: none;
  margin: 0 auto;
  padding: 0 0 6px;
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  background: var(--surface);
  box-shadow: var(--shadow-card);
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
.pref-section {
  margin-top: 16px;
  padding: 16px;
  border: 1px solid var(--border);
  border-radius: var(--r-md);
  background: var(--surface);
}
.pref-head strong {
  display: block;
  color: var(--text);
  font-size: 14px;
  font-weight: 600;
}
.pref-head small {
  display: block;
  margin-top: 3px;
  color: var(--text-muted);
  font-size: 12px;
}
.pref-grid {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-end;
  gap: 12px;
  margin-top: 12px;
}
.pref-field {
  display: flex;
  flex-direction: column;
  gap: 5px;
  min-width: 160px;
}
.oil-key-field { min-width: min(280px, 100%); flex: 1; }
.pref-field span {
  color: var(--text-subtle);
  font-family: var(--font-mono);
  font-size: 10.5px;
  font-weight: 600;
  letter-spacing: 0.6px;
}
.pref-note {
  margin: 12px 0 0;
  color: var(--text-subtle);
  font-size: 12px;
}
.quota-display-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 9px;
  margin-top: 12px;
}
.quota-display-option {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 9px;
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: var(--r-sm);
  color: var(--text);
  font-size: 13px;
  cursor: pointer;
}
.quota-display-option:hover {
  border-color: var(--border-strong);
  background: var(--surface-2);
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

.panel.embedded .panel-head {
  align-items: baseline;
  margin: 0;
  padding: 18px 22px 14px;
  border-bottom: 1px solid var(--border);
}
.panel.embedded .panel-head h2 {
  margin: 0 0 3px;
  font-size: 16px;
  font-weight: 680;
}
.panel.embedded .panel-head p {
  margin: 0;
  color: var(--text-subtle);
  font-size: 12px;
}
.panel.embedded .connector-list {
  display: block;
}
.panel.embedded .connector-row {
  gap: 14px;
  padding: 14px 22px;
  border: 0;
  border-bottom: 1px solid var(--border);
  border-radius: 0;
}
.panel.embedded .conn-mark {
  width: 36px;
  height: 36px;
  flex-basis: 36px;
}
.panel.embedded .connector-note {
  margin: 14px 22px;
}
.panel.embedded .pref-section {
  margin: 14px 22px 4px;
}
.panel.embedded .panel-foot {
  margin: 0;
  padding: 14px 22px 10px;
  border: 0;
}
.panel.embedded .actions {
  display: none;
}

@media (max-width: 640px) {
  .quota-display-grid {
    grid-template-columns: 1fr;
  }
  .connector-row {
    flex-wrap: wrap;
  }
  .conn-side {
    width: 100%;
    justify-content: flex-start;
    padding-left: 46px;
  }
}

</style>
