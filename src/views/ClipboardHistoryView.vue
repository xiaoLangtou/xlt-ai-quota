<script lang="ts" setup>
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
    AlignLeft,
    ArrowLeft,
    Check,
    ChevronUp,
    Clipboard,
    ClipboardCheck,
    Clock3,
    Copy,
    File,
    FileText,
    Image,
    LoaderCircle,
    Maximize2,
    Minimize2,
    Pause,
    Pin,
    PinOff,
    Play,
    Plus,
    Search,
    ShieldAlert,
    Trash2,
    X,
} from "lucide-vue-next";
import { DialogDescription, DialogTitle, PopoverContent, PopoverPortal, PopoverRoot, PopoverTrigger } from "reka-ui";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Dialog } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { pushToast } from "@/composables/useToast";
import { isTauriDesktop } from "@/connectors/types";
import { clipboardService } from "@/services/clipboard-service";
import type {
    ClipboardFilePreview,
    ClipboardItem,
    ClipboardKind,
    ClipboardScope,
    ClipboardStatus,
} from "@/types/clipboard";

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
const loading = ref(desktop);
const loadError = ref("");
const workspace = ref<HTMLElement | null>(null);
const searchField = ref<HTMLElement | null>(null);
const historyScroll = ref<HTMLElement | null>(null);
const copiedId = ref<string | null>(null);
let copiedTimer: number | undefined;
const imageDataUrl = ref("");
const filePreview = ref<ClipboardFilePreview | null>(null);
const filePreviewIndex = ref(0);
const filePreviewLoading = ref(false);
const filePreviewError = ref("");
const thumbnailUrls = ref<Record<string, string>>({});
const previewExpanded = ref(false);
const clearOpen = ref(false);
const clearPinned = ref(false);
const permissionOpen = ref(false);
let searchTimer: number | undefined;
let unlisten: UnlistenFn | undefined;
let unlistenFocus: UnlistenFn | undefined;
let disposed = false;
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
const displayedItems = computed(() => groups.value.flatMap((group) => group.items));
const selectedPosition = computed(() => displayedItems.value.findIndex((item) => item.id === selectedId.value) + 1);
const searchShortcut = /Mac|iPhone|iPad/.test(navigator.platform) ? "⌘ F" : "Ctrl F";
const filePaths = computed(() => {
    if (selected.value?.kind !== "files") return [];
    try {
        return JSON.parse(selected.value.content) as string[];
    } catch {
        return [];
    }
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

function scopeCount(value: ClipboardScope): number {
    if (value === "all") return status.value?.total ?? items.value.length;
    if (value === "text") return status.value?.textCount ?? 0;
    if (value === "image") return status.value?.imageCount ?? 0;
    if (value === "files") return status.value?.fileCount ?? 0;
    return status.value?.pinned ?? 0;
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

function itemTitle(item: ClipboardItem): string {
    if (item.kind === "image") return `图片 · ${item.imageWidth ?? 0} × ${item.imageHeight ?? 0}`;
    if (item.kind === "files") {
        try {
            const paths = JSON.parse(item.content) as string[];
            const name = paths[0]?.split(/[\\/]/).pop() ?? "文件";
            return paths.length > 1 ? `${name} 等 ${paths.length} 个文件` : name;
        } catch {
            return "文件";
        }
    }
    // 标题在样式上仅展示两行，无需对整段内容做正则归一，截取前缀即可，
    // 否则千行文本会在列表渲染时反复处理数十 KB 字符串。
    return item.content.slice(0, 200).replace(/\s+/g, " ").trim();
}

function formatTime(value: string, full = false): string {
    const date = new Date(value);
    if (full) return new Intl.DateTimeFormat("zh-CN", {
        month: "long",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
    }).format(date);
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

/**
 * 会话内稳定排序：复制/收藏会刷新 updatedAt，若每次都按时间重排，
 * 快速点击时条目会跳位。这里记录每个 id 首次出现时的序号，
 * 已知条目保持原相对顺序，仅让新条目置顶。
 */
const displayOrder = new Map<string, number>();
let displaySeq = 0;

function sortByStableOrder(nextItems: ClipboardItem[]): ClipboardItem[] {
    for (const item of nextItems) {
        if (!displayOrder.has(item.id)) displayOrder.set(item.id, displaySeq++);
    }
    if (displayOrder.size > nextItems.length * 3 + 500) {
        const alive = new Set(nextItems.map((item) => item.id));
        for (const key of displayOrder.keys()) if (!alive.has(key)) displayOrder.delete(key);
    }
    return [...nextItems].sort((left, right) =>
        (displayOrder.get(right.id) ?? 0) - (displayOrder.get(left.id) ?? 0));
}

async function load(keepSelection = true): Promise<void> {
    if (!desktop) return;
    const id = ++request;
    loading.value = true;
    loadError.value = "";
    try {
        const kind = ["text", "image", "files"].includes(scope.value) ? scope.value as ClipboardKind : null;
        const [nextItems, nextStatus] = await Promise.all([
            clipboardService.list({
                query: query.value.trim() || null,
                kind,
                pinnedOnly: scope.value === "pinned",
                limit: 1000,
            }),
            clipboardService.status(),
        ]);
        if (id !== request) return;
        items.value = sortByStableOrder(nextItems);
        status.value = nextStatus;
        if (!keepSelection || !nextItems.some((item) => item.id === selectedId.value)) {
            selectedId.value = displayedItems.value[0]?.id ?? null;
        }
        void loadThumbnails(nextItems);
    } catch (reason) {
        if (id !== request) return;
        loadError.value = reason instanceof Error ? reason.message : String(reason);
        pushToast(loadError.value, "error");
    } finally {
        if (id === request) loading.value = false;
    }
}

async function loadThumbnails(values: ClipboardItem[]): Promise<void> {
    const images = values.filter((item) => item.kind === "image" && !thumbnailUrls.value[item.id]).slice(0, 36);
    try {
        const loaded = await Promise.all(images.map(async (item) => [item.id, await clipboardService.imageThumbnailDataUrl(item.id, 72)] as const));
        thumbnailUrls.value = { ...thumbnailUrls.value, ...Object.fromEntries(loaded) };
    } catch (reason) {
        pushToast(reason instanceof Error ? reason.message : String(reason), "error");
    }
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

function showCopied(id: string): void {
    if (copiedTimer !== undefined) window.clearTimeout(copiedTimer);
    copiedId.value = id;
    copiedTimer = window.setTimeout(() => { copiedId.value = null; }, 1800);
}

function resetFilters(): void {
    query.value = "";
    scope.value = "all";
}

async function copy(item: ClipboardItem): Promise<void> {
    try {
        const updated = await clipboardService.copy(item.id);
        items.value = items.value.map((current) => current.id === updated.id ? updated : current);
        showCopied(item.id);
        pushToast("已复制到系统剪贴板。", "success");
    } catch (reason) {
        pushToast(reason instanceof Error ? reason.message : String(reason), "error");
    }
}

async function copyPlain(item: ClipboardItem): Promise<void> {
    try {
        const updated = await clipboardService.copyPlain(item.id);
        items.value = items.value.map((current) => current.id === updated.id ? updated : current);
        showCopied(item.id);
        pushToast("已以纯文本写入系统剪贴板。", "success");
    } catch (reason) {
        pushToast(reason instanceof Error ? reason.message : String(reason), "error");
    }
}

async function togglePin(item: ClipboardItem): Promise<void> {
    try {
        await clipboardService.setPinned(item.id, !item.pinned);
        await load();
        pushToast(item.pinned ? "已取消收藏。" : "已收藏，不参与自动清理。", "success");
    } catch (reason) {
        pushToast(reason instanceof Error ? reason.message : String(reason), "error");
    }
}

async function remove(item: ClipboardItem): Promise<void> {
    try {
        await clipboardService.delete(item.id);
        await load(false);
        pushToast("记录已删除。", "success");
    } catch (reason) {
        pushToast(reason instanceof Error ? reason.message : String(reason), "error");
    }
}

async function saveAsSnippet(item: ClipboardItem): Promise<void> {
    try {
        await clipboardService.saveAsSnippet(item.id);
        pushToast("已保存到片段库。", "success");
    } catch (reason) {
        pushToast(reason instanceof Error ? reason.message : String(reason), "error");
    }
}

async function clearHistory(): Promise<void> {
    try {
        const count = await clipboardService.clear(clearPinned.value);
        clearOpen.value = false;
        clearPinned.value = false;
        await load(false);
        pushToast(`已删除 ${count} 条记录。`, "success");
    } catch (reason) {
        pushToast(reason instanceof Error ? reason.message : String(reason), "error");
    }
}

async function refreshAccessibility(): Promise<void> {
    try {
        status.value = await clipboardService.status();
        if (status.value.accessibilityGranted) pushToast("辅助功能权限已生效。", "success");
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

async function toggleCapture(): Promise<void> {
    if (!status.value) return;
    try {
        status.value = await clipboardService.updateSettings({
            ...status.value.settings,
            enabled: !status.value.settings.enabled,
        });
        pushToast(status.value.settings.enabled ? "剪贴板采集已恢复。" : "剪贴板采集已暂停。", "success");
    } catch (reason) {
        pushToast(reason instanceof Error ? reason.message : String(reason), "error");
    }
}

async function navigate(offset: number): Promise<void> {
    const visible = displayedItems.value;
    if (!visible.length) return;
    const index = Math.max(0, visible.findIndex((item) => item.id === selectedId.value));
    selectedId.value = visible[Math.min(visible.length - 1, Math.max(0, index + offset))]?.id ?? null;
    await nextTick();
    const active = historyScroll.value?.querySelector<HTMLButtonElement>(".history-item.selected .item-select");
    active?.focus({ preventScroll: true });
    active?.scrollIntoView({ block: "nearest" });
}

function keyboard(event: KeyboardEvent): void {
    const target = event.target as HTMLElement;
    if (clearOpen.value || permissionOpen.value || target.closest('[role="dialog"]')) return;
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "f") {
        event.preventDefault();
        searchField.value?.querySelector("input")?.focus();
        return;
    }
    if (event.key === "Escape" && previewExpanded.value) {
        event.preventDefault();
        previewExpanded.value = false;
        return;
    }
    if (!workspace.value?.contains(target) || ["INPUT", "TEXTAREA"].includes(target.tagName) || target.isContentEditable) return;
    if (target.closest("button, a, [role='button']") && !target.closest(".item-select")) return;
    if (event.key === "ArrowDown") {
        event.preventDefault();
        navigate(1);
    }
    if (event.key === "ArrowUp") {
        event.preventDefault();
        navigate(-1);
    }
    if (event.key === " " && selected.value) {
        event.preventDefault();
        previewExpanded.value = !previewExpanded.value;
    }
    if (event.key === "Enter" && event.shiftKey && selected.value?.kind === "text") {
        event.preventDefault();
        void copyPlain(selected.value);
    } else if (event.key === "Enter" && selected.value) {
        event.preventDefault();
        void copy(selected.value);
    }
    if ((event.key === "Delete" || event.key === "Backspace") && selected.value) {
        event.preventDefault();
        void remove(selected.value);
    }
}

watch(query, scheduleLoad);
watch(scope, () => void load(false));
watch(() => status.value?.accessibilityGranted, (granted) => {
    if (granted) permissionOpen.value = false;
});
watch(selectedId, () => { previewExpanded.value = false; });
watch(selected, async (item) => {
    imageDataUrl.value = "";
    filePreviewRequest += 1;
    filePreview.value = null;
    filePreviewIndex.value = 0;
    filePreviewLoading.value = false;
    filePreviewError.value = "";
    if (item?.kind === "image") {
        try {
            const dataUrl = await clipboardService.imageDataUrl(item.id);
            if (selected.value?.id === item.id) imageDataUrl.value = dataUrl;
        } catch (reason) {
            pushToast(reason instanceof Error ? reason.message : String(reason), "error");
        }
    } else if (item?.kind === "files") {
        await selectFile(0);
    }
});

onMounted(async () => {
    if (!desktop) return;
    window.addEventListener("keydown", keyboard);
    void load(false);
    try {
        const stopHistory = await listen("clipboard-history-updated", () => void load());
        if (disposed) { stopHistory(); return; }
        unlisten = stopHistory;
        const stopFocus = await getCurrentWindow().onFocusChanged(({ payload }) => {
            if (payload) void refreshAccessibility();
        });
        if (disposed) stopFocus();
        else unlistenFocus = stopFocus;
    } catch (reason) {
        if (!disposed) pushToast(`自动更新暂不可用，请重新打开页面重试：${String(reason)}`, "warn");
    }
});
onUnmounted(() => {
    disposed = true;
    request += 1;
    if (searchTimer !== undefined) window.clearTimeout(searchTimer);
    if (copiedTimer !== undefined) window.clearTimeout(copiedTimer);
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
    <section v-else ref="workspace" class="clipboard-workspace">
        <section class="clipboard-card">
            <header aria-label="剪贴板历史操作" class="clipboard-toolbar">
                <div class="toolbar-identity">
                    <span :class="{ paused: status && !status.settings.enabled, pending: !status }"
                        class="capture-state" role="status"><i />{{
                            !status ? (loading ? "正在读取" : "未连接") : status.settings.enabled ? "正在记录" : "已暂停"
                        }}</span>
                </div>
                <div ref="searchField" class="search-field">
                    <Search :size="17" />
                    <Input v-model="query" aria-label="搜索剪贴板历史" placeholder="搜索内容或来源应用…" />
                    <button v-if="query" aria-label="清除搜索" type="button" @click="query = ''">
                        <X :size="15" />
                    </button>
                    <kbd v-else>{{ searchShortcut }}</kbd>
                </div>
                <div class="toolbar-actions">
                    <Button :disabled="!status" size="sm" type="button" variant="ghost" @click="toggleCapture">
                        <Pause v-if="!status || status.settings.enabled" :size="15" />
                        <Play v-else :size="15" />
                        {{ !status || status.settings.enabled ? "暂停记录" : "恢复记录" }}
                    </Button>
                    <Button :disabled="!status?.total" aria-label="清空历史" size="icon" title="清空历史" type="button"
                        variant="ghost" @click="clearOpen = true">
                        <Trash2 :size="17" />
                    </Button>
                </div>
            </header>

            <nav aria-label="内容类型筛选" class="clipboard-sidebar">
                <button v-for="item in scopes" :key="item.value" :class="{ active: scope === item.value }"
                    :aria-pressed="scope === item.value" type="button" @click="scope = item.value">
                    <component :is="item.icon" :size="15" />
                    <span>{{ item.label }}</span>
                    <small>{{ status ? scopeCount(item.value) : '–' }}</small>
                </button>

            </nav>

            <section v-if="loadError || !items.length" class="workspace-state" :aria-busy="loading">
                <div v-if="loadError" class="workspace-state-content" role="alert">
                    <ShieldAlert :size="32" />
                    <strong>暂时无法读取历史</strong>
                    <p>{{ loadError }}</p>
                    <Button size="sm" variant="outline" :disabled="loading" @click="load()">重新加载</Button>
                </div>
                <div v-else-if="loading" class="workspace-state-content" role="status">
                    <LoaderCircle :size="28" class="state-loading" />
                    <strong>正在读取剪贴板历史</strong>
                    <p>正在连接本机记录，请稍候。</p>
                </div>
                <div v-else class="workspace-state-content">
                    <Search v-if="query.trim() || scope !== 'all'" :size="32" />
                    <Pause v-else-if="status && !status.settings.enabled" :size="32" />
                    <ClipboardCheck v-else :size="32" />
                    <strong>{{ query.trim() || scope !== 'all' ? '没有匹配的记录' :
                        status && !status.settings.enabled ? '剪贴板记录已暂停' : '还没有剪贴板记录' }}</strong>
                    <p>{{ query.trim() || scope !== 'all' ? '试试其他关键词，或查看全部记录。' :
                        status && !status.settings.enabled ? '恢复记录后，新复制的内容会自动保存在这里。' :
                        '复制一段文字、一张图片或一个文件，即可在这里找回。' }}</p>
                    <Button v-if="query.trim() || scope !== 'all'" size="sm" variant="outline"
                        @click="resetFilters">查看全部记录</Button>
                    <Button v-else-if="status && !status.settings.enabled" size="sm" variant="outline" @click="toggleCapture">
                        <Play :size="14" />恢复记录
                    </Button>
                </div>
            </section>
            <div v-else :class="{ 'preview-expanded': previewExpanded }" class="clipboard-layout">
                <main class="history-pane" aria-label="历史记录" :aria-busy="loading">
                    <div ref="historyScroll" class="history-scroll">
                        <section v-for="group in groups" :key="group.key" class="history-group">
                            <header>
                                <Pin v-if="group.key === 'pinned'" :size="12" /><span>{{ group.label }}</span><small>{{
                                    group.items.length }}</small>
                            </header>
                            <article v-for="item in group.items" :key="item.id"
                                :class="{ selected: selectedId === item.id }" class="history-item">
                                <button class="item-select" type="button" :aria-pressed="selectedId === item.id"
                                    @focus="selectedId = item.id" @click="selectedId = item.id" @dblclick="copy(item)">
                                    <span :data-kind="item.kind" class="kind-mark"><img
                                            v-if="item.kind === 'image' && thumbnailUrls[item.id]"
                                            :src="thumbnailUrls[item.id]" alt="" />
                                        <component :is="kindIcon(item.kind)" v-else :size="16" />
                                    </span>
                                    <div class="item-copy"><strong>{{
                                        itemTitle(item)
                                            }}</strong><span>{{ item.sourceApp || "未知来源" }} · {{
                                                formatTime(item.updatedAt)
                                            }}<template v-if="item.copyCount > 1"> · 使用 {{
                                                item.copyCount
                                                }} 次</template></span></div>
                                </button>
                                <div class="item-actions">
                                    <button aria-label="复制" title="复制" type="button" @click.stop="copy(item)">
                                        <Check v-if="copiedId === item.id" :size="14" />
                                        <Copy v-else :size="14" />
                                    </button>
                                    <button :aria-label="item.pinned ? '取消收藏' : '收藏'"
                                        :title="item.pinned ? '取消收藏' : '收藏'" type="button"
                                        @click.stop="togglePin(item)">
                                        <PinOff v-if="item.pinned" :size="14" />
                                        <Pin v-else :size="14" />
                                    </button>
                                </div>
                                <Pin v-if="item.pinned" :size="13" class="pin-mark" />
                            </article>
                        </section>
                    </div>
                </main>

                <aside class="preview-pane" aria-label="内容预览">
                    <div class="preview-label"><span>内容预览</span><span v-if="selected">{{ selectedPosition }} / {{
                            items.length }}</span>
                    </div>
                    <div v-if="selected" class="preview-content">
                        <header class="preview-head">
                            <div class="preview-heading">
                                <span :data-kind="selected.kind" class="kind-chip">
                                    <component :is="kindIcon(selected.kind)" :size="13" />{{
                                        kindLabel(selected.kind)
                                    }}
                                </span>
                                <strong :title="itemTitle(selected)">{{ itemTitle(selected) }}</strong>
                                <span>{{ selected.kind === 'text' ? selected.content.length.toLocaleString() + ' 字符' :
                                    formatBytes(selected.sizeBytes) }}<template v-if="selected.pinned"> ·
                                        已收藏，长期保留</template></span>
                            </div>
                            <div class="preview-head-actions">

                                <button :aria-label="previewExpanded ? '收起预览' : '展开预览'"
                                    :title="previewExpanded ? '收起预览' : '展开预览'" type="button"
                                    @click="previewExpanded = !previewExpanded">
                                    <Minimize2 v-if="previewExpanded" :size="15" />
                                    <Maximize2 v-else :size="15" />
                                </button>
                                <button :aria-label="selected.pinned ? '取消收藏' : '收藏'"
                                    :title="selected.pinned ? '取消收藏' : '收藏'" type="button" @click="togglePin(selected)">
                                    <PinOff v-if="selected.pinned" :size="16" />
                                    <Pin v-else :size="16" />
                                </button>
                            </div>
                        </header>
                        <div :class="{ 'file-preview-body': selected.kind === 'files' }" class="preview-body">
                            <pre v-if="selected.kind === 'text'">{{ selected.content }}</pre>
                            <div v-else-if="selected.kind === 'image'" class="image-preview"><img v-if="imageDataUrl"
                                    :src="imageDataUrl" alt="剪贴板图片预览" /><span v-else>正在读取图片…</span></div>
                            <div v-else :class="{ 'has-file-list': filePaths.length > 1 }" class="file-preview">
                                <nav v-if="filePaths.length > 1" aria-label="选择要预览的文件" class="file-preview-list">
                                    <button v-for="(path, index) in filePaths" :key="path"
                                        :class="{ active: filePreviewIndex === index }" type="button"
                                        @click="selectFile(index)">
                                        <File :size="15" />
                                        <span>{{ path.split(/[\\/]/).pop() }}</span>
                                    </button>
                                </nav>
                                <section class="file-preview-content">
                                    <header v-if="filePreview">
                                        <div>
                                            <File :size="15" />
                                            <strong>{{ filePreview.name }}</strong>
                                        </div>
                                        <span>{{ formatBytes(filePreview.sizeBytes) }}</span>
                                    </header>
                                    <div v-if="filePreviewLoading" class="file-preview-state">正在读取文件…</div>
                                    <div v-else-if="filePreviewError" class="file-preview-state error">
                                        {{ filePreviewError }}
                                    </div>
                                    <img v-else-if="filePreview?.kind === 'image' && filePreview.dataUrl"
                                        :alt="filePreview.name" :src="filePreview.dataUrl" />
                                    <iframe v-else-if="filePreview?.kind === 'pdf' && filePreview.dataUrl"
                                        :src="filePreview.dataUrl" :title="filePreview.name" />
                                    <pre v-else-if="filePreview?.kind === 'text'">{{ filePreview.content }}</pre>
                                    <div v-else-if="filePreview" class="file-preview-state">
                                        <File :size="30" />
                                        <strong>无法预览此文件</strong><span>{{
                                            filePreview.message
                                            }}</span><small>{{ filePreview.path }}</small>
                                    </div>
                                </section>
                            </div>
                        </div>
                        <section class="preview-detail-footer">
                            <dl class="preview-meta">
                                <div>
                                    <dt>来源</dt>
                                    <dd>{{ selected.sourceApp || "未知来源" }}</dd>
                                </div>
                                <div>
                                    <dt>时间</dt>
                                    <dd>{{ formatTime(selected.updatedAt, true) }}</dd>
                                </div>
                                <div>
                                    <dt>大小</dt>
                                    <dd>{{ formatBytes(selected.sizeBytes) }}</dd>
                                </div>
                                <div v-if="selected.kind === 'image'">
                                    <dt>尺寸</dt>
                                    <dd>{{ selected.imageWidth }} × {{ selected.imageHeight }}</dd>
                                </div>
                            </dl>
                            <footer class="preview-actions">
                                <Button class="primary-copy" type="button" @click="copy(selected)">
                                    <Check v-if="copiedId === selected.id" :size="15" />
                                    <Copy v-else :size="15" />
                                    {{ copiedId === selected.id ? '已复制' : '复制内容' }}
                                    <kbd>↵</kbd>
                                </Button>
                                <Button v-if="selected.kind === 'text'" type="button" variant="outline"
                                    @click="copyPlain(selected)">
                                    <AlignLeft :size="15" />
                                    复制纯文本
                                </Button>
                                <Button v-if="selected.kind === 'text'" type="button" variant="outline"
                                    @click="saveAsSnippet(selected)">
                                    <Plus :size="15" />
                                    保存为片段
                                </Button>
                                <Button aria-label="删除记录" title="删除记录" class="delete-btn" size="icon" type="button"
                                    variant="ghost" @click="remove(selected)">
                                    <Trash2 :size="16" />
                                </Button>
                            </footer>
                        </section>
                    </div>
                    <div v-else class="preview-empty">
                        <span class="preview-empty-icon">
                            <Clipboard :size="30" />
                        </span>
                        <strong>复制过的内容，随时找回</strong>
                        <span>在左侧选择记录，查看完整内容并再次复制。</span>
                    </div>
                    <button v-if="previewExpanded" class="back-to-list" type="button" @click="previewExpanded = false">
                        <ArrowLeft :size="14" />返回记录列表 <kbd>Esc</kbd>
                    </button>
                </aside>
            </div>

            <footer class="clipboard-statusbar">
                <PopoverRoot v-if="status && !status.accessibilityGranted" v-model:open="permissionOpen">
                    <PopoverTrigger as-child>
                        <button class="permission-status" type="button" aria-label="辅助功能未授权，查看授权说明">
                            <ShieldAlert :size="14" />
                            <span>辅助功能未授权</span>
                            <ChevronUp :size="12" />
                        </button>
                    </PopoverTrigger>
                    <PopoverPortal>
                        <PopoverContent class="clipboard-permission-popover" side="top" align="start"
                            :side-offset="12" :collision-padding="16" aria-label="辅助功能权限">
                            <div class="permission-details">
                                <strong>开启辅助功能权限</strong>
                                <p>在“系统设置 → 隐私与安全性 → 辅助功能”中允许当前运行程序，返回此窗口后会自动检查。</p>
                                <div v-if="status.accessibilityTarget" class="permission-target">
                                    <span>当前运行程序</span>
                                    <code>{{ status.accessibilityTarget }}</code>
                                </div>
                                <div class="permission-actions">
                                    <Button size="sm" type="button" @click="requestAccessibility">去授权</Button>
                                    <Button size="sm" type="button" variant="outline" @click="refreshAccessibility">重新检查</Button>
                                </div>
                            </div>
                        </PopoverContent>
                    </PopoverPortal>
                </PopoverRoot>
                <span v-else-if="items.length && !loadError" class="keyboard-hints"><span><kbd>↑</kbd><kbd>↓</kbd> 切换</span><span><kbd>↵</kbd>
                        复制</span><span><kbd>Space</kbd> 展开预览</span></span>
                <span v-else class="local-note">历史记录仅保存在本机</span>
                <span v-if="status" class="retention-note" title="收藏记录不参与自动清理">
                    <Clock3 :size="13" />未收藏保留 {{
                        status?.settings.ttlDays ?? 3
                    }} 天 · 上限 {{ status?.settings.maxItems ?? 100 }} 条
                </span>
                <span v-if="status?.settings.shortcut" class="panel-shortcut" title="在任意应用中打开剪贴板快捷面板">快捷面板 <kbd>{{
                    shortcutLabel(status?.settings.shortcut ??
                        "CommandOrControl+Shift+V") }}</kbd></span>
            </footer>
        </section>


        <Dialog v-model:open="clearOpen" content-class="clear-dialog">
            <DialogTitle class="dialog-title">清空剪贴板历史？</DialogTitle>
            <DialogDescription class="dialog-desc">将删除 {{ unpinnedCount }} 条未收藏记录，此操作无法撤销。
            </DialogDescription>
            <label class="clear-pinned">
                <Checkbox v-model="clearPinned" />
                <span>同时删除 {{ status?.pinned ?? 0 }} 条收藏记录</span>
            </label>
            <footer class="dialog-actions">
                <Button type="button" variant="outline" @click="clearOpen = false">取消</Button>
                <Button type="button" variant="destructive" @click="clearHistory">确认清空</Button>
            </footer>
        </Dialog>
    </section>
</template>

<style scoped>
.clipboard-workspace {
    display: flex;
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    color: var(--text);
    container: clipboard / inline-size;
}

.clipboard-card {
    display: flex;
    width: 100%;
    min-height: 0;
    flex: 1;
    flex-direction: column;
    overflow: hidden;
    background: transparent;
}

.clipboard-toolbar {
    display: grid;
    min-height: 64px;
    flex: 0 0 auto;
    grid-template-columns: minmax(200px, 360px) minmax(0, 1fr) auto;
    align-items: center;
    gap: 12px;
    padding: 14px 24px 10px;
}

.toolbar-identity,
.toolbar-actions {
    display: flex;
    align-items: center;
}

.toolbar-identity {
    grid-column: 2;
    grid-row: 1;
    min-width: 0;
    justify-self: end;
}

.toolbar-actions {
    grid-column: 3;
    grid-row: 1;
    justify-content: flex-end;
    gap: 4px;
}

.capture-state {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 0 8px;
    color: var(--u-ok);
    font-size: 11px;
    font-weight: 550;
    white-space: nowrap;
}

.capture-state i {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: currentColor;
    opacity: .85;
}

.capture-state.paused {
    color: var(--u-warn);
}

.search-field {
    display: flex;
    grid-column: 1;
    grid-row: 1;
    width: 100%;
    min-width: 0;
    height: 36px;
    align-items: center;
    gap: 8px;
    padding: 0 10px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: transparent;
    color: var(--text-muted);
    transition: border-color .15s, box-shadow .15s;
}

.search-field:focus-within {
    border-color: var(--accent);
    background: var(--surface);
    box-shadow: 0 0 0 3px var(--accent-weak);
}

.search-field .cn-input {
    min-width: 0;
    height: 32px;
    border: 0;
    background: transparent;
    box-shadow: none;
    padding: 0;
    font-size: 13px;
}

.search-field button {
    display: grid;
    place-items: center;
    padding: 4px;
    border: 0;
    background: transparent;
    color: var(--text-subtle);
    cursor: pointer;
}

.permission-status {
    display: inline-flex;
    min-height: 26px;
    flex: 0 0 auto;
    align-items: center;
    gap: 6px;
    padding: 0;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--text-muted);
    font: inherit;
    cursor: pointer;
}

.permission-status>svg:first-child {
    color: var(--u-warn);
}

.permission-status:hover,
.permission-status[data-state="open"] {
    color: var(--text);
}

:global(.clipboard-permission-popover) {
    z-index: 60;
    width: min(360px, calc(100vw - 32px));
    max-height: var(--reka-popover-content-available-height);
    overflow-y: auto;
    padding: 16px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface);
    box-shadow: 0 8px 24px rgb(0 0 0 / 12%);
    color: var(--text);
}

.permission-details {
    display: grid;
    gap: 12px;
}

.permission-details>strong {
    font-size: 13px;
    font-weight: 600;
}

.permission-details p {
    margin: 0;
    color: var(--text-muted);
    font-size: 12px;
    line-height: 1.75;
}

.permission-target {
    display: grid;
    min-width: 0;
    gap: 5px;
    color: var(--text-subtle);
    font-size: 11px;
}

.permission-target code {
    color: var(--text-muted);
    font-family: var(--font-mono);
    line-height: 1.6;
    overflow-wrap: anywhere;
    user-select: text;
}

.permission-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
}

.clipboard-sidebar {
    display: flex;
    min-width: 0;
    flex: 0 0 auto;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
    padding: 0 24px 12px;
}

/* 原型风格：类型筛选做成药丸（active = accent-soft 底 + accent 文字）。 */
.clipboard-sidebar button {
    display: inline-flex;
    height: 28px;
    align-items: center;
    gap: 6px;
    padding: 0 12px;
    border: 1px solid transparent;
    border-radius: 20px;
    background: transparent;
    color: var(--text-muted);
    font: inherit;
    font-size: 12.5px;
    font-weight: 500;
    cursor: pointer;
    transition: background-color .18s, color .18s;
}

.clipboard-sidebar button:hover {
    background: var(--surface-2);
    color: var(--text);
}

.clipboard-sidebar button.active {
    background: var(--accent-weak);
    color: var(--accent);
    font-weight: 600;
}

.sidebar-foot {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-left: auto;
    padding-right: 4px;
    color: var(--text-subtle);
    font-size: 11px;
    white-space: nowrap;
}

.clipboard-layout {
    display: grid;
    min-height: 0;
    flex: 1;
    grid-template-columns: minmax(280px, 34%) minmax(0, 1fr);
    overflow: hidden;
}

.history-pane {
    display: flex;
    min-width: 0;
    min-height: 0;
    flex-direction: column;
    border-right: 1px solid var(--border);
}

.history-scroll {
    min-height: 0;
    flex: 1;
    overflow-x: hidden;
    overflow-y: auto;
    overscroll-behavior: contain;
    scrollbar-gutter: stable;
    scrollbar-color: color-mix(in srgb, var(--text) 22%, transparent) transparent;
    scrollbar-width: thin;
}

.history-scroll::-webkit-scrollbar,
.preview-body::-webkit-scrollbar {
    width: 8px;
}

.history-scroll::-webkit-scrollbar-track,
.preview-body::-webkit-scrollbar-track {
    background: transparent;
}

.history-scroll::-webkit-scrollbar-thumb,
.preview-body::-webkit-scrollbar-thumb {
    border: 2px solid transparent;
    border-radius: 999px;
    background: color-mix(in srgb, var(--text) 22%, transparent);
    background-clip: padding-box;
}

.history-group>header {
    display: flex;
    position: sticky;
    top: 0;
    z-index: 1;
    align-items: center;
    gap: 6px;
    padding: 10px 20px;
    background: var(--bg);
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 550;
}

.history-group>header::after {
    content: "";
    height: 1px;
    flex: 1;
    margin-left: 6px;
    background: var(--border);
}

.history-group>header small {
    font-size: 10px;
    color: var(--text-subtle);
}

.history-item {
    position: relative;
    display: flex;
    min-height: 78px;
    align-items: center;
    margin: 0 10px 9px;
    border: 1px solid var(--border);
    border-radius: 11px;
    background: var(--surface);
    content-visibility: auto;
    contain-intrinsic-size: 78px;
    transition: border-color .18s, background-color .18s;
}

.item-select {
    display: flex;
    flex: 1;
    min-width: 0;
    min-height: 76px;
    align-items: center;
    gap: 12px;
    padding: 12px 38px 12px 12px;
    border: 0;
    border-radius: inherit;
    background: transparent;
    color: inherit;
    text-align: left;
    cursor: pointer;
}

.history-item:hover {
    border-color: var(--border-strong);
}

.history-item.selected {
    border-color: var(--accent);
    background: var(--accent-weak);
}

.kind-mark {
    display: grid;
    width: 40px;
    height: 40px;
    flex: 0 0 40px;
    place-items: center;
    border-radius: 10px;
    background: var(--accent-weak);
    color: var(--accent);
}

.kind-mark[data-kind="image"] {
    background: color-mix(in srgb, var(--u-ok) 12%, var(--surface));
    color: var(--u-ok);
}

.kind-mark[data-kind="files"] {
    background: color-mix(in srgb, var(--u-warn) 10%, var(--surface));
    color: var(--u-warn);
}

.item-copy {
    min-width: 0;
    flex: 1;
}

.item-copy strong {
    display: -webkit-box;
    overflow: hidden;
    color: var(--text);
    font-family: var(--font-mono);
    font-size: 12.5px;
    font-weight: 500;
    line-height: 1.5;
    word-break: break-all;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
}

.item-copy span {
    display: block;
    margin-top: 5px;
    overflow: hidden;
    color: var(--text-muted);
    font-size: 11px;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.pin-mark {
    flex: 0 0 auto;
    color: var(--u-warn);
}

.item-actions {
    position: absolute;
    right: 7px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    opacity: 0;
    transition: opacity .13s;
}

.history-item:hover .item-actions,
.history-item:focus-within .item-actions,
.history-item.selected .item-actions {
    opacity: 1;
}

.item-actions button {
    display: grid;
    width: 28px;
    height: 28px;
    place-items: center;
    padding: 0;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--text-subtle);
    cursor: pointer;
}

.item-actions button:hover {
    background: var(--surface);
    color: var(--accent);
}

.history-item .pin-mark {
    position: absolute;
    right: 14px;
    display: none;
    pointer-events: none;
}

.history-item:not(:hover):not(:focus-within):not(.selected) .pin-mark {
    display: block;
}

.preview-pane {
    display: flex;
    min-width: 0;
    min-height: 0;
    flex-direction: column;
    padding: 0 24px 20px;
}

.preview-content {
    display: flex;
    flex: 1;
    min-height: 0;
    flex-direction: column;
}

.preview-head {
    display: flex;
    min-height: 48px;
    flex: 0 0 auto;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 18px;
    padding: 0;
}

.preview-head button {
    display: grid;
    width: 32px;
    height: 32px;
    place-items: center;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--text-subtle);
    cursor: pointer;
}

.preview-head button:hover {
    background: var(--surface-3);
    color: var(--text);
}

.kind-chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 3px 7px;
    border-radius: 5px;
    background: var(--surface-2);
    color: var(--u-ok);
    font-size: 11px;
    font-weight: 550;
}

.kind-chip[data-kind="files"] {
    color: var(--u-warn);
}

.kind-chip[data-kind="text"] {
    color: var(--accent);
}

.preview-body {
    min-height: 0;
    flex: 1;
    overflow-x: hidden;
    overflow-y: auto;
    padding: 8px 0;
    scrollbar-color: color-mix(in srgb, var(--text) 22%, transparent) transparent;
    scrollbar-width: thin;
}

.preview-body pre {
    min-height: 100%;
    margin: 0;
    color: var(--text);
    font-family: var(--font-mono);
    font-size: 13px;
    line-height: 1.85;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
}

.image-preview {
    display: grid;
    height: 100%;
    min-height: 0;
    place-items: center;
    color: var(--text-muted);
}

.image-preview img {
    display: block;
    width: 100%;
    height: 100%;
    min-height: 0;
    max-height: 100%;
    object-fit: contain;
    border-radius: 7px;
}

.preview-body.file-preview-body {
    overflow: hidden;
    padding: 0;
    border: 0;
    background: transparent;
}

.file-preview {
    display: grid;
    width: 100%;
    height: 100%;
    min-width: 0;
    min-height: 0;
}

.file-preview.has-file-list {
    grid-template-columns: minmax(100px, 30%) minmax(0, 1fr);
}

.file-preview-list {
    min-width: 0;
    overflow-x: hidden;
    overflow-y: auto;
    padding: 8px;
}

.file-preview-list button {
    display: flex;
    width: 100%;
    height: 38px;
    align-items: center;
    gap: 8px;
    padding: 0 9px;
    border: 0;
    border-radius: 7px;
    background: transparent;
    color: var(--text-muted);
    font: inherit;
    cursor: pointer;
}

.file-preview-list button:hover {
    background: var(--surface-2);
}

.file-preview-list button.active {
    background: var(--accent-weak);
    color: var(--accent);
}

.file-preview-list span {
    overflow: hidden;
    font-size: .68rem;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.file-preview-content {
    display: flex;
    min-width: 0;
    min-height: 0;
    flex-direction: column;
    overflow: auto;
}

.file-preview-content>header {
    display: flex;
    min-height: 40px;
    flex: 0 0 auto;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 0 14px;
    color: var(--text-subtle);
}

.file-preview-content>header div {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 7px;
}

.file-preview-content>header strong {
    overflow: hidden;
    color: var(--text-muted);
    font-size: .68rem;
    font-weight: 620;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.file-preview-content>header span {
    flex: none;
    font-size: .62rem;
}

.file-preview-content>img {
    display: block;
    width: 100%;
    height: 100%;
    min-height: 0;
    padding: 16px;
    object-fit: contain;
}

.file-preview-content>iframe {
    width: 100%;
    min-height: 420px;
    flex: 1;
    border: 0;
    background: var(--surface-2);
}

.file-preview-content>pre {
    flex: 1;
    margin: 0;
    padding: 16px 18px;
    color: var(--text);
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 1.75;
    white-space: pre-wrap;
    word-break: break-word;
}

.file-code-preview {
    flex: 1;
    padding: 16px 18px;
}

.file-preview-state {
    display: grid;
    min-height: 220px;
    flex: 1;
    place-content: center;
    justify-items: center;
    gap: 7px;
    padding: 28px;
    color: var(--text-subtle);
    font-size: .72rem;
    text-align: center;
}

.file-preview-state strong {
    color: var(--text-muted);
    font-size: .78rem;
}

.file-preview-state span {
    max-width: 360px;
    line-height: 1.5;
}

.file-preview-state small {
    max-width: 420px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.file-preview-state.error {
    color: var(--u-crit);
}

.preview-meta {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    align-items: start;
    gap: 14px;
    margin: 0 0 16px;
    padding: 0;
    color: var(--text-muted);
}

.preview-meta:has(> div:nth-child(4)) {
    grid-template-columns: repeat(4, minmax(0, 1fr));
}

.preview-meta div {
    display: grid;
    min-width: 0;
    gap: 5px;
}

.preview-meta dt,
.preview-meta dd {
    overflow: hidden;
    margin: 0;
    font-size: 11px;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.preview-meta dd {
    color: var(--text);
    font-weight: 500;
}

.preview-actions {
    display: flex;
    min-height: 40px;
    flex: 0 0 auto;
    align-items: center;
    gap: 8px;
    margin-top: 0;
    padding-top: 14px;
    border-top: 1px solid var(--border);
}

.preview-actions .cn-button {
    height: 36px;
    padding: 0 12px;
    border-radius: 7px;
    font-size: 12px;
    font-weight: 550;
}

.preview-actions .cn-button[data-variant="outline"] {
    border: 1px solid var(--border);
    background: var(--surface);
}

.preview-actions .cn-button[data-variant="outline"]:hover {
    border-color: var(--border-strong);
    background: var(--surface-2);
}

.preview-actions .delete-btn {
    margin-left: auto;
    width: 40px;
    padding: 0;
    color: var(--u-crit);
}

.preview-actions .delete-btn:hover {
    background: color-mix(in srgb, var(--u-crit) 8%, var(--surface));
}

.preview-empty,
.center-state {
    display: grid;
    height: 100%;
    place-content: center;
    justify-items: center;
    padding: 28px;
    color: var(--text-subtle);
    font-size: .78rem;
    text-align: center;
}

.empty-state .empty-icon {
    display: grid;
    width: 52px;
    height: 52px;
    place-items: center;
    margin-bottom: 12px;
    border-radius: 16px;
    background: var(--accent-weak);
    color: var(--accent);
}

.empty-state strong {
    color: var(--text);
    font-size: .9rem;
}

.empty-state p {
    max-width: 240px;
    margin: 7px 0 0;
    line-height: 1.55;
}

.clipboard-statusbar {
    display: flex;
    min-height: 42px;
    flex: 0 0 auto;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 18px;
    padding: 8px 24px;
    border-top: 1px solid var(--border);
    color: var(--text-muted);
    font-size: 11px;
}

.clipboard-statusbar .retention-note {
    display: flex;
    align-items: center;
    gap: 5px;
    margin-left: auto;
}

.clipboard-statusbar .retention-note {
    white-space: nowrap;
}

.dialog-title {
    margin: 0;
    color: var(--text);
    font-size: 1.05rem;
}

.dialog-desc {
    margin: 6px 0 18px;
    color: var(--text-muted);
    font-size: .78rem;
}

.settings-form {
    display: grid;
    gap: 16px;
}

.settings-form>label,
.settings-grid label {
    display: grid;
    gap: 6px;
    color: var(--text);
    font-size: .76rem;
    font-weight: 650;
}

.settings-form small {
    color: var(--text-subtle);
    font-size: .67rem;
    font-weight: 400;
}

.toggle-row {
    display: flex !important;
    align-items: center;
    justify-content: space-between;
    padding: 12px;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: var(--surface-2);
}

.toggle-row div {
    display: grid;
    gap: 3px;
}

.toggle-row span {
    color: var(--text-muted);
    font-size: .69rem;
    font-weight: 400;
}

.settings-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
}

.settings-form footer,
.dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding-top: 4px;
}

.clear-pinned {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 14px 0 18px;
    color: var(--text-muted);
    font-size: .78rem;
}

.shortcut-recorder {
    display: flex;
    width: 100%;
    min-height: 44px;
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
    transition: border-color .15s, box-shadow .15s, background .15s;
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

.shortcut-recorder.recording>svg {
    color: var(--accent);
}

.shortcut-recorder>span {
    color: var(--accent);
    font-weight: 650;
}

.shortcut-recorder kbd {
    padding: 4px 8px;
    border: 1px solid var(--border-strong);
    border-radius: 6px;
    background: var(--surface);
    box-shadow: 0 1px 0 var(--border-strong);
    color: var(--text);
    font-family: var(--font-sans);
    font-size: .75rem;
    font-weight: 700;
}

.shortcut-recorder em {
    margin-left: auto;
    color: var(--text-subtle);
    font-size: .66rem;
    font-style: normal;
    font-weight: 500;
}

.settings-form small.shortcut-error {
    color: var(--u-crit);
}

.settings-permission {
    display: flex;
    align-items: flex-start;
    gap: 9px;
    padding: 11px 12px;
    border: 1px solid color-mix(in srgb, var(--u-warn) 28%, var(--border));
    border-radius: 9px;
    background: color-mix(in srgb, var(--u-warn) 6%, var(--surface));
    color: var(--u-warn);
}

.settings-permission>span {
    display: grid;
    min-width: 0;
    gap: 3px;
}

.settings-permission strong {
    font-size: .75rem;
}

.settings-permission small {
    overflow: hidden;
    color: var(--text-muted);
    font-family: var(--font-mono);
    line-height: 1.5;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.settings-permission :deep(.cn-button) {
    margin-left: auto;
    flex: 0 0 auto;
}

.unavailable {
    display: grid;
    min-height: 420px;
    place-content: center;
    justify-items: center;
    gap: 9px;
    color: var(--text-subtle);
    text-align: center;
}

.unavailable strong {
    color: var(--text);
}

.unavailable p {
    max-width: 420px;
    margin: 0;
    font-size: .8rem;
    line-height: 1.6;
}

:global(.clipboard-settings-dialog) {
    width: min(560px, calc(100vw - 32px));
    padding: 22px;
}

:global(.clear-dialog) {
    width: min(420px, calc(100vw - 32px));
    padding: 22px;
}

.preview-head-actions {
    flex: 0 0 auto;
}

.kind-mark img {
    width: 40px;
    height: 40px;
    border-radius: 10px;
    object-fit: cover;
}

.preview-head-actions {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 5px;
}

.preview-head .special-action {
    display: inline-flex;
    width: auto;
    height: 28px;
    gap: 5px;
    padding: 0 8px;
    border: 1px solid var(--border);
    font-size: 10px;
}

.preview-head .special-action i {
    width: 10px;
    height: 10px;
    border: 1px solid var(--border-strong);
    border-radius: 3px;
}

.clipboard-layout.preview-expanded {
    grid-template-columns: 1fr;
}

.clipboard-layout.preview-expanded .history-pane {
    display: none;
}

.clipboard-layout.preview-expanded .preview-pane {
    grid-column: 1;
    grid-row: 1;
}

.clipboard-layout.preview-expanded .preview-body {
    padding-right: 44px;
    padding-left: 44px;
}

/* 分组时间线与独立阅读区，沿用工作台的亮暗主题。 */
.preview-label {
    display: flex;
    flex: 0 0 auto;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 600;
}

.preview-label>span:last-child:not(:first-child) {
    color: var(--text-subtle);
    font-size: 11px;
    font-weight: 400;
}

.preview-label {
    min-height: 48px;
    margin-bottom: 12px;
}

.preview-heading {
    display: grid;
    min-width: 0;
    gap: 5px;
    grid-template-columns: auto minmax(0, 1fr);
    align-items: center;
}

.preview-heading>strong {
    overflow: hidden;
    font-size: 14px;
    font-weight: 650;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.preview-heading>span:last-child {
    grid-column: 1 / -1;
    color: var(--text-muted);
    font-size: 11px;
}

.preview-detail-footer {
    flex: 0 0 auto;
    margin-top: 16px;
}

.preview-empty {
    height: auto;
    min-height: 0;
    flex: 1;
    gap: 12px;
}

.preview-empty strong {
    color: var(--text-muted);
    font-size: 14px;
    font-weight: 600;
}

.preview-empty>span:last-child {
    max-width: 260px;
    line-height: 1.8;
}

.preview-empty-icon {
    display: grid;
    width: 72px;
    height: 80px;
    place-items: center;
    color: var(--text-subtle);
    margin-bottom: 12px;
}

.back-to-list {
    display: flex;
    align-items: center;
    align-self: flex-start;
    gap: 8px;
    margin-top: 14px;
    padding: 6px 0;
    border: 0;
    background: transparent;
    color: var(--accent);
    font-size: 12px;
    cursor: pointer;
}

.clipboard-sidebar button small {
    min-width: 12px;
    background: transparent;
    color: var(--text-subtle);
    font-family: var(--font-mono);
    font-size: 11px;
    opacity: .75;
    font-variant-numeric: tabular-nums;
}

.clipboard-sidebar button.active small {
    background: transparent;
    color: var(--accent);
    opacity: .9;
}

.workspace-state {
    display: grid;
    flex: 1;
    min-height: 0;
    overflow: auto;
    place-items: center;
    padding: 40px 24px;
}

.workspace-state-content {
    display: flex;
    width: 100%;
    max-width: 340px;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    color: var(--text-subtle);
    text-align: center;
}

.workspace-state-content strong {
    margin-top: 4px;
    color: var(--text);
    font-size: 14px;
    font-weight: 600;
}

.workspace-state-content p {
    margin: 0;
    color: var(--text-muted);
    font-size: 13px;
    line-height: 1.75;
    overflow-wrap: anywhere;
}

.workspace-state-content .cn-button {
    margin-top: 4px;
}

.state-loading {
    animation: state-spin 1s linear infinite;
}

@keyframes state-spin {
    to { transform: rotate(360deg); }
}

.capture-state.pending {
    color: var(--text-muted);
}

.toolbar-actions .cn-button {
    height: 34px;
    font-size: 12px;
}

.clipboard-workspace kbd {
    flex: none;
    padding: 2px 5px;
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--text-muted);
    background: var(--surface);
    font-family: var(--font-sans);
    font-size: 10px;
    font-weight: 500;
    line-height: 1.4;
    white-space: nowrap;
}

.primary-copy kbd {
    margin-left: 6px;
    border-color: currentColor;
    color: inherit;
    background: transparent;
    opacity: .65;
}

.keyboard-hints,
.keyboard-hints>span,
.panel-shortcut {
    display: inline-flex;
    align-items: center;
    gap: 5px;
}

.keyboard-hints {
    gap: 14px;
}

.panel-shortcut {
    margin-left: auto;
    white-space: nowrap;
}

.empty-state {
    gap: 10px;
}

.empty-state .cn-button {
    margin-top: 8px;
}

.empty-state p {
    overflow-wrap: anywhere;
}

.history-skeleton {
    padding: 0 16px;
}

.history-skeleton>div {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 82px;
}

.history-skeleton i,
.history-skeleton b {
    display: block;
    border-radius: 6px;
    background: var(--surface-3);
    animation: skeleton-pulse 1.5s ease-in-out infinite alternate;
}

.history-skeleton i {
    width: 38px;
    height: 42px;
}

.history-skeleton span {
    display: grid;
    flex: 1;
    gap: 10px;
}

.history-skeleton b {
    height: 10px;
    width: 85%;
}

.history-skeleton b:last-child {
    width: 55%;
}

@keyframes skeleton-pulse {
    to {
        opacity: .4;
    }
}

.clipboard-workspace button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
}

.item-select:focus-visible {
    outline-offset: -3px !important;
}

@container clipboard (max-width: 920px) {

    .sidebar-foot,
    .clipboard-statusbar .retention-note {
        display: none;
    }

    .preview-meta {
        grid-template-columns: repeat(2, minmax(0, 1fr));
        row-gap: 10px;
    }

    .preview-actions {
        flex-wrap: wrap;
    }

    .preview-pane {
        padding: 0 20px 16px;
    }
}

@container clipboard (max-width: 680px) {
    .clipboard-toolbar {
        grid-template-columns: 1fr auto;
        gap: 10px;
        padding: 14px;
    }

    .search-field {
        grid-column: 1 / -1;
        grid-row: 1;
    }

    .toolbar-identity {
        grid-column: 1;
        grid-row: 2;
        justify-self: start;
    }

    .toolbar-actions {
        grid-column: 2;
        grid-row: 2;
    }

    .clipboard-sidebar {
        padding: 0 14px;
        gap: 18px;
        overflow-x: auto;
    }

    .clipboard-sidebar button {
        padding: 0 0 2px;
        flex-shrink: 0;
    }

    .clipboard-layout {
        grid-template-columns: 1fr;
        grid-template-rows: minmax(180px, 42%) minmax(0, 1fr);
    }

    .history-pane {
        border-right: 0;
        border-bottom: 1px solid var(--border);
    }

    .preview-pane {
        padding: 0 16px 12px;
    }

    .preview-label {
        min-height: 36px;
        margin-bottom: 8px;
    }

    .preview-head {
        min-height: 36px;
        margin-bottom: 10px;
    }

    .preview-meta {
        display: none;
    }

    .preview-body {
        padding: 14px;
    }

    .preview-detail-footer {
        margin-top: 10px;
    }

    .preview-actions {
        padding-top: 0;
        border-top: 0;
        gap: 6px;
    }

    .preview-actions .cn-button {
        height: 32px;
        padding: 0 9px;
        font-size: 11px;
    }

    .clipboard-layout.preview-expanded {
        grid-template-rows: minmax(0, 1fr);
    }

    .clipboard-layout.preview-expanded .preview-body {
        padding: 16px;
    }

    .clipboard-statusbar {
        padding: 8px 14px;
    }

    .keyboard-hints {
        display: none;
    }

    .panel-shortcut {
        margin-left: 0;
    }

    .file-preview.has-file-list {
        grid-template-columns: 1fr;
        grid-template-rows: auto minmax(0, 1fr);
    }

    .file-preview-list {
        display: flex;
        overflow-x: auto;
        padding: 4px;
    }

    .file-preview-list button {
        width: auto;
        max-width: 160px;
        flex-shrink: 0;
    }

}

@media (prefers-reduced-motion: reduce) {

    .clipboard-workspace *,
    .clipboard-workspace *::before {
        animation: none !important;
        transition: none !important;
    }
}
</style>
