<script setup lang="ts">
import { useNow } from "@vueuse/core";
import { attentionReasons } from "@/utils/todo-workspace";
import { today } from "@/utils/productivity";
import { computed } from "vue";
import ZentaoActions from "@/components/calendar/ZentaoActions.vue";
import { CATEGORY_LABELS, EXTERNAL_TYPE_LABELS, PRIORITY_LABELS, type TodoPriority, type TodoTask } from "@/types/productivity";
import { openExternal } from "@/services/zentao-service";
import {
  CATEGORY_DOT_BG,
  externalStatusChip,
  formatEstimate,
  type ExternalStatusTone,
} from "@/utils/productivity";

const props = defineProps<{
  task: TodoTask;
  showDate?: boolean;
  active?: boolean;
  /** 批量选择模式：该行是否可勾选（仅可批量解决的 Bug）。 */
  selectable?: boolean;
  /** 批量选择模式下是否已选中。 */
  selected?: boolean;
}>();
const emit = defineEmits<{
  (event: "toggle", payload: TodoTask): void;
  (event: "edit", payload: TodoTask): void;
  (event: "delete", payload: TodoTask): void;
  (event: "select", payload: TodoTask): void;
  (event: "updated", payload: TodoTask): void;
}>();

const PRIORITY_COLOR: Record<TodoPriority, ExternalStatusTone> = {
  high: "error",
  medium: "warning",
  low: "neutral",
};

const clock = useNow({ interval: 60_000 });
// 优先级已由右上角标签展示，这里不再重复“高优先级”。
const reasons = computed(() => attentionReasons(props.task, today(clock.value)).filter((reason) => reason !== "高优先级"));
const cleanTitle = computed(() => props.task.title.replace(/^【#\d+】\s*/, ""));
const externalNumber = computed(() => props.task.externalId?.split(":").pop());
const done = computed(() => props.task.status === "done");
const isZentao = computed(() => props.task.sourceType === "zentao");
const statusChip = computed(() => (isZentao.value ? externalStatusChip(props.task.externalStatus) : null));
const typeLabel = computed(() => EXTERNAL_TYPE_LABELS[props.task.externalType ?? ""] ?? "条目");
const schedule = computed(() => {
  if (props.showDate && props.task.plannedDate) {
    return `计划 ${props.task.plannedDate}${props.task.plannedTime ? ` ${props.task.plannedTime}` : ""}`;
  }
  return props.task.plannedTime;
});
const checkboxValue = computed(() => (props.selectable ? Boolean(props.selected) : done.value));
const checkboxDisabled = computed(() => !props.selectable && isZentao.value);
const checkboxTitle = computed(() => {
  if (props.selectable) return "选择此 Bug 批量解决";
  return isZentao.value ? "在详情中操作禅道状态" : undefined;
});

function activate(): void {
  if (props.selectable) {
    emit("select", props.task);
    return;
  }
  emit("edit", props.task);
}

function toggle(): void {
  if (props.selectable) {
    emit("select", props.task);
    return;
  }
  emit("toggle", props.task);
}
</script>

<template>
  <article
    class="group @container grid grid-cols-[auto_minmax(0,1fr)_auto] items-start gap-x-3 gap-y-2 px-3 py-2.5 transition-colors hover:bg-elevated/70 @md:grid-cols-[auto_minmax(0,1fr)_auto_auto]"
    :class="[selectable && 'cursor-pointer', active && 'bg-primary/5 ring-1 ring-inset ring-primary/25']"
    @click="selectable && emit('select', task)"
  >
    <div v-if="!isZentao || selectable" class="col-start-1 row-start-1 pt-0.5" @click.stop>
      <UCheckbox
        :model-value="checkboxValue"
        :disabled="checkboxDisabled"
        :title="checkboxTitle"
        :aria-label="selectable ? '选择此 Bug' : done ? '标记为未完成' : '标记为完成'"
        @update:model-value="toggle"
      />
    </div>

    <div v-else class="col-start-1 row-start-1 flex size-7 shrink-0 items-center justify-center rounded-md bg-elevated text-muted" :title="typeLabel">
      <UIcon :name="task.externalType === 'bug' ? 'i-lucide-bug' : 'i-lucide-check-square'" class="size-4" />
    </div>
    <div class="col-start-2 row-start-1 min-w-0" :class="!selectable && 'cursor-pointer'" @click.stop="activate">
      <button type="button"
        @click.stop="activate"
        class="w-full text-left text-[13px] leading-5 font-medium break-words"
        :class="done ? 'text-dimmed line-through decoration-1' : 'text-highlighted'"
      >
        {{ isZentao ? cleanTitle : task.title }}
      </button>
      <div class="mt-1.5 flex flex-wrap items-center gap-x-2 gap-y-1 text-[11px] leading-none text-dimmed [&>*]:shrink-0 [&>*]:whitespace-nowrap">
        <span v-if="isZentao" class="font-mono text-dimmed">{{ typeLabel }} #{{ externalNumber }}</span>
        <span v-if="!isZentao" class="inline-flex items-center gap-1.5">
          <span class="size-1.5 shrink-0 rounded-full" :class="CATEGORY_DOT_BG[task.category]" />
          {{ CATEGORY_LABELS[task.category] }}
        </span>
        <UBadge v-if="statusChip" :color="statusChip.color" size="sm" variant="subtle">
          {{ statusChip.label }}
        </UBadge>
        <span v-if="schedule" class="inline-flex items-center gap-1">
          <UIcon name="i-lucide-calendar-days" class="size-3" />
          {{ schedule }}
        </span>
        <span v-if="task.estimateMinutes" class="inline-flex items-center gap-1">
          <UIcon name="i-lucide-timer" class="size-3" />
          {{ formatEstimate(task.estimateMinutes) }}
        </span>

        <UBadge v-if="task.sourceType === 'document'" color="primary" size="sm" variant="subtle">文档</UBadge>
        <span v-for="reason in reasons" :key="reason" class="text-primary">{{ reason }}</span>
        <span v-if="task.externalTransferred" class="inline-flex items-center gap-1">
          <UIcon name="i-lucide-arrow-right-left" class="size-3" />
          {{ task.externalAssignedTo ? `已转交 · ${task.externalAssignedTo}` : "已转出" }}
        </span>
      </div>
      <div
        v-if="isZentao && ((task.externalAssignedTo && !task.externalTransferred) || task.externalUpdatedDate)"
        class="mt-1 truncate text-[11px] leading-4 text-dimmed"
      >
        <template v-if="task.externalAssignedTo && !task.externalTransferred">指派给 {{ task.externalAssignedTo }}</template>
        <template v-if="task.externalAssignedTo && !task.externalTransferred && task.externalUpdatedDate"> · </template>
        <template v-if="task.externalUpdatedDate">更新于 {{ task.externalUpdatedDate.slice(0, 16) }}</template>
      </div>
    </div>

    <div
      v-if="isZentao && !selectable && !task.externalTransferred"
      class="col-span-2 col-start-2 row-start-2 [&:not(:has(button))]:hidden @md:col-span-1 @md:col-start-3 @md:row-start-1"
    >
      <ZentaoActions :task="task" compact @updated="emit('updated', $event)" />
    </div>

    <div class="col-start-3 row-start-1 flex items-center gap-0.5 pt-0.5 @md:col-start-4">
      <UBadge :color="PRIORITY_COLOR[task.priority]" size="sm" variant="subtle">
        {{ PRIORITY_LABELS[task.priority] }}
      </UBadge>
      <UButton
        v-if="task.externalUrl"
        icon="i-lucide-external-link"
        color="neutral"
        variant="ghost"
        size="xs"
        square
        title="在禅道中打开"
        @click.stop="openExternal(task.externalUrl).catch(() => {})"
      />
      <UButton
        v-if="!selectable && !isZentao"
        icon="i-lucide-trash-2"
        color="error"
        variant="ghost"
        size="xs"
        square
        title="删除"
        class="opacity-0 transition group-hover:opacity-100 focus-visible:opacity-100"
        @click.stop="emit('delete', task)"
      />
    </div>
  </article>
</template>
