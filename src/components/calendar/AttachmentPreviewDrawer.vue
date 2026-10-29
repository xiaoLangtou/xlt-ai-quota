<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { pushToast } from "@/composables/useToast";
import { zentaoService } from "@/services/zentao-service";
import { renderMarkdown } from "@/utils/skills-markdown";
import type { ZentaoArchiveEntry, ZentaoFile, ZentaoFileContent } from "@/types/productivity";

// 附件预览抽屉：左侧列出任务全部附件（zip 展开为目录树），右侧渲染内容。
// 独立于详情抽屉打开，宽度 95%，可从详情抽屉的某个附件直接进入。
const props = defineProps<{
  open: boolean;
  files: ZentaoFile[];
  /** 初始选中的附件 id（从详情抽屉点某个附件的「预览」进入）。 */
  initialFileId?: number | null;
}>();
const emit = defineEmits<{
  (event: "update:open", value: boolean): void;
}>();

const activeFileId = ref<number | null>(null);
// zip 条目按附件缓存；collapsedDirs 键为 `${fileId}:${dirPath}`。
const archiveEntries = ref<Map<number, ZentaoArchiveEntry[]>>(new Map());
const expandedZips = ref<Set<number>>(new Set());
const collapsedDirs = ref<Set<string>>(new Set());
const archiveLoadingId = ref<number | null>(null);
// 当前右侧内容对应的节点：普通附件为 null（用 activeFile），zip 条目带 index。
const activeNode = ref<{ fileId: number; index: number; path: string } | null>(null);
const content = ref<ZentaoFileContent | null>(null);
const contentLoading = ref(false);
const contentError = ref("");

// 图片查看层：点任意图片后全屏查看。
const lightbox = ref<string | null>(null);

const activeFile = computed(() => props.files.find((file) => file.id === activeFileId.value) ?? null);

function extOf(target: { extension: string; title: string }): string {
  return (target.extension || target.title.split(".").pop() || "").toLowerCase();
}

function isZip(file: ZentaoFile): boolean {
  return extOf(file) === "zip";
}

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    reset();
    const initial = props.files.find((file) => file.id === props.initialFileId) ?? props.files[0];
    if (initial) void selectFile(initial);
  },
);

function reset(): void {
  activeFileId.value = null;
  archiveEntries.value = new Map();
  expandedZips.value = new Set();
  collapsedDirs.value = new Set();
  archiveLoadingId.value = null;
  activeNode.value = null;
  content.value = null;
  contentLoading.value = false;
  contentError.value = "";
  lightbox.value = null;
}

async function selectFile(file: ZentaoFile): Promise<void> {
  activeFileId.value = file.id;
  activeNode.value = null;
  content.value = null;
  contentError.value = "";
  if (isZip(file)) {
    // zip：展开并加载条目树，内容区等用户点具体文件。
    const expanded = new Set(expandedZips.value);
    expanded.add(file.id);
    expandedZips.value = expanded;
    if (!archiveEntries.value.has(file.id)) {
      archiveLoadingId.value = file.id;
      try {
        const entries = await zentaoService.readArchive(file.id);
        const map = new Map(archiveEntries.value);
        map.set(file.id, entries);
        archiveEntries.value = map;
        // 自动打开第一个文件条目，右侧不留空白。
        const first = entries[0];
        if (first) await loadContent(file.id, first.index, first.path);
      } catch (reason) {
        contentError.value = reason instanceof Error ? reason.message : String(reason);
      } finally {
        archiveLoadingId.value = null;
      }
    }
    return;
  }
  await loadContent(file.id, null, file.title);
}

async function openArchiveEntry(fileId: number, node: { index: number; path: string }): Promise<void> {
  if (activeNode.value?.fileId === fileId && activeNode.value?.index === node.index) return;
  await loadContent(fileId, node.index, node.path);
}

async function loadContent(fileId: number, entryIndex: number | null, path: string): Promise<void> {
  activeNode.value = entryIndex === null ? null : { fileId, index: entryIndex, path };
  content.value = null;
  contentError.value = "";
  contentLoading.value = true;
  try {
    if (entryIndex === null) {
      const file = props.files.find((item) => item.id === fileId);
      content.value = await zentaoService.readFile(fileId, extOf(file ?? { extension: "", title: "" }));
    } else {
      content.value = await zentaoService.readArchiveEntry(fileId, entryIndex);
    }
  } catch (reason) {
    contentError.value = reason instanceof Error ? reason.message : String(reason);
  } finally {
    contentLoading.value = false;
  }
}

/** zip 目录树节点（由条目路径推导，目录本身不是独立条目）。 */
interface ArchiveNode {
  name: string;
  path: string;
  isDir: boolean;
  size: number;
  index: number;
  children: ArchiveNode[];
}

function buildArchiveTree(entries: ZentaoArchiveEntry[]): ArchiveNode[] {
  const root: ArchiveNode = { name: "", path: "", isDir: true, size: 0, index: -1, children: [] };
  const dirIndex = new Map<string, ArchiveNode>([["", root]]);
  for (const entry of entries) {
    const parts = entry.path.split("/");
    let parent = root;
    for (let i = 0; i < parts.length - 1; i++) {
      const dirPath = parts.slice(0, i + 1).join("/");
      let dir = dirIndex.get(dirPath);
      if (!dir) {
        dir = { name: parts[i], path: dirPath, isDir: true, size: 0, index: -1, children: [] };
        dirIndex.set(dirPath, dir);
        parent.children.push(dir);
      }
      parent = dir;
    }
    parent.children.push({
      name: parts[parts.length - 1],
      path: entry.path,
      isDir: false,
      size: entry.size,
      index: entry.index,
      children: [],
    });
  }
  const sortNodes = (nodes: ArchiveNode[]): void => {
    nodes.sort((a, b) => (a.isDir === b.isDir ? a.name.localeCompare(b.name) : a.isDir ? -1 : 1));
    for (const node of nodes) if (node.isDir) sortNodes(node.children);
  };
  sortNodes(root.children);
  return root.children;
}

/** 指定 zip 的目录树展开为扁平行（缩进由 depth 控制），折叠目录不展开子级。 */
function archiveRowsOf(fileId: number): Array<ArchiveNode & { depth: number }> {
  const rows: Array<ArchiveNode & { depth: number }> = [];
  const walk = (nodes: ArchiveNode[], depth: number): void => {
    for (const node of nodes) {
      rows.push({ ...node, depth });
      if (node.isDir && !collapsedDirs.value.has(`${fileId}:${node.path}`)) walk(node.children, depth + 1);
    }
  };
  walk(buildArchiveTree(archiveEntries.value.get(fileId) ?? []), 0);
  return rows;
}

function toggleArchiveDir(fileId: number, node: ArchiveNode): void {
  const key = `${fileId}:${node.path}`;
  const collapsed = new Set(collapsedDirs.value);
  if (collapsed.has(key)) collapsed.delete(key);
  else collapsed.add(key);
  collapsedDirs.value = collapsed;
}

function toggleZipExpand(file: ZentaoFile): void {
  const expanded = new Set(expandedZips.value);
  if (expanded.has(file.id)) {
    expanded.delete(file.id);
    expandedZips.value = expanded;
    return;
  }
  expanded.add(file.id);
  expandedZips.value = expanded;
  // 已选中的 zip 只展开/收起，不重复加载内容。
  if (activeFileId.value !== file.id) void selectFile(file);
}

async function download(file: ZentaoFile): Promise<void> {
  try {
    const path = await zentaoService.downloadFile(file.id, file.extension, file.title);
    pushToast(`附件已保存：${path}`, "success");
  } catch (reason) {
    pushToast(reason instanceof Error ? reason.message : String(reason), "error");
  }
}

function formatSize(size: number): string {
  if (!size) return "";
  if (size >= 1024 * 1024) return `${(size / 1024 / 1024).toFixed(1)} MB`;
  if (size >= 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${size} B`;
}

// 右侧内容渲染：当前节点为 zip 条目时扩展名取条目路径，否则取附件扩展名。
const activeExt = computed(() => {
  if (activeNode.value) {
    const name = activeNode.value.path.split("/").pop() ?? "";
    return name.includes(".") ? (name.split(".").pop() ?? "").toLowerCase() : "";
  }
  return activeFile.value ? extOf(activeFile.value) : "";
});

const contentIsHtml = computed(
  () => content.value?.kind === "text" && (activeExt.value === "html" || activeExt.value === "htm"),
);

const contentHtml = computed(() => {
  if (content.value?.kind !== "text" || contentIsHtml.value) return "";
  return renderMarkdown(content.value.content);
});

/** 右侧标题：zip 条目与附件都只显示文件名（完整路径在 title 提示里）。 */
const activeTitle = computed(() => {
  const raw = activeNode.value?.path ?? activeFile.value?.title ?? "";
  return raw.split("/").pop() || raw;
});
</script>

<template>
  <USlideover
    :open="open"
    title="附件预览"
    :ui="{
      content: 'w-[95vw] max-w-[95vw]',
    }"
    @update:open="emit('update:open', $event)"
  >
    <template #body>
      <div class="preview-layout">
        <!-- 左：全部附件 + zip 目录树 -->
        <aside class="file-pane">
          <p v-if="!files.length" class="pane-tip">没有附件。</p>
          <template v-for="file in files" :key="file.id">
            <button
              type="button"
              class="file-item"
              :class="{ active: activeFileId === file.id && !(isZip(file) && activeNode?.fileId === file.id) }"
              :title="file.title"
              @click="isZip(file) ? toggleZipExpand(file) : selectFile(file)"
            >
              <span
                :class="[
                  isZip(file)
                    ? expandedZips.has(file.id)
                      ? 'i-lucide-chevron-down'
                      : 'i-lucide-chevron-right'
                    : 'i-lucide-file',
                  'item-icon',
                ]"
              />
              <span class="item-name">{{ file.title }}</span>
              <small class="item-meta">{{ formatSize(file.size) }}</small>
            </button>
            <!-- zip 展开：目录树 -->
            <div v-if="isZip(file) && expandedZips.has(file.id)" class="archive-block">
              <p v-if="archiveLoadingId === file.id" class="pane-tip">正在读取压缩包条目…</p>
              <template v-else-if="archiveEntries.get(file.id)?.length">
                <button
                  v-for="row in archiveRowsOf(file.id)"
                  :key="`${file.id}:${row.path}`"
                  type="button"
                  class="file-item archive-row"
                  :class="{ active: activeNode?.fileId === file.id && activeNode?.index === row.index }"
                  :style="{ paddingLeft: `${10 + row.depth * 14}px` }"
                  :title="row.path"
                  @click="row.isDir ? toggleArchiveDir(file.id, row) : openArchiveEntry(file.id, row)"
                >
                  <span
                    :class="row.isDir ? 'i-lucide-folder' : 'i-lucide-file-text'"
                    class="item-icon"
                  />
                  <span class="item-name">{{ row.name }}</span>
                  <small v-if="!row.isDir" class="item-meta">{{ formatSize(row.size) }}</small>
                </button>
              </template>
              <p v-else class="pane-tip">压缩包内没有文件。</p>
            </div>
          </template>
        </aside>

        <!-- 右：内容渲染 -->
        <section class="content-pane">
          <div v-if="activeTitle" class="content-head">
            <span class="content-title" :title="activeTitle">{{ activeTitle }}</span>
            <UButton
              v-if="activeFile"
              icon="i-lucide-download"
              color="neutral"
              variant="ghost"
              size="xs"
              title="下载附件到本机"
              @click="download(activeFile)"
            />
          </div>
          <p v-if="contentLoading" class="pane-tip">正在读取内容…</p>
          <p v-else-if="contentError" class="pane-tip error">{{ contentError }}</p>
          <template v-else-if="content">
            <img
              v-if="content.kind === 'image'"
              :src="content.content"
              alt="附件预览"
              class="preview-image"
              title="点击查看大图"
              @click="lightbox = content.content"
            />
            <iframe
              v-else-if="contentIsHtml"
              :srcdoc="content.content"
              sandbox="allow-scripts"
              class="preview-frame"
              title="附件预览"
            />
            <!-- eslint-disable-next-line vue/no-v-html -->
            <div v-else-if="contentHtml" class="preview-md" v-html="contentHtml" />
            <pre v-else class="preview-text">{{ content.content }}</pre>
          </template>
          <p v-else class="pane-tip">选择左侧文件查看内容。</p>
        </section>
      </div>
    </template>
  </USlideover>

  <!-- 图片查看层：全屏展示，点击空白或 Esc 关闭 -->
  <Teleport to="body">
    <div v-if="lightbox" class="lightbox" @click="lightbox = null">
      <img :src="lightbox" alt="查看大图" @click.stop />
      <button type="button" class="lightbox-close" aria-label="关闭" @click="lightbox = null">
        <span class="i-lucide-x" />
      </button>
    </div>
  </Teleport>
</template>

<style scoped>
.preview-layout {
  display: grid;
  grid-template-columns: minmax(220px, 300px) minmax(0, 1fr);
  gap: 12px;
  height: 100%;
  min-height: 0;
}
/* 窄窗口：左列表与右内容上下堆叠 */
@media (max-width: 720px) {
  .preview-layout {
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: minmax(140px, 38%) minmax(0, 1fr);
  }
}
.file-pane {
  min-height: 0;
  padding: 8px;
  overflow-y: auto;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--surface-2);
}
.file-item {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  padding: 5px 8px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--text);
  font: inherit;
  font-size: 11.5px;
  text-align: left;
  cursor: pointer;
}
.file-item:hover {
  background: var(--surface-3);
}
.file-item.active {
  color: var(--text-highlighted, var(--text));
  background: var(--surface-3);
}
.item-icon {
  flex: none;
  width: 13px;
  height: 13px;
  color: var(--text-subtle, var(--text-dimmed));
}
.item-name {
  min-width: 0;
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.item-meta {
  flex: none;
  color: var(--text-subtle, var(--text-dimmed));
  font-size: 10px;
}
.archive-block {
  padding-bottom: 4px;
}
.content-pane {
  min-height: 0;
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
  gap: 8px;
}
.content-head {
  display: flex;
  align-items: center;
  gap: 8px;
}
.content-title {
  min-width: 0;
  flex: 1;
  overflow: hidden;
  color: var(--text-highlighted, var(--text));
  font-size: 12px;
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.pane-tip {
  margin: 0;
  padding: 12px;
  color: var(--text-dimmed, var(--text-subtle));
  font-size: 11.5px;
}
.pane-tip.error {
  color: var(--text-muted, var(--text-dimmed));
}
.preview-image {
  display: block;
  max-width: 100%;
  max-height: 100%;
  margin: 0 auto;
  border-radius: 6px;
  cursor: zoom-in;
  align-self: center;
  justify-self: center;
}
.preview-frame {
  width: 100%;
  height: 100%;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: #fff;
}
.preview-md {
  padding: 4px 2px;
  overflow-y: auto;
  color: var(--text);
  font-size: 12.5px;
  line-height: 1.8;
}
.preview-md :deep(h1),
.preview-md :deep(h2),
.preview-md :deep(h3) {
  margin: 14px 0 6px;
  font-weight: 700;
}
.preview-md :deep(h1) { font-size: 17px; }
.preview-md :deep(h2) { font-size: 15px; }
.preview-md :deep(h3) { font-size: 13.5px; }
.preview-md :deep(p) { margin: 6px 0; }
.preview-md :deep(ul),
.preview-md :deep(ol) { margin: 6px 0; padding-left: 20px; }
.preview-md :deep(table) {
  border-collapse: collapse;
  margin: 8px 0;
  width: 100%;
  font-size: 11.5px;
}
.preview-md :deep(th),
.preview-md :deep(td) {
  border: 1px solid var(--border);
  padding: 4px 8px;
  text-align: left;
}
.preview-md :deep(code) {
  padding: 1px 4px;
  border-radius: 4px;
  background: var(--surface-3);
  font-family: var(--font-mono);
  font-size: 11px;
}
.preview-md :deep(img) {
  max-width: 100%;
  border-radius: 6px;
  cursor: zoom-in;
}
.preview-md :deep(pre) {
  overflow-x: auto;
  margin: 8px 0;
  padding: 10px;
  border-radius: 6px;
  background: var(--surface-3);
}
.preview-text {
  margin: 0;
  padding: 4px 2px;
  overflow: auto;
  color: var(--text);
  font-size: 12px;
  line-height: 1.7;
  white-space: pre-wrap;
  word-break: break-word;
  font-family: var(--font-mono);
}
.lightbox {
  position: fixed;
  inset: 0;
  z-index: 100;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 32px;
  background: rgb(0 0 0 / 82%);
  cursor: zoom-out;
}
.lightbox img {
  max-width: 100%;
  max-height: 100%;
  border-radius: 8px;
  box-shadow: 0 12px 48px rgb(0 0 0 / 45%);
}
.lightbox-close {
  position: absolute;
  top: 18px;
  right: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border: 0;
  border-radius: 999px;
  background: rgb(255 255 255 / 14%);
  color: #fff;
  cursor: pointer;
}
.lightbox-close:hover {
  background: rgb(255 255 255 / 26%);
}
</style>
