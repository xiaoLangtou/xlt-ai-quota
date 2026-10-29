<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useRoute } from "vue-router";
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
import { invoke } from "@tauri-apps/api/core";
import { httpGet, isTauriDesktop } from "@/connectors/types";
import { clipboardService } from "@/services/clipboard-service";
import { dailyReportService } from "@/services/daily-report-service";
import { AI_PROVIDERS, AI_PROVIDER_OPTIONS, aiModelOptions } from "@/config/ai-providers";
import { pushToast } from "@/composables/useToast";
import type { ClipboardSettingsDraft, ClipboardStatus } from "@/types/clipboard";
import type { AiProvider } from "@/types/daily-report";
import { useUsageDashboard } from "@/composables/useUsageDashboard";
import { usageService } from "@/services/usage-service";
import { useTheme } from "@/composables/useTheme";
import { useSidebarNavigation } from "@/composables/useSidebarNavigation";
import { NAV_GROUPS } from "@/config/navigation";
import { zentaoService } from "@/services/zentao-service";
import type { ZentaoStatus } from "@/types/productivity";
import type { OilGrade } from "@/types/oil";
import { Dialog } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { DatePicker } from "@/components/ui/date-picker";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import ToolLogo from "@/components/ToolLogo.vue";
import TranslateSettings from "@/components/translate/TranslateSettings.vue";
import { type QuotaDisplayTarget, type UsageCacheStats } from "@/types/usage";

type ArkStatusPayload = {
  logged_in?: boolean;
  ok?: boolean;
  control_plane_auth?: { status?: string; reason?: unknown };
  volc_sso?: { expired?: boolean; identity?: { is_root?: boolean } };
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
const { hiddenPaths: hiddenMenuPaths, setMenuVisible, showAllMenus } = useSidebarNavigation();

const arkBase = ref("");
const deepseekApiKey = ref("");
// ---- 禅道同步：登录态写入 ~/.zentao-token.json，密码不进应用存储 ----
const zentaoAvailable = isTauriDesktop();
const zentaoForm = ref({ url: "", account: "", password: "" });
const zentaoStatus = ref<ZentaoStatus>({ configured: false, url: "", account: "" });
const zentaoLoggingIn = ref(false);
const zentaoShowForm = ref(false);

async function loadZentaoStatus(): Promise<void> {
  if (!zentaoAvailable) return;
  try {
    zentaoStatus.value = await zentaoService.status();
    if (zentaoStatus.value.configured) {
      zentaoForm.value.url = zentaoStatus.value.url;
      zentaoForm.value.account = zentaoStatus.value.account;
    }
  } catch {
    zentaoStatus.value = { configured: false, url: "", account: "" };
  }
}

async function zentaoLogin(): Promise<void> {
  zentaoLoggingIn.value = true;
  try {
    zentaoStatus.value = await zentaoService.login(
      zentaoForm.value.url,
      zentaoForm.value.account,
      zentaoForm.value.password,
    );
    zentaoForm.value.password = "";
    zentaoShowForm.value = false;
    pushToast(`禅道已登录：${zentaoStatus.value.account}`, "success");
  } catch (reason) {
    pushToast(reason instanceof Error ? reason.message : String(reason), "error");
  } finally {
    zentaoLoggingIn.value = false;
  }
}

async function zentaoLogout(): Promise<void> {
  try {
    await zentaoService.logout();
    zentaoStatus.value = { configured: false, url: "", account: "" };
    zentaoShowForm.value = true;
    pushToast("已退出禅道登录。", "success");
  } catch (reason) {
    pushToast(reason instanceof Error ? reason.message : String(reason), "error");
  }
}

const timezone = ref("");
const statsSince = ref("");
const cacheStats = ref<UsageCacheStats | null>(null);
const cacheBusy = ref(false);

function formatBytes(bytes: number): string {
  if (!bytes) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  const index = Math.min(units.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)));
  return `${(bytes / 1024 ** index).toFixed(index === 0 ? 0 : 1)} ${units[index]}`;
}

function loadCacheStats(): void {
  cacheStats.value = usageService.getCacheStats();
}

async function cleanupCache(): Promise<void> {
  if (cacheBusy.value) return;
  cacheBusy.value = true;
  try {
    cacheStats.value = await usageService.cleanupCache();
    pushToast("已清理过期用量缓存", "success");
  } catch (reason) {
    pushToast(reason instanceof Error ? reason.message : String(reason), "error");
  } finally {
    cacheBusy.value = false;
  }
}

async function clearCache(): Promise<void> {
  if (cacheBusy.value) return;
  cacheBusy.value = true;
  try {
    cacheStats.value = await usageService.clearUsageCache();
    pushToast("已清空本地用量缓存", "success");
  } catch (reason) {
    pushToast(reason instanceof Error ? reason.message : String(reason), "error");
  } finally {
    cacheBusy.value = false;
  }
}
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
const addingReportModel = ref(false);
const customReportModel = ref("");
const reportModelOptions = computed(() => aiModelOptions(
  reportProvider.value,
  reportPrefs.value.aiCustomModels?.[reportProvider.value],
  reportModel.value,
));

function cancelAddReportModel(): void {
  addingReportModel.value = false;
  customReportModel.value = "";
}

function addReportModel(): void {
  const model = customReportModel.value.trim();
  if (!model) return;
  const provider = reportProvider.value;
  if (!reportModelOptions.value.some((option) => option.value === model)) {
    reportPrefs.value = {
      ...reportPrefs.value,
      aiCustomModels: {
        ...reportPrefs.value.aiCustomModels,
        [provider]: [...(reportPrefs.value.aiCustomModels?.[provider] ?? []), model],
      },
    };
  }
  reportModel.value = model;
  persistReportAi();
  cancelAddReportModel();
}

function fillReportAi(): void {
  cancelAddReportModel();
  reportPrefs.value = dailyReportService.loadPreferences();
  const provider = reportPrefs.value.ai?.provider ?? "minimax";
  reportProvider.value = provider;
  reportModel.value = reportPrefs.value.ai?.model ?? AI_PROVIDERS[provider].models[0];
  reportApiKey.value = reportPrefs.value.aiApiKeys?.[provider] ?? "";
}

function persistReportAi(): void {
  const prefs = { ...reportPrefs.value };
  const model = reportModel.value.trim();
  if (!model) return;
  prefs.ai = { provider: reportProvider.value, model };
  prefs.aiSelectedModels = { ...prefs.aiSelectedModels, [reportProvider.value]: model };
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
  cancelAddReportModel();
  reportProvider.value = next;
  reportModel.value = reportPrefs.value.aiSelectedModels?.[next] ?? AI_PROVIDERS[next].models[0];
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
  { value: "deepseek", label: "DeepSeek", platform: "deepseek" },
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
  { key: "deepseek", name: "DeepSeek", desc: "DSH 客户端会话 Token（真实）+ 官方账户余额（需 API Key）", badge: "余额+Token", tone: "ok" },
] as const;

// ---- 设置页布局：顶部 Tabs + 分区卡片（全部默认展开，不可折叠） ----
type SettingsTab = "general" | "connectors" | "translate" | "clipboard" | "advanced";

const TABS: { key: SettingsTab; label: string }[] = [
  { key: "general", label: "通用" },
  { key: "connectors", label: "平台连接" },
  { key: "translate", label: "翻译" },
  { key: "clipboard", label: "剪贴板" },
  { key: "advanced", label: "高级" },
];

const activeTab = ref<SettingsTab>("general");

// 支持外部深链到指定 tab（例如文档拆解跳转到「高级」完成 AI 配置）。
const route = useRoute();
watch(
  () => route.query.tab,
  (tab) => {
    if (typeof tab === "string" && TABS.some((item) => item.key === tab)) {
      activeTab.value = tab as SettingsTab;
    }
  },
  { immediate: true },
);

function fillForm() {
  const cfg = loadSettings();
  arkBase.value = cfg.ark.baseUrl;
  deepseekApiKey.value = cfg.deepseek.apiKey;
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

const loggingIn = ref<"ark" | "qoder" | null>(null);
async function loginConnector(provider: "ark" | "qoder"): Promise<void> {
  if (loggingIn.value) return;
  loggingIn.value = provider;
  pushToast("正在打开浏览器，请完成账号授权。", "info");
  try {
    await invoke("connector_login", { provider });
    if (provider === "ark") {
      await checkArkStatus();
      if (!arkStatus.value.ok) throw new Error(arkStatus.value.message);
    } else {
      await httpGet({ baseUrl: "/api/qoder", path: "/usage" });
    }
    pushToast("登录已验证，请点击同步更新用量。", "success");
  } catch (reason) {
    pushToast(reason instanceof Error ? reason.message : String(reason), "error");
  } finally { loggingIn.value = null; }
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
      message: e instanceof Error ? e.message : String(e),
    };
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
      void loadZentaoStatus();
      void loadClipboardStatus();
      loadCacheStats();
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
    deepseek: { apiKey: deepseekApiKey.value },
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
              <span class="sec-icon tone-info"><Database :size="18" /></span>
              <span class="sec-copy">
                <strong>本地用量缓存</strong>
                <small>~/.xlt-workbench · 按月 JSONL · 支持定期与手动清理</small>
              </span>
            </div>
            <div class="sec-body">
              <div v-if="cacheStats" class="pref-grid">
                <div class="pref-field">
                  <span>数据目录</span>
                  <strong>{{ cacheStats.dataDir }}</strong>
                </div>
                <div class="pref-field">
                  <span>总记录</span>
                  <strong>{{ cacheStats.totalRows.toLocaleString() }}</strong>
                </div>
                <div class="pref-field">
                  <span>占用空间</span>
                  <strong>{{ formatBytes(cacheStats.totalBytes) }}</strong>
                </div>
                <div class="pref-field">
                  <span>时间范围</span>
                  <strong>{{ cacheStats.oldestHour?.slice(0, 10) || "—" }} ~ {{ cacheStats.newestHour?.slice(0, 10) || "—" }}</strong>
                </div>
              </div>
              <div class="flex flex-wrap gap-2 pt-2">
                <Button variant="ghost" size="sm" :disabled="cacheBusy" @click="cleanupCache">清理过期月份</Button>
                <Button variant="ghost" size="sm" :disabled="cacheBusy" @click="clearCache">清空全部缓存</Button>
                <Button variant="ghost" size="sm" :disabled="cacheBusy" @click="loadCacheStats">刷新</Button>
              </div>
              <p class="pref-note">清理按「保留天数」删除整月文件；清空会移除全部用量桶与额度快照，配置和账单保留。</p>
            </div>
          </section>

          <section class="sec">
            <div class="sec-head solo">
              <span class="sec-icon tone-info"><LayoutGrid :size="18" /></span>
              <span class="sec-copy">
                <strong>侧边栏菜单</strong>
                <small>勾选需要显示的入口，修改后自动保存并立即生效</small>
              </span>
            </div>
            <div class="sec-body">
              <div class="flex items-center justify-between gap-3 pb-2">
                <p class="pref-note">设置入口始终保留，方便随时调整。</p>
                <Button variant="ghost" size="sm" :disabled="!hiddenMenuPaths.length" @click="showAllMenus">显示全部</Button>
              </div>
              <div v-for="group in NAV_GROUPS" :key="group.title" class="border-t border-default py-3 last:pb-0">
                <h3 class="mb-3 text-xs font-semibold text-muted">{{ group.title }}</h3>
                <div class="quota-display-grid">
                  <label v-for="item in group.items" :key="item.to" class="quota-display-option">
                    <Checkbox :model-value="!hiddenMenuPaths.includes(item.to)" @update:model-value="setMenuVisible(item.to, $event === true)" />
                    <UIcon :name="item.icon" class="size-4 shrink-0 text-muted" />
                    <span>{{ item.label }}</span>
                  </label>
                </div>
              </div>
              <p class="pref-note mt-3">隐藏仅影响侧边栏显示，页面仍可通过快捷键或链接访问。</p>
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
                  <Button v-if="isTauriDesktop()" size="sm" :disabled="loggingIn !== null" @click="loginConnector('ark')">{{ loggingIn === 'ark' ? '等待浏览器授权…' : '登录方舟' }}</Button>
                  <Button variant="ghost" size="sm" :disabled="arkStatus.checking || loggingIn !== null" @click="checkArkStatus">重新检测</Button>
                </div>
              </div>
              <div v-if="!arkStatus.ok && !arkStatus.checking" class="connector-note">
                <p>
                  方舟用量通过本机 arkcli 的 SSO 登录查询。点击「登录方舟」并在浏览器授权，应用不保存账号密码。
                </p>
                <p>有效凭证由 CLI 自动续期；授权失效后需重新完成浏览器授权。</p>
                <p v-if="arkStatus.message" class="conn-msg">{{ arkStatus.message }}</p>
              </div>

              <div class="connector-note">
                <p>Token 统计范围：账号全量，包含当前账号下可见资源的调用用量，可能包含子用户调用。</p>
              </div>

              <!-- 禅道：登录态写入 ~/.zentao-token.json，待办同步只读（仅桌面端） -->
              <div v-if="zentaoAvailable" class="connector-row">
                <span class="conn-mark"><span class="conn-mark-icon">禅</span></span>
                <div class="conn-info">
                  <strong>禅道</strong>
                  <small>指派给我的任务 / Bug / 需求 → 待办 · 需登录（凭证仅存本机）</small>
                </div>
                <div class="conn-side">
                  <span class="conn-badge" :class="zentaoStatus.configured ? 'ok' : 'off'">
                    {{ zentaoStatus.configured ? `已登录 · ${zentaoStatus.account}` : "未登录" }}
                  </span>
                  <Button v-if="zentaoStatus.configured" variant="ghost" size="sm" @click="zentaoLogout">退出</Button>
                  <Button v-else variant="ghost" size="sm" @click="zentaoShowForm = !zentaoShowForm">
                    {{ zentaoShowForm ? "收起" : "登录" }}
                  </Button>
                </div>
              </div>
              <div v-if="zentaoAvailable && zentaoShowForm && !zentaoStatus.configured" class="connector-note">
                <p>登录后凭证（token）写入 <code>~/.zentao-token.json</code>，密码不保存；可在「待办」页手动同步，应用每 15 分钟自动同步一次。</p>
                <div class="pref-grid" style="margin-top: 10px;">
                  <label class="pref-field">
                    <span>服务器地址</span>
                    <Input v-model="zentaoForm.url" type="text" autocomplete="off" placeholder="https://zentao.example.com" />
                  </label>
                  <label class="pref-field">
                    <span>账号</span>
                    <Input v-model="zentaoForm.account" type="text" autocomplete="off" placeholder="禅道登录账号" />
                  </label>
                  <label class="pref-field">
                    <span>密码</span>
                    <Input v-model="zentaoForm.password" type="password" autocomplete="off" placeholder="仅用于本次登录" @keyup.enter="zentaoLogin" />
                  </label>
                </div>
                <div class="clip-actions">
                  <Button type="button" :disabled="zentaoLoggingIn || !zentaoForm.url || !zentaoForm.account || !zentaoForm.password" @click="zentaoLogin">
                    {{ zentaoLoggingIn ? "登录中…" : "登录禅道" }}
                  </Button>
                </div>
              </div>

              <!-- 其他本机 CLI：自动采集，无需登录配置 -->
              <div v-for="c in CONNECTORS" :key="c.key" class="connector-row">
                <span class="conn-mark"><ToolLogo :platform="c.key" :size="22" /></span>
                <div class="conn-info">
                  <strong>{{ c.name }}</strong>
                  <small>{{ c.desc }}</small>
                </div>
                <div class="conn-side">
                  <Button v-if="c.key === 'qoder' && isTauriDesktop()" size="sm" :disabled="loggingIn !== null" @click="loginConnector('qoder')">{{ loggingIn === 'qoder' ? '等待浏览器授权…' : '登录 Qoder' }}</Button>
                  <span class="conn-badge" :class="c.tone">{{ c.badge }}</span>
                </div>
              </div>

              <!-- DeepSeek 账户余额需 API Key，仅本机保存；不填则只采集 DSH 会话 Token -->
              <div class="connector-note">
                <p><b>DeepSeek 账户余额</b>（费用中心）经官方 <code>/user/balance</code> 查询，需 API Key，仅保存在本机。</p>
                <label class="pref-field deepseek-key-field">
                  <span>DeepSeek API KEY（可选）</span>
                  <Input v-model="deepseekApiKey" type="password" autocomplete="off" placeholder="sk-… 仅本机保存" />
                </label>
              </div>
            </div>
          </section>
        </template>

        <!-- ===================== 翻译 ===================== -->
        <template v-else-if="activeTab === 'translate'">
          <TranslateSettings />
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
                <strong>AI 配置</strong>
                <small>Git 报告与文档拆解共用的 Provider、模型与 API Key（仅保存在本机）</small>
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
                <div class="pref-field">
                  <div class="flex items-center justify-between gap-2">
                    <span>模型</span>
                    <Button type="button" size="sm" variant="ghost" @click="addingReportModel = true">新增模型</Button>
                  </div>
                  <Select v-model="reportModel" :options="reportModelOptions" placeholder="选择模型"
                    @update:model-value="persistReportAi" />
                </div>
                <label class="pref-field">
                  <span>API Key</span>
                  <Input v-model="reportApiKey" type="password" autocomplete="off" placeholder="本机保存"
                    @change="persistReportAi" />
                </label>
              </div>
              <div v-if="addingReportModel" class="mt-3 space-y-2 rounded-lg border border-default p-3">
                <label class="pref-field">
                  <span>新增 {{ AI_PROVIDERS[reportProvider].label }} 模型</span>
                  <Input v-model="customReportModel" aria-label="自定义模型名称" autocomplete="off" placeholder="输入服务商提供的完整 model name"
                    @keydown.enter.prevent="addReportModel" @keydown.esc="cancelAddReportModel" />
                </label>
                <div class="flex justify-end gap-2">
                  <Button type="button" size="sm" variant="ghost" @click="cancelAddReportModel">取消</Button>
                  <Button type="button" size="sm" :disabled="!customReportModel.trim()" @click="addReportModel">添加并选用</Button>
                </div>
              </div>
              <p class="pref-note">模型按服务商分别保存；新增后自动选用。修改后立即保存，Git 报告与文档拆解共用此配置。</p>
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
.conn-mark-icon {
  display: grid;
  width: 22px;
  height: 22px;
  place-items: center;
  border-radius: 6px;
  background: color-mix(in srgb, #4f8cff 16%, transparent);
  color: #4f8cff;
  font-size: 12px;
  font-weight: 700;
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
.deepseek-key-field { flex: 1 1 100%; min-width: min(240px, 100%); margin-top: 10px; }
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
