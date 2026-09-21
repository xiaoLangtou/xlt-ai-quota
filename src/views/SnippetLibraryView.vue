<script lang="ts" setup>
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import {
    BookMarked,
    ClipboardCopy,
    Code2,
    Command,
    Copy,
    Eye,
    FileText,
    Link,
    Pencil,
    Pin,
    PinOff,
    Plus,
    Search,
    Sparkles,
    Tag,
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
import { Textarea } from "@/components/ui/textarea";
import HighlightedCode from "@/components/HighlightedCode.vue";
import SnippetCodeEditor from "@/components/SnippetCodeEditor.vue";
import { pushToast } from "@/composables/useToast";
import { isTauriDesktop } from "@/connectors/types";
import { snippetService } from "@/services/snippet-service";
import type { Snippet, SnippetDraft, SnippetKind, SnippetSort, SnippetTag } from "@/types/snippet";

type KindFilter = "all" | SnippetKind;
type LibraryView = "all" | "recent" | "pinned";

const kinds: { value: SnippetKind; label: string }[] = [
    { value: "code", label: "代码" }, { value: "command", label: "命令" }, {
        value: "prompt",
        label: "Prompt",
    }, { value: "text", label: "文本" }, { value: "link", label: "链接" },
];
const kindOptions = [ { value: "all", label: "所有类型" }, ...kinds ];
const sorts = [ { value: "recent", label: "最近使用" }, { value: "updated", label: "最近更新" }, {
    value: "created",
    label: "创建时间",
}, { value: "title", label: "标题" }, { value: "usage", label: "使用次数" } ];
const languages = [ "bash", "typescript", "javascript", "vue", "rust", "python", "json", "sql", "markdown", "text" ].map((value) => ( {
    value,
    label: value,
} ));
const desktop = isTauriDesktop();
const snippets = ref<Snippet[]>([]);
const tags = ref<SnippetTag[]>([]);
const query = ref("");
const view = ref<LibraryView>("all");
const kind = ref<KindFilter>("all");
const activeTag = ref("");
const sort = ref<SnippetSort>("recent");
const loading = ref(false);
const error = ref("");
const editorOpen = ref(false);
const composerOpen = ref(false);
const preview = ref<Snippet | null>(null);
const deleting = ref<Snippet | null>(null);
const saving = ref(false);
const tagText = ref("");
const searchRoot = ref<HTMLElement | null>(null);
let timer: number | undefined;
let sequence = 0;

function blank(type: SnippetKind = "code"): SnippetDraft {
    return {
        title: "",
        kind: type,
        content: "",
        language: type === "code" ? "typescript" : type === "command" ? "bash" : null,
        description: "",
        tags: [],
        pinned: false,
    };
}

const form = ref<SnippetDraft>(blank());
const results = computed(() => view.value === "pinned" ? snippets.value.filter((item) => item.pinned) : view.value === "recent" ? snippets.value.filter((item) => item.lastUsedAt) : snippets.value);
const canSave = computed(() => Boolean(form.value.title.trim() && form.value.content.trim()));

function label(type: SnippetKind): string {
    return kinds.find((item) => item.value === type)?.label ?? "文本";
}

function icon(type: SnippetKind) {
    return type === "code" ? Code2 : type === "command" ? Command : type === "prompt" ? Sparkles : type === "link" ? Link : FileText;
}

function excerpt(content: string): string {
    return content.replace(/\s+/g, " ").trim();
}

function date(value: string | null): string {
    if ( !value ) return "尚未使用";
    return new Intl.DateTimeFormat("zh-CN", {
        month: "numeric",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
    }).format(new Date(value));
}

async function load(): Promise<void> {
    if ( !desktop ) return;
    const request = ++sequence;
    loading.value = true;
    error.value = "";
    try {
        const [ items, nextTags ] = await Promise.all([
            snippetService.list({
                query: query.value.trim() || null,
                kind: kind.value === "all" ? null : kind.value,
                tag: activeTag.value || null,
                sort: sort.value,
            }),
            snippetService.listTags(),
        ]);
        if ( request !== sequence ) return;
        snippets.value = items;
        tags.value = nextTags;
        if ( preview.value ) preview.value = items.find((item) => item.id === preview.value?.id) ?? null;
    } catch ( reason ) {
        if ( request !== sequence ) return;
        error.value = reason instanceof Error ? reason.message : String(reason);
        snippets.value = [];
    } finally {
        if ( request === sequence ) loading.value = false;
    }
}

function debounce(): void {
    if ( timer !== undefined ) window.clearTimeout(timer);
    timer = window.setTimeout(() => void load(), 180);
}

function create(): void {
    form.value = blank(kind.value === "all" ? "code" : kind.value);
    tagText.value = activeTag.value;
    composerOpen.value = true;
}

function edit(item: Snippet): void {
    form.value = {
        id: item.id,
        title: item.title,
        kind: item.kind,
        content: item.content,
        language: item.language,
        description: item.description,
        tags: [ ...item.tags ],
        pinned: item.pinned,
    };
    tagText.value = item.tags.join(", ");
    preview.value = null;
    editorOpen.value = true;
}

function changeType(value: string): void {
    form.value.kind = value as SnippetKind;
    if ( form.value.kind !== "code" && form.value.kind !== "command" ) form.value.language = null;
    else if ( form.value.language === null ) form.value.language = form.value.kind === "code" ? "typescript" : "bash";
}

async function save(): Promise<void> {
    if ( !canSave.value ) return;
    saving.value = true;
    try {
        const editing = Boolean(form.value.id);
        await snippetService.save({
            ...form.value,
            tags: tagText.value.split(/[,，]/).map((item) => item.trim()).filter(Boolean),
        });
        editorOpen.value = false;
        composerOpen.value = false;
        await load();
        pushToast(editing ? "片段已更新。" : "片段已保存。", "success");
    } catch ( reason ) {
        pushToast(reason instanceof Error ? reason.message : String(reason), "error");
    } finally {
        saving.value = false;
    }
}

async function copy(item: Snippet): Promise<void> {
    try {
        const updated = await snippetService.copy(item.id);
        snippets.value = snippets.value.map((current) => current.id === updated.id ? updated : current);
        if ( preview.value?.id === updated.id ) preview.value = updated;
        if ( sort.value === "recent" ) await load();
        pushToast("已复制到剪贴板。", "success");
    } catch ( reason ) {
        pushToast(reason instanceof Error ? reason.message : String(reason), "error");
    }
}

async function pin(item: Snippet): Promise<void> {
    try {
        await snippetService.save({
            id: item.id,
            title: item.title,
            kind: item.kind,
            content: item.content,
            language: item.language,
            description: item.description,
            tags: item.tags,
            pinned: !item.pinned,
        });
        await load();
        pushToast(item.pinned ? "已取消置顶。" : "已置顶。", "success");
    } catch ( reason ) {
        pushToast(reason instanceof Error ? reason.message : String(reason), "error");
    }
}

async function remove(): Promise<void> {
    if ( !deleting.value ) return;
    try {
        await snippetService.delete(deleting.value.id);
        deleting.value = null;
        preview.value = null;
        await load();
        pushToast("片段已删除。", "success");
    } catch ( reason ) {
        pushToast(reason instanceof Error ? reason.message : String(reason), "error");
    }
}

function shortcut(event: KeyboardEvent): void {
    if ( !( event.metaKey || event.ctrlKey ) ) return;
    if ( event.key.toLowerCase() === "k" ) {
        event.preventDefault();
        searchRoot.value?.querySelector<HTMLInputElement>("input")?.focus();
    }
    if ( event.shiftKey && event.key.toLowerCase() === "n" ) {
        event.preventDefault();
        if ( !editorOpen.value ) create();
    }
}

watch(query, debounce);
watch([ kind, activeTag, sort ], () => void load());
onMounted(() => {
    if ( !desktop ) return;
    window.addEventListener("keydown", shortcut);
    void load();
});
onUnmounted(() => {
    if ( timer !== undefined ) window.clearTimeout(timer);
    window.removeEventListener("keydown", shortcut);
});
</script>

<template>
    <section v-if="!desktop" class="unavailable">
        <BookMarked :size="38"/>
        <strong>片段库仅在桌面端可用</strong>
        <p>片段保存在本机 SQLite 数据库中。请使用 Tauri 桌面应用打开此功能。</p></section>
    <section v-else class="library">
        <header class="library-header">
            <div>
                <div class="library-crumbs">效率工具 <i>/</i> <b>Snippets</b></div>
                <h1>代码片段</h1>
            </div>
            <Button type="button" @click="create">
                <Plus :size="15"/>
                新建 <kbd>⇧⌘N</kbd></Button>
        </header>
        <div class="library-toolbar">
            <div ref="searchRoot" class="search">
                <Search :size="15"/>
                <Input v-model="query" aria-label="搜索片段" placeholder="搜索片段标题、内容或标签…"/>
                <kbd v-if="!query">⌘K</kbd>
                <Button v-else aria-label="清除搜索" size="icon" type="button" variant="ghost" @click="query = ''">
                    <X :size="15"/>
                </Button>
            </div>
            <div aria-label="片段范围" class="scope">
                <button :class="{ active: view === 'all' }" type="button" @click="view = 'all'">全部</button>
                <button :class="{ active: view === 'recent' }" type="button" @click="view = 'recent'">最近</button>
                <button :class="{ active: view === 'pinned' }" type="button" @click="view = 'pinned'">置顶</button>
            </div>
            <div class="toolbar-right">
                <Select :model-value="kind" :options="kindOptions" aria-label="类型筛选" class="filter-select"
                        @update:model-value="kind = $event as KindFilter"/>
                <Select :model-value="sort" :options="sorts" aria-label="排序方式" class="filter-select"
                        @update:model-value="sort = $event as SnippetSort"/>
            </div>
        </div>
        <div v-if="tags.length" class="tagbar">
            <Tag :size="14"/>
            <button :class="{ active: !activeTag }" type="button" @click="activeTag = ''">所有</button>
            <button v-for="item in tags" :key="item.name" :class="{ active: activeTag === item.name }" type="button"
                    @click="activeTag = activeTag === item.name ? '' : item.name">{{ item.name }} <em>{{
                    item.count
                }}</em></button>
        </div>

        <form v-if="composerOpen" class="composer" @submit.prevent="save">
            <header>
                <div><strong>新建片段</strong><span>保存一条将来可以直接插入的内容。</span></div>
                <Button aria-label="取消新建" size="icon" type="button" variant="ghost" @click="composerOpen = false">
                    <X :size="16"/>
                </Button>
            </header>
            <div class="fields"><label class="wide"><span>标题 *</span><Input v-model="form.title" autofocus
                                                                              placeholder="例如：检查 TypeScript 类型"/></label><label><span>类型 *</span><Select
                :model-value="form.kind" :options="kinds" @update:model-value="changeType"/></label><label
                v-if="form.kind === 'code' || form.kind === 'command'"><span>语言</span><Select
                :model-value="form.language ?? ''" :options="languages" placeholder="选择语言"
                @update:model-value="form.language = $event"/></label><label class="wide"><span>内容 *</span>
                <SnippetCodeEditor v-model="form.content" :language="form.language"/>
            </label><label><span>用途说明</span><Input v-model="form.description"
                                                       placeholder="可选"/></label><label><span>标签</span><Input
                v-model="tagText" placeholder="git, debug"/></label><label class="pin">
                <Checkbox v-model="form.pinned"/>
                <span>置顶此片段</span></label></div>
            <footer>
                <Button type="button" variant="outline" @click="composerOpen = false">取消</Button>
                <Button :disabled="saving || !canSave" type="submit">{{ saving ? "保存中…" : "保存片段" }}</Button>
            </footer>
        </form>

        <p v-if="error" class="error">{{ error }}</p>
        <div v-if="loading && !snippets.length" class="empty">正在读取本机片段…</div>
        <div v-else-if="!results.length" class="empty">
            <ClipboardCopy :size="36"/>
            <strong>{{
                    query || kind !== "all" || activeTag || view !== "all" ? "没有匹配的片段" : "这里还没有片段"
                }}</strong><span>{{
                query || kind !== "all" || activeTag || view !== "all" ? "换个词试试，或清除筛选条件。" : "从常用命令、代码模板或高质量 Prompt 开始。"
            }}</span>
            <Button v-if="!query && kind === 'all' && !activeTag && view === 'all'" type="button" variant="outline"
                    @click="create">
                <Plus :size="15"/>
                新建第一个片段
            </Button>
        </div>
        <main v-else aria-label="片段列表" class="snippet-stream">
            <div class="stream-head"><span>{{ results.length }} 个片段</span><span>点击片段复制到剪贴板</span></div>
            <article v-for="item in results" :key="item.id" :aria-label="'复制片段：' + item.title" :class="{ pinned: item.pinned }"
                     :data-kind="item.kind" class="snippet-entry" role="button" tabindex="0"
                     @click="copy(item)" @keydown.enter.prevent="copy(item)" @keydown.space.prevent="copy(item)">
                <header class="entry-head">
                    <div class="entry-title"><span class="kind-icon"><component :is="icon(item.kind)"
                                                                                :size="16"/></span>
                        <h3>
                            <Pin v-if="item.pinned" :size="13"/>
                            {{ item.title }}
                        </h3>
                        <Badge v-if="item.language">{{ item.language }}</Badge>
                    </div>
                    <div class="row-actions">
                        <Button :aria-label="item.pinned ? '取消置顶' : '置顶'" size="icon" type="button"
                                variant="ghost" @click.stop="pin(item)">
                            <PinOff v-if="item.pinned" :size="15"/>
                            <Pin v-else :size="15"/>
                        </Button>
                        <Button aria-label="查看完整内容" size="icon" type="button" variant="ghost"
                                @click.stop="preview = item">
                            <Eye :size="15"/>
                        </Button>
                        <Button aria-label="编辑片段" size="icon" type="button" variant="ghost"
                                @click.stop="edit(item)">
                            <Pencil :size="15"/>
                        </Button>
                        <Button aria-label="删除片段" class="delete" size="icon" type="button" variant="ghost"
                                @click.stop="deleting = item">
                            <Trash2 :size="15"/>
                        </Button>
                    </div>
                </header>
                <div v-if="item.kind === 'code' || item.kind === 'command'" class="entry-code">
                    <HighlightedCode :code="item.content" :language="item.language"/>
                </div>
                <p v-else class="entry-text">{{ excerpt(item.content) }}</p>
                <footer class="entry-foot">
                    <Badge>{{ label(item.kind) }}</Badge>
                    <Badge v-for="tag in item.tags.slice(0, 3)" :key="tag">{{ tag }}</Badge>
                    <span class="entry-meta">{{
                            item.useCount ? `已复制 ${ item.useCount } 次` : "尚未使用"
                        }}</span><span class="entry-meta">更新于 {{ date(item.updatedAt) }}</span></footer>
            </article>
        </main>
    </section>

    <Dialog :open="editorOpen" content-class="snippet-dialog" @update:open="editorOpen = $event">
        <form @submit.prevent="save">
            <header class="dialog-head">
                <div>
                    <DialogTitle>编辑片段</DialogTitle>
                    <DialogDescription>修改内容或调整分类信息。</DialogDescription>
                </div>
                <Button aria-label="关闭" size="icon" type="button" variant="ghost" @click="editorOpen = false">
                    <X :size="16"/>
                </Button>
            </header>
            <div class="dialog-body">
                <div class="fields"><label class="wide"><span>标题 *</span><Input v-model="form.title" autofocus
                                                                                  placeholder="例如：检查 TypeScript 类型"/></label><label><span>类型 *</span><Select
                    :model-value="form.kind" :options="kinds" @update:model-value="changeType"/></label><label
                    v-if="form.kind === 'code' || form.kind === 'command'"><span>语言</span><Select
                    :model-value="form.language ?? ''" :options="languages" placeholder="选择语言"
                    @update:model-value="form.language = $event"/></label><label
                    class="wide"><span>内容 *</span><Textarea v-model="form.content" class="content-input"
                                                              placeholder="粘贴或输入可复用内容"/></label><label
                    class="wide"><span>用途说明</span><Textarea v-model="form.description"
                                                                placeholder="说明用途、前置条件或注意事项"/></label><label
                    class="wide"><span>标签</span><Input v-model="tagText"
                                                         placeholder="git, debug（逗号分隔）"/></label><label class="pin">
                    <Checkbox v-model="form.pinned"/>
                    <span>置顶此片段</span></label></div>
            </div>
            <footer class="dialog-footer">
                <Button type="button" variant="outline" @click="editorOpen = false">取消</Button>
                <Button :disabled="saving || !canSave" type="submit">{{ saving ? "保存中…" : "保存片段" }}</Button>
            </footer>
        </form>
    </Dialog>
    <Dialog :open="Boolean(preview)" content-class="snippet-preview-dialog"
            @update:open="(open) => { if (!open) preview = null; }">
        <template v-if="preview">
            <header class="dialog-head">
                <div>
                    <DialogTitle>{{ preview.title }}</DialogTitle>
                    <DialogDescription>{{ label(preview.kind) }} · {{
                            preview.language || "未指定语言"
                        }}
                    </DialogDescription>
                </div>
                <Button aria-label="关闭" size="icon" type="button" variant="ghost" @click="preview = null">
                    <X :size="16"/>
                </Button>
            </header>
            <div class="preview-body"><p v-if="preview.description">{{ preview.description }}</p>
                <div class="preview-code">
                    <HighlightedCode :code="preview.content" :language="preview.language"/>
                </div>
            </div>
            <footer class="dialog-footer">
                <Button type="button" variant="outline" @click="edit(preview)">
                    <Pencil :size="15"/>
                    编辑
                </Button>
                <Button type="button" @click="copy(preview)">
                    <Copy :size="15"/>
                    复制
                </Button>
            </footer>
        </template>
    </Dialog>
    <Dialog :open="Boolean(deleting)" content-class="snippet-confirm"
            @update:open="(open) => { if (!open) deleting = null; }">
        <div class="dialog-head">
            <div>
                <DialogTitle>删除这个片段？</DialogTitle>
                <DialogDescription>“{{ deleting?.title }}”删除后无法恢复。</DialogDescription>
            </div>
        </div>
        <footer class="dialog-footer">
            <Button type="button" variant="outline" @click="deleting = null">取消</Button>
            <Button type="button" variant="destructive" @click="remove">删除</Button>
        </footer>
    </Dialog>
</template>

<style scoped>
.unavailable, .empty {
    display: grid;
    min-height: min(570px, calc(100vh - 150px));
    place-items: center;
    align-content: center;
    gap: 10px;
    border: 1px dashed var(--border-strong);
    border-radius: var(--r-lg);
    color: var(--text-muted);
    text-align: center
}

.unavailable strong, .empty strong {
    color: var(--text);
    font-size: 16px
}

.unavailable p {
    max-width: 460px;
    margin: 0;
    line-height: 1.6
}

.library {
    width: 100%;
    margin: 0 auto;
    padding: 4px 0 32px
}

.library-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 20px;
    padding: 4px 0 12px
}

.library-crumbs {
    color: var(--text-subtle);
    font-size: 12px
}

.library-crumbs i {
    font-style: normal;
    opacity: .55;
    padding: 0 2px
}

.library-crumbs b {
    color: var(--text-muted);
    font-weight: 600
}

.library-header h1 {
    margin: 3px 0 0;
    color: var(--text);
    font-size: 20px;
    font-weight: 700;
    letter-spacing: -.01em;
    line-height: 1.1
}

.library-header kbd {
    margin-left: 6px;
    padding: 1px 5px;
    border: 1px solid rgba(255, 255, 255, .28);
    border-radius: 4px;
    background: rgba(255, 255, 255, .14);
    color: var(--accent-contrast);
    font-family: var(--font-mono);
    font-size: 10px
}

.library-toolbar {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 12px
}

.search {
    position: relative;
    display: flex;
    min-width: 180px;
    max-width: 420px;
    flex: 1;
    height: 34px;
    align-items: center;
    gap: 8px;
    padding: 0 8px 0 10px;
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    background: var(--surface);
    color: var(--text-subtle)
}

.search:focus-within {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-weak)
}

.search :deep(.cn-input) {
    min-width: 0;
    height: 32px;
    padding: 0;
    border: 0;
    background: transparent;
    box-shadow: none;
    font-size: 13px
}

.search kbd {
    padding: 3px 6px;
    border: 1px solid var(--border);
    border-bottom-width: 2px;
    border-radius: 5px;
    background: var(--surface);
    color: var(--text-subtle);
    font-family: var(--font-mono);
    font-size: 10px;
    white-space: nowrap
}

.toolbar-right {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-left: auto
}

.scope {
    flex: 0 0 auto;
    display: inline-flex;
    align-items: center;
    gap: 2px;
    padding: 3px;
    border: 1px solid var(--border);
    border-radius: var(--r-sm);
    background: var(--segment-bg)
}

.scope button, .tagbar button {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    border: 0;
    background: transparent;
    color: var(--text-muted);
    font: inherit;
    font-size: 12px;
    font-weight: 600;
    white-space: nowrap;
    cursor: pointer
}

.scope button {
    min-width: 52px;
    padding: 5px 11px;
    border-radius: 5px
}

.scope button:hover, .tagbar button:hover {
    color: var(--text)
}

.scope button.active {
    background: var(--surface);
    box-shadow: var(--shadow-sm);
    color: var(--accent);
    font-weight: 650
}

.filter-select {
    min-width: 136px
}

.tagbar {
    min-height: 30px;
    gap: 4px;
    padding: 0 0 14px;
    overflow: auto;
    color: var(--text-subtle)
}

.tagbar > svg {
    flex: 0 0 auto;
    margin-right: 3px
}

.tagbar button {
    padding: 4px 11px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface);
    color: var(--text-muted);
    font-weight: 600
}

.tagbar button.active {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--accent-contrast);
    font-weight: 650
}

em {
    font-family: var(--font-mono);
    font-size: 9px;
    font-style: normal;
    opacity: .72
}

.composer {
    display: grid;
    gap: 16px;
    margin: 0 0 16px;
    padding: 18px;
    border: 1px solid color-mix(in srgb, var(--accent) 38%, var(--border));
    border-radius: var(--r-md);
    background: var(--surface);
    box-shadow: var(--shadow-sm)
}

.composer > header, .composer > footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px
}

.composer > header > div {
    display: grid;
    gap: 3px
}

.composer > header strong {
    color: var(--text);
    font-size: 14px
}

.composer > header span {
    color: var(--text-muted);
    font-size: 11px
}

.composer > footer {
    justify-content: flex-end
}

.error {
    margin: 0 0 12px;
    padding: 9px 11px;
    border: 1px solid color-mix(in srgb, var(--u-crit) 35%, var(--border));
    border-radius: var(--r-sm);
    background: color-mix(in srgb, var(--u-crit) 8%, var(--surface));
    color: var(--u-crit);
    font-size: 12px
}

.snippet-stream {
    display: grid;
    gap: 10px
}

.stream-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 1px 1px 3px;
    color: var(--text-subtle);
    font-size: 11px
}

.stream-head span:last-child {
    font-family: var(--font-mono);
    font-size: 10px
}

.snippet-entry {
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: var(--r-lg);
    background: var(--surface);
    box-shadow: var(--shadow-card);
    cursor: pointer;
    transition: border-color .15s, box-shadow .15s
}

.snippet-entry:hover, .snippet-entry:focus-visible {
    border-color: var(--border-strong);
    outline: 0
}

.snippet-entry[data-kind='code'] {
    --kind: var(--brand-codex)
}

.snippet-entry[data-kind='command'] {
    --kind: var(--u-warn)
}

.snippet-entry[data-kind='prompt'] {
    --kind: var(--brand-kiro)
}

.snippet-entry[data-kind='text'] {
    --kind: var(--u-ok)
}

.snippet-entry[data-kind='link'] {
    --kind: var(--u-crit)
}

.snippet-entry.pinned {
    border-color: color-mix(in srgb, var(--accent) 35%, var(--border))
}

.entry-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 12px 16px 9px
}

.entry-title h3 {
    font-size: 13.5px
}

.entry-title {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 8px
}

.kind-icon {
    display: grid;
    width: 36px;
    height: 36px;
    flex: 0 0 36px;
    place-items: center;
    border-radius: 10px;
    background: color-mix(in srgb, var(--kind) 12%, var(--surface));
    color: var(--kind)
}

.entry-title h3 {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 4px;
    margin: 0;
    overflow: hidden;
    color: var(--text);
    font-size: 13px;
    font-weight: 680;
    text-overflow: ellipsis;
    white-space: nowrap
}

.entry-title h3 svg {
    flex: 0 0 auto;
    color: var(--u-warn)
}

.entry-title :deep(.cn-badge) {
    font-size: 9px
}

.row-actions {
    display: flex;
    flex: 0 0 auto;
    gap: 1px;
    opacity: 0;
    transition: opacity .14s
}

.snippet-entry:hover .row-actions, .snippet-entry:focus-within .row-actions {
    opacity: 1
}

.row-actions :deep(.cn-button) {
    width: 27px;
    height: 27px
}

.delete {
    color: var(--u-crit)
}

.entry-code {
    max-height: 114px;
    overflow: hidden;
    margin: 0 12px;
    border: 1px solid var(--border);
    border-radius: 7px;
    background: #1a1d26;
    font-family: var(--font-mono);
    font-size: 11px;
    color: #c7d2fe;
    line-height: 1.55
}

.entry-code :deep(pre.shiki) {
    min-height: 100%;
    padding: 10px
}

.entry-text {
    min-height: 46px;
    margin: 0 16px;
    padding: 11px 12px;
    border-radius: var(--r-sm);
    background: var(--surface-2);
    color: var(--text-muted);
    font-size: 12px;
    line-height: 1.55;
    white-space: pre-wrap;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    overflow: hidden
}

.entry-foot {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    padding: 10px 16px 12px;
    color: var(--text-subtle);
    font-size: 10px
}

.entry-foot :deep(.cn-badge) {
    font-size: 9.5px
}

.entry-meta {
    color: var(--text-subtle);
    font-size: 11px
}

.entry-meta:last-child {
    margin-left: auto
}

.empty {
    min-height: 390px;
    border: 1px dashed var(--border)
}

.dialog-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    padding: 20px 22px 14px;
    border-bottom: 1px solid var(--border)
}

.dialog-head :deep([data-slot='dialog-title']) {
    color: var(--text);
    font-size: 16px;
    font-weight: 720
}

.dialog-head :deep([data-slot='dialog-description']) {
    margin-top: 5px;
    color: var(--text-muted);
    font-size: 12px;
    line-height: 1.5
}

.dialog-body, .preview-body {
    max-height: min(580px, 65vh);
    overflow: auto;
    padding: 20px 22px
}

.fields {
    display: grid;
    grid-template-columns:repeat(2, minmax(0, 1fr));
    gap: 14px
}

.fields label {
    display: grid;
    min-width: 0;
    gap: 6px;
    color: var(--text);
    font-size: 12px;
    font-weight: 650
}

.fields label.wide {
    grid-column: 1/-1
}

.content-input {
    min-height: 180px;
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 1.6
}

.fields .pin {
    display: flex;
    align-items: center;
    gap: 8px
}

.dialog-footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 14px 22px 20px;
    border-top: 1px solid var(--border)
}

.preview-body p {
    margin: 0 0 14px;
    color: var(--text-muted);
    font-size: 12px;
    line-height: 1.6;
    white-space: pre-wrap
}

.preview-code {
    overflow: auto;
    border-radius: var(--r-sm);
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 1.65
}

.preview-code :deep(pre.shiki) {
    padding: 14px
}

@media (max-width: 760px) {
    .library {
        padding: 14px 10px 24px
    }

    .library-header {
        align-items: center
    }

    .library-header :deep(.cn-button) {
        width: auto
    }

    .library-toolbar {
        align-items: stretch;
        flex-direction: column
    }

    .scope {
        align-self: flex-start
    }

    .toolbar-right {
        flex-wrap: wrap;
        margin-left: 0
    }

    .row-actions {
        display: none
    }

    .entry-code {
        max-height: 76px
    }

    .fields {
        grid-template-columns:1fr
    }

    .fields label.wide {
        grid-column: auto
    }
}
</style>
