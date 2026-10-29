<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from "vue";
import type { ZentaoDetail, ZentaoFile } from "@/types/productivity";
import { zentaoService } from "@/services/zentao-service";
import { pushToast } from "@/composables/useToast";
import RichTextContent from "@/components/calendar/RichTextContent.vue";
import AttachmentPreviewDrawer from "@/components/calendar/AttachmentPreviewDrawer.vue";

// 禅道详情正文：描述 + 处理历史 + 关联材料。
// 抽屉与详情页共用；附件预览与图片查看层由本组件自持。
defineProps<{
  detail: ZentaoDetail | null;
  loading: boolean;
  /** 实时详情缺失时回退展示的本地描述。 */
  description: string;
  /** 描述截图高度上限；详情页传 null 时按正文宽度等比展示。 */
  imageMaxHeight?: number | null;
}>();

const downloadingId = ref<number | null>(null);
// 附件预览抽屉：点附件行的「预览」打开，宽度 95%。
const attachmentOpen = ref(false);
const attachmentFileId = ref<number | null>(null);
// 图片查看层：点任意截图/描述图后全屏查看；点图片在「适配窗口 / 原始尺寸」间切换。
const lightbox = ref<string | null>(null);
const lightboxNatural = ref(false);

function showLightbox(src: string): void {
  lightbox.value = src;
  lightboxNatural.value = false;
}

// 图片查看层打开时监听 Esc 关闭。
function onLightboxKey(event: KeyboardEvent): void {
  if (event.key === "Escape") lightbox.value = null;
}
watch(lightbox, (value) => {
  if (value) window.addEventListener("keydown", onLightboxKey);
  else window.removeEventListener("keydown", onLightboxKey);
});
onBeforeUnmount(() => window.removeEventListener("keydown", onLightboxKey));

async function download(file: ZentaoFile): Promise<void> {
  downloadingId.value = file.id;
  try {
    const path = await zentaoService.downloadFile(file.id, file.extension, file.title);
    pushToast(`附件已保存：${path}`, "success");
  } catch (reason) {
    pushToast(reason instanceof Error ? reason.message : String(reason), "error");
  } finally {
    downloadingId.value = null;
  }
}

/** 打开附件预览抽屉并选中指定附件。 */
function openAttachmentPreview(file: ZentaoFile): void {
  attachmentFileId.value = file.id;
  attachmentOpen.value = true;
}

function formatSize(size: number): string {
  if (!size) return "";
  if (size >= 1024 * 1024) return `${(size / 1024 / 1024).toFixed(1)} MB`;
  if (size >= 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${size} B`;
}
</script>

<template>
  <div class="detail-content">
    <section id="zentao-source-description" class="detail-section">
      <h4>描述</h4>
      <p v-if="loading" class="detail-empty">正在获取最新描述…</p>
      <div v-else class="detail-text">
        <RichTextContent
          :text="description"
          :images="detail?.images ?? []"
          :image-max-height="imageMaxHeight"
          empty-tip="暂无描述。"
          @preview="showLightbox"
        />
      </div>
    </section>

    <section v-if="detail?.actions?.length" class="detail-section">
      <h4>动态（{{ detail.actions.length }}）</h4>
      <ol class="history-list">
        <li v-for="(action, index) in [...detail.actions].reverse()" :key="index" :id="`zentao-source-action-${detail.actions.length - 1 - index}`" class="history-item">
          <span class="history-node">{{ index + 1 }}</span>
          <div class="history-body">
            <p class="history-desc">{{ action.desc || `${action.actor} ${action.date}` }}</p>
            <details v-if="action.changes.length" class="history-changes">
              <summary>变更明细（{{ action.changes.length }}）</summary>
              <div class="history-changes-body">
                <div v-for="(change, ci) in action.changes" :key="ci" class="history-change-block">
                  <p class="history-change">
                    <template v-if="change.rich">{{ change.field }}：内容已更新</template>
                    <template v-else>{{ change.field }}：{{ change.old || "（空）" }} → {{ change.new || "（空）" }}</template>
                  </p>
                  <details v-if="change.rich && change.newText" class="history-diff">
                    <summary>查看变更内容</summary>
                    <div class="history-diff-body">
                      <RichTextContent
                        :text="change.newText"
                        :images="change.newImages"
                        @preview="showLightbox"
                      />
                    </div>
                  </details>
                </div>
              </div>
            </details>
            <p v-if="action.comment" class="history-comment">{{ action.comment }}</p>
            <div v-if="action.images.length" class="history-images">
              <img
                v-for="(src, ii) in action.images"
                :key="ii"
                :src="src"
                class="history-image"
                title="点击查看大图"
                alt="历史截图"
                @click="showLightbox(src)"
              />
            </div>
          </div>
        </li>
      </ol>
    </section>

    <section v-if="detail?.files?.length" class="detail-section">
      <h4>附件（{{ detail.files.length }}）</h4>
      <div v-for="file in detail.files" :key="file.id" class="file-row">
        <span class="file-name" :title="file.title">{{ file.title }}</span>
        <small>{{ [formatSize(file.size), file.extension, file.origin === "story" ? "需求附件" : ""].filter(Boolean).join(" · ") }}</small>
        <UButton
          variant="ghost"
          size="xs"
          @click="openAttachmentPreview(file)"
        >
          预览
        </UButton>
        <UButton
          icon="i-lucide-download"
          :loading="downloadingId === file.id"
          color="neutral"
          variant="ghost"
          size="xs"
          title="下载到本机"
          @click="download(file)"
        />
      </div>
    </section>

    <!-- 附件预览抽屉：宽度 95% -->
    <AttachmentPreviewDrawer
      v-model:open="attachmentOpen"
      :files="detail?.files ?? []"
      :initial-file-id="attachmentFileId"
    />

    <!-- 图片查看层：全屏展示，点击空白或 Esc 关闭；点图片在「适配窗口 / 原始尺寸」间切换 -->
    <Teleport to="body">
      <div v-if="lightbox" class="lightbox" @click="lightbox = null">
        <img
          :src="lightbox"
          class="lightbox-image"
          :class="{ natural: lightboxNatural }"
          :title="lightboxNatural ? '点击恢复适配窗口' : '点击查看原始尺寸'"
          alt="查看大图"
          @click.stop="lightboxNatural = !lightboxNatural"
        />
        <button type="button" class="lightbox-close" aria-label="关闭" @click="lightbox = null">
          <span class="i-lucide-x" />
        </button>
        <p class="lightbox-hint">{{ lightboxNatural ? "原始尺寸 · 点击图片恢复适配" : "适配窗口 · 点击图片查看原始尺寸" }}</p>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.detail-content {
  display: grid;
  gap: 24px;
}
.detail-section h4 {
  margin: 0 0 10px;
  color: var(--text-highlighted, var(--text));
  font-size: 14px;
  font-weight: 650;
}
.detail-text {
  margin: 0;
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--bg, var(--surface-2));
  color: var(--text);
  font-size: 12.5px;
  line-height: 1.75;
  white-space: pre-wrap;
  word-break: break-word;
}
.detail-empty {
  margin: 0;
  color: var(--text-dimmed, var(--text-subtle));
  font-size: 12px;
}
.file-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 7px 2px;
  border-bottom: 1px solid var(--border);
}
.file-row:last-child {
  border-bottom: 0;
}
.file-name {
  min-width: 0;
  flex: 1;
  overflow: hidden;
  color: var(--text);
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.file-row small {
  flex: none;
  color: var(--text-subtle, var(--text-dimmed));
  font-size: 10.5px;
}
.history-list {
  position: relative;
  margin: 0;
  padding: 0;
  display: grid;
  gap: 18px;
  list-style: none;
}
.history-item {
  position: relative;
  display: flex;
  gap: 10px;
}
/* 节点之间的竖线，形成时间线 */
.history-item:not(:last-child)::before {
  content: "";
  position: absolute;
  left: 12px;
  top: 28px;
  bottom: -18px;
  width: 1px;
  background: var(--border);
}
.history-node {
  z-index: 1;
  flex: none;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 25px;
  height: 25px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--surface-2);
  color: var(--text-muted);
  font-size: 12px;
  font-weight: 600;
}
.history-body {
  min-width: 0;
  flex: 1;
  display: grid;
  gap: 6px;
  overflow-wrap: anywhere;
}
.history-desc {
  margin: 0;
  color: var(--text);
  font-size: 14px;
  line-height: 1.75;
}
.history-change {
  margin: 0;
  color: var(--text-muted);
  font-size: 13px;
  line-height: 1.7;
}
.history-change-block + .history-change-block {
  margin-top: 8px;
}
.history-changes-body {
  margin-top: 8px;
  padding: 10px 12px;
  border-left: 2px solid var(--border);
}
.history-diff {
  margin-top: 4px;
}
.history-changes > summary,
.history-diff summary {
  cursor: pointer;
  color: var(--text-muted);
  font-size: 13px;
  line-height: 1.7;
  user-select: none;
}
.history-diff summary:hover {
  color: var(--text-muted);
}
.history-diff-body {
  margin-top: 6px;
  padding: 8px 10px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--surface-2);
}
.history-comment {
  margin: 2px 0 0;
  padding: 10px 12px;
  border-left: 2px solid var(--accent, var(--border));
  border-radius: 0 6px 6px 0;
  background: var(--surface-2);
  color: var(--text);
  font-size: 14px;
  line-height: 1.75;
  white-space: pre-wrap;
  word-break: break-word;
}
.history-images {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 4px;
}
.history-image {
  max-width: min(480px, 100%);
  max-height: 300px;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: var(--surface-2);
  object-fit: contain;
  cursor: zoom-in;
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
  /* 原始尺寸超出窗口时允许滚动查看。 */
  overflow: auto;
}
.lightbox-image {
  max-width: 100%;
  max-height: 100%;
  border-radius: 8px;
  box-shadow: 0 12px 48px rgb(0 0 0 / 45%);
  cursor: zoom-in;
}
.lightbox-image.natural {
  max-width: none;
  max-height: none;
  flex: none;
  cursor: zoom-out;
  margin: auto;
}
.lightbox-hint {
  position: fixed;
  bottom: 16px;
  left: 50%;
  margin: 0;
  padding: 4px 12px;
  border-radius: 999px;
  background: rgb(0 0 0 / 55%);
  color: rgb(255 255 255 / 85%);
  font-size: 11px;
  transform: translateX(-50%);
  pointer-events: none;
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
