<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  ClipboardList,
  Database,
  Fuel,
  Keyboard,
  LayoutGrid,
  Plug,
  ShieldAlert,
  Sparkles,
  Sun,
} from "lucide-vue-next";
import { httpGet, isTauriDesktop } from "@/connectors/types";
import { clipboardService } from "@/services/clipboard-service";
import { dailyReportService } from "@/services/daily-report-service";
import { AI_PROVIDERS, AI_PROVIDER_OPTIONS, aiModelOptions } from "@/config/ai-providers";
import { pushToast } from "@/composables/useToast";
import type { ClipboardSettingsDraft, ClipboardStatus } from "@/types/clipboard";
import type { AiProvider } from "@/types/daily-report";
import { useUsageDashboard } from "@/composables/useUsageDashboard";
import { useTheme } from "@/composables/useTheme";
import type { OilGrade } from "@/types/oil";
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
const oilGrade = ref<OilGrade>("92");
const OIL_GRADE_OPTIONS: readonly OilGrade[] = ["92", "95", "98", "0"];
const { pref: themePref, setPref: setThemePref } = useTheme();

// ---- 剪贴板历史设置（从剪贴板页迁移到设置页） ----
const clipStatus = ref<ClipboardStatus | null>(null);
const clipForm = ref<ClipboardSettingsDraft>({
  enabled: true,
  launchAtLogin: false,
  maxItems: 100,
  ttlDays: 3,
  shortcut: "CommandOrControl+Shift+V",
  excludedApps: [],
});
const clipExcludedText = ref("");
const clipSaving = ref(false);
const recordingShortcut = ref(false);
const shortcutRecordError = ref("");
const shortcutRecorder = ref<HTMLButtonElement | null>(null);
const clipboardAvailable = isTauriDesktop();

async function loadClipboardStatus(): Promise<void> {
  if (!clipboardAvailable) return;
  try {
    clipStatus.value = await clipboardService.status();
    clipForm.value = { ...clipStatus.value.settings, excludedApps: [...clipStatus.value.settings.excludedApps] };
    clipExcludedText.value = clipStatus.value.settings.excludedApps.join("\n");
  } catch (reason) {
    pushToast(reason instanceof Error ? reason.message : String(reason), "error");
  }
}

async function saveClipboardSettings(): Promise<void> {
  recordingShortcut.value = false;
  clipSaving.value = true;
  try {
    clipStatus.value = await clipboardService.updateSettings({
      ...clipForm.value,
      excludedApps: clipExcludedText.value.split(/[,，\n]/).map((item) => item.trim()).filter(Boolean),
    });
    pushToast("剪贴板设置已保存。", "success");
  } catch (reason) {
    pushToast(reason instanceof Error ? reason.message : String(reason), "error");
  } finally {
    clipSaving.value = false;
  }
}

async function recheckAccessibility(): Promise<void> {
  try {
    clipStatus.value = await clipboardService.status();
    if (clipStatus.value.accessibilityGranted) pushToast("辅助功能权限已生效。", "success");
  } catch (reason) {
    pushToast(reason instanceof Error ? reason.message : String(reason), "error");
  }
}

async function requestAccessibility(): Promise<void> {
  try {
    await clipboardService.requestAccessibility();
  } catch (reason) {
    pushToast(reason instanceof Error ? reason.message : String(reason), "error");
  }
}

function shortcutLabel(value: string): string {
  const mac = /Mac|iPhone|iPad/.test(navigator.platform);
  return value.split("+").map((part) => ({
    CommandOrControl: mac ? "⌘" : "Ctrl",
    Command: "⌘",
    Control: "Ctrl",
    Alt: mac ? "⌥" : "Alt",
    Shift: "⇧",
    Space: "Space",
  })[part] ?? part).join("  ");
}

function beginShortcutRecording(): void {
  shortcutRecordError.value = "";
  recordingShortcut.value = true;
  requestAnimationFrame(() => shortcutRecorder.value?.focus());
}

function shortcutKey(event: KeyboardEvent): string | null {
  if (/^Key[A-Z]$/.test(event.code)) return event.code.slice(3);
  if (/^Digit\d$/.test(event.code)) return event.code.slice(5);
  if (/^F([1-9]|1[0-2])$/.test(event.code)) return event.code;
  const keys: Record<string, string> = {
    ArrowUp: "ArrowUp", ArrowDown: "ArrowDown", ArrowLeft: "ArrowLeft", ArrowRight: "ArrowRight",
    Space: "Space", Enter: "Enter", Tab: "Tab", Backspace: "Backspace", Delete: "Delete",
    Home: "Home", End: "End", PageUp: "PageUp", PageDown: "PageDown",
    Minus: "-", Equal: "=", BracketLeft: "[", BracketRight: "]", Backslash: "\\",
    Semicolon: ";", Quote: "'", Comma: ",", Period: ".", Slash: "/", Backquote: "`",
  };
  return keys[event.code] ?? null;
}

function recordShortcut(event: KeyboardEvent): void {
  event.preventDefault();
  event.stopPropagation();
  if (event.key === "Escape") {
    recordingShortcut.value = false;
    shortcutRecordError.value = "";
    return;
  }
  const key = shortcutKey(event);
  if (!key) return;
  const standaloneFunctionKey = /^F([1-9]|1[0-2])$/.test(key);
  if (!(event.metaKey || event.ctrlKey || event.altKey || event.shiftKey) && !standaloneFunctionKey) {
    shortcutRecordError.value = "F1–F12 可单独使用，其他按键需要组合键。";
    return;
  }
  const modifiers: string[] = [];
  if (event.metaKey || event.ctrlKey) modifiers.push("CommandOrControl");
  if (event.altKey) modifiers.push("Alt");
  if (event.shiftKey) modifiers.push("Shift");
  clipForm.value.shortcut = [...modifiers, key].join("+");
  shortcutRecordError.value = "";
  recordingShortcut.value = false;
}

function setOilGrade(grade: OilGrade): void {
  oilGrade.value = grade;
  saveSettings({ oil: { province: oilProvince.value, apiKey: oilApiKey.value, grade } });
  window.dispatchEvent(new CustomEvent("xlt:oil-config-changed"));
}
const systemTz = ref("");
const arkStatus = ref<{ checking: boolean; ok: boolean | null; message: string }>({
  checking: false,
  ok: null,
  message: "",
});
const saved = ref(false);

// ---- AI 报告（Git 日报 / 周报）：读写 daily-report 偏好 ----
const reportPrefs = ref(dailyReportService.loadPreferences());
const reportProvider = ref<AiProvider>("minimax");
const reportModel = ref("");
const reportApiKey = ref("");
const reportProviderOptions = AI_PROVIDER_OPTIONS;
const reportModelOptions = computed(() => aiModelOptions(reportProvider.value));

function fillReportAi(): void {
  reportPrefs.value = dailyReportService.loadPreferences();
  const provider = reportPrefs.value.ai?.provider ?? "minimax";
  reportProvider.value = provider;
  reportModel.value = reportPrefs.value.ai?.model ?? AI_PROVIDERS[provider].models[0];
  reportApiKey.value = reportPrefs.value.aiApiKeys?.[provider] ?? "";
}

function persistReportAi(): void {
  const prefs = { ...reportPrefs.value };
  prefs.ai = { provider: reportProvider.value, model: reportModel.value };
  prefs.aiApiKeys = {
    ...prefs.aiApiKeys,
    [reportProvider.value]: reportApiKey.value.trim(),
  };
  reportPrefs.value = prefs;
  dailyReportService.savePreferences(prefs);
  saved.value = true;
}

function changeReportProvider(value: string): void {
  const next = value as AiProvider;
  persistReportAi();
  reportProvider.value = next;
  reportModel.value = AI_PROVIDERS[next].models[0];
  reportApiKey.value = reportPrefs.value.aiApiKeys?.[next] ?? "";
  persistReportAi();
}

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
  { value: "kimi", label: "Kimi", platform: "kimi" },
];

/** 除火山方舟外的本机 CLI 连接器：登录态由各自 CLI 维护，同步时自动汇总。 */
const CONNECTORS = [
  { key: "codex", name: "Codex", desc: "读取 ~/.codex/sessions 的逐请求 Token", badge: "自动采集", tone: "ok" },
  { key: "claude", name: "Claude Code", desc: "读取 ~/.claude/projects 的逐请求 Token", badge: "自动采集", tone: "ok" },
  { key: "opencode", name: "OpenCode", desc: "读取本机 OpenCode 数据库的逐请求 Token", badge: "自动采集", tone: "ok" },
  { key: "kiro", name: "Kiro CLI", desc: "官方 Credits + 本地会话 Token（estimateTokens 估算）", badge: "额度+估算", tone: "ok" },
  { key: "qoder", name: "Qoder", desc: "套餐 Credits + 本地 SQLite 会话 Token（真实计数）", badge: "额度+Token", tone: "ok" },
  { key: "kimi", name: "Kimi", desc: "会员额度：5h 滚动窗口 / 月 Code 额度 / 月总额度（本机 kimi CLI 登录态）", badge: "额度", tone: "ok" },
  { key: "gemini", name: "Gemini CLI", desc: "读取 ~/.gemini/tmp 会话的真实 Token", badge: "自动采集", tone: "ok" },
  { key: "copilot", name: "GitHub Copilot", desc: "读取 ~/.copilot 会话的真实 Token", badge: "自动采集", tone: "ok" },
] as const;

// ---- 设置页布局：顶部 Tabs + 分区卡片（全部默认展开，不可折叠） ----
type SettingsTab = "general" | "connectors" | "clipboard" | "advanced";

const TABS: { key: SettingsTab; label: string }[] = [
  { key: "general", label: "通用" },
  { key: "connectors", label: "平台连接" },
  { key: "clipboard", label: "剪贴板" },
  { key: "advanced", label: "高级" },
];

const activeTab = ref<SettingsTab>("general");

function fillForm() {
  const cfg = loadSettings();
  arkBase.value = cfg.ark.baseUrl;
  oilProvince.value = cfg.oil.province;
  oilApiKey.value = cfg.oil.apiKey;
  oilGrade.value = cfg.oil.grade ?? "92";
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
      fillReportAi();
      checkArkStatus();
      void loadClipboardStatus();
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

watch([oilProvince, oilApiKey], () => {
  saveSettings({ oil: { province: oilProvince.value, apiKey: oilApiKey.value, grade: oilGrade.value } });
  window.dispatchEvent(new CustomEvent("xlt:oil-config-changed"));
});

function persist(andSync: boolean) {
  saveSettings({
    ark: { baseUrl: arkBase.value },
    oil: { province: oilProvince.value, apiKey: oilApiKey.value, grade: oilGrade.value },
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
    <div v-if="embedded || open" class="settings-root" :class="{ embedded }">
      <header v-if="!embedded" class="page-head">
        <div>
          <h2>设置</h2>
          <p>本机 CLI 与公开数据接口统一汇总，登录态和配置只保存在本机。</p>
        </div>
        <Button v-if="!embedded" variant="ghost" size="icon" aria-label="关闭" @click="close">×</Button>
      </header>

      <nav class="tab-bar" role="tablist" aria-label="设置分类">
        <button
          v-for="tab in TABS"
          :key="tab.key"
          type="button"
          role="tab"
          :aria-selected="activeTab === tab.key"
          :class="{ active: activeTab === tab.key }"
          @click="activeTab = tab.key"
        >
          {{ tab.label }}
        </button>
      </nav>

      <div class="tab-body">
        <!-- ===================== 通用 ===================== -->
        <template v-if="activeTab === 'general'">
          <section class="sec">
            <div class="sec-head solo">
              <span class="sec-icon tone-accent"><Sun :size="18" /></span>
              <span class="sec-copy">
                <strong>显示设置</strong>
                <small>主题外观与用量统计口径</small>
              </span>
            </div>
            <div class="sec-body">
              <div class="setting-row">
                <div class="setting-copy"><strong>外观主题</strong><span>跟随系统，或固定亮色 / 暗色</span></div>
                <div class="mini-seg" role="group" aria-label="外观主题">
                  <button type="button" :class="{ active: themePref === 'light' }" @click="setThemePref('light')">亮</button>
                  <button type="button" :class="{ active: themePref === 'dark' }" @click="setThemePref('dark')">暗</button>
                  <button type="button" :class="{ active: themePref === 'system' }" @click="setThemePref('system')">自动</button>
                </div>
              </div>
              <div class="pref-grid">
                <label class="pref-field">
                  <span>统计时区</span>
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
              <p class="pref-note">时区 / 起始日调整后，点底部「保存」并重新同步，按新设置采集本机用量。</p>
            </div>
          </section>

          <section class="sec">
            <div class="sec-head solo">
              <span class="sec-icon tone-warn"><Fuel :size="18" /></span>
              <span class="sec-copy">
                <strong>油价监控</strong>
                <small>省级指导价、下一调价窗口与国家发改委正式公告</small>
              </span>
            </div>
            <div class="sec-body">
              <div class="setting-row">
                <div class="setting-copy"><strong>油品标号</strong><span>概览油价卡片重点展示的标号</span></div>
                <div class="mini-seg" role="group" aria-label="油品标号">
                  <button v-for="grade in OIL_GRADE_OPTIONS" :key="grade" type="button"
                    :class="{ active: oilGrade === grade }" @click="setOilGrade(grade)">{{ grade }}#</button>
                </div>
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
          </section>

          <section class="sec">
            <div class="sec-head solo">
              <span class="sec-icon tone-info"><LayoutGrid :size="18" /></span>
              <span class="sec-copy">
                <strong>套餐额度展示</strong>
                <small>只展示你正在订阅或需要关注的平台</small>
              </span>
            </div>
            <div class="sec-body">
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
          </section>
        </template>

        <!-- ===================== 平台连接 ===================== -->
        <template v-else-if="activeTab === 'connectors'">
          <section class="sec">
            <div class="sec-head solo">
              <span class="sec-icon tone-accent"><Plug :size="18" /></span>
              <span class="sec-copy">
                <strong>平台连接</strong>
                <small>经本机 CLI 登录态读取，无需配置 API Key</small>
              </span>
            </div>
            <div class="sec-body sec-body-flat">
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
          </section>
        </template>

        <!-- ===================== 剪贴板 ===================== -->
        <template v-else-if="activeTab === 'clipboard'">
          <section class="sec">
            <div class="sec-head solo">
              <span class="sec-icon tone-ok"><ClipboardList :size="18" /></span>
              <span class="sec-copy">
                <strong>剪贴板历史</strong>
                <small>采集范围、保留策略与全局快捷键</small>
              </span>
            </div>
            <div class="sec-body">
              <template v-if="clipboardAvailable">
                <div v-if="clipStatus && !clipStatus.accessibilityGranted" class="clip-permission">
                  <ShieldAlert :size="16" />
                  <span>
                    <strong>辅助功能权限未开启</strong>
                    <small>请允许当前运行程序：{{ clipStatus.accessibilityTarget }}</small>
                  </span>
                  <span class="clip-permission-actions">
                    <Button type="button" variant="outline" size="sm" @click="requestAccessibility">去授权</Button>
                    <Button type="button" variant="outline" size="sm" @click="recheckAccessibility">重新检查</Button>
                  </span>
                </div>
                <label class="clip-toggle">
                  <div><strong>自动记录剪贴板</strong><span>应用常驻时监听文本、图片和文件。</span></div>
                  <Checkbox v-model="clipForm.enabled" />
                </label>
                <label class="clip-toggle">
                  <div><strong>登录时自动启动</strong><span>在后台启动并继续记录，不打开主窗口。</span></div>
                  <Checkbox v-model="clipForm.launchAtLogin" />
                </label>
                <div class="clip-grid">
                  <label><span>容量上限</span><Input v-model="clipForm.maxItems" type="number" min="20" max="5000" /><small>收藏记录不占用名额</small></label>
                  <label><span>保留时长（天）</span><Input v-model="clipForm.ttlDays" type="number" min="1" max="365" /><small>到期时自动删除</small></label>
                </div>
                <label class="clip-field">
                  <span>全局快捷键</span>
                  <button ref="shortcutRecorder" class="shortcut-recorder" :class="{ recording: recordingShortcut }"
                    type="button" :aria-pressed="recordingShortcut" @click="beginShortcutRecording"
                    @keydown="recordShortcut" @blur="recordingShortcut = false">
                    <Keyboard :size="16" />
                    <span v-if="recordingShortcut">请按下快捷键…</span>
                    <kbd v-else>{{ shortcutLabel(clipForm.shortcut) }}</kbd>
                    <em>{{ recordingShortcut ? "Esc 取消" : "点击录制" }}</em>
                  </button>
                  <small :class="{ 'shortcut-error': shortcutRecordError }">{{ shortcutRecordError || "F1–F12 可单独使用，其他按键需要组合；保存时检查占用" }}</small>
                </label>
                <label class="clip-field">
                  <span>不记录这些应用</span>
                  <textarea v-model="clipExcludedText" class="cn-textarea" rows="4" placeholder="每行一个应用名称" />
                  <small>默认包含常见密码管理器</small>
                </label>
                <label v-if="clipStatus" class="clip-field">
                  <span>本机存储位置</span>
                  <Input :model-value="clipStatus.storagePath" readonly />
                  <small>历史数据库与图片只保存在应用数据目录。</small>
                </label>
                <div class="clip-actions">
                  <Button type="button" :disabled="clipSaving" @click="saveClipboardSettings">{{ clipSaving ? "保存中…" : "保存剪贴板设置" }}</Button>
                </div>
              </template>
              <p v-else class="pref-note">剪贴板历史仅在 Tauri 桌面端可用，浏览器预览不支持系统剪贴板监听。</p>
            </div>
          </section>
        </template>

        <!-- ===================== 高级 ===================== -->
        <template v-else>
          <section class="sec">
            <div class="sec-head solo">
              <span class="sec-icon tone-accent"><Sparkles :size="18" /></span>
              <span class="sec-copy">
                <strong>AI 报告</strong>
                <small>Git 日报 / 周报使用的 Provider、模型与 API Key（仅保存在本机）</small>
              </span>
            </div>
            <div class="sec-body">
              <div class="setting-row">
                <div class="setting-copy"><strong>Provider</strong><span>生成报告使用的 AI 服务</span></div>
                <Select
                  :model-value="reportProvider"
                  :options="reportProviderOptions"
                  placeholder="选择 Provider"
                  @update:model-value="changeReportProvider"
                />
              </div>
              <div class="pref-grid">
                <label class="pref-field">
                  <span>模型</span>
                  <Select v-model="reportModel" :options="reportModelOptions" placeholder="选择模型"
                    @update:model-value="persistReportAi" />
                </label>
                <label class="pref-field">
                  <span>API Key</span>
                  <Input v-model="reportApiKey" type="password" autocomplete="off" placeholder="本机保存"
                    @change="persistReportAi" />
                </label>
              </div>
              <p class="pref-note">修改后立即保存；Git 报告页按此处配置生成内容。</p>
            </div>
          </section>

          <section class="sec">
            <div class="sec-head solo">
              <span class="sec-icon tone-info"><Database :size="18" /></span>
              <span class="sec-copy">
                <strong>数据与存储</strong>
                <small>用量、片段与配置均只保存在本机</small>
              </span>
            </div>
            <div class="sec-body">
              <div class="setting-row">
                <div class="setting-copy"><strong>存储引擎</strong><span>Tauri 桌面端为 SQLite，浏览器预览为 localStorage</span></div>
                <span class="engine-badge">{{ isTauriDesktop() ? "SQLite" : "localStorage" }}</span>
              </div>
            </div>
          </section>
        </template>
      </div>

      <footer class="page-foot">
        <span class="foot-hint">
          <template v-if="saved">已保存</template>
          <template v-else>连接配置只保存在本机，不写入项目文件。</template>
        </span>
        <div class="actions">
          <Button v-if="!embedded" variant="ghost" @click="close">关闭</Button>
          <Button variant="outline" @click="persist(true)">立即同步</Button>
          <Button class="save-btn" @click="persist(false)">保存</Button>
        </div>
      </footer>
    </div>
  </Dialog>
</template>

<style scoped>
.settings-root {
  width: min(640px, 100%);
  max-height: 88vh;
  overflow: auto;
  background: var(--surface);
  border-radius: var(--r-xl);
  box-shadow: var(--shadow-pop);
  padding: 22px 24px 18px;
}
.settings-root.embedded {
  display: flex;
  width: 100%;
  max-width: none;
  height: 100%;
  min-height: 0;
  flex-direction: column;
  overflow: hidden;
  padding: 0 2px;
  border-radius: 0;
  background: transparent;
  box-shadow: none;
}
.page-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 20px;
  margin-bottom: 16px;
}
.page-head h2 {
  margin: 0;
  font-size: 20px;
  font-weight: 650;
}
.page-head p {
  margin: 6px 0 0;
  color: var(--text-muted);
  font-size: 12.5px;
}

/* 顶部分类 Tabs：胶囊分段，选中项实心高亮 */
.tab-bar {
  display: flex;
  flex: 0 0 auto;
  gap: 4px;
  padding: 4px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--segment-bg);
}
.settings-root.embedded .tab-bar {
  position: sticky;
  top: 0;
  z-index: 3;
  background: var(--bg);
}
.tab-bar button {
  flex: 1;
  padding: 7px 12px;
  border: 0;
  border-radius: 9px;
  background: transparent;
  color: var(--text-muted);
  font: inherit;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
}
.tab-bar button:hover {
  color: var(--text);
}
.tab-bar button.active {
  background: var(--accent);
  color: var(--accent-fg, #fff);
  font-weight: 650;
  box-shadow: var(--shadow-sm);
}

.tab-body {
  display: flex;
  min-height: 0;
  flex: 1;
  flex-direction: column;
  gap: 12px;
  margin-top: 16px;
  overflow-y: auto;
  padding-bottom: 8px;
}
.settings-root:not(.embedded) .tab-body {
  min-height: 0;
  flex: none;
  overflow: visible;
}

/* 可折叠分区卡片：彩色图标 + 标题/描述 + 右侧箭头 */
.sec {
  flex: 0 0 auto;
  overflow: hidden;
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  background: var(--surface);
  box-shadow: var(--shadow-card);
}
.sec-head {
  display: flex;
  width: 100%;
  align-items: center;
  gap: 14px;
  padding: 15px 18px;
  border: 0;
  background: transparent;
  color: inherit;
  font: inherit;
  text-align: left;
  cursor: pointer;
}
.sec-head:hover {
  background: var(--surface-2);
}
.sec-head.solo {
  cursor: default;
}
.sec-head.solo:hover {
  background: transparent;
}
.sec-icon {
  display: grid;
  width: 36px;
  height: 36px;
  flex: 0 0 36px;
  place-items: center;
  border-radius: 10px;
}
.sec-icon.tone-accent {
  background: color-mix(in srgb, var(--accent) 14%, transparent);
  color: var(--accent);
}
.sec-icon.tone-warn {
  background: color-mix(in srgb, var(--u-warn) 14%, transparent);
  color: var(--u-warn);
}
.sec-icon.tone-info {
  background: color-mix(in srgb, #4f8cff 14%, transparent);
  color: #4f8cff;
}
.sec-icon.tone-ok {
  background: color-mix(in srgb, var(--u-ok) 14%, transparent);
  color: var(--u-ok);
}
.sec-copy {
  min-width: 0;
  flex: 1;
}
.sec-copy strong {
  display: block;
  color: var(--text);
  font-size: 14px;
  font-weight: 650;
}
.sec-copy small {
  display: block;
  margin-top: 3px;
  color: var(--text-muted);
  font-size: 12px;
  line-height: 1.5;
}
.sec-body {
  padding: 4px 18px 18px;
  border-top: 1px solid var(--border);
}
.sec-body-flat {
  padding: 6px 0 0;
}

/* 平台连接行（嵌在连接分区内，上下贯通） */
.connector-row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 13px 18px;
  border-bottom: 1px solid var(--border);
}
.conn-mark {
  display: grid;
  width: 36px;
  height: 36px;
  flex: 0 0 36px;
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
  font-size: 13.5px;
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
  margin: 12px 18px;
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

/* 分区内表单元素 */
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
  flex: 1 1 150px;
  min-width: 150px;
}
.oil-key-field { flex: 1 1 100%; min-width: min(240px, 100%); }
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
.setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 11px 0;
}
.setting-row + .pref-grid {
  margin-top: 4px;
}
.setting-copy strong {
  display: block;
  color: var(--text);
  font-size: 12.5px;
  font-weight: 600;
}
.setting-copy span {
  display: block;
  margin-top: 2px;
  color: var(--text-subtle);
  font-size: 11px;
}
.mini-seg {
  display: inline-flex;
  flex: none;
  gap: 2px;
  padding: 3px;
  border: 1px solid var(--border);
  border-radius: var(--r-sm);
  background: var(--segment-bg);
}
.mini-seg button {
  min-width: 44px;
  padding: 4px 10px;
  border: 0;
  border-radius: 5px;
  background: transparent;
  color: var(--text-muted);
  font: inherit;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
}
.mini-seg button:hover {
  color: var(--text);
}
.mini-seg button.active {
  background: var(--surface);
  box-shadow: var(--shadow-sm);
  color: var(--accent);
  font-weight: 650;
}
.engine-badge {
  flex: none;
  padding: 3px 9px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--surface-2);
  color: var(--text-muted);
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 600;
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

/* 剪贴板设置 */
.clip-permission {
  display: flex;
  align-items: flex-start;
  gap: 9px;
  margin-top: 12px;
  padding: 11px 12px;
  border: 1px solid color-mix(in srgb, var(--u-warn) 28%, var(--border));
  border-radius: var(--r-sm);
  background: color-mix(in srgb, var(--u-warn) 6%, var(--surface));
  color: var(--u-warn);
}
.clip-permission > span {
  display: grid;
  min-width: 0;
  gap: 3px;
}
.clip-permission strong {
  color: var(--text);
  font-size: 12px;
}
.clip-permission small {
  overflow: hidden;
  color: var(--text-muted);
  font-family: var(--font-mono);
  font-size: 10.5px;
  line-height: 1.5;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.clip-permission-actions {
  display: flex;
  gap: 8px;
  margin-left: auto;
  flex: 0 0 auto;
}
.clip-toggle {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-top: 10px;
  padding: 11px 12px;
  border: 1px solid var(--border);
  border-radius: var(--r-sm);
  background: var(--surface-2);
}
.clip-toggle div {
  display: grid;
  gap: 3px;
}
.clip-toggle strong {
  color: var(--text);
  font-size: 12.5px;
  font-weight: 600;
}
.clip-toggle span {
  color: var(--text-muted);
  font-size: 11px;
}
.clip-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
  margin-top: 10px;
}
.clip-grid label,
.clip-field {
  display: grid;
  gap: 6px;
  color: var(--text);
  font-size: 12px;
  font-weight: 650;
}
.clip-grid label > span,
.clip-field > span {
  color: var(--text-subtle);
  font-size: 10.5px;
  font-weight: 600;
  letter-spacing: 0.4px;
}
.clip-grid small,
.clip-field small {
  color: var(--text-subtle);
  font-size: 10.5px;
  font-weight: 400;
}
.clip-field {
  margin-top: 10px;
}
.clip-actions {
  display: flex;
  justify-content: flex-end;
  margin-top: 12px;
}
.shortcut-recorder {
  display: flex;
  width: 100%;
  min-height: 40px;
  align-items: center;
  gap: 10px;
  padding: 0 12px;
  border: 1px solid var(--border);
  border-radius: var(--r-sm);
  outline: 0;
  background: var(--surface-2);
  color: var(--text-muted);
  font: inherit;
  cursor: pointer;
  transition: border-color 0.15s, box-shadow 0.15s, background 0.15s;
}
.shortcut-recorder:hover {
  border-color: var(--border-strong);
  background: var(--surface);
}
.shortcut-recorder:focus-visible,
.shortcut-recorder.recording {
  border-color: var(--accent);
  background: var(--surface);
  box-shadow: 0 0 0 3px var(--accent-weak);
}
.shortcut-recorder.recording > svg {
  color: var(--accent);
}
.shortcut-recorder > span {
  color: var(--accent);
  font-weight: 650;
}
.shortcut-recorder kbd {
  padding: 3px 8px;
  border: 1px solid var(--border-strong);
  border-radius: 5px;
  background: var(--surface);
  box-shadow: 0 1px 0 var(--border-strong);
  color: var(--text);
  font-family: var(--font-sans);
  font-size: 12px;
  font-weight: 700;
}
.shortcut-recorder em {
  margin-left: auto;
  color: var(--text-subtle);
  font-size: 10.5px;
  font-style: normal;
  font-weight: 500;
}
.clip-field small.shortcut-error {
  color: var(--u-crit);
}

.page-foot {
  position: sticky;
  bottom: 0;
  z-index: 2;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  margin: 18px -2px 0;
  padding: 14px 8px 10px;
  border-top: 1px solid var(--border);
  background: var(--surface);
}
.settings-root.embedded .page-foot {
  position: static;
  flex: 0 0 auto;
  margin: 12px -2px 0;
  background: var(--bg);
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
.save-btn {
  background: var(--accent);
  color: var(--accent-fg, #fff);
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
    padding-left: 48px;
  }
  .page-foot {
    flex-wrap: wrap;
  }
}
</style>
