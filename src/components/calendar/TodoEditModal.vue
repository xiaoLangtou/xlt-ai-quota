<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { useCalendarTodo } from "@/composables/useCalendarTodo";
import { DatePicker } from "@/components/ui/date-picker";
import {
  CATEGORY_LABELS,
  PRIORITY_LABELS,
  STATUS_LABELS,
  TODO_CATEGORIES,
  TODO_PRIORITIES,
  TODO_STATUSES,
  type TodoCategory,
  type TodoDraft,
  type TodoPriority,
  type TodoStatus,
  type TodoTask,
} from "@/types/productivity";
import { formatEstimate, today as todayString } from "@/utils/productivity";
import { openExternal } from "@/services/zentao-service";

const props = defineProps<{
  open: boolean;
  task?: TodoTask | null;
  defaultDate?: string | null;
  defaultTime?: string | null;
  defaultEstimate?: number | null;
  defaultStatus?: TodoStatus | null;
}>();

const emit = defineEmits<{
  (event: "update:open", value: boolean): void;
  (event: "saved"): void;
  (event: "deleted"): void;
}>();

const { createTask, deleteTask, allTasks } = useCalendarTodo();
const saving = ref(false);

/** 依赖项展示任务标题；未加载到时回退为短 id。 */
function depTitle(id: string): string {
  return allTasks.value.find((task) => task.id === id)?.title ?? id.slice(0, 8);
}

const form = reactive({
  title: "",
  description: "",
  priority: "medium" as TodoPriority,
  category: "work" as TodoCategory,
  status: "open" as TodoStatus,
  plannedDate: "",
  plannedTime: "",
  estimateMinutes: 0,
  acceptance: [] as string[],
});

const isEdit = computed(() => Boolean(props.task));
const isZentao = computed(() => props.task?.sourceType === "zentao");
const title = computed(() => isZentao.value ? "安排时间" : isEdit.value ? "编辑待办" : "新建待办");

const statusOptions = TODO_STATUSES.map((value) => ({ value, label: STATUS_LABELS[value] }));
const priorityOptions = TODO_PRIORITIES.map((value) => ({ value, label: PRIORITY_LABELS[value] }));
const categoryOptions = TODO_CATEGORIES.map((value) => ({ value, label: CATEGORY_LABELS[value] }));
const estimateOptions = [
  { value: 0, label: "未估" },
  { value: 30, label: "30m" },
  { value: 60, label: "1h" },
  { value: 90, label: "1.5h" },
  { value: 120, label: "2h" },
  { value: 180, label: "3h" },
  { value: 240, label: "4h" },
  { value: 480, label: "1d" },
];

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    const task = props.task;
    form.title = task?.title ?? "";
    form.description = task?.description ?? "";
    form.priority = task?.priority ?? "medium";
    form.category = task?.category ?? "work";
    form.status = task?.status ?? props.defaultStatus ?? "open";
    form.plannedDate = task?.plannedDate ?? props.defaultDate ?? "";
    form.plannedTime = task?.plannedTime ?? props.defaultTime ?? "";
    form.estimateMinutes = task?.estimateMinutes ?? props.defaultEstimate ?? 0;
    form.acceptance = task?.acceptanceCriteria.map((item) => item.content) ?? [];
  },
);

function addCriterion(): void {
  if (form.acceptance.length >= 6) return;
  form.acceptance.push("");
}

function removeCriterion(index: number): void {
  form.acceptance.splice(index, 1);
}

async function submit(): Promise<void> {
  if (!form.title.trim() || saving.value) return;
  saving.value = true;
  // 填了时间但没选日期时，默认落到今天，避免出现“有时间无日期”的悬空数据。
  const plannedDate = form.plannedDate || (form.plannedTime ? todayString() : "");
  const draft: TodoDraft = {
    id: props.task?.id ?? null,
    title: form.title,
    description: form.description,
    status: form.status,
    priority: form.priority,
    category: form.category,
    plannedDate: plannedDate || null,
    plannedTime: form.plannedTime || null,
    estimateMinutes: form.estimateMinutes || null,
    sourceType: props.task?.sourceType ?? "manual",
    sourceName: props.task?.sourceName ?? null,
    acceptanceCriteria: form.acceptance,
    dependencyIds: props.task?.dependencyIds ?? [],
  };
  const task = await createTask(draft);
  saving.value = false;
  if (task) {
    emit("update:open", false);
    emit("saved");
  }
}

async function remove(): Promise<void> {
  if (!props.task || saving.value) return;
  if (!window.confirm(`删除「${props.task.title}」及其关联日历事件？`)) return;
  saving.value = true;
  const ok = await deleteTask(props.task.id);
  saving.value = false;
  if (ok) {
    emit("update:open", false);
    emit("deleted");
  }
}
</script>

<template>
  <UModal :open="open" :title="title" @update:open="emit('update:open', $event)">
    <template #body>
      <div class="edit-form">
        <p v-if="isZentao" class="text-sm font-medium text-highlighted">{{ task?.title }}</p>
        <UFormField v-if="!isZentao" label="标题" required>
          <UInput v-model="form.title" placeholder="动词开头，说明具体产出" class="w-full" />
        </UFormField>
        <UFormField v-if="!isZentao" label="描述">
          <UTextarea v-model="form.description" :rows="6" placeholder="补充实现要点（可选）" class="w-full" />
        </UFormField>
        <div class="grid-2">
          <UFormField v-if="!isZentao" label="状态">
            <USelect v-model="form.status" :disabled="task?.sourceType === 'zentao'" :items="statusOptions" class="w-full" />
          </UFormField>
          <UFormField v-if="!isZentao" label="优先级">
            <USelect v-model="form.priority" :items="priorityOptions" class="w-full" />
          </UFormField>
          <UFormField label="分类">
            <USelect v-model="form.category" :items="categoryOptions" class="w-full" />
          </UFormField>
          <UFormField label="预估工作量">
            <USelect v-model="form.estimateMinutes" :items="estimateOptions" class="w-full" />
          </UFormField>
          <UFormField label="计划日期">
            <DatePicker v-model="form.plannedDate" />
          </UFormField>
          <UFormField label="计划时间">
            <UInput v-model="form.plannedTime" type="time" class="w-full" />
          </UFormField>
        </div>

        <UFormField v-if="!isZentao" label="验收标准">
          <div class="criteria">
            <div v-for="(_, index) in form.acceptance" :key="index" class="criterion">
              <UInput v-model="form.acceptance[index]" placeholder="可观察、可验证的结果" class="w-full" />
              <UButton
                icon="i-lucide-x"
                color="neutral"
                variant="ghost"
                square
                size="sm"
                aria-label="移除"
                @click="removeCriterion(index)"
              />
            </div>
            <UButton
              v-if="form.acceptance.length < 6"
              icon="i-lucide-plus"
              color="neutral"
              variant="outline"
              size="sm"
              @click="addCriterion"
            >
              添加验收标准
            </UButton>
          </div>
        </UFormField>

        <div v-if="task?.dependencyIds.length" class="deps">
          <span class="deps-label">依赖任务</span>
          <UBadge
            v-for="id in task.dependencyIds"
            :key="id"
            color="neutral"
            variant="subtle"
            size="sm"
            class="max-w-56 truncate"
          >
            {{ depTitle(id) }}
          </UBadge>
        </div>
        <p v-if="task?.sourceName" class="source">
          来源：{{ task.sourceType === "document" ? `文档拆解 · ${task.sourceName}` : task.sourceType === "zentao" ? `禅道同步 · ${task.sourceName}` : "手动创建" }}
          <UButton
            v-if="task?.externalUrl"
            icon="i-lucide-external-link"
            color="neutral"
            variant="link"
            size="xs"
            class="px-0"
            @click="task.externalUrl && openExternal(task.externalUrl).catch(() => {})"
          >
            在禅道中打开
          </UButton>
          <span v-if="task.estimateMinutes">· 预估 {{ formatEstimate(task.estimateMinutes) }}</span>
        </p>
      </div>
    </template>

    <template #footer>
      <div class="footer">
        <UButton v-if="isEdit && !isZentao" color="error" variant="soft" icon="i-lucide-trash-2" @click="remove">删除</UButton>
        <div class="footer-right">
          <UButton color="neutral" variant="ghost" @click="emit('update:open', false)">取消</UButton>
          <UButton icon="i-lucide-check" :loading="saving" :disabled="!form.title.trim()" @click="submit">
            {{ isEdit ? "保存" : "创建待办" }}
          </UButton>
        </div>
      </div>
    </template>
  </UModal>
</template>

<style scoped>
.edit-form {
  display: grid;
  gap: 14px;
}
.grid-2 {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 14px;
}
.criteria {
  display: grid;
  gap: 8px;
  width: 100%;
}
.criterion {
  display: flex;
  align-items: center;
  gap: 6px;
}
.deps {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}
.deps-label {
  color: var(--text-subtle);
  font-size: 11px;
}
.source {
  margin: 0;
  color: var(--text-subtle);
  font-size: 11px;
}
.footer {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
}
.footer-right {
  display: flex;
  gap: 8px;
  margin-left: auto;
}
</style>
