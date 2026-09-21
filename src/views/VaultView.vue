<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import {
  Copy,
  ExternalLink,
  Eye,
  EyeOff,
  FileDown,
  FileUp,
  KeyRound,
  LockKeyhole,
  Pencil,
  Pin,
  Plus,
  Search,
  Settings2,
  ShieldCheck,
  Trash2,
  X,
} from "lucide-vue-next";
import { DialogDescription, DialogTitle } from "reka-ui";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Dialog } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Textarea } from "@/components/ui/textarea";
import { pushToast } from "@/composables/useToast";
import { vaultService } from "@/services/vault-service";
import type { VaultRecord, VaultRecordDraft, VaultRecordType, VaultSort } from "@/types/vault";

type VaultFilter = "all" | VaultRecordType;

const TYPE_OPTIONS: { value: VaultRecordType; label: string }[] = [
  { value: "credential", label: "账号密码" },
  { value: "ai_key", label: "AI Key" },
  { value: "other_key", label: "其他 Key" },
];
const AI_PROVIDER_OPTIONS = [
  "OpenAI",
  "Anthropic (Claude)",
  "Google (Gemini)",
  "Azure OpenAI",
  "DeepSeek",
  "Moonshot (Kimi)",
  "智谱 GLM",
  "阿里云通义千问",
  "百度文心一言",
  "讯飞星火",
  "火山引擎豆包",
  "Mistral",
  "Cohere",
  "Groq",
  "OpenRouter",
].map((label) => ({ value: label, label }));
AI_PROVIDER_OPTIONS.push({ value: "__custom__", label: "自定义" });
const SORT_OPTIONS = [
  { value: "recent", label: "最近使用" },
  { value: "name", label: "名称" },
  { value: "created", label: "创建时间" },
];
const AUTO_LOCK_OPTIONS = [1, 5, 15, 30, 60].map((minutes) => ({
  value: String(minutes),
  label: `${minutes} 分钟`,
}));

const initializing = ref(true);
const initialized = ref(false);
const unlocked = ref(vaultService.isUnlocked());
const authBusy = ref(false);
const authError = ref("");
const password = ref("");
const passwordConfirm = ref("");
const records = ref<VaultRecord[]>([]);
const query = ref("");
const activeFilter = ref<VaultFilter>("all");
const activeTag = ref("");
const sort = ref<VaultSort>("recent");
const recordDialogOpen = ref(false);
const settingsOpen = ref(false);
const deleteTarget = ref<VaultRecord | null>(null);
const saving = ref(false);
const tagText = ref("");
const providerChoice = ref(AI_PROVIDER_OPTIONS[0].value);
const customProvider = ref("");
const revealed = ref<Record<string, string>>({});
const revealTimers = new Map<string, number>();
const autoLockMinutes = ref(String(vaultService.getAutoLockMinutes()));
const currentPassword = ref("");
const newPassword = ref("");
const newPasswordConfirm = ref("");
const passwordBusy = ref(false);
const importFile = ref<File | null>(null);
const importPassword = ref("");
const importInput = ref<HTMLInputElement | null>(null);
const importBusy = ref(false);
let clipboardTimer: number | null = null;
let clipboardSequence = 0;
let unsubscribe: (() => void) | undefined;

function emptyForm(type: VaultRecordType = "credential"): VaultRecordDraft {
  return {
    type,
    name: "",
    account: "",
    provider: "",
    secret: "",
    loginUrl: "",
    baseUrl: "",
    note: "",
    tags: [],
    favorite: false,
  };
}

const form = ref<VaultRecordDraft>(emptyForm());
const tags = computed(() => [...new Set(records.value.flatMap((record) => record.tags))].sort((a, b) => a.localeCompare(b, "zh-CN")));
const typeCounts = computed<Record<VaultRecordType, number>>(() => ({
  credential: records.value.filter((record) => record.type === "credential").length,
  ai_key: records.value.filter((record) => record.type === "ai_key").length,
  other_key: records.value.filter((record) => record.type === "other_key").length,
}));
const filteredRecords = computed(() => {
  const keyword = query.value.trim().toLocaleLowerCase();
  const result = records.value.filter((record) => {
    if (activeFilter.value !== "all" && record.type !== activeFilter.value) return false;
    if (activeTag.value && !record.tags.includes(activeTag.value)) return false;
    if (!keyword) return true;
    return [record.name, record.account, record.provider, record.note, ...record.tags]
      .join(" ")
      .toLocaleLowerCase()
      .includes(keyword);
  });
  return result.sort((left, right) => {
    if (left.favorite !== right.favorite) return left.favorite ? -1 : 1;
    if (sort.value === "name") return left.name.localeCompare(right.name, "zh-CN");
    if (sort.value === "created") return right.createdAt.localeCompare(left.createdAt);
    return (right.lastUsedAt ?? right.updatedAt).localeCompare(left.lastUsedAt ?? left.updatedAt);
  });
});

function loadRecords(): void {
  records.value = vaultService.list();
}

function clearRevealed(): void {
  revealed.value = {};
  for (const timer of revealTimers.values()) window.clearTimeout(timer);
  revealTimers.clear();
}

function clearSensitiveUi(): void {
  clearRevealed();
  form.value = emptyForm();
  recordDialogOpen.value = false;
  settingsOpen.value = false;
  deleteTarget.value = null;
  password.value = "";
  passwordConfirm.value = "";
  currentPassword.value = "";
  newPassword.value = "";
  newPasswordConfirm.value = "";
  importPassword.value = "";
}

async function submitAuth(): Promise<void> {
  authError.value = "";
  const wasInitialized = initialized.value;
  if (!initialized.value && password.value !== passwordConfirm.value) {
    authError.value = "两次输入的主密码不一致";
    return;
  }
  authBusy.value = true;
  try {
    if (initialized.value) await vaultService.unlock(password.value);
    else {
      await vaultService.setup(password.value);
      initialized.value = true;
    }
    unlocked.value = true;
    password.value = "";
    passwordConfirm.value = "";
    loadRecords();
    pushToast(wasInitialized ? "密钥库已解锁。" : "密钥库已创建。", "success");
  } catch (reason) {
    authError.value = reason instanceof Error ? reason.message : String(reason);
  } finally {
    authBusy.value = false;
  }
}

function lockVault(): void {
  clearSensitiveUi();
  vaultService.lock();
  unlocked.value = false;
  records.value = [];
}

function setFilter(filter: VaultFilter): void {
  activeFilter.value = filter;
}

function typeLabel(type: VaultRecordType): string {
  return TYPE_OPTIONS.find((item) => item.value === type)?.label ?? "其他 Key";
}

function safeUrl(value: string): string | null {
  if (!value) return null;
  try {
    const url = new URL(value);
    return ["http:", "https:"].includes(url.protocol) ? url.href : null;
  } catch {
    return null;
  }
}

function secondaryValue(record: VaultRecord): string {
  return record.type === "credential" ? record.account : record.provider;
}

function openCreate(): void {
  const type = activeFilter.value === "credential" || activeFilter.value === "ai_key" || activeFilter.value === "other_key"
    ? activeFilter.value
    : "credential";
  form.value = emptyForm(type);
  providerChoice.value = AI_PROVIDER_OPTIONS[0].value;
  customProvider.value = "";
  tagText.value = activeTag.value;
  recordDialogOpen.value = true;
}

function openEdit(record: VaultRecord): void {
  form.value = vaultService.get(record.id);
  tagText.value = record.tags.join(", ");
  if (record.type === "ai_key" && AI_PROVIDER_OPTIONS.some((option) => option.value === record.provider)) {
    providerChoice.value = record.provider;
    customProvider.value = "";
  } else {
    providerChoice.value = "__custom__";
    customProvider.value = record.provider;
  }
  recordDialogOpen.value = true;
}

function updateFormType(value: string | number): void {
  form.value.type = value as VaultRecordType;
  if (form.value.type === "ai_key" && providerChoice.value !== "__custom__") form.value.provider = providerChoice.value;
}

async function saveRecord(): Promise<void> {
  saving.value = true;
  try {
    form.value.tags = tagText.value.split(/[,，]/);
    if (form.value.type === "ai_key") {
      form.value.provider = providerChoice.value === "__custom__" ? customProvider.value : providerChoice.value;
    }
    await vaultService.save(form.value);
    loadRecords();
    recordDialogOpen.value = false;
    pushToast("已加密保存到本机密钥库。", "success");
  } catch (reason) {
    pushToast(reason instanceof Error ? reason.message : String(reason), "error");
  } finally {
    saving.value = false;
  }
}

function hideSecret(id: string): void {
  const next = { ...revealed.value };
  delete next[id];
  revealed.value = next;
  const timer = revealTimers.get(id);
  if (timer !== undefined) window.clearTimeout(timer);
  revealTimers.delete(id);
}

function toggleSecret(record: VaultRecord): void {
  if (revealed.value[record.id]) {
    hideSecret(record.id);
    return;
  }
  revealed.value = { ...revealed.value, [record.id]: vaultService.get(record.id).secret };
  const currentTimer = revealTimers.get(record.id);
  if (currentTimer !== undefined) window.clearTimeout(currentTimer);
  revealTimers.set(record.id, window.setTimeout(() => hideSecret(record.id), 15_000));
  void vaultService.touch(record.id).then(loadRecords);
}

async function copySecret(record: VaultRecord): Promise<void> {
  try {
    const secret = vaultService.get(record.id).secret;
    await navigator.clipboard.writeText(secret);
    await vaultService.touch(record.id);
    loadRecords();
    const sequence = ++clipboardSequence;
    if (clipboardTimer !== null) window.clearTimeout(clipboardTimer);
    clipboardTimer = window.setTimeout(async () => {
      if (sequence !== clipboardSequence) return;
      try {
        if (await navigator.clipboard.readText() === secret) await navigator.clipboard.writeText("");
      } catch {
        // Clipboard read permission can be unavailable after the user leaves the app.
      }
    }, 30_000);
    pushToast("已复制，剪贴板将在 30 秒后清空。", "success");
  } catch (reason) {
    pushToast(reason instanceof Error ? reason.message : String(reason), "error");
  }
}

async function deleteRecord(): Promise<void> {
  if (!deleteTarget.value) return;
  try {
    await vaultService.remove(deleteTarget.value.id);
    hideSecret(deleteTarget.value.id);
    deleteTarget.value = null;
    loadRecords();
    pushToast("记录已删除。", "success");
  } catch (reason) {
    pushToast(reason instanceof Error ? reason.message : String(reason), "error");
  }
}

function updateAutoLock(value: string): void {
  autoLockMinutes.value = value;
  vaultService.setAutoLockMinutes(Number(value));
  pushToast(`闲置 ${value} 分钟后将自动锁定。`, "success");
}

async function changePassword(): Promise<void> {
  if (newPassword.value !== newPasswordConfirm.value) {
    pushToast("两次输入的新主密码不一致。", "error");
    return;
  }
  passwordBusy.value = true;
  try {
    await vaultService.changePassword(currentPassword.value, newPassword.value);
    currentPassword.value = "";
    newPassword.value = "";
    newPasswordConfirm.value = "";
    pushToast("主密码已修改，现有记录已重新加密。", "success");
  } catch (reason) {
    pushToast(reason instanceof Error ? reason.message : String(reason), "error");
  } finally {
    passwordBusy.value = false;
  }
}

async function exportBackup(): Promise<void> {
  try {
    const raw = await vaultService.exportBackup();
    const blob = new Blob([raw], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = `xlt-vault-${new Date().toISOString().slice(0, 10)}.json`;
    link.click();
    URL.revokeObjectURL(url);
    pushToast("加密备份已导出。", "success");
  } catch (reason) {
    pushToast(reason instanceof Error ? reason.message : String(reason), "error");
  }
}

function chooseImportFile(): void {
  importInput.value?.click();
}

function selectImportFile(event: Event): void {
  importFile.value = (event.target as HTMLInputElement).files?.[0] ?? null;
}

async function importBackup(): Promise<void> {
  if (!importFile.value) {
    pushToast("请先选择加密备份文件。", "error");
    return;
  }
  importBusy.value = true;
  try {
    const count = await vaultService.importBackup(await importFile.value.text(), importPassword.value);
    loadRecords();
    importFile.value = null;
    importPassword.value = "";
    pushToast(`已导入 ${count} 条记录。`, "success");
  } catch (reason) {
    pushToast(reason instanceof Error ? reason.message : String(reason), "error");
  } finally {
    importBusy.value = false;
  }
}

onMounted(async () => {
  unsubscribe = vaultService.subscribe((active) => {
    unlocked.value = active;
    if (!active) {
      records.value = [];
      clearSensitiveUi();
    }
  });
  try {
    initialized.value = await vaultService.isInitialized();
    if (unlocked.value) loadRecords();
  } catch (reason) {
    authError.value = reason instanceof Error ? reason.message : String(reason);
  } finally {
    initializing.value = false;
  }
});

onUnmounted(() => {
  unsubscribe?.();
  clearSensitiveUi();
});
</script>

<template>
  <section v-if="initializing" class="vault-auth-shell">
    <div class="vault-auth-card">正在读取本机密钥库…</div>
  </section>

  <section v-else-if="!unlocked" class="vault-lock" aria-label="解锁密钥库">
    <form class="vault-lock-card" @submit.prevent="submitAuth">
      <span class="lock-circle">
        <LockKeyhole :size="32" />
      </span>
      <h3>{{ initialized ? "解锁密钥库" : "设置主密码" }}</h3>
      <p class="vl-sub">{{ initialized ? "输入主密码以访问本机加密记录" : "主密码不会保存，遗忘后无法恢复密钥库" }}</p>
      <div class="vl-form" :class="{ inline: initialized }">
        <label class="sr-label"><span class="sr-only">主密码</span><Input v-model="password" type="password" autofocus
            autocomplete="current-password" placeholder="主密码" /></label>
        <label v-if="!initialized" class="sr-label"><span class="sr-only">确认主密码</span><Input v-model="passwordConfirm"
            type="password" autocomplete="new-password" placeholder="确认主密码" /></label>
        <Button type="submit" :disabled="authBusy || !password">{{ authBusy ? "处理中…" : initialized ? "解锁" : "创建并解锁"
          }}</Button>
      </div>
      <p v-if="authError" class="vault-auth-error">{{ authError }}</p>
      <div class="vl-tip">
        <ShieldCheck :size="13" />
        AES-256-GCM · PBKDF2 · 数据仅保存在本机
      </div>
    </form>
  </section>

  <section v-else class="vault-layout" aria-label="密钥库">
    <div class="vault-toolbar">
      <div class="type-seg" role="group" aria-label="记录类型">
        <button :class="{ active: activeFilter === 'all' }" type="button" :aria-pressed="activeFilter === 'all'"
          @click="setFilter('all')">全部 {{ records.length }}</button>
        <button v-for="option in TYPE_OPTIONS" :key="option.value" :class="{ active: activeFilter === option.value }"
          type="button" :aria-pressed="activeFilter === option.value" @click="setFilter(option.value)">{{ option.label }}
          {{ typeCounts[option.value] }}</button>
      </div>
      <label class="vault-search">
        <Search :size="15" /><Input v-model="query" placeholder="搜索名称或账号…" />
        <Button v-if="query" type="button" variant="ghost" size="icon" aria-label="清除搜索" @click="query = ''">
          <X :size="15" />
        </Button>
      </label>
      <div class="vault-toolbar-actions">
        <Select :model-value="sort" :options="SORT_OPTIONS" @update:model-value="sort = $event as VaultSort" />
        <Button type="button" variant="ghost" size="icon" title="密钥库设置" aria-label="密钥库设置"
          @click="settingsOpen = true">
          <Settings2 :size="16" />
        </Button>
        <Button type="button" size="sm" @click="openCreate">
          <Plus :size="15" />新增记录
        </Button>
      </div>
    </div>

    <div v-if="tags.length" class="vault-tags" aria-label="记录标签">
      <button :class="{ active: !activeTag }" type="button" :aria-pressed="!activeTag" @click="activeTag = ''">所有标签</button>
      <button v-for="tag in tags" :key="tag" :class="{ active: activeTag === tag }" type="button"
        :aria-pressed="activeTag === tag" @click="activeTag = tag">{{ tag }}</button>
    </div>

    <div class="vault-content">
      <div v-if="!filteredRecords.length" class="vault-empty">
        <KeyRound :size="38" /><strong>{{ records.length ? "没有匹配的记录" : "还没有保存任何密钥" }}</strong><span>{{ records.length ?
          "调整类型、标签或搜索词后再试" : "新增后将以密文形式保存在本机。" }}</span><Button v-if="!records.length" type="button" variant="outline"
          @click="openCreate">
          <Plus :size="15" />添加第一条记录
        </Button>
      </div>
      <div v-else class="vault-table">
        <div class="vault-row head" aria-hidden="true">
          <span></span>
          <span>名称</span>
          <span>账号 / 服务商</span>
          <span>密码 / Key</span>
          <span class="cell-actions-label">操作</span>
        </div>
        <article v-for="record in filteredRecords" :key="record.id" class="vault-row" :title="record.note || undefined">
          <span class="vault-avatar" :data-type="record.type">{{ (record.provider || record.name).slice(0, 1) }}</span>
          <div class="vr-name">
            <strong>{{ record.name }}<Pin v-if="record.favorite" class="vr-fav" :size="12" /></strong>
            <span class="vr-badges">
              <Badge>{{ typeLabel(record.type) }}</Badge>
              <Badge v-for="tag in record.tags.slice(0, 3)" :key="tag">{{ tag }}</Badge>
            </span>
          </div>
          <div class="vr-account">
            <span class="vr-account-value">{{ secondaryValue(record) || "—" }}</span>
            <a v-if="safeUrl(record.loginUrl || record.baseUrl)" class="vr-link"
              :href="safeUrl(record.loginUrl || record.baseUrl) ?? undefined" target="_blank" rel="noreferrer">
              <ExternalLink :size="11" />打开地址
            </a>
          </div>
          <div class="vr-secret">
            <code>{{ revealed[record.id] ?? "••••••••••••" }}</code>
            <Button type="button" variant="ghost" size="icon" :aria-label="revealed[record.id] ? '隐藏' : '显示'"
              :title="revealed[record.id] ? '隐藏（15 秒后自动隐藏）' : '显示 15 秒'" @click="toggleSecret(record)">
              <EyeOff v-if="revealed[record.id]" :size="14" />
              <Eye v-else :size="14" />
            </Button>
            <Button type="button" variant="ghost" size="icon" aria-label="复制" title="复制，30 秒后清空剪贴板"
              @click="copySecret(record)">
              <Copy :size="14" />
            </Button>
          </div>
          <div class="vr-actions">
            <Button type="button" variant="ghost" size="icon" title="编辑" aria-label="编辑" @click="openEdit(record)">
              <Pencil :size="14" />
            </Button>
            <Button class="delete-button" type="button" variant="ghost" size="icon" title="删除" aria-label="删除"
              @click="deleteTarget = record">
              <Trash2 :size="14" />
            </Button>
          </div>
        </article>
      </div>
    </div>
  </section>

  <Dialog :open="recordDialogOpen" content-class="vault-dialog" @update:open="recordDialogOpen = $event">
    <form @submit.prevent="saveRecord">
      <header class="vault-dialog-head">
        <div>
          <DialogTitle>{{ form.id ? "编辑记录" : "新增记录" }}</DialogTitle>
          <DialogDescription>敏感内容会在保存前于本机加密。</DialogDescription>
        </div><Button type="button" variant="ghost" size="icon" aria-label="关闭" @click="recordDialogOpen = false">
          <X :size="16" />
        </Button>
      </header>
      <div class="vault-dialog-body">
        <Tabs :model-value="form.type" @update:model-value="updateFormType">
          <TabsList class="vault-type-tabs">
            <TabsTrigger v-for="option in TYPE_OPTIONS" :key="option.value" :value="option.value">{{ option.label }}
            </TabsTrigger>
          </TabsList>
        </Tabs>
        <div class="vault-fields">
          <template v-if="form.type === 'credential'">
            <label><span>名称 / 备注 *</span><Input v-model="form.name" autofocus placeholder="例如：公司邮箱" /></label>
            <label><span>账号</span><Input v-model="form.account" placeholder="用户名 / 邮箱 / 手机号" /></label>
            <label class="wide"><span>密码 *</span><Input v-model="form.secret" type="password"
                autocomplete="new-password" placeholder="输入密码" /></label>
            <label class="wide"><span>登录地址</span><Input v-model="form.loginUrl" type="url"
                placeholder="https://" /></label>
          </template>
          <template v-else-if="form.type === 'ai_key'">
            <label><span>服务商 *</span><Select v-model="providerChoice" :options="AI_PROVIDER_OPTIONS" /></label>
            <label><span>Key 名称 *</span><Input v-model="form.name" autofocus placeholder="例如：生产环境 Key" /></label>
            <label v-if="providerChoice === '__custom__'" class="wide"><span>自定义服务商 *</span><Input
                v-model="customProvider" placeholder="服务商名称" /></label>
            <label class="wide"><span>Key 值 *</span><Input v-model="form.secret" type="password"
                autocomplete="new-password" placeholder="sk-..." /></label>
            <label class="wide"><span>Base URL</span><Input v-model="form.baseUrl" type="url"
                placeholder="https://api.example.com" /></label>
          </template>
          <template v-else>
            <label><span>服务商 *</span><Input v-model="form.provider" autofocus placeholder="例如：阿里云 OSS" /></label>
            <label><span>Key 名称 *</span><Input v-model="form.name" placeholder="例如：AccessKey" /></label>
            <label class="wide"><span>Key 值 *</span><Input v-model="form.secret" type="password"
                autocomplete="new-password" placeholder="输入 Key 值" /></label>
          </template>
          <label class="wide"><span>标签</span><Input v-model="tagText" placeholder="工作, 生产环境（逗号分隔）" /></label>
          <label class="wide"><span>用途备注</span><Textarea v-model="form.note" placeholder="记录用途、所属项目等" /></label>
          <label class="favorite-field">
            <Checkbox v-model="form.favorite" /><span>收藏并置顶</span>
          </label>
        </div>
      </div>
      <footer class="vault-dialog-footer"><Button variant="outline" type="button"
          @click="recordDialogOpen = false">取消</Button><Button type="submit" :disabled="saving">{{ saving ? "保存中…" :
          "保存"
          }}</Button></footer>
    </form>
  </Dialog>

  <Dialog :open="Boolean(deleteTarget)" content-class="vault-confirm"
    @update:open="(open) => { if (!open) deleteTarget = null; }">
    <div class="vault-dialog-head">
      <div>
        <DialogTitle>删除这条记录？</DialogTitle>
        <DialogDescription>“{{ deleteTarget?.name }}”删除后无法恢复。</DialogDescription>
      </div>
    </div>
    <footer class="vault-dialog-footer"><Button type="button" variant="outline"
        @click="deleteTarget = null">取消</Button><Button type="button" variant="destructive"
        @click="deleteRecord">删除</Button></footer>
  </Dialog>

  <Dialog :open="settingsOpen" content-class="vault-settings" @update:open="settingsOpen = $event">
    <div class="vault-dialog-head">
      <div>
        <DialogTitle>密钥库设置</DialogTitle>
        <DialogDescription>管理锁定策略、主密码与加密备份。</DialogDescription>
      </div><Button type="button" variant="ghost" size="icon" aria-label="关闭" @click="settingsOpen = false">
        <X :size="16" />
      </Button>
    </div>
    <div class="vault-settings-body">
      <section>
        <div>
          <h3>自动锁定</h3>
          <p>闲置达到设定时长后清除内存中的解密 key。</p>
        </div><Select :model-value="autoLockMinutes" :options="AUTO_LOCK_OPTIONS"
          @update:model-value="updateAutoLock" />
      </section>
      <section class="settings-password">
        <div>
          <h3>修改主密码</h3>
          <p>修改后会用新密码重新加密所有记录。</p>
        </div>
        <div class="settings-fields"><Input v-model="currentPassword" type="password" autocomplete="current-password"
            placeholder="当前主密码" /><Input v-model="newPassword" type="password" autocomplete="new-password"
            placeholder="新主密码（至少 8 个字符）" /><Input v-model="newPasswordConfirm" type="password"
            autocomplete="new-password" placeholder="确认新主密码" /><Button type="button" variant="outline"
            :disabled="passwordBusy" @click="changePassword">{{ passwordBusy ? "处理中…" : "修改主密码" }}</Button></div>
      </section>
      <section>
        <div>
          <h3>导出备份</h3>
          <p>导出文件保持加密，恢复时需要主密码。</p>
        </div><Button type="button" variant="outline" @click="exportBackup">
          <FileDown :size="15" />导出加密 JSON
        </Button>
      </section>
      <section class="settings-import">
        <div>
          <h3>导入备份</h3>
          <p>校验备份主密码后，将覆盖当前密钥库记录。</p>
        </div>
        <div class="settings-fields"><input ref="importInput" class="sr-only" type="file"
            accept="application/json,.json" @change="selectImportFile"><Button type="button" variant="outline"
              @click="chooseImportFile">
              <FileUp :size="15" />{{ importFile?.name ?? "选择备份文件" }}
            </Button><Input v-model="importPassword" type="password" autocomplete="off" placeholder="备份文件的主密码" /><Button
              type="button" variant="destructive" :disabled="importBusy || !importFile || !importPassword"
              @click="importBackup">{{ importBusy ? "导入中…" : "验证并覆盖导入" }}</Button></div>
      </section>
    </div>
    <footer class="vault-dialog-footer"><Button type="button" variant="outline"
        @click="lockVault(); settingsOpen = false">
        <LockKeyhole :size="15" />立即锁定
      </Button><Button type="button" @click="settingsOpen = false">完成</Button></footer>
  </Dialog>
</template>

<style scoped>
/* ---------- 解锁视图（原型 vault-lock） ---------- */
.vault-lock {
  display: grid;
  min-height: min(570px, calc(100vh - 150px));
  place-items: center;
}

.vault-lock-card {
  display: flex;
  width: min(420px, 100%);
  flex-direction: column;
  align-items: center;
  padding: 60px 0 40px;
  text-align: center;
}

.lock-circle {
  display: grid;
  width: 76px;
  height: 76px;
  place-items: center;
  margin-bottom: 20px;
  border-radius: 24px;
  background: var(--accent-weak);
  color: var(--accent);
}

.vault-lock-card h3 {
  margin: 0;
  color: var(--text);
  font-size: 18px;
  font-weight: 700;
}

.vl-sub {
  margin: 7px 0 22px;
  color: var(--text-muted);
  font-size: 12.5px;
}

.vl-form {
  display: grid;
  gap: 8px;
  width: min(320px, 100%);
}

.vl-form.inline {
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
}

.sr-label {
  display: contents;
}

.vl-form :deep(.cn-input) {
  height: 34px;
}

.vl-form :deep(.cn-button) {
  height: 34px;
}

.vault-auth-error {
  margin: 12px 0 0;
  color: var(--u-crit);
  font-size: 12px;
}

.vl-tip {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 22px;
  color: var(--text-subtle);
  font-size: 11.5px;
}

/* ---------- 解锁后布局 ---------- */
.vault-layout {
  display: flex;
  min-height: min(600px, calc(100vh - 148px));
  flex-direction: column;
  gap: 12px;
}

.vault-toolbar {
  display: flex;
  min-width: 0;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
}

.type-seg {
  display: inline-flex;
  flex: 0 0 auto;
  gap: 2px;
  max-width: 100%;
  padding: 3px;
  overflow-x: auto;
  border: 1px solid var(--border);
  border-radius: var(--r-sm);
  background: var(--segment-bg);
}

.type-seg button {
  display: inline-flex;
  align-items: center;
  padding: 5px 11px;
  border: 0;
  border-radius: 5px;
  background: transparent;
  color: var(--text-muted);
  font: inherit;
  font-size: 12px;
  font-weight: 600;
  white-space: nowrap;
  cursor: pointer;
}

.type-seg button:hover {
  color: var(--text);
}

.type-seg button.active {
  background: var(--surface);
  box-shadow: var(--shadow-sm);
  color: var(--accent);
}

.vault-search {
  display: flex;
  min-width: 180px;
  max-width: 300px;
  height: 32px;
  flex: 1;
  align-items: center;
  gap: 6px;
  padding: 0 8px;
  border: 1px solid var(--border);
  border-radius: var(--r-sm);
  background: var(--surface);
  color: var(--text-subtle);
}

.vault-search:focus-within {
  border-color: var(--accent);
  box-shadow: 0 0 0 3px var(--accent-weak);
}

.vault-search :deep(.cn-input) {
  min-width: 0;
  height: 30px;
  padding: 0;
  border: 0;
  background: transparent;
  box-shadow: none;
}

.vault-search :deep(.cn-button) {
  width: 24px;
  height: 24px;
}

.vault-toolbar-actions {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  gap: 7px;
  margin-left: auto;
}

.vault-toolbar-actions :deep(.cn-select-trigger) {
  width: 108px;
  height: 32px;
}

/* 标签 chips */
.vault-tags {
  display: flex;
  min-height: 26px;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
}

.vault-tags button {
  display: inline-flex;
  align-items: center;
  padding: 4px 11px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--surface);
  color: var(--text-muted);
  font: inherit;
  font-size: 11.5px;
  font-weight: 600;
  white-space: nowrap;
  cursor: pointer;
}

.vault-tags button:hover {
  border-color: var(--border-strong);
  color: var(--text);
}

.vault-tags button.active {
  border-color: var(--accent);
  background: var(--accent);
  color: var(--accent-contrast);
  font-weight: 650;
}

/* ---------- 记录表（原型 vault-row） ---------- */
.vault-content {
  min-width: 0;
  flex: 1;
}

.vault-table {
  overflow: hidden;
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  background: var(--surface);
  box-shadow: var(--shadow-card);
}

.vault-row {
  display: grid;
  grid-template-columns: 34px 1.4fr 1.2fr 1.4fr auto;
  gap: 12px;
  align-items: center;
  padding: 11px 16px;
  border-bottom: 1px solid var(--border);
  font-size: 12.5px;
}

.vault-row:last-child {
  border-bottom: 0;
}

.vault-row:not(.head):hover {
  background: var(--surface-2);
}

.vault-row.head {
  padding: 9px 16px;
  background: var(--surface-2);
  color: var(--text-muted);
  font-size: 11px;
  font-weight: 600;
}

.vault-avatar {
  display: grid;
  width: 30px;
  height: 30px;
  place-items: center;
  border-radius: 9px;
  color: var(--accent-contrast);
  font-size: 12px;
  font-weight: 700;
}

.vault-avatar[data-type="credential"] {
  background: var(--brand-open);
}

.vault-avatar[data-type="ai_key"] {
  background: var(--brand-ark);
}

.vault-avatar[data-type="other_key"] {
  background: var(--brand-codex);
}

.vr-name {
  display: grid;
  min-width: 0;
  gap: 4px;
}

.vr-name strong {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 5px;
  overflow: hidden;
  color: var(--text);
  font-weight: 650;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.vr-fav {
  flex: none;
  color: var(--u-warn);
}

.vr-badges {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.vr-account {
  display: grid;
  min-width: 0;
  gap: 3px;
}

.vr-account-value {
  overflow: hidden;
  color: var(--text-muted);
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.vr-link {
  display: inline-flex;
  width: fit-content;
  align-items: center;
  gap: 4px;
  color: var(--accent);
  font-size: 10.5px;
  text-decoration: none;
}

.vr-link:hover {
  text-decoration: underline;
  text-underline-offset: 3px;
}

.vr-secret {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 4px;
}

.vr-secret code {
  min-width: 0;
  flex: 1;
  overflow: hidden;
  color: var(--text-muted);
  font-family: var(--font-mono);
  font-size: 12px;
  letter-spacing: 1px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.vr-secret :deep(.cn-button),
.vr-actions :deep(.cn-button) {
  width: 26px;
  height: 26px;
  flex: none;
}

.vr-secret :deep(.cn-button):hover {
  color: var(--accent);
}

.vr-actions {
  display: flex;
  justify-content: flex-end;
  gap: 2px;
}

.vr-actions :deep(.delete-button:hover) {
  color: var(--u-crit);
}

.vault-empty {
  display: flex;
  min-height: 300px;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  border: 1px dashed var(--border-strong);
  border-radius: var(--r-lg);
  color: var(--text-subtle);
  font-size: 12px;
}

.vault-empty strong {
  color: var(--text);
  font-size: 14px;
}

.vault-empty :deep(.cn-button) {
  margin-top: 8px;
}

/* ---------- 对话框（保持原功能样式） ---------- */
.vault-dialog {
  width: min(590px, calc(100vw - 32px));
}

.vault-confirm {
  width: min(420px, calc(100vw - 32px));
}

.vault-settings {
  width: min(640px, calc(100vw - 32px));
}

.vault-dialog-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  padding: 22px 22px 0;
}

.vault-dialog-head :deep(h2) {
  margin: 0;
  color: var(--text);
  font-size: 17px;
}

.vault-dialog-head :deep(p) {
  margin: 4px 0 0;
  color: var(--text-muted);
  font-size: 11px;
}

.vault-dialog-body {
  padding: 18px 22px;
}

.vault-type-tabs {
  width: 100%;
}

.vault-type-tabs :deep(.cn-tabs-trigger) {
  flex: 1;
}

.vault-fields {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 13px;
  margin-top: 17px;
}

.vault-fields label {
  display: grid;
  gap: 6px;
  color: var(--text);
  font-size: 11px;
  font-weight: 650;
}

.vault-fields .wide {
  grid-column: 1 / -1;
}

.vault-fields .favorite-field {
  display: flex;
  grid-column: 1 / -1;
  align-items: center;
  gap: 8px;
}

.vault-fields :deep(.cn-textarea) {
  min-height: 76px;
}

.vault-dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 15px 22px 20px;
  border-top: 1px solid var(--border);
}

.vault-dialog-footer :deep(.cn-button) {
  gap: 6px;
}

.vault-settings-body {
  display: grid;
  padding: 17px 22px 5px;
}

.vault-settings-body section {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 240px;
  gap: 18px;
  padding: 15px 0;
  border-top: 1px solid var(--border);
}

.vault-settings-body section:first-child {
  border-top: 0;
}

.vault-settings-body h3 {
  margin: 0;
  color: var(--text);
  font-size: 13px;
}

.vault-settings-body p {
  margin: 4px 0 0;
  color: var(--text-muted);
  font-size: 10px;
  line-height: 1.5;
}

.settings-fields {
  display: grid;
  gap: 7px;
}

.settings-fields :deep(.cn-button) {
  gap: 6px;
  overflow: hidden;
}

.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
}

/* ---------- 响应式 ---------- */
@media (max-width: 980px) {
  .vault-row {
    grid-template-columns: 34px 1.4fr 1.2fr;
  }

  .vault-row.head .cell-actions-label,
  .vault-row .vr-secret,
  .vault-row.head span:nth-child(4) {
    display: none;
  }

  .vault-row .vr-actions {
    grid-column: 3;
    grid-row: 2;
    justify-content: flex-start;
    padding-top: 2px;
  }
}

@media (max-width: 760px) {
  .vault-search {
    max-width: none;
    order: 3;
    flex-basis: 100%;
  }

  .vault-row {
    grid-template-columns: 34px minmax(0, 1fr) auto;
  }

  .vault-row.head {
    display: none;
  }

  .vault-row .vr-account {
    grid-column: 2;
  }

  .vault-row .vr-actions {
    grid-column: 3;
    grid-row: 2;
  }
}

@media (max-width: 520px) {
  .vault-settings-body section {
    grid-template-columns: 1fr;
  }

  .vault-fields {
    grid-template-columns: 1fr;
  }

  .vault-fields label {
    grid-column: 1;
  }
}
</style>
