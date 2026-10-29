<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useLocalStorage } from "@vueuse/core";
import { clipboardService } from "@/services/clipboard-service";
import { zentaoCopyText } from "@/utils/todo-workspace";
import ZentaoComment from "@/components/calendar/ZentaoComment.vue";
import ZentaoSummary from "@/components/calendar/ZentaoSummary.vue";
import { useRouter } from "vue-router";
import ZentaoActions from "@/components/calendar/ZentaoActions.vue";
import ZentaoDetailContent from "@/components/calendar/ZentaoDetailContent.vue";
import { zentaoService, openExternal } from "@/services/zentao-service";
import { pushToast } from "@/composables/useToast";
import { externalStatusChip, formatEstimate } from "@/utils/productivity";
import { PRIORITY_LABELS, type TodoTask, type ZentaoDetail } from "@/types/productivity";

const props = defineProps<{ task: TodoTask; focused?: boolean; previous?: boolean; next?: boolean; syncVersion?: number }>();
const emit = defineEmits<{
  submitting: [task: TodoTask]; handled: [task: TodoTask]; close: []; navigate: [direction: number]; updated: [task: TodoTask]; schedule: [task: TodoTask];
}>();
const router = useRouter();
const detail = ref<ZentaoDetail | null>(null);
const loading = ref(false);
const error = ref("");
const fetchedAt = ref<number | null>(null);
const autoNext = useLocalStorage("xlt.todo.auto-next", false);
const cleanTitle = computed(() => props.task.title.replace(/^【#\d+】\s*/, ""));
async function copyIdentity(): Promise<void> {
  try { await clipboardService.writeText(zentaoCopyText(props.task)); pushToast("编号与标题已复制"); }
  catch (reason) { pushToast(String(reason), "error"); }
}
const chip = computed(() => externalStatusChip(detail.value?.status ?? props.task.externalStatus));
let generation = 0;
async function load(force = false): Promise<void> {
  const task = props.task;
  const request = ++generation;
  const id = Number(task.externalId?.split(":").pop());
  detail.value = null;
  fetchedAt.value = null;
  error.value = "";
  loading.value = true;
  try {
    if (!Number.isInteger(id) || id <= 0 || !task.externalType) throw new Error("禅道条目身份不完整");
    const result = await zentaoService.detail(task.externalType, id, { force });
    if (request !== generation) return;
    detail.value = result.detail;
    fetchedAt.value = result.fetchedAt;
  } catch (reason) {
    if (request === generation) error.value = reason instanceof Error ? reason.message : String(reason);
  } finally {
    if (request === generation) loading.value = false;
  }
}
watch(() => props.task.id, () => void load(), { immediate: true });
watch(() => props.syncVersion, () => void load(true));
onBeforeUnmount(() => generation++);
function updated(task: TodoTask): void {
  if (autoNext.value && (task.status === "done" || task.externalTransferred)) emit("handled", task);
  else emit("updated", task);
  void load(true);
}
function openExternalPage(): void {
  if (props.task.externalUrl) void openExternal(props.task.externalUrl).catch((reason) => pushToast(String(reason), "error"));
}
</script>

<template>
  <aside class="flex h-full min-h-0 flex-col bg-default" aria-label="禅道条目详情">
    <header class="shrink-0 space-y-2 border-b border-default px-4 py-3">
      <div class="flex items-start gap-2">
        <UButton v-if="focused" icon="i-lucide-arrow-left" color="neutral" variant="ghost" size="xs" class="shrink-0" @click="emit('close')" aria-label="返回列表" />
        <h2 class="min-w-0 flex-1 text-xl font-semibold leading-7 break-words text-highlighted">
          <button type="button" class="inline whitespace-nowrap hover:text-primary" title="复制编号与标题" @click="copyIdentity">【#{{ task.externalId?.split(':').pop() }}】</button>{{ cleanTitle }}
        </h2>
        <div class="flex shrink-0 items-center">
          <UButton icon="i-lucide-chevron-up" color="neutral" variant="ghost" size="xs" :disabled="!previous" aria-label="上一条" @click="emit('navigate', -1)" />
          <UButton icon="i-lucide-chevron-down" color="neutral" variant="ghost" size="xs" :disabled="!next" aria-label="下一条" @click="emit('navigate', 1)" />
          <UButton icon="i-lucide-maximize-2" color="neutral" variant="ghost" size="xs" aria-label="展开完整详情" @click="router.push({ name: 'todo-detail', params: { taskId: task.id } })" />
          <UButton icon="i-lucide-x" color="neutral" variant="ghost" size="xs" aria-label="关闭详情" @click="emit('close')" />
        </div>
      </div>
      <div class="flex flex-wrap items-center gap-x-3 gap-y-2 text-xs text-muted">
        <UBadge v-if="chip" :color="chip.color" variant="subtle">{{ chip.label }}</UBadge>
        <span>{{ PRIORITY_LABELS[task.priority] }}优先级</span>
        <span v-if="task.externalAssignedTo">指派给 {{ task.externalAssignedTo }}</span>
        <UBadge v-if="task.externalTransferred" color="warning" variant="subtle">已转交</UBadge>
        <button type="button" class="inline-flex items-center gap-1 hover:text-primary" @click="emit('schedule', task)">
          <UIcon name="i-lucide-calendar-days" class="size-3.5" />{{ task.plannedDate ? `计划 ${task.plannedDate}${task.plannedTime ? ` ${task.plannedTime}` : ''}` : '安排时间' }}<span v-if="task.estimateMinutes"> · {{ formatEstimate(task.estimateMinutes) }}</span>
        </button>
        <ZentaoComment :key="task.id" :task="task" @commented="load(true)" />
        <ZentaoActions :key="task.id" :task="task" compact show-assign @submitting="emit('submitting', $event)" @updated="updated" />
        <UCheckbox v-model="autoNext" label="处理后下一条" size="sm" />
        <div class="ml-auto flex items-center">
          <UButton icon="i-lucide-refresh-cw" color="neutral" variant="ghost" size="xs" :loading="loading" aria-label="刷新详情" @click="load(true)" />
          <UButton v-if="task.externalUrl" icon="i-lucide-external-link" color="neutral" variant="ghost" size="xs" aria-label="在禅道中打开" @click="openExternalPage" />
        </div>
      </div>
    </header>
    <div class="min-h-0 flex-1 overflow-y-auto p-4">
      <UAlert v-if="error" color="error" variant="subtle" title="详情读取失败" :description="error" />
      <p v-else-if="loading" class="py-8 text-center text-sm text-muted">正在读取禅道详情…</p>
      <template v-else-if="detail"><ZentaoSummary :task="task" :detail="detail" :fetched-at="fetchedAt" /><ZentaoDetailContent :detail="detail" :loading="false" :description="detail.description" /></template>
      <p v-if="fetchedAt" class="mt-5 text-[11px] text-dimmed">详情读取于 {{ new Date(fetchedAt).toLocaleString('zh-CN') }}</p>
    </div>
  </aside>
</template>
