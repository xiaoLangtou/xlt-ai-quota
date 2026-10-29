<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useLocalStorage, useNow } from "@vueuse/core";
import { invoke } from "@tauri-apps/api/core";
import { useRouter } from "vue-router";
import { AI_PROVIDERS } from "@/config/ai-providers";
import { readAiConfig } from "@/services/document-decomposition-service";
import { summaryImages, summarySources, summaryFingerprint, validSummary, type ZentaoSummaryResult } from "@/utils/zentao-summary";
import type { TodoTask, ZentaoDetail } from "@/types/productivity";

const props = defineProps<{ task: TodoTask; detail: ZentaoDetail; fetchedAt: number | null }>();
const router = useRouter();
interface CachedSummary { taskId: string; fingerprint: string; provider: string; model: string; generatedAt: number; result: ZentaoSummaryResult }
const cache = useLocalStorage<CachedSummary[]>("xlt.zentao.summaries.v3", []);
const clock = useNow({ interval: 60_000 });
const fingerprint = ref("");
const sources = computed(() => summarySources(props.task, props.detail));
const result = computed(() => cache.value.find(item => item.taskId === props.task.id));
const stale = computed(() => !result.value || result.value.fingerprint !== fingerprint.value || clock.value.getTime() - result.value.generatedAt > 7 * 86400000);
const loading = ref(false);
const error = ref("");
const consentOpen = ref(false);
const config = ref<{ provider: string; model: string } | null>(null);
const images = computed(() => summaryImages(props.detail));
let generation = 0;
watch(sources, async value => {
  cache.value = cache.value.filter(item => Date.now() - item.generatedAt < 7 * 86400000).slice(0, 20);
  const request = ++generation;
  loading.value = false;
  error.value = ""; fingerprint.value = ""; consentOpen.value = false;
  const hash = await summaryFingerprint(value);
  if (request === generation) fingerprint.value = hash;
}, { immediate: true });
onBeforeUnmount(() => { generation++; });
const sections = [
  { key: "overview" as const, label: "当前事项" }, { key: "purpose" as const, label: "目的" },
  { key: "frontend" as const, label: "前端涉及的改动" }, { key: "backend" as const, label: "后端涉及的改动" },
];
function prepare(): void {
  const current = readAiConfig();
  if (!current) { error.value = "请先在设置 › 高级中完成 AI 配置"; return; }
  config.value = { provider: current.provider, model: current.model };
  error.value = ""; consentOpen.value = true;
}
async function generate(): Promise<void> {
  if (loading.value || !fingerprint.value) return;
  const current = readAiConfig();
  if (!current || current.provider !== config.value?.provider || current.model !== config.value?.model) {
    error.value = "AI 配置已变化，请重新确认发送范围"; consentOpen.value = false; return;
  }
  const inputSources = sources.value.map(source => ({ ...source }));
  const hash = fingerprint.value;
  const taskId = props.task.id;
  const request = ++generation;
  loading.value = true; error.value = ""; consentOpen.value = false;
  try {
    const summary = await invoke<ZentaoSummaryResult>("zentao_summary_generate", {
      input: { ...current, sources: inputSources.map(({ id, text, label }) => ({ id, text: id.startsWith('image-') ? label : text })), images: images.value, files: props.detail.files.map(({ id, title, extension }) => ({ id, title, extension })) },
    });
    if (request !== generation) return;
    if (!validSummary(summary, inputSources)) throw new Error("摘要格式或证据引用无效");
    const now = Date.now();
    cache.value = [{ taskId, fingerprint: hash, provider: current.provider, model: current.model, generatedAt: now, result: summary },
      ...cache.value.filter(item => item.taskId !== taskId && now - item.generatedAt < 7 * 86400000)].slice(0, 20);
  } catch (reason) {
    if (request === generation) error.value = String(reason);
  } finally { if (request === generation) loading.value = false; }
}
</script>

<template>
  <section class="mb-5 space-y-3 rounded-lg border border-primary/20 bg-primary/5 p-3" aria-label="智能摘要">
    <div class="flex flex-wrap items-center gap-2">
      <UIcon name="i-lucide-sparkles" class="size-4 text-primary" />
      <h3 class="text-sm font-semibold">智能摘要</h3>
      <span v-if="result && stale" class="text-xs text-warning">内容已更新或摘要已过期</span>
      <UButton class="ml-auto" size="xs" variant="soft" :disabled="!fingerprint || loading" @click="prepare">{{ result ? '重新生成' : '生成摘要' }}</UButton>
      <UButton v-if="loading" size="xs" variant="ghost" color="neutral" @click="generation++; loading = false">停止等待</UButton>
    </div>
    <p v-if="loading" class="text-sm text-muted" role="status">正在生成摘要，切换事项不会显示迟到结果。停止等待后，已发送的请求仍可能继续计费。</p>
    <UAlert v-if="error" color="error" variant="subtle" :description="error" />
    <UButton v-if="error.includes('AI 配置')" size="xs" variant="link" @click="router.push({ name: 'settings' })">前往设置</UButton>
    <template v-if="result">
      <p class="text-xs text-muted">{{ result.provider }} · {{ result.model }} · 生成于 {{ new Date(result.generatedAt).toLocaleString('zh-CN') }}</p>
      <div v-for="section in sections" :key="section.key" class="space-y-1 text-sm leading-6">
        <h4 class="font-medium">{{ section.label }}</h4>
        <p v-if="!result.result[section.key].length" class="text-muted">暂无足够信息</p>
        <div v-for="(statement, index) in result.result[section.key]" :key="index">
          <span>{{ statement.text }}</span>
        </div>
      </div>
      <section v-if="detail.externalType === 'bug'" class="space-y-1 text-sm leading-6">
        <h4 class="font-medium">历史记录</h4>
        <p v-if="!detail.actions.length" class="text-muted">暂无历史记录</p>
        <ol v-else class="space-y-2">
          <li v-for="(action, index) in [...detail.actions].reverse()" :key="index" class="border-l-2 border-default pl-2">
            <p class="text-xs text-muted">{{ action.date }} · {{ action.actor }}</p>
            <p v-if="action.desc">{{ action.desc }}</p>
            <p v-if="action.comment" class="whitespace-pre-wrap">{{ action.comment }}</p>
          </li>
        </ol>
      </section>
    </template>
    <p v-else-if="!loading" class="text-xs leading-5 text-muted">结合描述、动态、图片和附件整理事项、目的及前后端改动。生成前确认发送范围。</p>
    <p v-if="fetchedAt" class="text-xs text-dimmed">数据截至 {{ new Date(fetchedAt).toLocaleString('zh-CN') }} · AI 摘要请结合原文核对</p>
  </section>
  <UModal v-model:open="consentOpen" title="确认生成禅道智能摘要" description="确认后将文字、图片和附件内容发送给当前配置的 AI 服务商。">
    <template #body>
      <div class="space-y-3 text-sm leading-6">
        <p>接收方：{{ config ? AI_PROVIDERS[config.provider as keyof typeof AI_PROVIDERS]?.label : '' }} · {{ config?.model }}</p>
        <p>发送事项编号、标题、状态、指派人、优先级、截止日期、描述前 12000 字符，以及最近 {{ Math.min(detail.actions.length, 30) }} 条动态（每条最多 850 字符）。文字中可能包含人员姓名及工作信息。</p>
        <p>同时发送 {{ images.length }} 张描述或动态图片，以及 {{ detail.files.length }} 个附件的正文和图片。PDF、DOCX、文本及 ZIP 会读取内容后发送；无法读取的附件会报错。</p>
        <p v-if="detail.files.length" class="text-xs text-muted">附件：{{ detail.files.map(file => file.title).join('、') }}</p>
        <details class="rounded border border-default p-2"><summary class="cursor-pointer">查看本次发送的事项文字及附件清单</summary><pre class="max-h-64 overflow-auto whitespace-pre-wrap text-xs">{{ sources.filter(source => !source.id.startsWith('image-')).map(source => `${source.label}\n${source.text}`).join('\n\n') }}</pre></details>
        <div class="flex justify-end gap-2"><UButton color="neutral" variant="ghost" @click="consentOpen = false">取消</UButton><UButton @click="generate">确认发送并生成</UButton></div>
      </div>
    </template>
  </UModal>
</template>
