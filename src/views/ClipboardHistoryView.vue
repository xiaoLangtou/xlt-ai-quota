<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  AlignLeft, Clipboard, ClipboardCheck, Clock3, Copy, ExternalLink, File, FileText, Image, Maximize2, Minimize2, Pause,
  Keyboard, Pin, PinOff, Play, Plus, Search, Settings2, ShieldCheck, ShieldAlert,
  Trash2, X,
} from "lucide-vue-next";
import { DialogDescription, DialogTitle } from "reka-ui";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Dialog } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { pushToast } from "@/composables/useToast";
import { isTauriDesktop } from "@/connectors/types";
import { clipboardService } from "@/services/clipboard-service";
import HighlightedCode from "@/components/HighlightedCode.vue";
import type {
  ClipboardFilePreview, ClipboardItem, ClipboardKind, ClipboardScope, ClipboardSettingsDraft, ClipboardStatus,
} from "@/types/clipboard";
import { detectClipboardCodeLanguage, detectClipboardSpecialContent, type ClipboardSpecialContent } from "@/utils/clipboard-content";

interface ClipboardGroup {
  key: string;
  label: string;
  items: ClipboardItem[];
}

const desktop = isTauriDesktop();
const items = ref<ClipboardItem[]>([]);
const status = ref<ClipboardStatus | null>(null);
const query = ref("");
const scope = ref<ClipboardScope>("all");
const selectedId = ref<string | null>(null);
const loading = ref(false);
const imageDataUrl = ref("");
const filePreview = ref<ClipboardFilePreview | null>(null);
const filePreviewIndex = ref(0);
const filePreviewLoading = ref(false);
const filePreviewError = ref("");
const thumbnailUrls = ref<Record<string, string>>({});
const previewExpanded = ref(false);
const settingsOpen = ref(false);
const clearOpen = ref(false);
const clearPinned = ref(false);
const settingsSaving = ref(false);
const recordingShortcut = ref(false);
const shortcutRecordError = ref("");
const shortcutRecorder = ref<HTMLButtonElement | null>(null);
const excludedText = ref("");
const settingsForm = ref<ClipboardSettingsDraft>({
  enabled: true,
  launchAtLogin: false,
  maxItems: 100,
  ttlDays: 3,
  shortcut: "CommandOrControl+Shift+V",
  excludedApps: [],
});
let searchTimer: number | undefined;
let unlisten: UnlistenFn | undefined;
let unlistenFocus: UnlistenFn | undefined;
let request = 0;
let filePreviewRequest = 0;

const scopes: { value: ClipboardScope; label: string; icon: typeof Clipboard }[] = [
  { value: "all", label: "全部", icon: Clipboard },
  { value: "text", label: "文本", icon: FileText },
  { value: "image", label: "图片", icon: Image },
  { value: "files", label: "文件", icon: File },
  { value: "pinned", label: "已收藏", icon: Pin },
];

const selected = computed(() => items.value.find((item) => item.id === selectedId.value) ?? null);
const specialContent = computed(() => selected.value?.kind === "text" ? detectClipboardSpecialContent(selected.value.content) : []);
const codeLanguage = computed(() => selected.value?.kind === "text" ? detectClipboardCodeLanguage(selected.value.content) : null);
const fileCodeLanguage = computed(() => filePreview.value?.kind === "text" && filePreview.value.content
  ? detectClipboardCodeLanguage(filePreview.value.content)
  : null);
const unpinnedCount = computed(() => Math.max(0, (status.value?.total ?? 0) - (status.value?.pinned ?? 0)));
const groups = computed<ClipboardGroup[]>(() => {
  const grouped = new Map<string, ClipboardGroup>();
  for (const item of items.value) {
    const group = item.pinned
      ? { key: "pinned", label: "已收藏" }
      : dateGroup(item.updatedAt);
    if (!grouped.has(group.key)) grouped.set(group.key, { ...group, items: [] });
    grouped.get(group.key)?.items.push(item);
  }
  return [...grouped.values()];
});
const filePaths = computed(() => {
  if (selected.value?.kind !== "files") return [];
  try { return JSON.parse(selected.value.content) as string[]; }
  catch { return []; }
});

function dateGroup(value: string): { key: string; label: string } {
  const date = new Date(value);
  const today = new Date();
  const start = new Date(today.getFullYear(), today.getMonth(), today.getDate()).getTime();
  const day = new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
  const diff = Math.round((start - day) / 86_400_000);
  if (diff <= 0) return { key: "today", label: "今天" };
  if (diff === 1) return { key: "yesterday", label: "昨天" };
  if (diff < 7) return { key: "week", label: "过去 7 天" };
  return { key: "earlier", label: "更早" };
}

function kindLabel(kind: ClipboardKind): string {
  return kind === "text" ? "文本" : kind === "image" ? "图片" : "文件";
}

function kindIcon(kind: ClipboardKind) {
  return kind === "text" ? FileText : kind === "image" ? Image : File;
}

function specialLabel(item: ClipboardSpecialContent): string {
  return item.kind === "url" ? "链接" : item.kind === "email" ? "邮箱" : "颜色";
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
  settingsForm.value.shortcut = [...modifiers, key].join("+");
  shortcutRecordError.value = "";
  recordingShortcut.value = false;
}

function itemTitle(item: ClipboardItem): string {
  if (item.kind === "image") return `图片 · ${item.imageWidth ?? 0} × ${item.imageHeight ?? 0}`;
  if (item.kind === "files") {
    try {
      const paths = JSON.parse(item.content) as string[];
      const name = paths[0]?.split(/[\\/]/).pop() ?? "文件";
      return paths.length > 1 ? `${name} 等 ${paths.length} 个文件` : name;
    } catch { return "文件"; }
  }
  return item.content.replace(/\s+/g, " ").trim();
}

function formatTime(value: string, full = false): string {
  const date = new Date(value);
  if (full) return new Intl.DateTimeFormat("zh-CN", { month: "long", day: "numeric", hour: "2-digit", minute: "2-digit" }).format(date);
  const diff = Date.now() - date.getTime();
  if (diff < 60_000) return "刚刚";
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)} 分钟前`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)} 小时前`;
  return new Intl.DateTimeFormat("zh-CN", { month: "numeric", day: "numeric" }).format(date);
}

function formatBytes(value: number): string {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / 1024 / 1024).toFixed(1)} MB`;
}

async function load(keepSelection = true): Promise<void> {
  if (!desktop) return;
  const id = ++request;
  loading.value = true;
  try {
    const kind = ["text", "image", "files"].includes(scope.value) ? scope.value as ClipboardKind : null;
    const [nextItems, nextStatus] = await Promise.all([
      clipboardService.list({ query: query.value.trim() || null, kind, pinnedOnly: scope.value === "pinned", limit: 1000 }),
      clipboardService.status(),
    ]);
    if (id !== request) return;
    items.value = nextItems;
    status.value = nextStatus;
    if (!keepSelection || !nextItems.some((item) => item.id === selectedId.value)) {
      selectedId.value = nextItems[0]?.id ?? null;
    }
    void loadThumbnails(nextItems);
  } catch (reason) {
    pushToast(reason instanceof Error ? reason.message : String(reason), "error");
  } finally {
    if (id === request) loading.value = false;
  }
}

async function loadThumbnails(values: ClipboardItem[]): Promise<void> {
  const images = values.filter((item) => item.kind === "image" && !thumbnailUrls.value[item.id]).slice(0, 36);
  try {
    const loaded = await Promise.all(images.map(async (item) => [item.id, await clipboardService.imageThumbnailDataUrl(item.id, 72)] as const));
    thumbnailUrls.value = { ...thumbnailUrls.value, ...Object.fromEntries(loaded) };
  } catch (reason) { pushToast(reason instanceof Error ? reason.message : String(reason), "error"); }
}

async function selectFile(index: number): Promise<void> {
  const item = selected.value;
  if (!item || item.kind !== "files") return;
  const id = ++filePreviewRequest;
  filePreviewIndex.value = index;
  filePreview.value = null;
  filePreviewError.value = "";
  filePreviewLoading.value = true;
  try {
    const preview = await clipboardService.filePreview(item.id, index);
    if (id === filePreviewRequest && selected.value?.id === item.id) filePreview.value = preview;
  } catch (reason) {
    if (id === filePreviewRequest) filePreviewError.value = reason instanceof Error ? reason.message : String(reason);
  } finally {
    if (id === filePreviewRequest) filePreviewLoading.value = false;
  }
}

function scheduleLoad(): void {
  if (searchTimer !== undefined) window.clearTimeout(searchTimer);
  searchTimer = window.setTimeout(() => void load(false), 160);
}

async function copy(item: ClipboardItem): Promise<void> {
  try {
    const updated = await clipboardService.copy(item.id);
    items.value = items.value.map((current) => current.id === updated.id ? updated : current);
    pushToast("已复制到系统剪贴板。", "success");
  } catch (reason) { pushToast(reason instanceof Error ? reason.message : String(reason), "error"); }
}

async function copyPlain(item: ClipboardItem): Promise<void> {
  try {
    const updated = await clipboardService.copyPlain(item.id);
    items.value = items.value.map((current) => current.id === updated.id ? updated : current);
    pushToast("已以纯文本写入系统剪贴板。", "success");
  } catch (reason) { pushToast(reason instanceof Error ? reason.message : String(reason), "error"); }
}

async function useSpecial(item: ClipboardSpecialContent): Promise<void> {
  try {
    if (item.kind === "color") {
      await clipboardService.writeText(item.value);
      pushToast("已复制色值。", "success");
    } else {
      await clipboardService.openSpecial(item.kind, item.value);
    }
  } catch (reason) { pushToast(reason instanceof Error ? reason.message : String(reason), "error"); }
}

async function togglePin(item: ClipboardItem): Promise<void> {
  try {
    await clipboardService.setPinned(item.id, !item.pinned);
    await load();
    pushToast(item.pinned ? "已取消收藏。" : "已收藏，不参与自动清理。", "success");
  } catch (reason) { pushToast(reason instanceof Error ? reason.message : String(reason), "error"); }
}

async function remove(item: ClipboardItem): Promise<void> {
  try {
    await clipboardService.delete(item.id);
    await load(false);
    pushToast("记录已删除。", "success");
  } catch (reason) { pushToast(reason instanceof Error ? reason.message : String(reason), "error"); }
}

async function saveAsSnippet(item: ClipboardItem): Promise<void> {
  try {
    await clipboardService.saveAsSnippet(item.id);
    pushToast("已保存到片段库。", "success");
  } catch (reason) { pushToast(reason instanceof Error ? reason.message : String(reason), "error"); }
}

async function clearHistory(): Promise<void> {
  try {
    const count = await clipboardService.clear(clearPinned.value);
    clearOpen.value = false;
    clearPinned.value = false;
    await load(false);
    pushToast(`已删除 ${count} 条记录。`, "success");
  } catch (reason) { pushToast(reason instanceof Error ? reason.message : String(reason), "error"); }
}

function openSettings(): void {
  if (!status.value) return;
  settingsForm.value = { ...status.value.settings, excludedApps: [...status.value.settings.excludedApps] };
  excludedText.value = status.value.settings.excludedApps.join("\n");
  recordingShortcut.value = false;
  shortcutRecordError.value = "";
  settingsOpen.value = true;
}

async function refreshAccessibility(): Promise<void> {
  try {
    status.value = await clipboardService.status();
    if (status.value.accessibilityGranted) pushToast("辅助功能权限已生效。", "success");
  } catch (reason) { pushToast(reason instanceof Error ? reason.message : String(reason), "error"); }
}

async function saveSettings(): Promise<void> {
  recordingShortcut.value = false;
  settingsSaving.value = true;
  try {
    status.value = await clipboardService.updateSettings({
      ...settingsForm.value,
      excludedApps: excludedText.value.split(/[,，\n]/).map((item) => item.trim()).filter(Boolean),
    });
    settingsOpen.value = false;
    await load();
    pushToast("剪贴板设置已保存。", "success");
  } catch (reason) { pushToast(reason instanceof Error ? reason.message : String(reason), "error"); }
  finally { settingsSaving.value = false; }
}

async function toggleCapture(): Promise<void> {
  if (!status.value) return;
  settingsForm.value = { ...status.value.settings, enabled: !status.value.settings.enabled };
  try {
    status.value = await clipboardService.updateSettings(settingsForm.value);
    pushToast(status.value.settings.enabled ? "剪贴板采集已恢复。" : "剪贴板采集已暂停。", "success");
  } catch (reason) { pushToast(reason instanceof Error ? reason.message : String(reason), "error"); }
}

function navigate(offset: number): void {
  if (!items.value.length) return;
  const index = Math.max(0, items.value.findIndex((item) => item.id === selectedId.value));
  selectedId.value = items.value[Math.min(items.value.length - 1, Math.max(0, index + offset))]?.id ?? null;
}

function keyboard(event: KeyboardEvent): void {
  const target = event.target as HTMLElement;
  if (["INPUT", "TEXTAREA"].includes(target.tagName)) return;
  if (event.key === "ArrowDown") { event.preventDefault(); navigate(1); }
  if (event.key === "ArrowUp") { event.preventDefault(); navigate(-1); }
  if (event.key === " ") { event.preventDefault(); previewExpanded.value = !previewExpanded.value; }
  if (event.key === "Enter" && event.shiftKey && selected.value?.kind === "text") { event.preventDefault(); void copyPlain(selected.value); }
  else if (event.key === "Enter" && selected.value) { event.preventDefault(); void copy(selected.value); }
  if ((event.key === "Delete" || event.key === "Backspace") && selected.value) { event.preventDefault(); void remove(selected.value); }
}

watch(query, scheduleLoad);
watch(scope, () => void load(false));
watch(selected, async (item) => {
  previewExpanded.value = false;
  imageDataUrl.value = "";
  filePreviewRequest += 1;
  filePreview.value = null;
  filePreviewIndex.value = 0;
  filePreviewLoading.value = false;
  filePreviewError.value = "";
  if (item?.kind === "image") {
    try { imageDataUrl.value = await clipboardService.imageDataUrl(item.id); }
    catch (reason) { pushToast(reason instanceof Error ? reason.message : String(reason), "error"); }
  } else if (item?.kind === "files") {
    await selectFile(0);
  }
});

onMounted(async () => {
  if (!desktop) return;
  window.addEventListener("keydown", keyboard);
  unlisten = await listen("clipboard-history-updated", () => void load());
  unlistenFocus = await getCurrentWindow().onFocusChanged(({ payload }) => {
    if (payload) void refreshAccessibility();
  });
  await load(false);
});
onUnmounted(() => {
  if (searchTimer !== undefined) window.clearTimeout(searchTimer);
  window.removeEventListener("keydown", keyboard);
  unlisten?.();
  unlistenFocus?.();
});
</script>

<template>
  <section v-if="!desktop" class="unavailable">
    <Clipboard :size="38" />
    <strong>剪贴板历史仅在桌面端可用</strong>
    <p>系统监听、全局快捷键和本机历史记录需要通过 Tauri 桌面应用运行。</p>
  </section>
  <section v-else class="clipboard-workspace">
    <section class="clipboard-card">
      <header class="clipboard-toolbar" aria-label="剪贴板历史操作">
        <div class="toolbar-identity">
          <h1>剪贴板历史</h1>
          <span class="capture-state" :class="{ paused: !status?.settings.enabled }"><i />{{ status?.settings.enabled ? "正在记录" : "已暂停" }}</span>
        </div>
        <div class="search-field"><Search :size="17" /><Input v-model="query" aria-label="搜索剪贴板历史" placeholder="搜索内容或来源应用…" /><button v-if="query" type="button" aria-label="清除搜索" @click="query = ''"><X :size="15" /></button></div>
        <div class="toolbar-actions">
          <Button type="button" variant="ghost" size="sm" @click="toggleCapture"><Pause v-if="status?.settings.enabled" :size="15" /><Play v-else :size="15" />{{ status?.settings.enabled ? "暂停" : "继续" }}</Button>
          <Button type="button" variant="ghost" size="icon" title="清空历史" aria-label="清空历史" @click="clearOpen = true"><Trash2 :size="17" /></Button>
          <Button type="button" variant="ghost" size="icon" aria-label="剪贴板设置" @click="openSettings"><Settings2 :size="18" /></Button>
        </div>
      </header>

      <div v-if="status && !status.accessibilityGranted" class="permission-banner">
        <ShieldAlert :size="17" />
        <div><strong>需要辅助功能权限</strong><span>请在“系统设置 → 隐私与安全性 → 辅助功能”中允许当前运行程序，然后返回此窗口复检。</span><small>{{ status.accessibilityTarget }}</small></div><Button type="button" variant="outline" size="sm" @click="refreshAccessibility">重新检查</Button>
      </div>

      <nav class="clipboard-sidebar" aria-label="内容类型筛选">
        <button v-for="item in scopes" :key="item.value" type="button" :class="{ active: scope === item.value }" @click="scope = item.value">
          <component :is="item.icon" :size="15" /><span>{{ item.label }}</span>
        </button>
        <div class="sidebar-foot"><ShieldCheck :size="14" /><span>内容仅存储在本机</span></div>
      </nav>

      <div class="clipboard-layout" :class="{ 'preview-expanded': previewExpanded }">
      <main class="history-pane">
        <div v-if="loading && !items.length" class="center-state">正在读取本机历史…</div>
        <div v-else-if="!items.length" class="center-state empty-state"><span class="empty-icon"><ClipboardCheck :size="27" /></span><strong>{{ query || scope !== 'all' ? "没有匹配的记录" : "等待第一次复制" }}</strong><p>{{ query || scope !== 'all' ? "换个关键词，或查看其他内容类型。" : "复制文本、图片或文件后，它们会自动出现在这里。" }}</p></div>
        <div v-else class="history-scroll">
          <section v-for="group in groups" :key="group.key" class="history-group">
            <header><span>{{ group.label }}</span></header>
            <article v-for="item in group.items" :key="item.id" class="history-item" :class="{ selected: selectedId === item.id }" tabindex="0" @click="selectedId = item.id" @dblclick="copy(item)" @keydown.enter.prevent="copy(item)">
              <span class="kind-mark" :data-kind="item.kind"><img v-if="item.kind === 'image' && thumbnailUrls[item.id]" :src="thumbnailUrls[item.id]" alt="" /><component :is="kindIcon(item.kind)" v-else :size="16" /></span>
              <div class="item-copy"><strong>{{ itemTitle(item) }}</strong><span>{{ item.sourceApp || "未知来源" }} · {{ formatTime(item.updatedAt) }}<template v-if="item.copyCount > 1"> · 使用 {{ item.copyCount }} 次</template></span></div>
              <div class="item-actions">
                <button type="button" title="复制" aria-label="复制" @click.stop="copy(item)"><Copy :size="14" /></button>
                <button type="button" :title="item.pinned ? '取消收藏' : '收藏'" :aria-label="item.pinned ? '取消收藏' : '收藏'" @click.stop="togglePin(item)"><PinOff v-if="item.pinned" :size="14" /><Pin v-else :size="14" /></button>
              </div>
              <Pin v-if="item.pinned" class="pin-mark" :size="13" />
            </article>
          </section>
        </div>
      </main>

      <aside class="preview-pane">
        <div v-if="selected" class="preview-content">
          <header class="preview-head">
            <span class="kind-chip" :data-kind="selected.kind"><component :is="kindIcon(selected.kind)" :size="13" />{{ kindLabel(selected.kind) }}</span>
            <div class="preview-head-actions">
              <button v-for="item in specialContent" :key="item.kind + item.value" type="button" class="special-action" :title="item.kind === 'color' ? '复制色值' : `打开${specialLabel(item)}`" @click="useSpecial(item)"><i v-if="item.kind === 'color'" :style="{ background: item.value }" /><ExternalLink v-else :size="12" />{{ specialLabel(item) }}</button>
              <button v-if="selected.kind === 'text' && selected.content.length > 600" type="button" :aria-label="previewExpanded ? '收起预览' : '展开预览'" @click="previewExpanded = !previewExpanded"><Minimize2 v-if="previewExpanded" :size="15" /><Maximize2 v-else :size="15" /></button>
              <button type="button" :aria-label="selected.pinned ? '取消收藏' : '收藏'" @click="togglePin(selected)"><PinOff v-if="selected.pinned" :size="16" /><Pin v-else :size="16" /></button>
            </div>
          </header>
          <div class="preview-body" :class="{ 'file-preview-body': selected.kind === 'files' }">
            <HighlightedCode v-if="selected.kind === 'text' && codeLanguage" class="clipboard-code-preview" :code="selected.content" :language="codeLanguage" appearance="light" />
            <pre v-else-if="selected.kind === 'text'">{{ selected.content }}</pre>
            <div v-else-if="selected.kind === 'image'" class="image-preview"><img v-if="imageDataUrl" :src="imageDataUrl" alt="剪贴板图片预览" /><span v-else>正在读取图片…</span></div>
            <div v-else class="file-preview" :class="{ 'has-file-list': filePaths.length > 1 }">
              <nav v-if="filePaths.length > 1" class="file-preview-list" aria-label="选择要预览的文件">
                <button v-for="(path, index) in filePaths" :key="path" type="button" :class="{ active: filePreviewIndex === index }" @click="selectFile(index)">
                  <File :size="15" /><span>{{ path.split(/[\\/]/).pop() }}</span>
                </button>
              </nav>
              <section class="file-preview-content">
                <header v-if="filePreview"><div><File :size="15" /><strong>{{ filePreview.name }}</strong></div><span>{{ formatBytes(filePreview.sizeBytes) }}</span></header>
                <div v-if="filePreviewLoading" class="file-preview-state">正在读取文件…</div>
                <div v-else-if="filePreviewError" class="file-preview-state error">{{ filePreviewError }}</div>
                <img v-else-if="filePreview?.kind === 'image' && filePreview.dataUrl" :src="filePreview.dataUrl" :alt="filePreview.name" />
                <iframe v-else-if="filePreview?.kind === 'pdf' && filePreview.dataUrl" :src="filePreview.dataUrl" :title="filePreview.name" />
                <HighlightedCode v-else-if="filePreview?.kind === 'text' && filePreview.content && fileCodeLanguage" class="clipboard-code-preview file-code-preview" :code="filePreview.content" :language="fileCodeLanguage" appearance="light" />
                <pre v-else-if="filePreview?.kind === 'text'">{{ filePreview.content }}</pre>
                <div v-else-if="filePreview" class="file-preview-state"><File :size="30" /><strong>无法预览此文件</strong><span>{{ filePreview.message }}</span><small>{{ filePreview.path }}</small></div>
              </section>
            </div>
          </div>
          <dl class="preview-meta"><div><dt>来源</dt><dd>{{ selected.sourceApp || "未知来源" }}</dd></div><div><dt>时间</dt><dd>{{ formatTime(selected.updatedAt, true) }}</dd></div><div><dt>大小</dt><dd>{{ formatBytes(selected.sizeBytes) }}</dd></div><div v-if="selected.kind === 'image'"><dt>尺寸</dt><dd>{{ selected.imageWidth }} × {{ selected.imageHeight }}</dd></div></dl>
          <footer class="preview-actions"><Button type="button" @click="copy(selected)"><Copy :size="15" />复制</Button><Button v-if="selected.kind === 'text'" type="button" variant="outline" @click="copyPlain(selected)"><AlignLeft :size="15" />纯文本</Button><Button v-if="selected.kind === 'text'" type="button" variant="outline" @click="saveAsSnippet(selected)"><Plus :size="15" />保存为片段</Button><Button type="button" variant="ghost" size="icon" aria-label="删除记录" @click="remove(selected)"><Trash2 :size="16" /></Button></footer>
        </div>
        <div v-else class="preview-empty"><Clipboard :size="28" /><span>选择一条记录查看完整内容</span></div>
      </aside>
      </div>

      <footer class="clipboard-statusbar">
        <span>{{ items.length }} 条记录<template v-if="status?.pinned"> · {{ status.pinned }} 条收藏</template></span>
        <span class="retention-note"><Clock3 :size="13" />{{ status?.settings.ttlDays ?? 3 }} 天 · 最多 {{ status?.settings.maxItems ?? 100 }} 条</span>
        <kbd>{{ shortcutLabel(status?.settings.shortcut ?? "CommandOrControl+Shift+V") }}</kbd>
      </footer>
    </section>

    <Dialog v-model:open="settingsOpen" content-class="clipboard-settings-dialog">
      <DialogTitle class="dialog-title">剪贴板设置</DialogTitle>
      <DialogDescription class="dialog-desc">控制采集范围、保留策略和快捷面板。</DialogDescription>
      <form class="settings-form" @submit.prevent="saveSettings">
        <div v-if="status && !status.accessibilityGranted" class="settings-permission"><ShieldAlert :size="17" /><span><strong>辅助功能权限未开启</strong><small>请允许当前运行程序：{{ status.accessibilityTarget }}</small></span><Button type="button" variant="outline" size="sm" @click="refreshAccessibility">重新检查</Button></div>
        <label class="toggle-row"><div><strong>自动记录剪贴板</strong><span>应用常驻时监听文本、图片和文件。</span></div><Checkbox v-model="settingsForm.enabled" /></label>
        <label class="toggle-row"><div><strong>登录时自动启动</strong><span>在后台启动并继续记录，不打开主窗口。</span></div><Checkbox v-model="settingsForm.launchAtLogin" /></label>
        <div class="settings-grid"><label><span>容量上限</span><Input v-model="settingsForm.maxItems" type="number" min="20" max="5000" /><small>收藏记录不占用名额</small></label><label><span>保留时长（天）</span><Input v-model="settingsForm.ttlDays" type="number" min="1" max="365" /><small>到期时自动删除</small></label></div>
        <label class="shortcut-field"><span>全局快捷键</span><button ref="shortcutRecorder" class="shortcut-recorder" :class="{ recording: recordingShortcut }" type="button" :aria-pressed="recordingShortcut" @click="beginShortcutRecording" @keydown="recordShortcut" @blur="recordingShortcut = false"><Keyboard :size="17" /><span v-if="recordingShortcut">请按下快捷键…</span><kbd v-else>{{ shortcutLabel(settingsForm.shortcut) }}</kbd><em>{{ recordingShortcut ? "Esc 取消" : "点击录制" }}</em></button><small :class="{ 'shortcut-error': shortcutRecordError }">{{ shortcutRecordError || "F1–F12 可单独使用，其他按键需要组合；保存时检查占用" }}</small></label>
        <label><span>不记录这些应用</span><textarea v-model="excludedText" class="cn-textarea" rows="5" placeholder="每行一个应用名称" /><small>默认包含常见密码管理器</small></label>
        <label v-if="status"><span>本机存储位置</span><Input :model-value="status.storagePath" readonly /><small>历史数据库与图片只保存在应用数据目录。</small></label>
        <footer><Button type="button" variant="outline" @click="settingsOpen = false">取消</Button><Button type="submit" :disabled="settingsSaving">{{ settingsSaving ? "保存中…" : "保存设置" }}</Button></footer>
      </form>
    </Dialog>

    <Dialog v-model:open="clearOpen" content-class="clear-dialog">
      <DialogTitle class="dialog-title">清空剪贴板历史？</DialogTitle>
      <DialogDescription class="dialog-desc">将删除 {{ unpinnedCount }} 条未收藏记录，此操作无法撤销。</DialogDescription>
      <label class="clear-pinned"><Checkbox v-model="clearPinned" /><span>同时删除 {{ status?.pinned ?? 0 }} 条收藏记录</span></label>
      <footer class="dialog-actions"><Button type="button" variant="outline" @click="clearOpen = false">取消</Button><Button type="button" variant="destructive" @click="clearHistory">确认清空</Button></footer>
    </Dialog>
  </section>
</template>

<style scoped>
.clipboard-workspace { display:flex; width:100%; height:100%; min-width:0; min-height:0; overflow:hidden; color:var(--text); }
.clipboard-card { display:flex; width:100%; min-height:0; flex:1; flex-direction:column; overflow:hidden; border:1px solid var(--border); border-radius:var(--r-lg); background:var(--surface); box-shadow:var(--shadow-card); }
.clipboard-toolbar { display:grid; min-height:60px; flex:0 0 auto; grid-template-columns:auto minmax(260px,560px) auto; align-items:center; gap:18px; padding:9px 14px 9px 18px; border-bottom:1px solid var(--border); }
.toolbar-identity,.toolbar-actions { display:flex; align-items:center; }
.toolbar-identity { min-width:210px; gap:10px; }
.toolbar-identity h1 { margin:0; color:var(--text); font-size:16px; font-weight:720; letter-spacing:-.02em; white-space:nowrap; }
.toolbar-actions { justify-content:flex-end; gap:2px; }
.capture-state { display:inline-flex; align-items:center; gap:6px; color:var(--u-ok); font-size:10px; font-weight:650; white-space:nowrap; }
.capture-state i { width:6px; height:6px; border-radius:50%; background:currentColor; box-shadow:0 0 0 3px color-mix(in srgb,currentColor 12%,transparent); }
.capture-state.paused { color:var(--u-warn); }
.search-field { display:flex; width:100%; min-width:180px; height:36px; align-items:center; gap:7px; padding:0 10px; border:1px solid var(--border); border-radius:8px; background:var(--surface-2); color:var(--text-subtle); }
.search-field:focus-within { border-color:color-mix(in srgb,var(--accent) 48%,var(--border)); background:var(--surface); box-shadow:0 0 0 3px var(--accent-weak); }
.search-field .cn-input { min-width:0; height:32px; border:0; background:transparent; box-shadow:none; padding:0; }
.search-field button { display:grid; place-items:center; padding:4px; border:0; background:transparent; color:var(--text-subtle); cursor:pointer; }
.permission-banner { display:flex; min-height:38px; flex:0 0 auto; align-items:center; gap:9px; padding:7px 16px; border-bottom:1px solid color-mix(in srgb,var(--u-warn) 24%,var(--border)); background:color-mix(in srgb,var(--u-warn) 5%,var(--surface)); color:var(--u-warn); }
.permission-banner div { display:grid; min-width:0; gap:2px; }.permission-banner strong { font-size:.72rem; }.permission-banner span,.permission-banner small { overflow:hidden; color:var(--text-muted); font-size:.69rem; text-overflow:ellipsis; white-space:nowrap; }.permission-banner small { color:var(--text-subtle); font-family:var(--font-mono); }.permission-banner :deep(.cn-button) { margin-left:auto; flex:0 0 auto; }
.clipboard-sidebar { display:flex; min-width:0; min-height:46px; flex:0 0 auto; align-items:center; gap:2px; padding:6px 12px; border-bottom:1px solid var(--border); }
.clipboard-sidebar button { display:flex; height:32px; align-items:center; gap:7px; padding:0 11px; border:0; border-radius:7px; background:transparent; color:var(--text-muted); font:inherit; font-size:11px; cursor:pointer; }
.clipboard-sidebar button:hover { background:var(--surface-2); color:var(--text); }.clipboard-sidebar button.active { background:var(--accent-weak); color:var(--accent); font-weight:700; }
.sidebar-foot { display:flex; align-items:center; gap:6px; margin-left:auto; padding-right:4px; color:var(--text-subtle); font-size:.65rem; white-space:nowrap; }
.clipboard-layout { display:grid; min-height:0; flex:1; grid-template-columns:minmax(330px,42%) minmax(420px,58%); overflow:hidden; }
.history-pane { display:flex; min-width:0; min-height:0; flex-direction:column; border-right:1px solid var(--border); }
.history-scroll { min-height:0; flex:1; overflow-x:hidden; overflow-y:auto; overscroll-behavior:contain; scrollbar-gutter:stable; scrollbar-color:color-mix(in srgb,var(--text) 22%,transparent) transparent; scrollbar-width:thin; }
.history-scroll::-webkit-scrollbar,.preview-body::-webkit-scrollbar { width:8px; }.history-scroll::-webkit-scrollbar-track,.preview-body::-webkit-scrollbar-track { background:transparent; }.history-scroll::-webkit-scrollbar-thumb,.preview-body::-webkit-scrollbar-thumb { border:2px solid transparent; border-radius:999px; background:color-mix(in srgb,var(--text) 22%,transparent); background-clip:padding-box; }
.history-group>header { padding:12px 16px 5px; color:var(--text-subtle); font-size:.65rem; font-weight:700; letter-spacing:.02em; }
.history-item { display:flex; min-height:66px; align-items:center; gap:11px; margin:1px 8px; padding:9px 10px; border:0; border-radius:8px; content-visibility:auto; contain-intrinsic-size:66px; cursor:default; transition:background-color .13s; }
.history-item:hover { background:var(--surface-2); }.history-item.selected { background:color-mix(in srgb,var(--accent) 8%,var(--surface-2)); }.history-item:focus-visible { outline:2px solid var(--accent-ring); outline-offset:-2px; }
.kind-mark { display:grid; width:30px; height:30px; flex:0 0 30px; place-items:center; border-radius:7px; background:color-mix(in srgb,#4f8ff7 9%,var(--surface)); color:#3978d4; }.kind-mark[data-kind="image"] { background:color-mix(in srgb,var(--u-ok) 9%,var(--surface)); color:var(--u-ok); }.kind-mark[data-kind="files"] { background:color-mix(in srgb,#8b67d8 9%,var(--surface)); color:#7567d8; }
.item-copy { min-width:0; flex:1; }.item-copy strong { display:-webkit-box; overflow:hidden; color:var(--text); font-size:.76rem; font-weight:630; line-height:1.45; -webkit-box-orient:vertical; -webkit-line-clamp:2; }.item-copy span { display:block; margin-top:3px; overflow:hidden; color:var(--text-subtle); font-size:.65rem; text-overflow:ellipsis; white-space:nowrap; }.pin-mark { flex:0 0 auto; color:var(--u-warn); }.item-actions { display:flex; flex:0 0 auto; gap:2px; opacity:0; transition:opacity .13s; }.history-item:hover .item-actions,.history-item:focus-within .item-actions,.history-item.selected .item-actions { opacity:1; }.item-actions button { display:grid; width:27px; height:27px; place-items:center; padding:0; border:0; border-radius:6px; background:transparent; color:var(--text-subtle); cursor:pointer; }.item-actions button:hover { background:var(--surface); color:var(--accent); }.history-item .pin-mark { display:none; }.history-item:not(:hover):not(:focus-within):not(.selected) .pin-mark { display:block; }
.preview-pane { min-width:0; min-height:0; background:var(--surface); }.preview-content { display:flex; height:100%; min-height:0; flex-direction:column; }.preview-head { display:flex; min-height:44px; flex:0 0 auto; align-items:center; justify-content:space-between; padding:0 16px; border-bottom:1px solid var(--border); }.preview-head button { display:grid; width:29px; height:29px; place-items:center; border:0; border-radius:6px; background:transparent; color:var(--text-muted); cursor:pointer; }.preview-head button:hover { background:var(--surface-2); color:var(--accent); }
.kind-chip { display:inline-flex; align-items:center; gap:6px; color:var(--text-muted); font-size:.69rem; font-weight:680; }.kind-chip[data-kind="image"] { color:var(--u-ok); }.kind-chip[data-kind="files"] { color:#7567d8; }
.preview-body { min-height:150px; flex:1; overflow-x:hidden; overflow-y:auto; padding:18px 20px; scrollbar-color:color-mix(in srgb,var(--text) 22%,transparent) transparent; scrollbar-width:thin; }.preview-body pre { min-height:100%; margin:0; color:var(--text); font-family:var(--font-mono); font-size:12px; line-height:1.75; white-space:pre-wrap; word-break:break-word; }.image-preview { display:grid; height:100%; min-height:190px; place-items:center; border-radius:8px; background:var(--surface-2); color:var(--text-subtle); }.image-preview img { display:block; width:100%; height:100%; max-height:420px; object-fit:contain; border-radius:7px; }
.clipboard-code-preview { min-height:100%; color:var(--text); font-family:var(--font-mono); font-size:12px; line-height:1.7; }
.preview-body.file-preview-body { overflow:hidden; padding:0; }
.file-preview { display:grid; width:100%; height:100%; min-width:0; min-height:0; }.file-preview.has-file-list { grid-template-columns:180px minmax(0,1fr); }
.file-preview-list { min-width:0; overflow-x:hidden; overflow-y:auto; padding:8px; border-right:1px solid var(--border); }.file-preview-list button { display:flex; width:100%; height:38px; align-items:center; gap:8px; padding:0 9px; border:0; border-radius:7px; background:transparent; color:var(--text-muted); font:inherit; cursor:pointer; }.file-preview-list button:hover { background:var(--surface-2); }.file-preview-list button.active { background:var(--accent-weak); color:var(--accent); }.file-preview-list span { overflow:hidden; font-size:.68rem; text-overflow:ellipsis; white-space:nowrap; }
.file-preview-content { display:flex; min-width:0; min-height:0; flex-direction:column; overflow:auto; }.file-preview-content>header { display:flex; min-height:40px; flex:0 0 auto; align-items:center; justify-content:space-between; gap:12px; padding:0 14px; border-bottom:1px solid var(--border); color:var(--text-subtle); }.file-preview-content>header div { display:flex; min-width:0; align-items:center; gap:7px; }.file-preview-content>header strong { overflow:hidden; color:var(--text-muted); font-size:.68rem; font-weight:620; text-overflow:ellipsis; white-space:nowrap; }.file-preview-content>header span { flex:none; font-size:.62rem; }
.file-preview-content>img { display:block; width:100%; height:100%; min-height:0; padding:16px; object-fit:contain; }.file-preview-content>iframe { width:100%; min-height:420px; flex:1; border:0; background:var(--surface-2); }.file-preview-content>pre { flex:1; margin:0; padding:16px 18px; color:var(--text); font-family:var(--font-mono); font-size:12px; line-height:1.75; white-space:pre-wrap; word-break:break-word; }.file-code-preview { flex:1; padding:16px 18px; }
.file-preview-state { display:grid; min-height:220px; flex:1; place-content:center; justify-items:center; gap:7px; padding:28px; color:var(--text-subtle); font-size:.72rem; text-align:center; }.file-preview-state strong { color:var(--text-muted); font-size:.78rem; }.file-preview-state span { max-width:360px; line-height:1.5; }.file-preview-state small { max-width:420px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }.file-preview-state.error { color:var(--u-crit); }
.preview-meta { display:flex; min-height:36px; flex:0 0 auto; align-items:center; gap:18px; margin:0; padding:6px 16px; overflow:hidden; border-top:1px solid var(--border); color:var(--text-subtle); }.preview-meta div { display:flex; min-width:0; align-items:center; gap:5px; }.preview-meta dt,.preview-meta dd { overflow:hidden; margin:0; font-size:.64rem; text-overflow:ellipsis; white-space:nowrap; }.preview-meta dt::after { content:":"; }.preview-meta dd { color:var(--text-muted); }.preview-actions { display:flex; min-height:52px; flex:0 0 auto; align-items:center; gap:7px; padding:8px 16px; border-top:1px solid var(--border); }.preview-actions .cn-button:last-child { margin-left:auto; color:var(--u-crit); }
.preview-empty,.center-state { display:grid; height:100%; place-content:center; justify-items:center; padding:28px; color:var(--text-subtle); font-size:.78rem; text-align:center; }.empty-state .empty-icon { display:grid; width:52px; height:52px; place-items:center; margin-bottom:12px; border-radius:16px; background:var(--accent-weak); color:var(--accent); }.empty-state strong { color:var(--text); font-size:.9rem; }.empty-state p { max-width:240px; margin:7px 0 0; line-height:1.55; }
.clipboard-statusbar { display:flex; min-height:34px; flex:0 0 auto; align-items:center; gap:14px; padding:0 14px; border-top:1px solid var(--border); color:var(--text-subtle); font-size:10px; }.clipboard-statusbar .retention-note { display:flex; align-items:center; gap:5px; margin-left:auto; }.clipboard-statusbar kbd { padding:2px 6px; border:1px solid var(--border); border-radius:5px; background:var(--surface-2); color:var(--text-muted); font-family:inherit; font-size:10px; }
.dialog-title { margin:0; color:var(--text); font-size:1.05rem; }.dialog-desc { margin:6px 0 18px; color:var(--text-muted); font-size:.78rem; }.settings-form { display:grid; gap:16px; }.settings-form>label,.settings-grid label { display:grid; gap:6px; color:var(--text); font-size:.76rem; font-weight:650; }.settings-form small { color:var(--text-subtle); font-size:.67rem; font-weight:400; }.toggle-row { display:flex!important; align-items:center; justify-content:space-between; padding:12px; border:1px solid var(--border); border-radius:9px; background:var(--surface-2); }.toggle-row div { display:grid; gap:3px; }.toggle-row span { color:var(--text-muted); font-size:.69rem; font-weight:400; }.settings-grid { display:grid; grid-template-columns:1fr 1fr; gap:12px; }.settings-form footer,.dialog-actions { display:flex; justify-content:flex-end; gap:8px; padding-top:4px; }.clear-pinned { display:flex; align-items:center; gap:8px; margin:14px 0 18px; color:var(--text-muted); font-size:.78rem; }
.shortcut-recorder { display:flex; width:100%; min-height:44px; align-items:center; gap:10px; padding:0 12px; border:1px solid var(--border); border-radius:var(--r-sm); outline:0; background:var(--surface-2); color:var(--text-muted); font:inherit; cursor:pointer; transition:border-color .15s,box-shadow .15s,background .15s; }
.shortcut-recorder:hover { border-color:var(--border-strong); background:var(--surface); }
.shortcut-recorder:focus-visible,.shortcut-recorder.recording { border-color:var(--accent); background:var(--surface); box-shadow:0 0 0 3px var(--accent-weak); }
.shortcut-recorder.recording>svg { color:var(--accent); }
.shortcut-recorder>span { color:var(--accent); font-weight:650; }
.shortcut-recorder kbd { padding:4px 8px; border:1px solid var(--border-strong); border-radius:6px; background:var(--surface); box-shadow:0 1px 0 var(--border-strong); color:var(--text); font-family:var(--font-sans); font-size:.75rem; font-weight:700; }
.shortcut-recorder em { margin-left:auto; color:var(--text-subtle); font-size:.66rem; font-style:normal; font-weight:500; }
.settings-form small.shortcut-error { color:var(--u-crit); }
.settings-permission { display:flex; align-items:flex-start; gap:9px; padding:11px 12px; border:1px solid color-mix(in srgb,var(--u-warn) 28%,var(--border)); border-radius:9px; background:color-mix(in srgb,var(--u-warn) 6%,var(--surface)); color:var(--u-warn); }.settings-permission>span { display:grid; min-width:0; gap:3px; }.settings-permission strong { font-size:.75rem; }.settings-permission small { overflow:hidden; color:var(--text-muted); font-family:var(--font-mono); line-height:1.5; text-overflow:ellipsis; white-space:nowrap; }.settings-permission :deep(.cn-button) { margin-left:auto; flex:0 0 auto; }
.unavailable { display:grid; min-height:420px; place-content:center; justify-items:center; gap:9px; color:var(--text-subtle); text-align:center; }.unavailable strong { color:var(--text); }.unavailable p { max-width:420px; margin:0; font-size:.8rem; line-height:1.6; }
:global(.clipboard-settings-dialog) { width:min(560px,calc(100vw - 32px)); padding:22px; }
:global(.clear-dialog) { width:min(420px,calc(100vw - 32px)); padding:22px; }
@media (max-width: 1050px) { .clipboard-toolbar { grid-template-columns:auto minmax(220px,1fr) auto; gap:10px; }.toolbar-identity { min-width:0; }.toolbar-identity h1 { display:none; }.clipboard-layout { grid-template-columns:minmax(310px,44%) minmax(360px,56%); }.sidebar-foot { display:none; }.clipboard-statusbar .retention-note { display:none; } }
.kind-mark img { width:30px; height:30px; border-radius:7px; object-fit:cover; }
.preview-head-actions { display:flex; min-width:0; align-items:center; gap:5px; }
.preview-head .special-action {
  display:inline-flex;
  width:auto;
  height:28px;
  gap:5px;
  padding:0 8px;
  border:1px solid var(--border);
  font-size:10px;
}
.preview-head .special-action i { width:10px; height:10px; border:1px solid var(--border-strong); border-radius:3px; }
.clipboard-layout.preview-expanded { grid-template-columns:1fr; }
.clipboard-layout.preview-expanded .history-pane { display:none; }
.clipboard-layout.preview-expanded .preview-pane { grid-column:1; grid-row:1; }
.clipboard-layout.preview-expanded .preview-body { padding-right:44px; padding-left:44px; }
</style>
