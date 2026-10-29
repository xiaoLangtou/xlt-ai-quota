<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useLocalStorage } from "@vueuse/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useRouter } from "vue-router";
import { ClipboardCopy, Languages, Loader2, Settings2 } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Select } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import TranslateHistoryPanel from "@/components/translate/TranslateHistoryPanel.vue";
import { pushToast } from "@/composables/useToast";
import { useTranslateRequest } from "@/composables/useTranslateRequest";
import { useTranslationHistory } from "@/composables/useTranslationHistory";
import { completedTranslation, mergeTranslationSegment, type TranslationHistoryEntry } from "@/utils/translate-workspace";
import { clipboardService } from "@/services/clipboard-service";
import { translateService } from "@/services/translate-service";
import { DEFAULT_TRANSLATE_CONFIG, TRANSLATE_ENGINES, TRANSLATE_ENGINE_LABELS, TRANSLATE_PAIRS, translateConfigured, languageLabel,
  type TranslateConfig, type TranslateSegment, type TranslateProgress } from "@/types/translate";

const router = useRouter();
const { pendingRequest, take } = useTranslateRequest();
const history = useTranslationHistory();
const historyOpen = ref(false);
const restoredHistory = ref(false);
const source = ref("");
const pairValue = useLocalStorage("xlt.translate.page.language-pair", "0");
const customPair = useLocalStorage("xlt.translate.page.custom-pair", { from: "auto", to: "zh-CHS" });
const segments = ref<TranslateSegment[]>([]);
let activeRequest = "";
let session: { source: string; from: string; to: string; version: string; historyId: string; engine: string } | null = null;
const lang = ref("");
const elapsed = ref(0);
const requests = ref(0);
const translating = ref(false);
const cancelling = ref(false);
const error = ref("");
const progress = ref({ done: 0, total: 0 });
const config = ref<TranslateConfig>({ ...DEFAULT_TRANSLATE_CONFIG });
const configReady = ref(false);
const statusLabels = { queued: "等待中", running: "翻译中", done: "已完成", failed: "失败", cancelled: "已取消" };
const completedCount = computed(() => segments.value.filter(item => item.status === "done").length);
const partial = computed(() => completedCount.value < segments.value.length || translating.value);
const pairOptions = computed(() => [
  ...TRANSLATE_PAIRS.map((item, index) => ({ label: item.label, value: String(index) })),
  ...(pairValue.value === "panel" ? [{ label: `${languageLabel(customPair.value.from)} → ${languageLabel(customPair.value.to)}`, value: "panel" }] : []),
]);
const engineOptions = TRANSLATE_ENGINES;
const pair = computed(() => pairValue.value === "panel" ? customPair.value : TRANSLATE_PAIRS[Number(pairValue.value)] ?? TRANSLATE_PAIRS[0]);
const configured = computed(() => translateConfigured(config.value));
const engineLabel = computed(() => TRANSLATE_ENGINE_LABELS[config.value.engine] ?? config.value.engine);
const paragraphCount = computed(() => source.value.split(/\n\s*\n/).filter(block => block.trim()).length);
const charCount = computed(() => source.value.length);
const langLabel = computed(() => lang.value.replace("2", " → "));
let stopProgress: UnlistenFn | undefined;
let disposed = false;
let handoff = 0;

onMounted(async () => {
  try { config.value = await translateService.getConfig(); }
  catch (reason) { error.value = String(reason); }
  if (disposed) return;
  try {
    stopProgress = await listen<TranslateProgress>("translate:progress", ({ payload: update }) => {
      if (!translating.value || update.requestId !== activeRequest) return;
      progress.value = { done: Math.max(progress.value.done, update.done), total: update.total };
      if (session) session.version = update.configVersion;
      segments.value = mergeTranslationSegment(segments.value, { index: update.index, original: update.original, translated: update.translated, status: update.status, error: update.error });
    });
    configReady.value = true;
  } catch (reason) { error.value = `无法接收翻译进度：${String(reason)}`; }
  if (disposed) { stopProgress?.(); return; }
  void consumePending();
});
onUnmounted(() => {
  disposed = true;
  const id = activeRequest; activeRequest = "";
  if (id) void translateService.cancel(id).catch(() => undefined);
  stopProgress?.();
});
watch(pendingRequest, value => { if (value) void consumePending(); });
function selectPair(from: string, to: string): void {
  const index = TRANSLATE_PAIRS.findIndex(item => item.from === from && item.to === to);
  if (index >= 0) pairValue.value = String(index);
  else { customPair.value = { from, to }; pairValue.value = "panel"; }
}
async function consumePending(): Promise<void> {
  if (!configReady.value) return;
  const incoming = take();
  if (!incoming?.text.trim()) return;
  const ticket = ++handoff;
  if (!(await cancelCurrent()) || ticket !== handoff || disposed) return;
  activeRequest = ""; translating.value = false;
  source.value = incoming.text;
  selectPair(incoming.from, incoming.to);
  void run();
}
async function cancelCurrent(): Promise<boolean> {
  if (!activeRequest || !translating.value) return true;
  cancelling.value = true;
  try { await translateService.cancel(activeRequest); return true; }
  catch (reason) { if (!disposed) pushToast(`取消失败：${String(reason)}`, "error"); return false; }
  finally { cancelling.value = false; }
}
async function run(): Promise<void> { await execute(); }
async function retrySegment(segment: TranslateSegment): Promise<void> { await execute(segment); }
async function execute(retry?: TranslateSegment): Promise<void> {
  if (translating.value || !configReady.value || !source.value.trim()) return;
  if (!configured.value) { openSettings(); pushToast(`请先配置${engineLabel.value}`, "warn"); return; }
  if (retry && (!session?.version || !["failed", "cancelled"].includes(retry.status))) return;
  const requestId = crypto.randomUUID();
  restoredHistory.value = false;
  activeRequest = requestId;
  if (!retry) {
    session = { source: source.value.trim(), from: pair.value.from, to: pair.value.to, version: "", historyId: crypto.randomUUID(), engine: config.value.engine };
    segments.value = []; lang.value = ""; requests.value = 0;
  }
  const current = session!;
  const saveHistory = history.enabled.value;
  translating.value = true; error.value = "";
  progress.value = { done: 0, total: retry ? 1 : paragraphCount.value };
  try {
    const outcome = retry
      ? await translateService.retry(retry.original, retry.index, current.from, current.to, requestId, current.version)
      : await translateService.run(current.source, current.from, current.to, requestId);
    if (activeRequest !== requestId) return;
    for (const segment of outcome.segments) segments.value = mergeTranslationSegment(segments.value, segment);
    current.version = outcome.configVersion; current.engine = outcome.engine;
    lang.value = outcome.lang; elapsed.value = outcome.ms; requests.value = outcome.requests;
    if (outcome.cancelled) error.value = "已取消，保留完成的段落";
    else if (segments.value.some(item => item.status === "failed")) error.value = "部分段落失败，可在对应段落单独重试";
    const saved = saveHistory ? history.save({ id: current.historyId, createdAt: Date.now(), source: current.source, from: current.from, to: current.to, engine: current.engine, segments: segments.value }, outcome.cancelled) : "disabled";
    if (saved === "too-large") pushToast("本次内容超过 32KB，未保存到翻译历史", "warn");
  } catch (reason) { if (activeRequest === requestId) error.value = String(reason); }
  finally { if (activeRequest === requestId) { translating.value = false; activeRequest = ""; } }
}
watch([source, pairValue], () => {
  if (translating.value) return;
  segments.value = []; session = null; restoredHistory.value = false; error.value = ""; lang.value = "";
}, { flush: "sync" });
async function clearAll(): Promise<void> {
  if (!(await cancelCurrent())) return;
  activeRequest = ""; translating.value = false; session = null;
  source.value = ""; segments.value = []; lang.value = ""; error.value = "";
}
async function restoreHistory(entry: TranslationHistoryEntry): Promise<void> {
  if (!(await cancelCurrent())) return;
  activeRequest = ""; translating.value = false;
  source.value = entry.source; selectPair(entry.from, entry.to);
  session = null; error.value = ""; restoredHistory.value = true;
  segments.value = entry.segments.map(item => ({ ...item }));
  lang.value = `${entry.from}2${entry.to}`;
  historyOpen.value = false;
  pushToast(`已打开历史记录（${TRANSLATE_ENGINE_LABELS[entry.engine]}），重新翻译将使用当前服务`, "info");
}
const translatedText = computed(() => completedTranslation(segments.value));
const bilingualText = computed(() => completedTranslation(segments.value, true));
async function copy(text: string, label: string): Promise<void> {
  if (!text) return;
  try { await clipboardService.writeText(text); pushToast(`已复制${label}`, "success"); }
  catch (reason) { pushToast(String(reason), "error"); }
}
function openSettings(): void { void router.push({ path: "/settings", query: { tab: "translate" } }); }
async function changeEngine(value: string): Promise<void> {
  if (translating.value || value === config.value.engine) return;
  try { config.value = await translateService.setEngine(value); segments.value = []; session = null; lang.value = ""; error.value = ""; }
  catch (reason) { pushToast(String(reason), "error"); }
}
</script>
<template>
  <div class="tr-view">
    <section class="tr-toolbar">
      <div class="tr-pair" :inert="translating">
        <Select v-model="pairValue" :options="pairOptions" />
      </div>
      <div class="tr-engine" :inert="translating">
        <Select
          :model-value="config.engine"
          :options="engineOptions"
          placeholder="翻译服务"
          @update:model-value="changeEngine"
        />
      </div>
      <span class="tr-meta">
        {{ charCount }} 字符 · {{ paragraphCount }} 段
      </span>
      <span class="tr-spacer" />
      <Button variant="ghost" size="sm" :aria-expanded="historyOpen" @click="historyOpen = !historyOpen">历史记录</Button>
      <Button v-if="translating" variant="outline" :disabled="cancelling" @click="cancelCurrent">{{ cancelling ? '取消中…' : '取消翻译' }}</Button>
      <Button variant="ghost" size="sm" @click="openSettings">
        <Settings2 :size="15" />
        服务设置
      </Button>
      <Button :disabled="!configReady || translating || !source.trim()" @click="run">
        <Loader2 v-if="translating" :size="15" class="tr-spin" />
        <Languages v-else :size="15" />
        {{ translating ? "翻译中…" : "翻译" }}
      </Button>
    </section>

    <TranslateHistoryPanel v-if="historyOpen" @restore="restoreHistory" />
    <div class="tr-columns">
      <section class="tr-pane">
        <header class="tr-pane-head">
          <span>原文</span>
          <Button v-if="source" variant="ghost" size="sm" @click="clearAll">清空</Button>
        </header>
        <Textarea
          v-model="source"
          :readonly="translating"
          class="tr-source"
          placeholder="粘贴英文文章或任意文本，按空行分段后逐段翻译。Ctrl / Cmd + Enter 开始翻译。"
          spellcheck="false"
          @keydown.ctrl.enter.prevent="run"
          @keydown.meta.enter.prevent="run"
        />
      </section>

      <section class="tr-pane">
        <header class="tr-pane-head">
          <span>译文</span>
          <span v-if="restoredHistory" class="tr-pane-meta">历史记录 · {{ langLabel }}</span>
          <span v-else-if="segments.length && !translating && !error" class="tr-pane-meta">
            {{ langLabel }} · 本次处理 {{ requests }} 段 · {{ (elapsed / 1000).toFixed(1) }}s
          </span>
        </header>

        <p v-if="translating && segments.length" class="tr-progress" role="status">已处理 {{ progress.done }} / {{ progress.total }} 段，译文陆续显示中…</p>
        <p v-if="error && segments.length" class="tr-partial-error" role="alert">{{ error }}</p>
        <div v-if="!configReady && !error" class="tr-empty">正在读取翻译配置…</div>

        <div v-else-if="!configured" class="tr-empty tr-guide">
          <p class="tr-guide-title">还没有配置{{ engineLabel }}</p>
          <p class="tr-guide-desc">
            <template v-if="config.engine === 'youdao'">
              到有道智云控制台创建应用，复制应用 ID 与应用密钥填入即可。凭据只保存在本机。
            </template>
            <template v-else>
              在 DeepSeek 控制台创建 API Key 并填入即可；模型与提示词也能在设置页调整。
            </template>
          </p>
          <Button size="sm" @click="openSettings">前往设置页配置</Button>
        </div>

        <div v-else-if="error && !segments.length" class="tr-empty tr-failed">
          <p class="tr-guide-title">翻译失败</p>
          <p class="tr-guide-desc">{{ error }}</p>
          <Button size="sm" variant="outline" @click="run">重试</Button>
        </div>

        <div v-else-if="translating && !segments.length" class="tr-empty">
          <Loader2 :size="18" class="tr-spin" />
          <p class="tr-guide-desc">
            正在逐段翻译 {{ progress.done }} / {{ progress.total || paragraphCount }} 段…
          </p>
        </div>

        <div v-else-if="segments.length" class="tr-result">
          <article v-for="segment in segments" :key="segment.index" class="tr-row">
            <div class="tr-segment-head"><span>第 {{ segment.index + 1 }} 段 · {{ statusLabels[segment.status] }}</span>
              <Button v-if="segment.status === 'done'" variant="ghost" size="sm" @click="copy(segment.translated, '本段译文')">复制本段</Button>
              <Button v-if="session && ['failed', 'cancelled'].includes(segment.status)" variant="outline" size="sm" :disabled="translating" @click="retrySegment(segment)">重试本段</Button>
            </div>
            <p class="tr-original">{{ segment.original }}</p>
            <p v-if="segment.status === 'done'" class="tr-translated">{{ segment.translated }}</p>
            <p v-else-if="segment.error" class="tr-segment-error">{{ segment.error }}</p>
            <p v-else class="tr-segment-pending">{{ statusLabels[segment.status] }}</p>
          </article>
        </div>

        <div v-else class="tr-empty">
          <p class="tr-guide-desc">译文会按段落显示在这里。长文按空行分段后逐段请求。</p>
        </div>
      </section>
    </div>

    <footer v-if="completedCount" class="tr-foot">
      <Button variant="outline" size="sm" @click="copy(translatedText, partial ? '已完成译文' : '译文')">
        <ClipboardCopy :size="15" />
        {{ partial ? '复制已完成译文' : '复制译文' }}
      </Button>
      <Button variant="outline" size="sm" @click="copy(bilingualText, '双语')">
        <ClipboardCopy :size="15" />
        {{ partial ? '复制已完成双语' : '复制双语' }}
      </Button>
      <span class="tr-spacer" />
      <span class="tr-meta">{{ engineLabel }} · 快捷键 {{ config.shortcut }} 唤起快捷面板</span>
    </footer>
  </div>
</template>

<style scoped>
.tr-view {
  display: flex;
  flex-direction: column;
  gap: 10px;
  height: 100%;
  min-height: 520px;
}
.tr-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.tr-pair {
  width: 200px;
  flex: none;
}
.tr-engine {
  width: 150px;
  flex: none;
}
.tr-meta {
  font-size: 12px;
  color: var(--text-subtle);
}
.tr-spacer {
  flex: 1;
}
.tr-columns {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  gap: 10px;
  flex: 1;
  min-height: 0;
}
.tr-pane {
  display: flex;
  flex-direction: column;
  min-height: 0;
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  background: var(--surface);
  overflow: hidden;
}
.tr-pane-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--border);
  font-size: 12px;
  font-weight: 600;
  color: var(--text-muted);
}
.tr-pane-meta {
  font-weight: 400;
  color: var(--text-subtle);
}
.tr-source {
  flex: 1;
  min-height: 0;
  border: 0;
  border-radius: 0;
  background: transparent;
  padding: 14px;
  resize: none;
  font-size: 14px;
  line-height: 1.75;
}
.tr-source:focus {
  border: 0;
  box-shadow: none;
}
.tr-result {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 4px 14px 14px;
}
.tr-row {
  padding: 12px 0;
  border-bottom: 1px solid var(--border);
}
.tr-row:last-child {
  border-bottom: 0;
}
.tr-original {
  margin: 0 0 6px;
  font-size: 12.5px;
  line-height: 1.7;
  color: var(--text-subtle);
}
.tr-translated {
  margin: 0;
  font-size: 16px;
  line-height: 1.85;
  color: var(--text);
}
.tr-empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  padding: 24px;
  text-align: center;
}
.tr-guide-title {
  margin: 0;
  font-size: 14px;
  font-weight: 600;
  color: var(--text);
}
.tr-guide-desc {
  margin: 0;
  max-width: 340px;
  font-size: 12.5px;
  line-height: 1.7;
  color: var(--text-subtle);
}
.tr-failed .tr-guide-title {
  color: var(--u-crit);
}
.tr-foot {
  display: flex;
  align-items: center;
  gap: 8px;
}
.tr-spin {
  animation: tr-spin 0.9s linear infinite;
}
@keyframes tr-spin {
  to {
    transform: rotate(360deg);
  }
}
@media (max-width: 980px) {
  .tr-columns {
    grid-template-columns: minmax(0, 1fr);
  }
  .tr-view {
    height: auto;
  }
  .tr-pane {
    min-height: 280px;
  }
}
</style>

<style scoped>
.tr-progress, .tr-partial-error { margin: 0; padding: 10px 16px; font-size: 12px; line-height: 1.6; border-bottom: 1px solid var(--border); }
.tr-progress { color: var(--accent); background: var(--accent-weak); }
.tr-partial-error { color: var(--u-crit); }
</style>

<style scoped>
.tr-segment-head { display: flex; align-items: center; gap: 8px; justify-content: space-between; margin-bottom: 8px; color: var(--text-subtle); font-size: 11px; }
.tr-segment-error { color: var(--u-crit); font-size: 12px; white-space: pre-wrap; }
.tr-segment-pending { color: var(--text-subtle); font-size: 12px; }
</style>
