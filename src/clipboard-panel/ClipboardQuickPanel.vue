<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { AlignLeft, BookmarkPlus, ChevronDown, ChevronLeft, Clipboard, ClipboardPaste, Command, CornerDownLeft, ExternalLink, File, FileText, Image, ListFilter, ListOrdered, Maximize2, Minimize2, MoreHorizontal, Pin, Search, ShieldAlert, Sparkles, Trash2, X } from "lucide-vue-next";
import { clipboardService } from "@/services/clipboard-service";
import { snippetService } from "@/services/snippet-service";
import type { ClipboardItem, ClipboardKind, ClipboardSequenceStatus, ClipboardStatus } from "@/types/clipboard";
import type { Snippet } from "@/types/snippet";
import { detectClipboardSpecialContent, type ClipboardSpecialContent } from "@/utils/clipboard-content";

type QuickScope = "all" | ClipboardKind | "pinned" | "snippets";

const items = ref<ClipboardItem[]>([]);
const snippets = ref<Snippet[]>([]);
const query = ref("");
const scope = ref<QuickScope>("all");
const selectedIndex = ref(0);
const loading = ref(false);
const error = ref("");
const imageDataUrl = ref("");
const thumbnailUrls = ref<Record<string, string>>({});
const previewExpanded = ref(false);
const sequenceMode = ref(false);
const sequenceIds = ref<string[]>([]);
const sequenceStatus = ref<ClipboardSequenceStatus>({ active: false, total: 0, nextIndex: 0, remaining: 0 });
const status = ref<ClipboardStatus | null>(null);
const previewVisible = ref(true);
const searchInput = ref<HTMLInputElement | null>(null);
const listScroll = ref<HTMLElement | null>(null);
const previewScroll = ref<HTMLElement | null>(null);
const listScrollbar = ref({ visible: false, top: 0, height: 0 });
const previewScrollbar = ref({ visible: false, top: 0, height: 0 });
const unlisteners: UnlistenFn[] = [];
let timer: number | undefined;
let request = 0;

const selected = computed(() => scope.value === "snippets" ? null : items.value[selectedIndex.value] ?? null);
const selectedSnippet = computed(() => scope.value === "snippets" ? snippets.value[selectedIndex.value] ?? null : null);
const activeCount = computed(() => scope.value === "snippets" ? snippets.value.length : items.value.length);
const selectedText = computed(() => selected.value?.kind === "text" ? selected.value.content : selectedSnippet.value?.content ?? "");
const specialContent = computed(() => detectClipboardSpecialContent(selectedText.value));
const mergeableSelection = computed(() => sequenceIds.value.length > 1 && sequenceIds.value.every((id) => items.value.find((item) => item.id === id)?.kind === "text"));
const filePaths = computed(() => {
  if (selected.value?.kind !== "files") return [];
  try { return JSON.parse(selected.value.content) as string[]; }
  catch { return []; }
});

function icon(kind: ClipboardKind) { return kind === "text" ? FileText : kind === "image" ? Image : File; }
function label(kind: ClipboardKind): string { return kind === "text" ? "文本" : kind === "image" ? "图片" : "文件"; }
function scopeLabel(value: QuickScope): string {
  return value === "all" ? "全部类型" : value === "pinned" ? "已收藏" : value === "snippets" ? "固定片段" : label(value);
}
function specialLabel(item: ClipboardSpecialContent): string { return item.kind === "url" ? "链接" : item.kind === "email" ? "邮箱" : "颜色"; }
function summary(item: ClipboardItem): string {
  if (item.kind === "image") return `图片 · ${item.imageWidth} × ${item.imageHeight}`;
  if (item.kind === "files") {
    try {
      const paths = JSON.parse(item.content) as string[];
      const name = paths[0]?.split(/[\\/]/).pop() ?? "文件";
      return paths.length > 1 ? `${name} 等 ${paths.length} 个文件` : name;
    } catch { return "文件"; }
  }
  // 列表摘要只展示短行，截取前缀避免对超长文本重复做正则归一。
  return item.content.slice(0, 200).replace(/\s+/g, " ").trim();
}
function time(value: string): string {
  const diff = Date.now() - new Date(value).getTime();
  if (diff < 60_000) return "刚刚";
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)} 分钟前`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)} 小时前`;
  return new Intl.DateTimeFormat("zh-CN", { month: "numeric", day: "numeric" }).format(new Date(value));
}
function fullTime(value: string): string {
  return new Intl.DateTimeFormat("zh-CN", { month: "numeric", day: "numeric", hour: "2-digit", minute: "2-digit" }).format(new Date(value));
}
function formatBytes(value: number): string {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / 1024 / 1024).toFixed(1)} MB`;
}

function measureScrollbar(element: HTMLElement | null, target: typeof listScrollbar): void {
  if (!element || element.scrollHeight <= element.clientHeight + 1) {
    target.value = { visible: false, top: 0, height: 0 };
    return;
  }
  const trackHeight = Math.max(0, element.clientHeight - 16);
  const height = Math.max(34, trackHeight * (element.clientHeight / element.scrollHeight));
  const progress = element.scrollTop / (element.scrollHeight - element.clientHeight);
  target.value = { visible: true, top: 8 + (trackHeight - height) * progress, height };
}

function updateScrollbars(): void {
  measureScrollbar(listScroll.value, listScrollbar);
  measureScrollbar(previewScroll.value, previewScrollbar);
}

async function load(): Promise<void> {
  const id = ++request;
  loading.value = true;
  error.value = "";
  try {
    if (scope.value === "snippets") {
      const next = await snippetService.list({ query: query.value.trim() || null, kind: null, tag: null, sort: "recent" });
      if (id !== request) return;
      snippets.value = next;
      items.value = [];
      selectedIndex.value = Math.min(selectedIndex.value, Math.max(0, next.length - 1));
      await nextTick();
      updateScrollbars();
      return;
    }
    const next = await clipboardService.list({
      query: query.value.trim() || null,
      kind: scope.value === "text" || scope.value === "image" || scope.value === "files" ? scope.value : null,
      pinnedOnly: scope.value === "pinned",
      limit: 50,
    });
    if (id !== request) return;
    items.value = next;
    snippets.value = [];
    selectedIndex.value = Math.min(selectedIndex.value, Math.max(0, next.length - 1));
    await nextTick();
    updateScrollbars();
    void loadThumbnails(next);
  } catch (reason) {
    if (id === request) error.value = reason instanceof Error ? reason.message : String(reason);
  } finally { if (id === request) loading.value = false; }
}

async function loadThumbnails(values: ClipboardItem[]): Promise<void> {
  const images = values.filter((item) => item.kind === "image" && !thumbnailUrls.value[item.id]).slice(0, 24);
  try {
    const loaded = await Promise.all(images.map(async (item) => [item.id, await clipboardService.imageThumbnailDataUrl(item.id, 72)] as const));
    thumbnailUrls.value = { ...thumbnailUrls.value, ...Object.fromEntries(loaded) };
  } catch (reason) { error.value = reason instanceof Error ? reason.message : String(reason); }
}

function scheduleLoad(): void {
  if (timer !== undefined) window.clearTimeout(timer);
  timer = window.setTimeout(() => void load(), 120);
}

async function refreshAccessibility(): Promise<void> {
  try { status.value = await clipboardService.status(); }
  catch (reason) { error.value = reason instanceof Error ? reason.message : String(reason); }
}

async function copy(item: ClipboardItem): Promise<void> {
  try {
    await clipboardService.copy(item.id);
    await getCurrentWindow().hide();
  } catch (reason) { error.value = reason instanceof Error ? reason.message : String(reason); }
}

async function copySnippet(item: Snippet): Promise<void> {
  try {
    await snippetService.copy(item.id);
    await getCurrentWindow().hide();
  } catch (reason) { error.value = reason instanceof Error ? reason.message : String(reason); }
}

async function copyPlain(item: ClipboardItem): Promise<void> {
  try {
    await clipboardService.copyPlain(item.id);
    await getCurrentWindow().hide();
  } catch (reason) { error.value = reason instanceof Error ? reason.message : String(reason); }
}

async function paste(item: ClipboardItem, plain = false): Promise<void> {
  try {
    await clipboardService.paste(item.id, plain);
  } catch (reason) { error.value = reason instanceof Error ? reason.message : String(reason); }
}

async function useSpecial(item: ClipboardSpecialContent): Promise<void> {
  try {
    if (item.kind === "color") await clipboardService.writeText(item.value);
    else await clipboardService.openSpecial(item.kind, item.value);
  } catch (reason) { error.value = reason instanceof Error ? reason.message : String(reason); }
}

function toggleSequenceItem(id: string): void {
  const index = sequenceIds.value.indexOf(id);
  if (index >= 0) sequenceIds.value.splice(index, 1);
  else sequenceIds.value.push(id);
}

function sequencePosition(id: string): number { return sequenceIds.value.indexOf(id) + 1; }

async function startSequence(): Promise<void> {
  try {
    sequenceStatus.value = await clipboardService.sequenceStart(sequenceIds.value);
    sequenceMode.value = false;
    sequenceIds.value = [];
    await pasteNextSequence();
  } catch (reason) { error.value = reason instanceof Error ? reason.message : String(reason); }
}

async function nextSequence(): Promise<void> {
  try {
    const step = await clipboardService.sequenceNext();
    sequenceStatus.value = step.status;
    await getCurrentWindow().hide();
  } catch (reason) { error.value = reason instanceof Error ? reason.message : String(reason); }
}

async function pasteNextSequence(): Promise<void> {
  try {
    const step = await clipboardService.sequencePasteNext();
    sequenceStatus.value = step.status;
  } catch (reason) { error.value = reason instanceof Error ? reason.message : String(reason); }
}

async function mergeSelection(pasteAfterCopy: boolean): Promise<void> {
  try {
    if (pasteAfterCopy) await clipboardService.pasteMerged(sequenceIds.value);
    else {
      await clipboardService.copyMerged(sequenceIds.value);
      await getCurrentWindow().hide();
    }
    sequenceMode.value = false;
    sequenceIds.value = [];
  } catch (reason) { error.value = reason instanceof Error ? reason.message : String(reason); }
}

async function cancelSequence(): Promise<void> {
  await clipboardService.sequenceCancel();
  sequenceStatus.value = { active: false, total: 0, nextIndex: 0, remaining: 0 };
}

async function pin(item: ClipboardItem): Promise<void> {
  try {
    await clipboardService.setPinned(item.id, !item.pinned);
    await load();
  } catch (reason) { error.value = reason instanceof Error ? reason.message : String(reason); }
}

async function remove(item: ClipboardItem): Promise<void> {
  try {
    await clipboardService.delete(item.id);
    await load();
  } catch (reason) { error.value = reason instanceof Error ? reason.message : String(reason); }
}

async function saveAsSnippet(item: ClipboardItem): Promise<void> {
  try {
    await clipboardService.saveAsSnippet(item.id);
  } catch (reason) { error.value = reason instanceof Error ? reason.message : String(reason); }
}

function move(delta: number): void {
  if (!activeCount.value) return;
  selectedIndex.value = Math.min(activeCount.value - 1, Math.max(0, selectedIndex.value + delta));
  nextTick(() => document.querySelector<HTMLElement>(`[data-panel-index="${selectedIndex.value}"]`)?.scrollIntoView({ block: "nearest" }));
}

function keyboard(event: KeyboardEvent): void {
  const command = event.metaKey || event.ctrlKey;
  if (command && event.key.toLowerCase() === "k") { event.preventDefault(); searchInput.value?.focus(); searchInput.value?.select(); }
  else if (command && event.key.toLowerCase() === "s" && selected.value?.kind === "text") { event.preventDefault(); void saveAsSnippet(selected.value); }
  else if (event.key === "ArrowDown") { event.preventDefault(); move(1); }
  else if (event.key === "ArrowUp") { event.preventDefault(); move(-1); }
  else if (event.key === "Enter" && command && sequenceStatus.value.active) { event.preventDefault(); void pasteNextSequence(); }
  else if (event.key === "Enter" && command && selected.value) { event.preventDefault(); void paste(selected.value, event.shiftKey); }
  else if (event.key === "Enter" && sequenceStatus.value.active) { event.preventDefault(); void nextSequence(); }
  else if (event.key === "Enter" && event.shiftKey && selected.value?.kind === "text") { event.preventDefault(); void copyPlain(selected.value); }
  else if (event.key === "Enter" && selected.value) { event.preventDefault(); void copy(selected.value); }
  else if (event.key === "Enter" && selectedSnippet.value) { event.preventDefault(); void copySnippet(selectedSnippet.value); }
  else if (event.key === "Escape") { event.preventDefault(); void getCurrentWindow().hide(); }
  else if (event.key === " " && event.target !== searchInput.value) { event.preventDefault(); previewVisible.value = !previewVisible.value; nextTick(updateScrollbars); }
  else if (event.key.toLowerCase() === "p" && selected.value && event.target !== searchInput.value) { event.preventDefault(); void pin(selected.value); }
  else if ((event.key === "Delete" || event.key === "Backspace") && selected.value && event.target !== searchInput.value) { event.preventDefault(); void remove(selected.value); }
}

watch(query, () => { selectedIndex.value = 0; scheduleLoad(); });
watch(scope, () => { selectedIndex.value = 0; void load(); });
watch(selected, async (item) => {
  previewExpanded.value = false;
  imageDataUrl.value = "";
  if (item?.kind === "image") {
    try { imageDataUrl.value = await clipboardService.imageDataUrl(item.id); }
    catch (reason) { error.value = reason instanceof Error ? reason.message : String(reason); }
  }
  await nextTick();
  updateScrollbars();
});
watch(selectedSnippet, () => { previewExpanded.value = false; nextTick(updateScrollbars); });

onMounted(async () => {
  window.addEventListener("keydown", keyboard);
  window.addEventListener("resize", updateScrollbars);
  const win = getCurrentWindow();
  unlisteners.push(await win.onFocusChanged(({ payload }) => {
    if (payload) {
      void Promise.all([load(), clipboardService.status().then((value) => { status.value = value; }), clipboardService.sequenceStatus().then((value) => { sequenceStatus.value = value; })]);
      nextTick(() => searchInput.value?.focus());
    }
    else void win.hide();
  }));
  unlisteners.push(await listen("clipboard-history-updated", () => {
    void load();
    void clipboardService.status().then((value) => { status.value = value; });
  }));
  await Promise.all([load(), clipboardService.status().then((value) => { status.value = value; }), clipboardService.sequenceStatus().then((value) => { sequenceStatus.value = value; })]);
  searchInput.value?.focus();
});
onUnmounted(() => {
  if (timer !== undefined) window.clearTimeout(timer);
  window.removeEventListener("keydown", keyboard);
  window.removeEventListener("resize", updateScrollbars);
  for (const stop of unlisteners) stop();
});
</script>

<template>
  <main class="quick-panel">
    <header class="panel-search" data-tauri-drag-region>
      <button class="back-button" type="button" aria-label="关闭快捷面板" @click="getCurrentWindow().hide()"><ChevronLeft :size="24" /></button>
      <Search class="search-icon" :size="19" />
      <input ref="searchInput" v-model="query" type="text" placeholder="输入关键词筛选…" aria-label="搜索剪贴板历史" />
      <button v-if="query" class="clear-button" type="button" aria-label="清除搜索" @click="query = ''"><X :size="16" /></button>
      <label class="type-filter">
        <ListFilter :size="18" />
        <span>{{ scopeLabel(scope) }}</span>
        <select v-model="scope" aria-label="筛选内容类型">
          <option value="all">全部类型</option>
          <option value="text">文本</option>
          <option value="image">图片</option>
          <option value="files">文件</option>
          <option value="pinned">已收藏</option>
          <option value="snippets">固定片段</option>
        </select>
        <ChevronDown :size="16" />
      </label>
    </header>
    <p v-if="status && (!status.accessibilityGranted || !status.settings.enabled)" class="panel-notice"><ShieldAlert :size="14" /><span>{{ !status.settings.enabled ? "剪贴板记录已暂停，可在主窗口设置中恢复。" : "辅助功能权限尚未对当前运行程序生效。" }}</span><button v-if="!status.accessibilityGranted" type="button" @click="refreshAccessibility">重新检查</button></p>
    <p v-if="error" class="panel-error">{{ error }}</p>
    <div class="panel-body" :class="{ 'preview-expanded': previewExpanded, 'preview-hidden': !previewVisible }">
      <section class="panel-list">
        <div v-if="loading && !activeCount" class="panel-empty">正在读取…</div>
        <div v-else-if="!activeCount" class="panel-empty"><Clipboard :size="26" /><strong>{{ query ? "没有匹配的内容" : scope === 'snippets' ? "还没有固定片段" : "还没有剪贴板记录" }}</strong><span>{{ query ? "换个关键词试试" : scope === 'snippets' ? "在片段库中创建或收藏常用内容" : "复制内容后会自动出现在这里" }}</span></div>
        <div v-else class="list-scroll-shell">
          <div ref="listScroll" class="list-scroll" @scroll="updateScrollbars">
            <button v-for="(item,index) in items" :key="item.id" type="button" class="panel-item" :class="{ active: index === selectedIndex, sequenced: sequencePosition(item.id) > 0 }" :data-panel-index="index" @click="selectedIndex = index; if (sequenceMode) toggleSequenceItem(item.id)" @dblclick="!sequenceMode && copy(item)">
              <span v-if="sequenceMode" class="sequence-order">{{ sequencePosition(item.id) || '' }}</span>
              <span class="item-icon" :data-kind="item.kind"><img v-if="item.kind === 'image' && thumbnailUrls[item.id]" :src="thumbnailUrls[item.id]" alt="" /><component :is="icon(item.kind)" v-else :size="14" /></span>
              <span class="item-text"><strong>{{ summary(item) }}</strong><small>{{ item.sourceApp || "未知来源" }} · {{ time(item.updatedAt) }}</small></span>
              <Pin v-if="item.pinned" class="item-pin" :size="12" />
            </button>
            <button v-for="(item,index) in snippets" :key="item.id" type="button" class="panel-item" :class="{ active: index === selectedIndex }" :data-panel-index="index" @click="selectedIndex = index" @dblclick="copySnippet(item)">
              <span class="item-icon snippet"><Sparkles :size="14" /></span>
              <span class="item-text"><strong>{{ item.title }}</strong><small>{{ item.kind }} · {{ item.useCount ? `已使用 ${item.useCount} 次` : '尚未使用' }}</small></span>
              <Pin v-if="item.pinned" class="item-pin" :size="12" />
            </button>
          </div>
          <span v-if="listScrollbar.visible" class="custom-scrollbar" aria-hidden="true"><i :style="{ height: `${listScrollbar.height}px`, transform: `translateY(${listScrollbar.top}px)` }" /></span>
        </div>
      </section>

      <section class="panel-preview">
        <template v-if="selected || selectedSnippet">
          <div class="preview-area">
            <div class="preview-tools">
              <button v-for="item in specialContent" :key="item.kind + item.value" type="button" :title="item.kind === 'color' ? '复制色值' : `打开${specialLabel(item)}`" @click="useSpecial(item)"><i v-if="item.kind === 'color'" :style="{ background: item.value }" /><ExternalLink v-else :size="12" />{{ specialLabel(item) }}</button>
              <button v-if="selectedText.length > 600" type="button" :title="previewExpanded ? '退出展开预览' : '展开大文本预览'" @click="previewExpanded = !previewExpanded"><Minimize2 v-if="previewExpanded" :size="13" /><Maximize2 v-else :size="13" />{{ previewExpanded ? "收起" : "展开" }}</button>
            </div>
            <div ref="previewScroll" class="preview-media" @scroll="updateScrollbars">
              <pre v-if="selected?.kind === 'text'">{{ selected.content }}</pre>
              <div v-else-if="selected?.kind === 'image'" class="panel-image"><img v-if="imageDataUrl" :src="imageDataUrl" alt="图片预览" @load="updateScrollbars" /></div>
              <ul v-else-if="selected?.kind === 'files'"><li v-for="path in filePaths" :key="path"><File :size="16" /><span>{{ path.split(/[\\/]/).pop() }}</span></li></ul>
              <pre v-else>{{ selectedSnippet?.content }}</pre>
            </div>
            <span v-if="previewScrollbar.visible" class="custom-scrollbar" aria-hidden="true"><i :style="{ height: `${previewScrollbar.height}px`, transform: `translateY(${previewScrollbar.top}px)` }" /></span>
          </div>
          <section v-if="selected" class="preview-information">
            <h2>信息</h2>
            <dl>
              <div><dt>来源</dt><dd><span class="source-mark"><Clipboard :size="12" /></span>{{ selected.sourceApp || "未知来源" }}</dd></div>
              <div><dt>类型</dt><dd>{{ label(selected.kind) }}</dd></div>
              <div v-if="selected.kind === 'image'"><dt>尺寸</dt><dd>{{ selected.imageWidth }} × {{ selected.imageHeight }}</dd></div>
              <div v-if="selected.kind === 'files'"><dt>文件数</dt><dd>{{ filePaths.length }}</dd></div>
              <div><dt>大小</dt><dd>{{ formatBytes(selected.sizeBytes) }}</dd></div>
              <div><dt>复制于</dt><dd>{{ fullTime(selected.updatedAt) }}</dd></div>
            </dl>
          </section>
          <section v-else-if="selectedSnippet" class="preview-information">
            <h2>信息</h2>
            <dl><div><dt>来源</dt><dd><span class="source-mark"><Sparkles :size="12" /></span>固定片段</dd></div><div><dt>类型</dt><dd>{{ selectedSnippet.kind }}</dd></div><div><dt>使用</dt><dd>{{ selectedSnippet.useCount }} 次</dd></div></dl>
          </section>
        </template>
        <div v-else class="preview-placeholder"><Clipboard :size="30" /><span>选择一条记录查看完整内容</span></div>
      </section>
    </div>
    <footer class="panel-footer">
      <span class="panel-brand"><Clipboard :size="15" />剪贴板历史</span>
      <div class="footer-actions" v-if="sequenceStatus.active || selected || selectedSnippet">
        <template v-if="sequenceStatus.active">
          <button class="copy-action" type="button" @click="nextSequence">序列下一条 {{ sequenceStatus.nextIndex + 1 }}/{{ sequenceStatus.total }} <CornerDownLeft :size="15" /></button>
          <button class="icon-action" type="button" title="粘贴序列下一条 · ⌘/Ctrl Enter" aria-label="粘贴序列下一条" @click="pasteNextSequence"><ClipboardPaste :size="16" /></button>
          <button class="icon-action" type="button" aria-label="取消粘贴序列" @click="cancelSequence"><X :size="16" /></button>
        </template>
        <template v-else-if="sequenceMode">
          <button class="copy-action" type="button" :disabled="sequenceIds.length < 2" @click="startSequence">开始序列·{{ sequenceIds.length }} 条 <CornerDownLeft :size="15" /></button>
          <button v-if="mergeableSelection" class="icon-action" type="button" title="合并复制" aria-label="合并复制" @click="mergeSelection(false)"><AlignLeft :size="16" /></button>
          <button v-if="mergeableSelection" class="icon-action" type="button" title="合并并粘贴" aria-label="合并并粘贴" @click="mergeSelection(true)"><ClipboardPaste :size="16" /></button>
          <button class="icon-action" type="button" aria-label="退出序列选择" @click="sequenceMode = false; sequenceIds = []"><X :size="16" /></button>
        </template>
        <button v-else-if="selectedSnippet" class="copy-action" type="button" @click="copySnippet(selectedSnippet)">复制片段 <CornerDownLeft :size="15" /></button>
        <button v-else-if="selected" class="copy-action" type="button" @click="copy(selected)">复制到剪贴板 <CornerDownLeft :size="15" /></button>
        <button v-if="selected && !sequenceMode && !sequenceStatus.active" class="icon-action" type="button" title="粘贴到当前应用 · ⌘/Ctrl Enter" aria-label="粘贴到当前应用" @click="paste(selected)"><ClipboardPaste :size="16" /></button>
        <button v-if="selected?.kind === 'text' && !sequenceMode && !sequenceStatus.active" class="icon-action" type="button" title="纯文本复制 · ⇧ Enter；纯文本粘贴 · ⌘/Ctrl ⇧ Enter" aria-label="纯文本复制" @click="copyPlain(selected)"><AlignLeft :size="16" /></button>
        <button v-if="selected && !sequenceMode && !sequenceStatus.active" class="icon-action" type="button" title="创建粘贴序列" aria-label="创建粘贴序列" @click="sequenceMode = true; toggleSequenceItem(selected.id)"><ListOrdered :size="16" /></button>
        <button v-if="selected?.kind === 'text' && !sequenceMode && !sequenceStatus.active" class="icon-action" type="button" title="保存为片段 · ⌘/Ctrl S" aria-label="保存为片段" @click="saveAsSnippet(selected)"><BookmarkPlus :size="16" /></button>
        <button v-if="selected && !sequenceMode && !sequenceStatus.active" class="icon-action" type="button" :aria-label="selected.pinned ? '取消收藏' : '收藏'" @click="pin(selected)"><Pin :size="16" /></button>
        <button v-if="selected && !sequenceMode && !sequenceStatus.active" class="icon-action danger" type="button" aria-label="删除当前记录" @click="remove(selected)"><Trash2 :size="16" /></button>
        <span class="shortcut-hint"><Command :size="13" /> K</span>
        <MoreHorizontal :size="18" />
      </div>
    </footer>
  </main>
</template>

<style scoped>
.quick-panel { --panel:color-mix(in srgb,var(--surface) 92%,transparent); position:relative; display:flex; width:100vw; height:100vh; flex-direction:column; overflow:hidden; border:1px solid color-mix(in srgb,var(--border-strong) 74%,transparent); border-radius:16px; background:var(--panel); box-shadow:inset 0 1px rgba(255,255,255,.35); color:var(--text); font-family:var(--font-sans); backdrop-filter:blur(28px) saturate(1.2); -webkit-backdrop-filter:blur(28px) saturate(1.2); }
.panel-grain { position:absolute; inset:0; opacity:.025; background-image:url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='70' height='70'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='.9' numOctaves='2'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)'/%3E%3C/svg%3E"); pointer-events:none; }
.panel-dragbar { position:relative; z-index:2; display:flex; min-height:30px; flex:0 0 30px; align-items:center; justify-content:center; border-bottom:1px solid color-mix(in srgb,var(--border) 70%,transparent); color:var(--text-subtle); cursor:grab; user-select:none; }.panel-dragbar:active { cursor:grabbing; }.panel-dragbar span { display:flex; align-items:center; gap:6px; pointer-events:none; font-size:.6rem; font-weight:700; letter-spacing:.02em; }.panel-dragbar button { position:absolute; right:8px; display:grid; width:22px; height:22px; place-items:center; padding:0; border:0; border-radius:6px; background:transparent; color:var(--text-subtle); cursor:pointer; }.panel-dragbar button:hover { background:var(--surface-2); color:var(--text); }
.panel-search { position:relative; z-index:1; display:flex; height:50px; flex:0 0 50px; align-items:center; gap:9px; margin:0 12px; border-bottom:1px solid var(--border); color:var(--text-subtle); }.panel-search input { min-width:0; flex:1; border:0; outline:0; background:transparent; color:var(--text); font:inherit; font-size:.86rem; }.panel-search input::placeholder { color:var(--text-subtle); }.panel-search button,.panel-footer button { display:grid; place-items:center; border:0; background:transparent; color:var(--text-subtle); cursor:pointer; }.panel-search kbd { padding:3px 6px; border:1px solid var(--border); border-radius:5px; background:var(--surface-2); color:var(--text-subtle); font:600 .58rem var(--font-sans); }
.panel-error { position:relative; z-index:1; margin:0; padding:7px 14px; border-bottom:1px solid color-mix(in srgb,var(--u-crit) 30%,var(--border)); background:color-mix(in srgb,var(--u-crit) 8%,transparent); color:var(--u-crit); font-size:.68rem; }
.panel-notice { position:relative; z-index:1; display:flex; min-height:30px; align-items:center; gap:7px; margin:0; padding:6px 20px; border-block:1px solid color-mix(in srgb,var(--u-warn) 24%,var(--border)); background:color-mix(in srgb,var(--u-warn) 6%,var(--surface)); color:var(--u-warn); font-size:10px; }.panel-notice span{min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.panel-notice button{margin-left:auto;padding:3px 6px;border:1px solid color-mix(in srgb,var(--u-warn) 34%,var(--border));border-radius:5px;background:transparent;color:inherit;font:inherit;white-space:nowrap;cursor:pointer}.panel-notice button:hover{background:color-mix(in srgb,var(--u-warn) 10%,transparent)}
.panel-body { position:relative; z-index:1; display:grid; min-height:0; flex:1; grid-template-columns:300px minmax(0,1fr); }.quick-panel.compact .panel-body { grid-template-columns:1fr; }.panel-list { display:flex; min-height:0; flex-direction:column; padding:8px 7px 7px; border-right:1px solid var(--border); }.quick-panel.compact .panel-list { border-right:0; }.list-label { display:flex; align-items:center; gap:6px; padding:3px 8px 7px; color:var(--text-subtle); font-size:.62rem; font-weight:750; letter-spacing:.06em; text-transform:uppercase; }.list-label em { font-style:normal; opacity:.65; }.list-scroll { min-height:0; flex:1; overflow:auto; }
.panel-item { display:flex; width:100%; min-height:50px; align-items:center; gap:9px; padding:6px 8px; border:1px solid transparent; border-radius:9px; background:transparent; color:var(--text); text-align:left; cursor:default; }.panel-item:hover { background:var(--surface-2); }.panel-item.active { border-color:color-mix(in srgb,var(--accent) 25%,var(--border)); background:var(--accent-weak); }.item-icon { display:grid; width:28px; height:28px; flex:0 0 28px; place-items:center; border-radius:7px; background:color-mix(in srgb,var(--brand-codex) 13%,transparent); color:var(--brand-codex); }.item-icon[data-kind="image"] { background:color-mix(in srgb,var(--u-ok) 13%,transparent); color:var(--u-ok); }.item-icon[data-kind="files"] { background:color-mix(in srgb,var(--brand-kiro) 13%,transparent); color:var(--brand-kiro); }.item-text { min-width:0; flex:1; }.item-text strong,.item-text small { display:block; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }.item-text strong { font-size:.72rem; font-weight:650; }.item-text small { margin-top:3px; color:var(--text-subtle); font-size:.6rem; }.item-pin { color:var(--u-warn); }
.panel-empty,.preview-placeholder { display:grid; flex:1; place-content:center; justify-items:center; gap:7px; padding:20px; color:var(--text-subtle); font-size:.68rem; text-align:center; }.panel-empty strong { color:var(--text-muted); font-size:.74rem; }.panel-empty span { font-size:.62rem; }.panel-preview { display:flex; min-width:0; min-height:0; flex-direction:column; }.panel-preview>header { display:flex; min-height:42px; align-items:center; gap:8px; padding:0 13px; border-bottom:1px solid var(--border); color:var(--text-subtle); }.panel-preview>header span { display:inline-flex; align-items:center; gap:4px; padding:3px 6px; border-radius:5px; background:var(--accent-weak); color:var(--accent); font-size:.6rem; font-weight:750; }.panel-preview>header small { overflow:hidden; flex:1; font-size:.6rem; text-overflow:ellipsis; white-space:nowrap; }.panel-preview>header>svg { color:var(--u-warn); }.preview-scroll { min-height:0; flex:1; overflow:auto; padding:14px; }.preview-scroll pre { margin:0; color:var(--text); font-family:var(--font-mono); font-size:.72rem; line-height:1.65; white-space:pre-wrap; word-break:break-word; }.panel-image { display:grid; width:100%; height:100%; place-items:center; border-radius:9px; background:var(--surface-2); }.panel-image img { width:100%; height:100%; object-fit:contain; border-radius:8px; }.preview-scroll ul { display:grid; gap:7px; margin:0; padding:0; list-style:none; }.preview-scroll li { display:flex; align-items:center; gap:8px; padding:9px; border:1px solid var(--border); border-radius:7px; background:var(--surface-2); color:var(--text-muted); font-size:.68rem; }
.panel-footer { position:relative; z-index:1; display:flex; min-height:38px; flex:0 0 38px; align-items:center; gap:14px; padding:0 13px; border-top:1px solid var(--border); background:color-mix(in srgb,var(--surface-2) 75%,transparent); color:var(--text-subtle); font-size:.6rem; }.panel-footer b { color:var(--text-muted); font-weight:750; }.panel-footer button { margin-left:auto; }
@media (max-width:520px) { .panel-body { grid-template-columns:1fr; }.panel-preview { display:none; }.panel-list { border-right:0; }.panel-footer span:nth-child(n+4) { display:none; } }

/* Large two-pane palette, aligned with the native clipboard-history reference. */
.quick-panel {
  --panel: color-mix(in srgb, var(--surface) 91%, var(--surface-2));
  border: 0;
  border-radius: 16px;
  background: var(--panel);
  box-shadow:none;
  backdrop-filter: blur(36px) saturate(1.35);
  -webkit-backdrop-filter: blur(36px) saturate(1.35);
}
.quick-panel::after {
  position:absolute;
  z-index:20;
  inset:0;
  border:1px solid color-mix(in srgb,var(--text) 14%,transparent);
  border-radius:inherit;
  content:"";
  pointer-events:none;
}
.panel-search {
  height: 70px;
  flex: 0 0 70px;
  gap: 12px;
  margin: 0;
  padding: 0 20px;
  border-bottom: 0;
  cursor: grab;
}
.panel-search:active { cursor: grabbing; }
.panel-search .search-icon { display: none; }
.panel-search input {
  font-size: 19px;
  font-weight: 450;
  letter-spacing: -.01em;
}
.back-button,.clear-button {
  display: grid;
  flex: none;
  place-items: center;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--text-muted);
  cursor: pointer;
}
.back-button { width: 26px; height: 36px; }
.clear-button { width: 28px; height: 28px; border-radius: 50%; background: var(--surface-2); }
.type-filter {
  position: relative;
  display: flex;
  min-width: 132px;
  height: 40px;
  flex: none;
  align-items:center;
  justify-content: flex-end;
  gap: 9px;
  color: var(--text-muted);
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
}
.type-filter select { position: absolute; inset: 0; width: 100%; cursor: pointer; opacity: 0; }
.panel-error { padding: 7px 22px; }
.panel-body { grid-template-columns: 38% minmax(0,1fr); background:var(--panel); }
.panel-list { padding:4px 12px 8px; border-right:1px solid color-mix(in srgb,var(--text) 13%,transparent); background:transparent; }
.list-label {
  padding: 4px 7px 8px;
  color: var(--text-muted);
  font-size: 12px;
  font-weight: 650;
  letter-spacing: 0;
  text-transform: none;
}
.list-label em { font-family: var(--font-mono); font-size: 10px; }
.list-scroll-shell { position:relative; min-height:0; flex:1; overflow:hidden; }
.list-scroll { height:100%; padding-right:12px; overscroll-behavior:contain; overflow-x:hidden; overflow-y:auto; }
.panel-item {
  min-height: 52px;
  gap: 10px;
  padding: 7px 9px;
  border: 0;
  border-radius: 11px;
}
.panel-item:hover { background: color-mix(in srgb,var(--text) 5%,transparent); }
.panel-item.active {
  border: 0;
  background: color-mix(in srgb,var(--text) 10%,transparent);
  box-shadow: none;
}
.item-icon { width: 31px; height: 31px; flex-basis: 31px; border-radius: 8px; }
.item-text strong { font-size: 12px; font-weight: 650; }
.item-text small { margin-top: 4px; font-size: 9.5px; }
.panel-preview { border-left: 0; background:transparent; }
.preview-area { position:relative; min-height:0; flex:1 1 58%; overflow:hidden; }
.preview-media {
  display: flex;
  width:100%;
  height:100%;
  min-height:0;
  overflow-x:hidden;
  overflow-y:auto;
  align-items:flex-start;
  justify-content: center;
  padding:18px 32px 12px 24px;
}
.preview-media pre {
  width: 100%;
  max-height:none;
  margin: 0;
  overflow:visible;
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 11px;
  line-height: 1.65;
  white-space: pre-wrap;
  word-break: break-word;
}
.preview-media ul { display:grid; width:100%; gap:7px; margin:0; padding:0; list-style:none; }
.preview-media li { display:flex; align-items:center; gap:8px; padding:9px; border:1px solid var(--border); border-radius:9px; background:var(--surface-2); color:var(--text-muted); font-size:11px; }
.panel-image {
  height: 100%;
  max-height: 270px;
  padding: 10px;
  border-radius: 15px;
  background: color-mix(in srgb,var(--surface-2) 82%,transparent);
  box-shadow: 0 10px 28px rgba(0,0,0,.08);
}
.panel-image img { object-fit: contain; border-radius: 9px; }
.preview-information { flex: 0 0 auto; padding: 0 24px 12px; }
.preview-information h2 { margin: 0 0 7px; color: var(--text-muted); font-size: 12px; font-weight: 550; }
.preview-information dl { display:grid; gap:2px; margin:0; }
.preview-information dl>div {
  display:flex;
  min-height:28px;
  align-items:center;
  justify-content:space-between;
  gap:14px;
  padding:0 9px;
  border-radius:8px;
  font-size:11px;
}
.preview-information dl>div:nth-child(odd) { background: color-mix(in srgb,var(--text) 3.5%,transparent); }
.preview-information dt { color:var(--text-subtle); }
.preview-information dd { display:flex; min-width:0; align-items:center; gap:7px; overflow:hidden; margin:0; color:var(--text); text-overflow:ellipsis; white-space:nowrap; }
.source-mark { display:grid; width:18px; height:18px; place-items:center; border-radius:5px; background:var(--accent-weak); color:var(--accent); }
.preview-placeholder { gap:10px; font-size:11px; }
.panel-footer {
  min-height: 58px;
  flex: 0 0 58px;
  justify-content: space-between;
  padding: 0 14px;
  border-top: 1px solid color-mix(in srgb,var(--border) 72%,transparent);
  background: color-mix(in srgb,var(--surface) 82%,transparent);
}
.panel-brand {
  display:inline-flex;
  align-items:center;
  gap:8px;
  padding:8px 12px;
  border:1px solid color-mix(in srgb,var(--border-strong) 68%,transparent);
  border-radius:999px;
  background:color-mix(in srgb,var(--surface) 82%,transparent);
  box-shadow:var(--shadow-sm);
  color:var(--text-muted);
  font-size:11px;
  font-weight:650;
}
.panel-brand svg { color:var(--accent); }
.footer-actions { display:flex; align-items:center; gap:6px; }
.footer-actions button { display:inline-flex; align-items:center; justify-content:center; border:0; font:inherit; cursor:pointer; }
.copy-action { min-height:34px; gap:9px; padding:0 13px; border-radius:999px!important; background:var(--text)!important; color:var(--surface)!important; font-size:11px!important; font-weight:700!important; }
.icon-action { width:32px; height:32px; border-radius:50%; background:var(--surface-2)!important; color:var(--text-muted)!important; }
.icon-action.danger:hover { color:var(--u-crit)!important; }
.shortcut-hint { display:inline-flex; align-items:center; gap:3px; margin-left:3px; color:var(--text-subtle); font-size:10px; }
.footer-actions>svg { color:var(--text-subtle); }
.list-scroll,.preview-media { scrollbar-width:none; }
.list-scroll::-webkit-scrollbar,.preview-media::-webkit-scrollbar { display:none; width:0; height:0; }
.custom-scrollbar {
  position:absolute;
  top:0;
  right:2px;
  bottom:0;
  width:6px;
  overflow:hidden;
  border-radius:999px;
  background:color-mix(in srgb,var(--text) 5%,transparent);
  pointer-events:none;
}
.preview-area>.custom-scrollbar { right:10px; }
.custom-scrollbar i {
  display:block;
  width:4px;
  margin-left:1px;
  border-radius:999px;
  background:color-mix(in srgb,var(--text) 28%,transparent);
  box-shadow:inset 0 0 0 1px color-mix(in srgb,var(--surface) 38%,transparent);
  transition:background-color .15s;
}
@media (max-width:640px) {
  .panel-body { grid-template-columns: 46% minmax(0,1fr); }
  .type-filter { min-width: 104px; }
  .type-filter span { display:none; }
  .preview-information { padding-right:14px; padding-left:14px; }
  .panel-brand { display:none; }
  .panel-footer { justify-content:flex-end; }
}

/* Quiet visual pass: solid surfaces, one divider, minimal decoration. */
.quick-panel {
  --panel:var(--surface);
  width:calc(100vw - 48px);
  height:calc(100vh - 48px);
  margin:24px;
  background:var(--surface);
  box-shadow:0 18px 40px -20px rgba(20,32,27,.38);
  backdrop-filter:none;
  -webkit-backdrop-filter:none;
}
.quick-panel::after { border-color:color-mix(in srgb,var(--text) 11%,transparent); }
.panel-search {
  height:64px;
  flex-basis:64px;
  border-bottom:1px solid var(--border);
  background:var(--surface);
}
.panel-search input { font-size:17px; font-weight:430; }
.clear-button { background:transparent; }
.type-filter { font-size:13px; font-weight:550; }
.panel-body { background:var(--surface); }
.panel-list {
  padding:8px 0 8px 12px;
  border-right-color:var(--border);
  background:var(--surface);
}
.list-scroll { padding-right:20px; }
.list-scroll-shell>.custom-scrollbar { right:1px; }
.panel-preview { background:var(--surface); }
.panel-item:hover { background:color-mix(in srgb,var(--text) 3%,transparent); }
.panel-item.active { background:color-mix(in srgb,var(--text) 7%,transparent); }
.item-icon { background:transparent; }
.item-icon[data-kind="image"],.item-icon[data-kind="files"] { background:transparent; }
.panel-image {
  max-height:280px;
  padding:0;
  border-radius:10px;
  background:transparent;
  box-shadow:none;
}
.panel-image img { border-radius:8px; }
.preview-information h2 { margin-bottom:5px; }
.preview-information dl { gap:0; }
.preview-information dl>div {
  min-height:27px;
  padding:0;
  border-radius:0;
}
.preview-information dl>div:nth-child(odd) { background:transparent; }
.source-mark { width:auto; height:auto; background:transparent; }
.panel-footer { border-top-color:var(--border); background:var(--surface); }
.panel-brand {
  padding:0 6px;
  border:0;
  border-radius:0;
  background:transparent;
  box-shadow:none;
}
.copy-action { min-height:32px; padding:0 12px; border-radius:8px!important; }
.icon-action { background:transparent!important; }
.icon-action:hover { background:var(--surface-2)!important; }
.custom-scrollbar { width:5px; background:transparent; }
.custom-scrollbar i {
  width:3px;
  margin-left:1px;
  background:color-mix(in srgb,var(--text) 22%,transparent);
  box-shadow:none;
}
.preview-information {
  display:flex;
  min-height:46px;
  align-items:center;
  padding:9px 24px 11px;
  border-top:1px solid var(--border);
}
.preview-information h2 { display:none; }
.preview-information dl {
  display:flex;
  min-width:0;
  align-items:center;
  flex-wrap:wrap;
  gap:5px 16px;
}
.preview-information dl>div {
  min-height:0;
  justify-content:flex-start;
  gap:5px;
  padding:0;
  font-size:10px;
  white-space:nowrap;
}
.preview-information dt::after { content:":"; }
.preview-information dd { color:var(--text-muted); }
.preview-information dl>div:first-child dd { color:var(--text); }
.item-icon img { width:31px; height:31px; border-radius:7px; object-fit:cover; }
.item-icon.snippet { color:var(--accent); }
.panel-item { position:relative; }
.panel-item.sequenced { background:color-mix(in srgb,var(--accent) 9%,transparent); }
.sequence-order {
  display:grid;
  width:18px;
  height:18px;
  flex:0 0 18px;
  place-items:center;
  border:1px solid var(--border-strong);
  border-radius:5px;
  color:var(--text-subtle);
  font-size:9px;
  font-weight:700;
}
.panel-item.sequenced .sequence-order { border-color:var(--accent); background:var(--accent); color:white; }
.preview-tools {
  position:absolute;
  z-index:3;
  top:10px;
  right:18px;
  display:flex;
  max-width:calc(100% - 36px);
  align-items:center;
  gap:5px;
}
.preview-tools button {
  display:inline-flex;
  height:26px;
  align-items:center;
  gap:5px;
  padding:0 8px;
  border:1px solid var(--border);
  border-radius:7px;
  background:var(--surface);
  box-shadow:var(--shadow-sm);
  color:var(--text-muted);
  font:600 9px var(--font-sans);
  cursor:pointer;
}
.preview-tools button:hover { border-color:var(--border-strong); color:var(--text); }
.preview-tools button i { width:10px; height:10px; border:1px solid color-mix(in srgb,var(--text) 18%,transparent); border-radius:3px; }
.preview-tools:not(:empty) + .preview-media { padding-top:46px; }
.panel-body.preview-expanded { grid-template-columns:0 minmax(0,1fr); }
.panel-body.preview-expanded .panel-list { visibility:hidden; padding:0; border-right:0; }
.panel-body.preview-expanded .preview-information { display:none; }
.panel-body.preview-expanded .preview-media { padding-right:42px; padding-left:42px; }
.copy-action:disabled { opacity:.42; cursor:not-allowed; }
.panel-body.preview-hidden { grid-template-columns:1fr 0; }
.panel-body.preview-hidden .panel-list { padding-right:12px; border-right:0; }
.panel-body.preview-hidden .panel-preview { visibility:hidden; overflow:hidden; }
</style>
