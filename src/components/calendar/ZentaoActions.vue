<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { useCalendarTodo } from "@/composables/useCalendarTodo";
import { zentaoService } from "@/services/zentao-service";
import { pushToast } from "@/composables/useToast";
import type { TodoTask, ZentaoActionContext, ZentaoActionInput, ZentaoOperation } from "@/types/productivity";

const props = defineProps<{ task: TodoTask; compact?: boolean; showAssign?: boolean }>();
const emit = defineEmits<{ (event: "updated", task: TodoTask): void; (event: "submitting", task: TodoTask): void }>();
const { refresh } = useCalendarTodo();
const operation = ref<ZentaoOperation | null>(null);
const context = ref<ZentaoActionContext | null>(null);
const loading = ref(false);
const saving = ref(false);
const error = ref("");
const needsRefresh = ref(false);
const labels = { assign: "指派 Bug", resolve: "解决 Bug", start: "开始任务", finish: "完成任务" };
const statusLabels: Record<string, string> = {
  active: "激活", resolved: "已解决", closed: "已关闭", wait: "未开始",
  doing: "进行中", pause: "已暂停", done: "已完成", cancel: "已取消",
};
const resolutions = [
  { value: "bydesign", label: "设计如此" }, { value: "duplicate", label: "重复 Bug" },
  { value: "external", label: "外部原因" }, { value: "fixed", label: "已解决" },
  { value: "notrepro", label: "无法重现" }, { value: "postponed", label: "延期处理" },
  { value: "willnotfix", label: "不予解决" },
];
const form = reactive({ assignedTo: "", resolution: "", duplicateBug: "",
  resolvedBuild: "", actionDate: "", realStarted: "", consumed: "",
  left: "", comment: "" });
const isBug = computed(() => props.task.externalType === "bug");
const open = computed({ get: () => operation.value !== null, set: (value) => { if (!value && !saving.value) operation.value = null; } });
const currentStatus = computed(() => context.value?.status ?? props.task.externalStatus ?? "");
const allowed = computed(() => {
  if (!context.value) return false;
  if (operation.value === "assign") return currentStatus.value !== "closed";
  if (operation.value === "resolve") return currentStatus.value === "active";
  if (operation.value === "start") return currentStatus.value === "wait";
  return ["wait", "doing", "pause"].includes(currentStatus.value);
});
function localDateTime(): string {
  const date = new Date();
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}`;
}
function apiDate(value: string): string { return `${value.replace("T", " ")}:00`; }
let generation = 0;
watch(() => props.task.id, () => { generation++; operation.value = null; context.value = null; });
async function loadContext(): Promise<void> {
  const id = props.task.id;
  const request = ++generation;
  loading.value = true;
  error.value = "";
  context.value = null;
  try {
    const result = await zentaoService.actionContext(id, operation.value!);
    if (generation !== request || id !== props.task.id) return;
    context.value = result;
    needsRefresh.value = false;
    form.assignedTo = operation.value === "assign" ? result.assignedTo : operation.value === "resolve" ? result.openedBy : "";
    if (operation.value === "resolve") form.resolvedBuild = "trunk";
    form.realStarted = result.realStarted ? result.realStarted.slice(0, 16).replace(" ", "T") : localDateTime();
    form.consumed = operation.value === "start" ? String(result.consumed) : "";
    form.left = String(result.left);
  } catch (reason) {
    if (generation === request) error.value = String(reason);
  } finally {
    if (generation === request) loading.value = false;
  }
}
function begin(value: ZentaoOperation): void {
  operation.value = value;
  Object.assign(form, { assignedTo: "", resolution: "", duplicateBug: "", resolvedBuild: "", actionDate: localDateTime(), realStarted: "", consumed: "", left: "", comment: "" });
  needsRefresh.value = false;
  void loadContext();
}
function validate(): { name: string; message: string }[] {
  const errors: { name: string; message: string }[] = [];
  const required = (name: string, value: unknown) => {
    if (value === "" || value == null) errors.push({ name, message: "请填写此项" });
  };
  if (operation.value === "assign") required("assignedTo", form.assignedTo);
  if (operation.value === "resolve") {
    required("resolution", form.resolution);
    required("actionDate", form.actionDate);
    if (form.resolution === "fixed") required("resolvedBuild", form.resolvedBuild);
    if (form.resolution === "duplicate" && (!Number.isInteger(Number(form.duplicateBug)) || Number(form.duplicateBug) <= 0)) {
      errors.push({ name: "duplicateBug", message: "请填写有效的 Bug 编号" });
    }
  }
  if (operation.value === "start" || operation.value === "finish") {
    required("realStarted", form.realStarted);
    required("consumed", form.consumed);
    if (Number(form.consumed) < 0) errors.push({ name: "consumed", message: "工时不能为负数" });
    if (operation.value === "start") {
      required("left", form.left);
      if (Number(form.left) < 0) errors.push({ name: "left", message: "工时不能为负数" });
    }
    if (operation.value === "finish") required("actionDate", form.actionDate);
  }
  return errors;
}
async function submit(): Promise<void> {
  if (!operation.value || saving.value || loading.value || !allowed.value || needsRefresh.value || validate().length) return;
  const taskId = props.task.id;
  const input: ZentaoActionInput = {
    taskId, operation: operation.value, assignedTo: form.assignedTo,
    resolution: form.resolution, duplicateBug: form.resolution === "duplicate" ? Number(form.duplicateBug) : null,
    resolvedBuild: form.resolvedBuild, actionDate: apiDate(form.actionDate), realStarted: apiDate(form.realStarted),
    consumed: Number(form.consumed), left: Number(form.left), comment: form.comment,
  };
  saving.value = true;
  emit("submitting", props.task);
  error.value = "";
  try {
    const result = await zentaoService.action(input);
    if (result.warning) {
      needsRefresh.value = true;
      error.value = result.warning;
      return;
    }
    if (result.task) emit("updated", result.task);
    operation.value = null;
    pushToast("禅道操作已生效", "success");
    await refresh();
  } catch (reason) {
    needsRefresh.value = true;
    error.value = String(reason);
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <div class="flex flex-wrap items-center gap-2" @click.stop @mousedown.stop>
    <UBadge v-if="!compact && task.externalStatus" color="neutral" variant="subtle">禅道：{{ statusLabels[task.externalStatus] ?? task.externalStatus }}</UBadge>
    <UBadge v-if="!compact && task.externalTransferred" color="warning" variant="subtle">{{ task.externalAssignedTo ? `已转交 · ${task.externalAssignedTo}` : '已移出我的指派' }}</UBadge>
    <template v-if="isBug">
      <UButton v-if="(!compact || showAssign) && task.externalStatus !== 'closed'" icon="i-lucide-user-round-plus" size="sm" color="neutral" variant="outline" @click="begin('assign')">指派</UButton>
      <UButton v-if="task.externalStatus === 'active' || (!task.externalStatus && task.status !== 'done')" icon="i-lucide-circle-check" size="sm" @click="begin('resolve')">解决</UButton>
    </template>
    <template v-else-if="task.externalType === 'task'">
      <UButton v-if="task.externalStatus === 'wait' || (!task.externalStatus && task.status === 'open')" icon="i-lucide-play" size="sm" @click="begin('start')">开始</UButton>
      <UButton v-if="['wait', 'doing', 'pause'].includes(task.externalStatus ?? '') || (!task.externalStatus && task.status !== 'done')" icon="i-lucide-circle-check" size="sm" color="success" @click="begin('finish')">完成</UButton>
    </template>
  </div>
  <UModal v-model:open="open" :title="operation ? labels[operation] : ''" :description="task.title" :dismissible="!saving" :close="saving ? false : undefined">
    <template #body>
      <p v-if="loading" class="text-sm text-muted">正在读取禅道当前状态和可选项…</p>
      <UAlert v-if="error" color="error" variant="subtle" :description="error" class="mb-4" />
      <template v-if="context && !loading">
        <p class="mb-4 text-xs text-muted">禅道状态：{{ statusLabels[context.status] ?? context.status }} · 当前指派：{{ context.assignedTo || '未指派' }}</p>
        <UAlert v-if="!allowed" color="warning" title="当前状态不支持此操作，请关闭后重新选择操作。" class="mb-4" />
        <UForm :state="form" :validate="validate" class="space-y-4" @submit="submit">
          <fieldset :disabled="saving || needsRefresh || !allowed" class="space-y-4">
            <UFormField v-if="operation === 'assign' || operation === 'resolve'" name="assignedTo" label="指派给" :required="operation === 'assign'">
              <USelectMenu v-model="form.assignedTo" :items="context.users" value-key="value" placeholder="选择人员" class="w-full" />
              <p v-if="operation === 'resolve'" class="mt-1 text-xs text-muted">默认指派给 Bug 创建人，可手动调整。</p>
            </UFormField>
            <template v-if="operation === 'resolve'">
              <UFormField name="resolution" label="解决方案" required><USelect v-model="form.resolution" :items="resolutions" placeholder="选择解决方案" class="w-full" /></UFormField>
              <UFormField v-if="form.resolution === 'duplicate'" name="duplicateBug" label="重复 Bug 编号" required><UInput v-model="form.duplicateBug" type="number" :min="1" :step="1" class="w-full" /></UFormField>
              <UFormField name="resolvedBuild" label="解决版本" :required="form.resolution === 'fixed'"><USelectMenu v-model="form.resolvedBuild" :items="context.builds" value-key="value" placeholder="选择解决版本" class="w-full" /></UFormField>
            </template>
            <template v-if="operation === 'start' || operation === 'finish'">
              <UFormField name="realStarted" label="实际开始时间" required><UInput v-model="form.realStarted" type="datetime-local" class="w-full" /></UFormField>
              <UFormField name="consumed" :label="operation === 'start' ? '累计消耗工时（小时）' : '本次消耗工时（小时）'" required><UInput v-model="form.consumed" type="number" :min="0" :step="0.1" class="w-full" /></UFormField>
              <UFormField v-if="operation === 'start'" name="left" label="预计剩余工时（小时）" required><UInput v-model="form.left" type="number" :min="0" :step="0.1" class="w-full" /></UFormField>
            </template>
            <UFormField v-if="operation === 'resolve' || operation === 'finish'" name="actionDate" :label="operation === 'resolve' ? '解决时间' : '完成时间'" required><UInput v-model="form.actionDate" type="datetime-local" class="w-full" /></UFormField>
            <UFormField name="comment" label="备注"><UTextarea v-model="form.comment" :rows="3" :maxlength="4000" class="w-full" /></UFormField>
          </fieldset>
          <div class="flex justify-end gap-2">
            <UButton color="neutral" variant="ghost" :disabled="saving" @click="open = false">取消</UButton>
            <UButton v-if="!needsRefresh" type="submit" :loading="saving" :disabled="!allowed">提交到禅道</UButton>
          </div>
        </UForm>
      </template>
      <UButton v-if="!loading && (error || needsRefresh)" class="mt-3" color="neutral" variant="outline" :disabled="saving" @click="loadContext">刷新禅道状态</UButton>
    </template>
  </UModal>
</template>
