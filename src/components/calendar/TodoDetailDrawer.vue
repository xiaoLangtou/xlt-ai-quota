<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  CATEGORY_LABELS,
  EXTERNAL_TYPE_LABELS,
  PRIORITY_LABELS,
  STATUS_LABELS,
  type TodoTask,
} from "@/types/productivity";
import type { ZentaoDetail } from "@/types/productivity";
import { openExternal, zentaoService } from "@/services/zentao-service";
import { pushToast } from "@/composables/useToast";
import ZentaoActions from "@/components/calendar/ZentaoActions.vue";
import ZentaoDetailContent from "@/components/calendar/ZentaoDetailContent.vue";
import { formatEstimate } from "@/utils/productivity";

// 待办详情：右侧抽屉。手动待办的轻量详情；禅道条目详情走独立页面
// （/todos/:taskId），大屏下阅读体验更好。
const props = defineProps<{
  open: boolean;
  task: TodoTask | null;
}>();
const emit = defineEmits<{
  (event: "update:open", value: boolean): void;
  (event: "edit", payload: TodoTask): void;
  (event: "updated", payload: TodoTask): void;
}>();

const detail = ref<ZentaoDetail | null>(null);
const detailLoading = ref(false);
const detailRefreshing = ref(false);
const detailFetchedAt = ref<number | null>(null);

const isZentao = computed(() => props.task?.sourceType === "zentao");
const externalLink = computed(() => props.task?.externalUrl ?? "");

watch(
  () => [props.open, props.task?.id] as const,
  ([open]) => {
    if (!open) return;
    detail.value = null;
    detailFetchedAt.value = null;
    void loadDetail();
  },
  { immediate: true },
);

async function loadDetail(force = false): Promise<void> {
  const task = props.task;
  if (!task || !isZentao.value || !task.externalId) return;
  const externalId = Number(task.externalId.split(":").pop());
  if (!Number.isFinite(externalId)) return;
  if (force) detailRefreshing.value = true;
  else detailLoading.value = true;
  try {
    const cached = await zentaoService.detail(task.externalType ?? "task", externalId, { force });
    detail.value = cached.detail;
    detailFetchedAt.value = cached.fetchedAt;
  } catch (reason) {
    if (force) {
      pushToast(reason instanceof Error ? reason.message : String(reason), "error");
    } else {
      // 首次拉取失败不阻塞抽屉，仍展示本地已同步的描述。
      detail.value = null;
    }
  } finally {
    detailLoading.value = false;
    detailRefreshing.value = false;
  }
}

/** 详情缓存时间，展示为「HH:MM 缓存」。 */
const detailCachedLabel = computed(() => {
  if (!detailFetchedAt.value) return "";
  const time = new Date(detailFetchedAt.value).toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" });
  return `${time} 缓存`;
});

/** 描述优先取禅道实时详情，失败时回退本地同步副本。 */
const description = computed(() => {
  const text = detail.value?.description || props.task?.description || "";
  return text.trim();
});

async function onZentaoUpdated(task: TodoTask): Promise<void> {
  emit("updated", task);
  await loadDetail(true);
}

function openInZentao(): void {
  if (!externalLink.value) return;
  openExternal(externalLink.value).catch((reason) => {
    pushToast(reason instanceof Error ? reason.message : String(reason), "error");
  });
}
</script>

<template>
  <USlideover
    :open="open"
    :title="task?.title ?? '待办详情'"
    :ui="{
      content: 'w-[min(940px,94vw)] max-w-[min(940px,94vw)]',
      title: 'pr-10 break-words text-base leading-relaxed',
    }"
    @update:open="emit('update:open', $event)"
  >

    <template #body>
      <div v-if="task" class="detail-body">
        <div class="meta-chips">
          <UBadge :color="task.status === 'done' ? 'success' : task.status === 'doing' ? 'info' : 'neutral'" variant="subtle" size="sm">
            {{ STATUS_LABELS[task.status] }}
          </UBadge>
          <UBadge :color="task.priority === 'high' ? 'error' : task.priority === 'medium' ? 'warning' : 'neutral'" variant="subtle" size="sm">
            {{ PRIORITY_LABELS[task.priority] }}
          </UBadge>
          <UBadge color="neutral" variant="subtle" size="sm">{{ CATEGORY_LABELS[task.category] }}</UBadge>
          <UBadge v-if="isZentao" color="info" variant="subtle" size="sm">
            禅道 · {{ EXTERNAL_TYPE_LABELS[task.externalType ?? ""] ?? "条目" }}
          </UBadge>
        </div>
        <p class="meta-line">
          <template v-if="task.plannedDate">计划 {{ task.plannedDate }}{{ task.plannedTime ? ` ${task.plannedTime}` : "" }} · </template>
          <template v-if="task.estimateMinutes">估时 {{ formatEstimate(task.estimateMinutes) }} · </template>
          <span>创建于 {{ new Date(task.createdAt).toLocaleDateString("zh-CN") }}</span>
          <template v-if="isZentao && detailCachedLabel"> · 详情 {{ detailCachedLabel }}</template>
        </p>

        <ZentaoDetailContent :detail="detail" :loading="detailLoading" :description="description" />
      </div>
    </template>

    <template #footer>
      <div class="detail-footer">
        <UButton
          v-if="isZentao && externalLink"
          icon="i-lucide-external-link"
          color="neutral"
          variant="outline"
          size="sm"
          @click="openInZentao"
        >
          在禅道中打开
        </UButton>
        <ZentaoActions v-if="task && isZentao" :key="task.id" :task="task" @updated="onZentaoUpdated" />
        <div class="footer-right">
          <UButton
            v-if="isZentao"
            icon="i-lucide-refresh-cw"
            :loading="detailRefreshing"
            color="neutral"
            variant="ghost"
            size="sm"
            title="重新从禅道拉取详情"
            @click="loadDetail(true)"
          />
          <UButton color="neutral" variant="ghost" size="sm" @click="emit('update:open', false)">关闭</UButton>
          <UButton icon="i-lucide-pencil" size="sm" @click="task && emit('edit', task)">编辑</UButton>
        </div>
      </div>
    </template>
  </USlideover>
</template>

<style scoped>
.detail-body {
  display: grid;
  gap: 14px;
}
.meta-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.meta-line {
  margin: 0;
  color: var(--text-subtle, var(--text-dimmed));
  font-size: 11px;
}
.detail-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  width: 100%;
}
.footer-right {
  display: flex;
  gap: 8px;
}
</style>
