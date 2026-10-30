<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import DecompositionTaskCard from "@/components/calendar/DecompositionTaskCard.vue";
import { pushToast } from "@/composables/useToast";
import { isTauriDesktop } from "@/connectors/types";
import {
  AiConfigMissingError,
  documentDecompositionService,
} from "@/services/document-decomposition-service";
import {
  MAX_DOCUMENT_CHARS,
  buildCommitInput,
  formatEstimate,
  isTooLongDocument,
  selectedDrafts,
} from "@/utils/productivity";
import type {
  DecompositionDraft,
  DecompositionResult,
  DocumentContent,
} from "@/types/productivity";

// 文档拆解独立页面：输入文档 → AI 拆解 → 确认后批量创建待办并回到日历。
const router = useRouter();
const source = ref<"upload" | "paste">("upload");
const documentFile = ref<DocumentContent | null>(null);
const pasted = ref("");
const dragging = ref(false);
const extracting = ref(false);
const generating = ref(false);
const committing = ref(false);
const error = ref("");
const configMissing = ref(false);
const result = ref<DecompositionResult | null>(null);
const drafts = ref<DecompositionDraft[]>([]);

interface DecompositionProgress {
  stage: string;
  detail: string;
  elapsedMs: number;
}

// 后端流回的调用阶段（不含正文、密钥或模型信息）。
const progress = ref<DecompositionProgress[]>([]);
const thinking = ref("");
const streamOutput = ref("");
const thinkingEl = ref<HTMLElement | null>(null);

// 模型输出是 JSON，流式过程中从原始输出里提取已出现的任务标题，实时展示为可读列表。
const liveTitles = computed(() => {
  const titles: string[] = [];
  const pattern = /"title"\s*:\s*"((?:[^"\\]|\\.)*)"/g;
  for (const match of streamOutput.value.matchAll(pattern)) {
    let title = match[1];
    try {
      title = JSON.parse(`"${title}"`) as string;
    } catch {
      // 转义解析失败就展示原始片段。
    }
    if (title.trim() && !titles.includes(title.trim())) titles.push(title.trim());
  }
  return titles;
});
// 完成后尝试美化 JSON，失败时原样展示。
const prettyStream = computed(() => {
  const raw = streamOutput.value.trim();
  try {
    return JSON.stringify(JSON.parse(raw), null, 2);
  } catch {
    return raw;
  }
});

const contentText = computed(() => (source.value === "upload" ? documentFile.value?.content ?? "" : pasted.value));
const charCount = computed(() => contentText.value.length);
const tooLong = computed(() => isTooLongDocument(contentText.value));
const sourceName = computed(() =>
  source.value === "upload" ? documentFile.value?.name ?? "未选择文档" : "粘贴内容",
);
const canGenerate = computed(
  () =>
    contentText.value.trim().length > 0 &&
    !tooLong.value &&
    !extracting.value &&
    !generating.value,
);

const selected = computed(() => selectedDrafts(drafts.value));
const totalEstimate = computed(() =>
  selected.value.reduce((total, draft) => total + (draft.estimateMinutes || 0), 0),
);
const dependencyTitles = computed(() =>
  Object.fromEntries(drafts.value.map((draft) => [draft.id, draft.title])),
);
const groups = computed(() =>
  (result.value?.groups ?? [])
    .map((group) => ({
      id: group.id,
      title: group.title,
      tasks: drafts.value.filter((draft) => draft.groupId === group.id),
    }))
    .filter((group) => group.tasks.length > 0),
);

function resetTransient(): void {
  error.value = "";
  configMissing.value = false;
}

function switchSource(next: "upload" | "paste"): void {
  if (extracting.value || generating.value) return;
  source.value = next;
  resetTransient();
}

function applyResult(res: DecompositionResult): void {
  result.value = res;
  drafts.value = res.groups.flatMap((group) =>
    group.tasks.map<DecompositionDraft>((task) => ({
      id: task.id,
      groupId: group.id,
      groupTitle: group.title,
      title: task.title,
      description: task.description,
      priority: task.priority,
      category: "work",
      plannedDate: task.suggestedDate ?? "",
      plannedTime: task.suggestedTime ?? "",
      estimateMinutes: task.estimatedMinutes ?? 0,
      acceptanceCriteria: [...task.acceptanceCriteria],
      dependencyIds: [...task.dependencyIds],
      selected: true,
    })),
  );
}

async function loadPath(path: string): Promise<void> {
  resetTransient();
  extracting.value = true;
  await nextTick();
  try {
    const loaded = await documentDecompositionService.readDocument(path);
    documentFile.value = loaded;
    source.value = "upload";
    if (loaded.charCount > MAX_DOCUMENT_CHARS) {
      error.value = `正文超过 ${MAX_DOCUMENT_CHARS.toLocaleString("zh-CN")} 个字符，请缩短内容后再拆解。`;
    }
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : String(reason);
  } finally {
    extracting.value = false;
  }
}

async function pickFile(): Promise<void> {
  if (extracting.value || generating.value) return;
  resetTransient();
  extracting.value = true;
  await nextTick();
  try {
    const loaded = await documentDecompositionService.pickDocument();
    if (!loaded) return;
    documentFile.value = loaded;
    source.value = "upload";
    if (loaded.charCount > MAX_DOCUMENT_CHARS) {
      error.value = `正文超过 ${MAX_DOCUMENT_CHARS.toLocaleString("zh-CN")} 个字符，请缩短内容后再拆解。`;
    }
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : String(reason);
  } finally {
    extracting.value = false;
  }
}

async function generate(): Promise<void> {
  resetTransient();
  if (!contentText.value.trim()) {
    error.value = "请先选择文件或粘贴内容。";
    return;
  }
  if (tooLong.value) {
    error.value = `正文超过 ${MAX_DOCUMENT_CHARS.toLocaleString("zh-CN")} 个字符，请缩短内容后再拆解。`;
    return;
  }
  generating.value = true;
  progress.value = [];
  thinking.value = "";
  streamOutput.value = "";
  await nextTick();
  try {
    const res = await documentDecompositionService.generate({
      sourceName: sourceName.value,
      content: contentText.value,
    });
    applyResult(res);
  } catch (reason) {
    if (reason instanceof AiConfigMissingError) {
      configMissing.value = true;
    } else {
      error.value = reason instanceof Error ? reason.message : String(reason);
    }
  } finally {
    generating.value = false;
  }
}

async function commit(): Promise<void> {
  if (!selected.value.length || committing.value) return;
  committing.value = true;
  error.value = "";
  await nextTick();
  try {
    const res = await documentDecompositionService.commit(buildCommitInput(sourceName.value, drafts.value));
    pushToast(`已创建 ${res.createdCount} 项待办，并加入建议日期`);
    await router.push("/calendar");
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : String(reason);
  } finally {
    committing.value = false;
  }
}

function goToSettings(): void {
  void router.push({ path: "/settings", query: { tab: "advanced" } });
}

let unlisten: (() => void) | undefined;
let progressUnlisten: (() => void) | undefined;

onMounted(async () => {
  if (!isTauriDesktop()) return;
  try {
    progressUnlisten = await listen<DecompositionProgress>("decomposition-progress", (event) => {
      const payload = event.payload;
      if (payload.stage === "reasoning") {
        thinking.value += payload.detail;
        void nextTick(() => {
          const el = thinkingEl.value;
          if (el) el.scrollTop = el.scrollHeight;
        });
      } else if (payload.stage === "content") {
        streamOutput.value += payload.detail;
      } else {
        progress.value.push(payload);
      }
    });
  } catch {
    // 事件通道不可用时不影响拆解，只是看不到过程。
  }
  try {
    unlisten = await getCurrentWebview().onDragDropEvent((event) => {
      const payload = event.payload;
      if (payload.type === "over") dragging.value = true;
      else if (payload.type === "leave") dragging.value = false;
      else if (payload.type === "drop") {
        dragging.value = false;
        const path = payload.paths?.[0];
        if (path) void loadPath(path);
      }
    });
  } catch {
    // 拖拽不可用时仍可通过点击选择文件。
  }
});

onBeforeUnmount(() => {
  unlisten?.();
  progressUnlisten?.();
});
</script>

<template>
  <div class="decompose-page">
    <div class="steps">
      <template v-for="(label, index) in ['导入文档', '智能拆解', '确认待办']" :key="label">
        <span v-if="index > 0" class="step-line" aria-hidden="true" />
        <div class="step">
          <span class="step-index">{{ index + 1 }}</span>
          <span>{{ label }}</span>
        </div>
      </template>
    </div>

    <div class="decompose-body">
      <div class="decompose-inner">
        <div class="source-tabs" role="tablist" aria-label="文档输入方式">
          <button
            type="button"
            class="source-tab"
            :class="{ active: source === 'upload' }"
            @click="switchSource('upload')"
          >
            <UIcon name="i-lucide-upload" /> 上传文档
          </button>
          <button
            type="button"
            class="source-tab"
            :class="{ active: source === 'paste' }"
            @click="switchSource('paste')"
          >
            <UIcon name="i-lucide-clipboard-paste" /> 复制粘贴
          </button>
        </div>

        <div v-show="source === 'upload'" class="source-panel">
          <div class="upload-zone" :class="{ dragging }" @click="pickFile">
            <div>
              <span class="upload-icon"><UIcon name="i-lucide-file-up" /></span>
              <strong v-if="documentFile">{{ documentFile.name }}</strong>
              <strong v-else>拖拽文档到这里，或<em>选择文件</em></strong>
              <small v-if="documentFile">{{ documentFile.charCount.toLocaleString("zh-CN") }} 字 · 已提取正文</small>
              <small v-else>支持 Markdown、TXT、Word、PDF · 单个文件不超过 20 MB</small>
            </div>
          </div>
        </div>

        <div v-show="source === 'paste'" class="source-panel">
          <UTextarea
            v-model="pasted"
            :rows="10"
            class="w-full"
            placeholder="粘贴 PRD、会议纪要、需求说明或其他待拆解内容…"
          />
          <div class="paste-meta">
            <span>内容仅用于本次智能拆解</span>
            <span>{{ pasted.length.toLocaleString("zh-CN") }} 字</span>
          </div>
        </div>

        <UAlert
          v-if="configMissing"
          class="config-alert"
          color="warning"
          variant="subtle"
          icon="i-lucide-key-round"
          title="请先在「设置 › 高级」中完成 AI 配置"
          description="文档拆解复用设置页「高级」中的 Provider、模型和 API Key，本页面不展示也不修改这些配置。"
        >
          <template #actions>
            <UButton color="warning" variant="soft" size="sm" @click="goToSettings">前往设置</UButton>
          </template>
        </UAlert>
        <UAlert
          v-else-if="error"
          class="config-alert"
          color="error"
          variant="subtle"
          icon="i-lucide-circle-alert"
          :title="error"
        />

        <div class="generate-row">
          <span class="privacy-hint">
            <UIcon name="i-lucide-shield-check" />
            点击后正文会发送给设置页「高级」中配置的 AI 服务商
          </span>
          <UButton
            icon="i-lucide-sparkles"
            :loading="generating"
            :disabled="!canGenerate"
            @click="generate"
          >
            {{ result ? "重新拆解" : "开始拆解" }}
          </UButton>
        </div>

        <div v-if="progress.length || thinking || generating" class="progress-panel">
          <div class="progress-head">
            <UIcon name="i-lucide-activity" />
            <span>调用过程</span>
            <span class="progress-note">不记录正文与密钥</span>
          </div>
          <ul>
            <li v-for="(item, index) in progress" :key="index" :class="`stage-${item.stage}`">
              <span class="dot" />
              <span class="time">{{ (item.elapsedMs / 1000).toFixed(1) }}s</span>
              <span class="detail">{{ item.detail }}</span>
            </li>
            <li v-if="generating" class="stage-pending">
              <span class="dot pending" />
              <span class="time">…</span>
              <span class="detail">{{ streamOutput ? `已输出 ${streamOutput.length} 字` : "处理中…" }}</span>
            </li>
          </ul>

          <div v-if="thinking" class="thinking">
            <div class="thinking-head">
              <UIcon name="i-lucide-brain" />
              <span>模型思考过程</span>
            </div>
            <pre ref="thinkingEl">{{ thinking }}</pre>
          </div>

          <details v-if="streamOutput" class="thinking" :open="generating">
            <summary class="thinking-head">
              <UIcon name="i-lucide-list-checks" />
              <span>模型输出（实时）</span>
              <span class="summary-hint">{{ generating ? `已识别 ${liveTitles.length} 条任务` : "点击展开/收起" }}</span>
            </summary>
            <!-- 格式化视图：流式识别出的任务标题列表 -->
            <ul v-if="liveTitles.length" class="live-tasks">
              <li v-for="(title, index) in liveTitles" :key="index">
                <span class="dot ok" />
                <span class="live-title">{{ title }}</span>
              </li>
            </ul>
            <p v-else-if="generating" class="live-pending">正在生成结构化结果…</p>
            <!-- 完成后可展开查看格式化 JSON -->
            <details v-if="!generating" class="raw-json">
              <summary>原始 JSON</summary>
              <pre>{{ prettyStream }}</pre>
            </details>
          </details>
        </div>

        <template v-if="result">
          <div class="result-divider">拆解结果</div>
          <div class="source-card">
            <div class="file-icon"><UIcon name="i-lucide-file-text" /></div>
            <div>
              <strong>{{ sourceName }}</strong>
              <p>{{ charCount.toLocaleString("zh-CN") }} 字 · {{ groups.length }} 个模块</p>
            </div>
            <span class="status-pill">已生成</span>
          </div>

          <div class="analysis-summary">
            <div class="summary-item"><strong>{{ groups.length }}</strong><span>核心模块</span></div>
            <div class="summary-item"><strong>{{ drafts.length }}</strong><span>建议待办</span></div>
            <div class="summary-item"><strong>{{ formatEstimate(totalEstimate || null) }}</strong><span>选中工作量</span></div>
          </div>

          <div v-if="result.summary || result.assumptions.length" class="ai-note">
            <UIcon name="i-lucide-sparkles" />
            <span>
              {{ result.summary }}
              <template v-if="result.assumptions.length">
                <br />前提假设：{{ result.assumptions.join("；") }}
              </template>
            </span>
          </div>

          <div class="select-row">
            <UButton color="neutral" variant="ghost" size="xs" @click="drafts.forEach((d) => (d.selected = true))">
              全选
            </UButton>
            <UButton color="neutral" variant="ghost" size="xs" @click="drafts.forEach((d) => (d.selected = false))">
              全不选
            </UButton>
          </div>

          <div v-for="group in groups" :key="group.id" class="requirement-group">
            <div class="requirement-title">
              <UIcon name="i-lucide-list-checks" />
              {{ group.title }}
              <span class="count">{{ group.tasks.length }} TASKS</span>
            </div>
            <DecompositionTaskCard
              v-for="draft in group.tasks"
              :key="draft.id"
              :draft="draft"
              :dependency-titles="dependencyTitles"
            />
          </div>
        </template>
      </div>
    </div>

    <footer class="decompose-foot">
      <span class="hint"><strong>{{ selected.length }}</strong> 项待办将被创建，并按建议日期加入日历</span>
      <UButton color="neutral" variant="ghost" @click="router.push('/calendar')">返回日历</UButton>
      <UButton
        icon="i-lucide-check"
        :loading="committing"
        :disabled="!selected.length"
        @click="commit"
      >
        创建 {{ selected.length }} 项待办
      </UButton>
    </footer>
  </div>
</template>

<style scoped>
.decompose-page {
  display: grid;
  grid-template-rows: auto minmax(0, 1fr) auto;
  min-width: 0;
  min-height: 0;
  height: 100%;
  background: transparent;
}
.steps {
  display: flex;
  align-items: center;
  justify-content: center;
  flex-wrap: wrap;
  gap: 12px;
  padding: 14px 22px;
  border-bottom: 1px solid var(--border);
  background: var(--surface-2);
}
.step-line {
  width: 56px;
  height: 1px;
  background: var(--border-strong);
}
.step {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  color: var(--accent);
  font-size: 10.5px;
  font-weight: 600;
}
.step-index {
  display: grid;
  place-items: center;
  width: 28px;
  height: 28px;
  flex: none;
  border: 1px solid var(--accent);
  border-radius: 50%;
  background: var(--accent);
  color: var(--accent-contrast);
  font-family: var(--font-mono);
  font-size: 10px;
}
.decompose-body {
  min-height: 0;
  padding: 18px 22px;
  overflow: auto;
}
.decompose-inner {
  max-width: 960px;
  margin: 0 auto;
}
.source-tabs {
  display: flex;
  gap: 2px;
  width: fit-content;
  margin-bottom: 12px;
  padding: 3px;
  border: 1px solid var(--border);
  border-radius: 9px;
  background: var(--surface-3);
}
.source-tab {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 27px;
  padding: 0 11px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--text-muted);
  font-size: 10.5px;
  font-weight: 650;
  cursor: pointer;
}
.source-tab.active {
  background: var(--surface);
  color: var(--accent);
  box-shadow: var(--shadow-sm);
}
.source-tab :deep(svg) {
  width: 13px;
  height: 13px;
}
.upload-zone {
  display: grid;
  place-items: center;
  min-height: 140px;
  padding: 17px;
  border: 1px dashed var(--border-strong);
  border-radius: 11px;
  background: var(--surface);
  text-align: center;
  cursor: pointer;
}
.upload-zone.dragging {
  border-color: var(--accent);
  background: var(--accent-weak);
}
.upload-icon {
  display: grid;
  place-items: center;
  width: 34px;
  height: 34px;
  margin: 0 auto 8px;
  border-radius: 9px;
  background: var(--accent-weak);
  color: var(--accent);
}
.upload-zone strong {
  display: block;
  font-size: 12px;
}
.upload-zone em {
  color: var(--accent);
  font-style: normal;
  font-weight: 650;
}
.upload-zone small {
  display: block;
  margin-top: 5px;
  color: var(--text-subtle);
  font-size: 10.5px;
}
.paste-meta {
  display: flex;
  justify-content: space-between;
  margin-top: 6px;
  color: var(--text-subtle);
  font-size: 10px;
}
.config-alert {
  margin-top: 12px;
}
.generate-row {
  display: flex;
  align-items: center;
  gap: 12px;
  margin: 14px 0 2px;
}
.privacy-hint {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  margin-right: auto;
  color: var(--text-subtle);
  font-size: 10.5px;
}
.privacy-hint :deep(svg) {
  width: 13px;
  height: 13px;
}
.progress-panel {
  margin-top: 12px;
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--surface-2);
}
.progress-head {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 6px;
  color: var(--text-muted);
  font-size: 10.5px;
  font-weight: 700;
  letter-spacing: 0.04em;
}
.progress-head :deep(svg) {
  width: 13px;
  height: 13px;
}
.progress-note {
  margin-left: auto;
  color: var(--text-subtle);
  font-weight: 500;
  letter-spacing: 0;
}
.progress-panel ul {
  margin: 0;
  padding: 0;
  list-style: none;
  display: grid;
  gap: 3px;
}
.progress-panel li {
  display: grid;
  grid-template-columns: 8px 44px minmax(0, 1fr);
  align-items: center;
  gap: 8px;
  color: var(--text-muted);
  font-size: 10.5px;
}
.progress-panel .dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--accent);
}
.progress-panel .time {
  color: var(--text-subtle);
  font-family: var(--font-mono);
  font-size: 10px;
  text-align: right;
}
.progress-panel .detail {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.progress-panel li.stage-error .dot {
  background: var(--u-crit);
}
.progress-panel li.stage-error .detail {
  color: var(--u-crit);
}
.progress-panel .dot.pending {
  background: var(--text-subtle);
  animation: progress-pulse 1s ease-in-out infinite;
}
.thinking {
  margin-top: 8px;
  padding-top: 8px;
  border-top: 1px dashed var(--border);
}
.thinking-head {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 5px;
  color: var(--text-muted);
  font-size: 10.5px;
  font-weight: 700;
}
.thinking-head :deep(svg) {
  width: 13px;
  height: 13px;
}
.thinking summary {
  cursor: pointer;
  list-style: none;
}
.thinking summary::-webkit-details-marker {
  display: none;
}
.summary-hint {
  margin-left: auto;
  color: var(--text-subtle);
  font-size: 9.5px;
  font-weight: 500;
}
.live-tasks {
  margin: 2px 0 6px;
  padding: 0;
  display: grid;
  gap: 3px;
  list-style: none;
}
.live-tasks li {
  display: flex;
  align-items: center;
  gap: 7px;
}
.live-tasks .dot {
  flex: none;
  width: 6px;
  height: 6px;
  border-radius: 999px;
  background: var(--u-ok);
}
.live-title {
  min-width: 0;
  overflow: hidden;
  color: var(--text);
  font-size: 11.5px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.live-pending {
  margin: 2px 0 6px;
  color: var(--text-subtle);
  font-size: 11px;
}
.raw-json summary {
  cursor: pointer;
  color: var(--text-subtle);
  font-size: 10.5px;
  user-select: none;
}
.raw-json summary:hover {
  color: var(--text-muted);
}
.raw-json pre {
  max-height: 240px;
  margin: 6px 0 0;
  overflow: auto;
}
.thinking pre {
  max-height: 220px;
  margin: 0;
  overflow: auto;
  color: var(--text-muted);
  font-family: var(--font-mono);
  font-size: 10.5px;
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-word;
}
@keyframes progress-pulse {
  0%, 100% { opacity: 0.3; }
  50% { opacity: 1; }
}
.result-divider {
  display: flex;
  align-items: center;
  gap: 9px;
  margin: 18px 0 14px;
  color: var(--text-subtle);
  font-size: 9.5px;
  font-weight: 700;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
.result-divider::before,
.result-divider::after {
  content: "";
  flex: 1;
  height: 1px;
  background: var(--border);
}
.source-card {
  display: grid;
  grid-template-columns: 36px minmax(0, 1fr) auto;
  align-items: center;
  gap: 11px;
  padding: 13px 14px;
  border: 1px solid var(--border);
  border-radius: 11px;
  background: var(--surface);
}
.file-icon {
  display: grid;
  place-items: center;
  width: 36px;
  height: 36px;
  border-radius: 9px;
  background: var(--accent-weak);
  color: var(--accent);
}
.source-card strong {
  display: block;
  font-size: 12px;
}
.source-card p {
  margin: 4px 0 0;
  color: var(--text-subtle);
  font-size: 10px;
}
.status-pill {
  padding: 4px 8px;
  border-radius: 999px;
  background: var(--accent-weak);
  color: var(--accent);
  font-size: 10px;
  font-weight: 700;
}
.analysis-summary {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 9px;
  margin: 14px 0;
}
.summary-item {
  padding: 10px 11px;
  border: 1px solid var(--border);
  border-radius: 9px;
  background: var(--surface);
}
.summary-item strong {
  display: block;
  font-family: var(--font-mono);
  font-size: 16px;
}
.summary-item span {
  display: block;
  margin-top: 3px;
  color: var(--text-subtle);
  font-size: 10px;
}
.ai-note {
  display: flex;
  gap: 8px;
  margin-bottom: 14px;
  padding: 10px 11px;
  border-radius: 9px;
  background: var(--accent-weak);
  color: var(--accent);
  font-size: 10.5px;
  line-height: 1.55;
}
.ai-note :deep(svg) {
  flex: none;
  margin-top: 1px;
  width: 14px;
  height: 14px;
}
.select-row {
  display: flex;
  gap: 4px;
  margin-bottom: 6px;
}
.requirement-group {
  margin-top: 16px;
}
.requirement-group :deep(.draft-card) {
  margin-bottom: 7px;
}
.requirement-title {
  display: flex;
  align-items: center;
  gap: 7px;
  margin-bottom: 7px;
  font-size: 11.5px;
  font-weight: 700;
}
.requirement-title :deep(svg) {
  width: 14px;
  height: 14px;
  color: var(--text-muted);
}
.requirement-title .count {
  color: var(--text-subtle);
  font-family: var(--font-mono);
  font-size: 9px;
}
.decompose-foot {
  display: flex;
  align-items: center;
  gap: 9px;
  padding: 13px 22px;
  border-top: 1px solid var(--border);
  background: var(--surface);
}
.decompose-foot .hint {
  margin-right: auto;
  color: var(--text-subtle);
  font-size: 10.5px;
}
.decompose-foot .hint strong {
  color: var(--text);
}
</style>
