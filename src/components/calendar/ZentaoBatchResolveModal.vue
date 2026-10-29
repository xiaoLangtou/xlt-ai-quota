<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { pushToast } from "@/composables/useToast";
import { zentaoService } from "@/services/zentao-service";
import type {
  TodoTask,
  ZentaoBatchItemResult,
  ZentaoBatchResolveResult,
} from "@/types/productivity";

const props = defineProps<{ tasks: TodoTask[] }>();
const open = defineModel<boolean>("open", { default: false });
const emit = defineEmits<{ finished: [result: ZentaoBatchResolveResult] }>();

// 重复 Bug 需要为每个条目单独填编号，批量场景不提供；其余方案可统一提交。
const resolutions = [
  { value: "fixed", label: "已解决" },
  { value: "bydesign", label: "设计如此" },
  { value: "external", label: "外部原因" },
  { value: "notrepro", label: "无法重现" },
  { value: "postponed", label: "延期处理" },
  { value: "willnotfix", label: "不予解决" },
];

const form = reactive({ resolution: "fixed", actionDate: localDateTime(), comment: "" });
const saving = ref(false);
const error = ref("");
const failures = ref<ZentaoBatchItemResult[]>([]);

const canSubmit = computed(() =>
  Boolean(form.resolution && form.actionDate) && props.tasks.length > 0 && !saving.value,
);

function localDateTime(): string {
  const date = new Date();
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

function apiDate(value: string): string {
  return `${value.replace("T", " ")}:00`;
}

watch(open, (value) => {
  if (!value) return;
  form.resolution = "fixed";
  form.actionDate = localDateTime();
  form.comment = "";
  error.value = "";
  failures.value = [];
});

async function submit(): Promise<void> {
  if (!canSubmit.value) return;
  saving.value = true;
  error.value = "";
  failures.value = [];
  try {
    const result = await zentaoService.batchResolve({
      taskIds: props.tasks.map((task) => task.id),
      resolution: form.resolution,
      resolvedBuild: form.resolution === "fixed" ? "trunk" : "",
      actionDate: apiDate(form.actionDate),
      comment: form.comment,
    });
    failures.value = result.items.filter((item) => !item.ok);
    emit("finished", result);
    if (result.failed === 0) {
      pushToast(`已解决 ${result.succeeded} 个 Bug`, "success");
      open.value = false;
    } else {
      pushToast(`已解决 ${result.succeeded} 个，失败 ${result.failed} 个`, "warn");
      error.value = "部分 Bug 未完成，可查看下方原因后重试。";
    }
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : String(reason);
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <UModal
    v-model:open="open"
    title="批量解决 Bug"
    :description="`将解决 ${tasks.length} 个禅道 Bug，逐条提交并回读核实。`"
    :dismissible="!saving"
    :close="saving ? false : undefined"
    :ui="{ content: 'max-w-xl' }"
  >
    <template #body>
      <div class="space-y-4">
        <div class="rounded-lg border border-default bg-elevated/40 p-3">
          <p class="mb-1.5 text-[11px] font-semibold text-muted">已选择</p>
          <ul class="max-h-36 space-y-1 overflow-y-auto text-xs text-default">
            <li v-for="task in tasks" :key="task.id" class="truncate" :title="task.title">
              {{ task.title }}
            </li>
          </ul>
        </div>

        <UFormField label="解决方案" required>
          <USelect v-model="form.resolution" :items="resolutions" class="w-full" />
        </UFormField>

        <UFormField label="解决时间" required>
          <UInput v-model="form.actionDate" type="datetime-local" class="w-full" />
        </UFormField>
        <p v-if="form.resolution === 'fixed'" class="text-[11px] text-dimmed">
          批量解决统一使用主干（trunk）作为解决版本；需要指定具体版本时请在单条详情里操作。
        </p>

        <UFormField label="备注">
          <UTextarea v-model="form.comment" :rows="3" :maxlength="4000" class="w-full" />
        </UFormField>

        <UAlert v-if="error" color="warning" variant="subtle" :description="error" />

        <div v-if="failures.length" class="rounded-lg border border-error/30 bg-error/5 p-3">
          <p class="mb-1.5 text-[11px] font-semibold text-error">未完成 {{ failures.length }} 个</p>
          <ul class="max-h-40 space-y-1 overflow-y-auto text-[11px] text-default">
            <li v-for="item in failures" :key="item.taskId">
              <span class="font-medium">{{ item.title }}</span>：{{ item.error }}
            </li>
          </ul>
        </div>
      </div>
    </template>

    <template #footer>
      <div class="flex w-full justify-end gap-2">
        <UButton color="neutral" variant="ghost" :disabled="saving" @click="open = false">取消</UButton>
        <UButton icon="i-lucide-circle-check" :loading="saving" :disabled="!canSubmit" @click="submit">
          解决选中的 {{ tasks.length }} 个 Bug
        </UButton>
      </div>
    </template>
  </UModal>
</template>
