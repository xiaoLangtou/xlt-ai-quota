<script setup lang="ts">
import { computed, ref, watch } from "vue";
import ZentaoActions from "@/components/calendar/ZentaoActions.vue";
import { DatePicker } from "@/components/ui/date-picker";
import { useCalendarTodo } from "@/composables/useCalendarTodo";
import {
  CATEGORY_LABELS,
  STATUS_LABELS,
  type CalendarEvent,
  type TodoTask,
} from "@/types/productivity";
import type { ZentaoDetail } from "@/types/productivity";
import { openExternal, zentaoService } from "@/services/zentao-service";
import { pushToast } from "@/composables/useToast";
import { formatDateLabel, formatEstimate, splitStartAt } from "@/utils/productivity";

// 点日历上的事件 = 查看关联待办详情：描述、快捷操作（勾选完成、改计划时间、进完整编辑）。
// 概念上日历就是待办的时间线，这里不再提供独立的“事件标题/分类”编辑。
const props = defineProps<{ event: CalendarEvent; task: TodoTask | null }>();
const emit = defineEmits<{
  (event: "close"): void;
  (event: "updated", task: TodoTask): void;
  (event: "patch-planned", payload: { task: TodoTask; date: string; time: string }): void;
  (event: "edit", task: TodoTask): void;
  (event: "remove", payload: { event: CalendarEvent; task: TodoTask | null }): void;
}>();

const { toggleTask } = useCalendarTodo();

const start = splitStartAt(props.event.startAt);
const plannedDate = ref(start.date);
const plannedTime = ref(props.event.allDay ? "" : start.time ?? "");

// 禅道任务拉实时详情（描述 + 附件），失败时回退本地同步副本。
const zentaoDetail = ref<ZentaoDetail | null>(null);
const detailLoading = ref(false);
watch(
  () => props.task?.id,
  () => {
    zentaoDetail.value = null;
    void loadZentaoDetail();
  },
  { immediate: true },
);

async function loadZentaoDetail(): Promise<void> {
  const task = props.task;
  if (!task || task.sourceType !== "zentao" || !task.externalId) return;
  const externalId = Number(task.externalId.split(":").pop());
  if (!Number.isFinite(externalId)) return;
  detailLoading.value = true;
  try {
    const cached = await zentaoService.detail(task.externalType ?? "task", externalId);
    zentaoDetail.value = cached.detail;
  } catch {
    zentaoDetail.value = null;
  } finally {
    detailLoading.value = false;
  }
}

/** 描述优先取禅道实时详情，失败回退本地同步副本；浮层不渲染图片，去掉截图占位符。 */
const description = computed(() => {
  const text = zentaoDetail.value?.description || props.task?.description || "";
  return text.replace(/【截图\d+】/g, "").trim();
});

// 修改计划日期/时间直接写回待办，后端会同步移动日历事件。
watch([plannedDate, plannedTime], () => {
  if (props.task) {
    emit("patch-planned", { task: props.task, date: plannedDate.value, time: plannedTime.value });
  }
});

function requestRemove(): void {
  const label = props.task
    ? `删除待办「${props.task.title}」及其日历事件？`
    : `删除日历事件「${props.event.title}」？`;
  if (!window.confirm(label)) return;
  emit("remove", { event: props.event, task: props.task });
}

function openInZentao(): void {
  if (!props.task?.externalUrl) return;
  openExternal(props.task.externalUrl).catch(() => {});
}

async function downloadFile(fileId: number, extension: string, title: string): Promise<void> {
  try {
    const path = await zentaoService.downloadFile(fileId, extension, title);
    pushToast(`附件已保存：${path}`, "success");
  } catch (reason) {
    pushToast(reason instanceof Error ? reason.message : String(reason), "error");
  }
}
</script>

<template>
  <div class="detail-popover">
    <div v-if="task" class="head">
      <UCheckbox
        v-if="task.sourceType !== 'zentao'"
        :model-value="task.status === 'done'"
        :aria-label="task.status === 'done' ? '标记未完成' : '标记完成'"
        @update:model-value="toggleTask(task)"
      />
      <button type="button" class="title" :class="{ done: task.status === 'done' }" @click="emit('edit', task)">
        {{ task.title }}
      </button>
    </div>
    <div v-else class="head">
      <strong class="title">{{ event.title }}</strong>
    </div>

    <div class="meta">
      <span class="badge" :data-category="event.category">{{ CATEGORY_LABELS[event.category] }}</span>
      <span v-if="task">{{ STATUS_LABELS[task.status] }}</span>
      <span v-if="task?.estimateMinutes">预估 {{ formatEstimate(task.estimateMinutes) }}</span>
      <span v-if="!task" class="subtle">无关联待办</span>
    </div>

    <div v-if="task" class="grid-2">
      <UFormField label="计划日期"><DatePicker v-model="plannedDate" /></UFormField>
      <UFormField label="计划时间"><UInput v-model="plannedTime" type="time" class="w-full" /></UFormField>
    </div>
    <p v-else class="plain-time">{{ formatDateLabel(start.date) }}<template v-if="start.time"> · {{ start.time }}</template></p>

    <ZentaoActions v-if="task?.sourceType === 'zentao'" :task="task" @updated="emit('updated', $event); loadZentaoDetail()" />

    <!-- 描述详情：禅道任务显示实时内容，本地任务显示同步副本 -->
    <div v-if="task" class="desc-section">
      <div class="desc-head">
        <span>描述</span>
        <UButton
          v-if="task.sourceType === 'zentao' && task.externalUrl"
          icon="i-lucide-external-link"
          color="neutral"
          variant="ghost"
          size="xs"
          @click="openInZentao"
        >
          在禅道中打开
        </UButton>
      </div>
      <p v-if="detailLoading && !description" class="desc-empty">正在获取最新描述…</p>
      <p v-else-if="description" class="desc-text">{{ description }}</p>
      <p v-else class="desc-empty">暂无描述。</p>

      <div v-if="zentaoDetail?.files?.length" class="desc-files">
        <div v-for="file in zentaoDetail.files" :key="file.id" class="file-row">
          <span class="file-name" :title="file.title">{{ file.title }}</span>
          <UButton
            icon="i-lucide-download"
            color="neutral"
            variant="ghost"
            size="xs"
            @click="downloadFile(file.id, file.extension, file.title)"
          />
        </div>
      </div>
    </div>

    <div class="actions">
      <UButton v-if="task" icon="i-lucide-list-checks" color="neutral" variant="ghost" size="sm" @click="emit('edit', task)">
        编辑待办
      </UButton>
      <UButton icon="i-lucide-trash-2" color="error" variant="soft" size="sm" @click="requestRemove">
        {{ task ? "删除待办" : "删除事件" }}
      </UButton>
      <div class="spacer" />
      <UButton color="neutral" variant="ghost" size="sm" @click="emit('close')">关闭</UButton>
    </div>
  </div>
</template>

<style scoped>
.detail-popover {
  width: 320px;
  padding: 13px;
  display: grid;
  gap: 11px;
}
.head {
  display: flex;
  align-items: flex-start;
  gap: 9px;
}
.title {
  min-width: 0;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--text);
  font-size: 13px;
  font-weight: 650;
  line-height: 1.45;
  text-align: left;
  cursor: pointer;
}
.title:hover {
  color: var(--accent);
}
.title.done {
  color: var(--text-subtle);
  text-decoration: line-through;
}
.meta {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  color: var(--text-muted);
  font-size: 10.5px;
}
.badge {
  padding: 2px 8px;
  border-radius: 999px;
  background: var(--surface-3);
  font-weight: 600;
}
.badge[data-category="work"] {
  color: var(--u-ok);
}
.badge[data-category="planning"] {
  color: var(--u-warn);
}
.badge[data-category="personal"] {
  color: #7c5bb7;
}
.subtle {
  color: var(--text-subtle);
}
.grid-2 {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
}
.plain-time {
  margin: 0;
  color: var(--text-muted);
  font-size: 11.5px;
}
.desc-section {
  display: grid;
  gap: 6px;
}
.desc-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  color: var(--text-highlighted, var(--text));
  font-size: 11px;
  font-weight: 650;
}
.desc-text {
  max-height: 190px;
  overflow-y: auto;
  margin: 0;
  padding: 8px 10px;
  border: 1px solid var(--border);
  border-radius: 7px;
  background: var(--bg, var(--surface-2));
  color: var(--text);
  font-size: 11.5px;
  line-height: 1.7;
  white-space: pre-wrap;
  word-break: break-word;
  scrollbar-width: thin;
}
.desc-empty {
  margin: 0;
  color: var(--text-subtle);
  font-size: 11px;
}
.desc-files {
  display: grid;
  gap: 2px;
}
.file-row {
  display: flex;
  align-items: center;
  gap: 8px;
}
.file-name {
  min-width: 0;
  flex: 1;
  overflow: hidden;
  color: var(--text-muted);
  font-size: 11px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.actions {
  display: flex;
  align-items: center;
  gap: 6px;
}
.actions .spacer {
  flex: 1;
}
</style>
