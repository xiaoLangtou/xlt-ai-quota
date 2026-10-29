<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { useLocalStorage } from "@vueuse/core";
import { emitTo, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  ArrowLeftRight,
  ChevronDown,
  ChevronUp,
  Copy,
  Eraser,
  Languages,
  Loader2,
  PanelTopOpen,
  Pin,
  SlidersHorizontal,
  AlertTriangle,
  X,
} from "lucide-vue-next";
import { Select } from "@/components/ui/select";
import { clipboardService } from "@/services/clipboard-service";
import { useTranslationHistory } from "@/composables/useTranslationHistory";
import { completedTranslation, mergeTranslationSegment } from "@/utils/translate-workspace";
import { translateService } from "@/services/translate-service";
import {
  TRANSLATE_ENGINE_LABELS,
  TRANSLATE_SOURCE_LANGUAGES,
  TRANSLATE_TARGET_LANGUAGES,
  translateConfigured,
  type TranslateProgress,
  type TranslateSegment,
} from "@/types/translate";

type PanelState = "idle" | "loading" | "done" | "error";

/** 显示后这段时间内的失焦不算「用户离开」，避免窗口刚出现就被隐藏。 */
const FOCUS_GRACE_MS = 300;

const text = ref("");
const from = useLocalStorage("xlt.translate.panel.source-language", "auto");
const to = useLocalStorage("xlt.translate.panel.target-language", "zh-CHS");
const state = ref<PanelState>("idle");
const translated = ref("");
const panelSegments = ref<TranslateSegment[]>([]);
let panelSession: { source: string; from: string; to: string; version: string; historyId: string; engine: string } | null = null;
const statusLabels = { queued: "等待中", running: "翻译中", done: "已完成", failed: "失败", cancelled: "已取消" };
const lang = ref("");
const elapsed = ref(0);
const error = ref("");
const configured = ref(true);
const engineLabel = ref("有道翻译");
const engineBadge = ref({ text: "有", color: "#e8564a" });
const pinned = ref(false);
const collapsed = ref(false);
const input = ref<HTMLTextAreaElement | null>(null);
const unlisteners: UnlistenFn[] = [];
const history = useTranslationHistory();
let activeRequestId = "";
let request = 0;
let focusedAt = 0;

const langLabel = computed(() => {
  if (!lang.value) return "";
  const [detected, target] = lang.value.split("2");
  return target ? `${detected} → ${target}` : lang.value;
});
const seconds = computed(() => (elapsed.value / 1000).toFixed(1));

watch([text, from, to], () => {
  request++;
  void cancelActive();
  state.value = "idle";
  translated.value = "";
  panelSegments.value = [];
  panelSession = null;
  lang.value = "";
  error.value = "";
}, { flush: "sync" });

function hide(): void {
  void getCurrentWindow().hide();
}

function swap(): void {
  if (from.value === "auto") {
    // 自动检测没法反向，交换后源语言取当前目标，目标退回另一种常用语言。
    const nextTarget = to.value === "zh-CHS" ? "en" : "zh-CHS";
    from.value = to.value;
    to.value = nextTarget;
  } else {
    const previous = from.value;
    from.value = to.value;
    to.value = previous;
  }
  if (text.value.trim()) void run();
}

async function cancelActive(): Promise<boolean> {
  const id = activeRequestId;
  if (!id) return true;
  try { await translateService.cancel(id); if (activeRequestId === id) activeRequestId = ""; return true; }
  catch (reason) { error.value = `取消失败：${String(reason)}`; state.value = "error"; return false; }
}
async function run(): Promise<void> { await executePanel(); }
async function executePanel(retry?: TranslateSegment): Promise<void> {
  const content = text.value.trim();
  if (!content || (retry && state.value === "loading")) return;
  if (!configured.value) {
    state.value = "error"; error.value = `尚未配置${engineLabel.value}，请先在设置页填写凭据`; return;
  }
  if (retry && !panelSession?.version) return;
  const token = ++request;
  if (!(await cancelActive()) || token !== request) return;
  const requestId = crypto.randomUUID();
  activeRequestId = requestId;
  if (!retry) {
    panelSegments.value = []; translated.value = "";
    panelSession = { source: content, from: from.value, to: to.value, version: "", historyId: requestId, engine: "" };
  }
  const current = panelSession!;
  const saveHistory = history.enabled.value;
  state.value = "loading"; error.value = "";
  try {
    const outcome = retry
      ? await translateService.retry(retry.original, retry.index, current.from, current.to, requestId, current.version)
      : await translateService.run(content, current.from, current.to, requestId);
    if (token !== request) return;
    for (const segment of outcome.segments) panelSegments.value = mergeTranslationSegment(panelSegments.value, segment);
    current.version = outcome.configVersion; current.engine = outcome.engine;
    translated.value = completedTranslation(panelSegments.value);
    const failures = panelSegments.value.filter(segment => segment.status === "failed");
    error.value = outcome.cancelled ? "已取消翻译" : failures.length ? "部分段落失败，可单独重试" : "";
    if (saveHistory) {
      const saved = history.save({ id: current.historyId, createdAt: Date.now(), source: current.source, from: current.from, to: current.to, engine: current.engine, segments: panelSegments.value }, outcome.cancelled);
      if (saved === "too-large") error.value += " 内容超过 32KB，未保存到历史。";
    }
    lang.value = outcome.lang; elapsed.value = outcome.ms;
    state.value = panelSegments.value.length ? "done" : outcome.cancelled ? "idle" : "error";
    collapsed.value = false;
  } catch (reason) {
    if (token !== request) return;
    error.value = String(reason); state.value = panelSegments.value.length ? "done" : "error";
  } finally { if (activeRequestId === requestId) activeRequestId = ""; }
}

async function copy(text_: string): Promise<void> {
  if (!text_) return;
  await clipboardService.writeText(text_);
}

function clear(): void {
  request++;
  void cancelActive();
  text.value = "";
  translated.value = "";
  panelSegments.value = [];
  panelSession = null;
  error.value = "";
  state.value = "idle";
  input.value?.focus();
}

async function openInPage(): Promise<void> {
  const content = text.value;
  if (!(await cancelActive())) return;
  request++;
  state.value = "idle";
  await invoke("show_dashboard");
  await emitTo("main", "translate:open", { text: content, from: from.value, to: to.value });
  hide();
}

async function openSettings(): Promise<void> {
  await invoke("show_dashboard");
  await emitTo("main", "translate:settings");
  hide();
}

function onKeydown(event: KeyboardEvent): void {
  if (event.defaultPrevented) return;
  if (event.key === "Escape") {
    event.preventDefault();
    if (pinned.value) return;
    hide();
    return;
  }
  if (event.key === "Enter" && event.target === input.value && !event.shiftKey && !event.isComposing) {
    event.preventDefault();
    void run();
  }
}

onMounted(async () => {
  window.addEventListener("keydown", onKeydown);
  const win = getCurrentWindow();
  unlisteners.push(await listen<TranslateProgress>("translate:progress", ({ payload: update }) => {
    if (update.requestId !== activeRequestId || state.value !== "loading") return;
    if (panelSession) panelSession.version = update.configVersion;
    panelSegments.value = mergeTranslationSegment(panelSegments.value, { index: update.index, original: update.original, translated: update.translated, status: update.status, error: update.error });
    translated.value = completedTranslation(panelSegments.value);
  }));
  unlisteners.push(
    await win.onFocusChanged(({ payload }) => {
      if (!payload) {
        // 面板是独立置顶窗口，刚显示时可能瞬间失焦；太早隐藏会表现成「按了没反应」。
        if (pinned.value) return;
        if (focusedAt && Date.now() - focusedAt < FOCUS_GRACE_MS) return;
        hide();
        return;
      }
      focusedAt = Date.now();
      void refreshConfig();
      nextTick(() => {
        input.value?.focus();
        input.value?.select();
      });
    }),
  );
  await refreshConfig();
  input.value?.focus();
});

onUnmounted(() => {
  request++;
  void cancelActive();
  window.removeEventListener("keydown", onKeydown);
  for (const stop of unlisteners) stop();
});

async function refreshConfig(): Promise<void> {
  try {
    const config = await translateService.getConfig();
    configured.value = translateConfigured(config);
    engineLabel.value = TRANSLATE_ENGINE_LABELS[config.engine] ?? config.engine;
    engineBadge.value =
      config.engine === "deepseek"
        ? { text: "D", color: "#4d6bfe" }
        : { text: "有", color: "#e8564a" };
  } catch {
    configured.value = false;
  }
}
</script>

<template>
  <main class="tp">
    <header class="tp-head" data-tauri-drag-region>
      <span class="tp-brand" data-tauri-drag-region><Languages :size="16" /> 快捷翻译</span>
      <span class="tp-spacer" />
      <button
        class="tp-icon"
        type="button"
        :aria-pressed="pinned"
        :title="pinned ? '取消固定（失焦不隐藏）' : '固定面板（失焦不隐藏）'"
        @click="pinned = !pinned"
      >
        <Pin :size="15" />
      </button>
      <button class="tp-icon" type="button" title="翻译设置" @click="openSettings">
        <SlidersHorizontal :size="15" />
      </button>
      <button class="tp-icon" type="button" title="关闭" @click="hide">
        <X :size="16" />
      </button>
    </header>

    <section class="tp-input">
      <label class="tp-section-label" for="quick-translate-text">原文</label>
      <textarea
        id="quick-translate-text"
        ref="input"
        v-model="text"
        class="tp-textarea"
        placeholder="输入或粘贴要翻译的文本，Enter 翻译"
        spellcheck="false"
      />
      <div class="tp-input-actions">
        <button class="tp-tool" type="button" title="复制原文" :disabled="!text.trim()" @click="copy(text)">
          <Copy :size="15" />
        </button>
        <button class="tp-tool" type="button" title="清空" :disabled="!text.trim()" @click="clear">
          <Eraser :size="15" />
        </button>
        <button class="tp-tool" type="button" title="在翻译页打开" :disabled="!text.trim()" @click="openInPage">
          <PanelTopOpen :size="15" />
        </button>
        <button v-if="state === 'loading'" class="tp-tool tp-cancel" type="button" @click="cancelActive">取消</button>
        <span class="tp-shortcut">Enter 翻译</span>
        <button
          class="tp-go"
          type="button"
          :disabled="!text.trim() || state === 'loading'"
          title="翻译（Enter）"
          @click="run"
        >
          <Loader2 v-if="state === 'loading'" :size="17" class="tp-spin" />
          <Languages v-else :size="17" />
          <span>{{ state === 'loading' ? '翻译中' : '翻译' }}</span>
        </button>
      </div>
    </section>

    <div class="tp-pairs">
      <Select v-model="from" :options="TRANSLATE_SOURCE_LANGUAGES" />
      <button class="tp-swap" type="button" title="交换语言方向" @click="swap">
        <ArrowLeftRight :size="14" />
      </button>
      <Select v-model="to" :options="TRANSLATE_TARGET_LANGUAGES" />
    </div>

    <section class="tp-results">
      <article v-if="panelSegments.length" class="tp-card">
        <header
          class="tp-card-head"
          role="button"
          tabindex="0"
          :aria-expanded="!collapsed"
          @click="collapsed = !collapsed"
          @keydown.enter.prevent="collapsed = !collapsed"
          @keydown.space.prevent="collapsed = !collapsed"
        >
          <span class="tp-badge" :style="{ background: engineBadge.color }">{{ engineBadge.text }}</span>
          <span class="tp-engine">{{ panelSession?.engine ? TRANSLATE_ENGINE_LABELS[panelSession.engine] : engineLabel }}</span>
          <span class="tp-card-meta">{{ state === 'loading' ? '翻译中…' : `${langLabel} · ${seconds}s` }}</span>
          <component :is="collapsed ? ChevronDown : ChevronUp" :size="15" class="tp-chevron" />
        </header>
        <div v-if="!collapsed" class="tp-card-body">
          <p v-if="error" class="tp-error-text" role="status">{{ error }}</p>
          <p class="tp-section-label tp-output-label">译文</p>
          <article v-for="segment in panelSegments" :key="segment.index" class="tp-segment">
            <div class="tp-segment-head"><span>第 {{ segment.index + 1 }} 段 · {{ statusLabels[segment.status] }}</span>
              <button v-if="segment.status === 'done'" type="button" class="tp-segment-action" @click="copy(segment.translated)">复制本段</button>
              <button v-if="panelSession && ['failed', 'cancelled'].includes(segment.status)" type="button" class="tp-segment-action" :disabled="state === 'loading'" @click="executePanel(segment)">重试本段</button>
            </div>
            <p v-if="segment.status === 'done'" class="tp-result">{{ segment.translated }}</p>
            <p v-else-if="segment.error" class="tp-error-text">{{ segment.error }}</p>
          </article>
          <footer class="tp-card-actions">
            <button class="tp-tool tp-copy-result" type="button" :disabled="!translated" title="复制已完成译文" @click="copy(translated)">
              <Copy :size="14" />
              <span>复制已完成译文</span>
            </button>
            <button class="tp-tool" type="button" title="在翻译页打开" @click="openInPage">
              <PanelTopOpen :size="14" />
            </button>
          </footer>
        </div>
      </article>

      <article v-else-if="state === 'error'" class="tp-card tp-card-error">
        <header class="tp-card-head">
          <span class="tp-badge" :style="{ background: engineBadge.color }">{{ engineBadge.text }}</span>
          <span class="tp-engine">{{ panelSession?.engine ? TRANSLATE_ENGINE_LABELS[panelSession.engine] : engineLabel }}</span>
          <span class="tp-spacer" />
          <AlertTriangle :size="15" class="tp-alert" />
        </header>
        <div class="tp-card-body">
          <p class="tp-error-text">{{ error }}</p>
          <footer class="tp-card-actions">
            <button class="tp-tool" type="button" title="重试" @click="run">
              <Loader2 :size="14" />
            </button>
          </footer>
        </div>
      </article>

      <div v-else-if="state === 'loading'" class="tp-hint tp-loading">
        <Loader2 :size="16" class="tp-spin" />
        <span>翻译中…</span>
      </div>

      <div v-else class="tp-hint">
        <Languages :size="24" />
        <span class="tp-empty-title">{{ error || '让语言更简单' }}</span>
        <span>输入文字，或粘贴需要翻译的内容</span>
      </div>
    </section>
    <footer class="tp-bottom"><span>Shift + Enter 换行</span><span>Esc 关闭</span></footer>
  </main>
</template>

<style scoped>
.tp {
  display: flex;
  flex-direction: column;
  gap: 12px;
  height: 100vh;
  padding: 12px 16px 10px;
  border: 1px solid var(--border-strong);
  border-radius: 18px;
  background: linear-gradient(160deg, var(--surface), var(--surface-2));
  color: var(--text);
  box-shadow: none;
  overflow: hidden;
}
.tp-head {
  display: flex;
  align-items: center;
  gap: 4px;
  height: 30px;
  flex: none;
}
.tp-spacer {
  flex: 1;
}
.tp-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border: 0;
  border-radius: 7px;
  background: transparent;
  color: var(--text-subtle);
  cursor: pointer;
}
.tp-icon:hover {
  background: var(--surface-3);
  color: var(--text);
}
.tp-icon[aria-pressed="true"] {
  background: var(--accent-weak);
  color: var(--accent);
}

.tp-input {
  display: flex;
  flex: none;
  flex-direction: column;
  gap: 8px;
  border: 1px solid var(--border);
  border-radius: 14px;
  background: var(--surface);
  padding: 12px;
  transition: border-color 150ms ease, box-shadow 150ms ease;
}
.tp-input:focus-within {
  border-color: color-mix(in srgb, var(--accent) 65%, var(--border));
  box-shadow: 0 0 0 3px var(--accent-weak);
}
.tp-textarea {
  width: 100%;
  min-height: 64px;
  max-height: 108px;
  resize: none;
  border: 0;
  background: transparent;
  font: inherit;
  font-size: 14px;
  line-height: 1.65;
  color: var(--text);
}
.tp-textarea:focus {
  outline: none;
}
.tp-textarea::placeholder {
  color: var(--text-subtle);
}
.tp-input-actions {
  display: flex;
  align-items: center;
  gap: 2px;
}
.tp-tool {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--text-subtle);
  cursor: pointer;
}
.tp-tool:hover:not(:disabled) {
  background: var(--surface-3);
  color: var(--text);
}
.tp-tool:disabled {
  opacity: 0.4;
  cursor: default;
}
.tp-shortcut {
  margin-left: auto;
  margin-right: 4px;
  font-size: 10px;
  color: var(--text-subtle);
}
.tp-go {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  min-width: 76px;
  padding: 0 12px;
  height: 34px;
  font-size: 12px;
  font-weight: 600;
  margin-left: 6px;
  border: 0;
  border-radius: 10px;
  background: var(--accent);
  color: var(--accent-contrast);
  cursor: pointer;
}
.tp-go:hover:not(:disabled) {
  background: var(--accent-hover);
}
.tp-go:disabled {
  opacity: 0.5;
  cursor: default;
}

.tp-pairs {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 32px minmax(0, 1fr);
  align-items: center;
  gap: 6px;
  flex: none;
}
.tp-swap {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--surface-2);
  color: var(--text-subtle);
  cursor: pointer;
}
.tp-swap:hover {
  border-color: var(--border-strong);
  color: var(--text);
}

.tp-results {
  display: flex;
  flex: 1;
  min-height: 0;
  flex-direction: column;
  gap: 8px;
  overflow-y: auto;
}
.tp-card {
  flex: none;
  overflow: hidden;
  border: 1px solid var(--border);
  border-radius: 14px;
  background: var(--surface);
  box-shadow: 0 2px 8px rgba(16, 18, 26, 0.03);
}
.tp-card-error {
  border-color: color-mix(in srgb, var(--u-crit) 40%, var(--border));
}
.tp-card-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 12px;
  border-bottom: 1px solid var(--border);
  background: color-mix(in srgb, var(--surface-2) 65%, var(--surface));
  cursor: pointer;
}
.tp-card-error .tp-card-head {
  cursor: default;
}
.tp-badge {
  display: grid;
  place-items: center;
  width: 20px;
  height: 20px;
  flex: none;
  border-radius: 6px;
  font-size: 11px;
  font-weight: 700;
  color: #fff;
}
.tp-engine {
  font-size: 12.5px;
  font-weight: 600;
  color: var(--text);
}
.tp-card-meta {
  margin-left: auto;
  font-size: 11px;
  color: var(--text-subtle);
}
.tp-chevron {
  color: var(--text-subtle);
}
.tp-alert {
  color: var(--u-crit);
}
.tp-card-body {
  padding: 14px 14px 10px;
}
.tp-result {
  margin: 0;
  white-space: pre-wrap;
  font-size: 14px;
  line-height: 1.8;
  overflow-wrap: anywhere;
  color: var(--text);
}
.tp-error-text {
  margin: 0;
  font-size: 12.5px;
  line-height: 1.7;
  color: var(--u-crit);
}
.tp-card-actions {
  display: flex;
  gap: 2px;
  align-items: center;
  margin-top: 12px;
  padding-top: 8px;
  border-top: 1px solid var(--border);
}
.tp-hint {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  flex: 1;
  padding: 12px;
  text-align: center;
  font-size: 12.5px;
  color: var(--text-subtle);
}
.tp-spin {
  animation: tp-spin 0.9s linear infinite;
}
@keyframes tp-spin {
  to {
    transform: rotate(360deg);
  }
}

.tp-brand { display: inline-flex; align-items: center; gap: 7px; font-size: 12px; font-weight: 600; color: var(--text); }
.tp-brand svg { color: var(--accent); }
.tp-section-label { font-size: 10px; font-weight: 600; color: var(--text-subtle); letter-spacing: .06em; }
.tp-output-label { margin: 0 0 6px; }
.tp-cancel { width: auto; padding: 0 8px; font-size: 12px; color: var(--accent); }
.tp-copy-result { width: auto; gap: 5px; padding: 0 8px; font-size: 11px; }
.tp-bottom { display: flex; justify-content: space-between; flex: none; padding: 0 2px; font-size: 10px; color: var(--text-subtle); }
.tp-hint { flex-direction: column; gap: 9px; }
.tp-loading { flex-direction: row; }
.tp-hint > svg { color: var(--accent); opacity: .65; }
.tp-empty-title { font-size: 14px; font-weight: 500; color: var(--text); }
.tp-icon:focus-visible, .tp-tool:focus-visible, .tp-go:focus-visible, .tp-swap:focus-visible, .tp-card-head:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
@media (max-width: 360px) { .tp { padding-inline: 10px; } .tp-shortcut { display: none; } .tp-go { margin-left: auto; } .tp-card-meta { font-size: 10px; } }
@media (prefers-reduced-motion: reduce) { .tp-input { transition: none; } }
</style>

<style scoped>
.tp-segment + .tp-segment { border-top: 1px solid var(--border); margin-top: 12px; padding-top: 12px; }
.tp-segment-head { display: flex; align-items: center; justify-content: space-between; gap: 8px; font-size: 10px; color: var(--text-subtle); margin-bottom: 6px; }
.tp-segment-action { border: 0; background: transparent; color: var(--accent); font-size: 11px; cursor: pointer; }
.tp-segment-action:disabled { opacity: .4; cursor: default; }
</style>
