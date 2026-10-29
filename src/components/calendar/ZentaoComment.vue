<script setup lang="ts">
import { ref, watch } from "vue";
import { zentaoService } from "@/services/zentao-service";
import { pushToast } from "@/composables/useToast";
import type { TodoTask } from "@/types/productivity";

const props = defineProps<{ task: TodoTask }>();
const emit = defineEmits<{ commented: [] }>();
const open = ref(false);
const comment = ref("");
const password = ref("");
const saving = ref(false);
const checking = ref(false);
const uncertain = ref(false);
const error = ref("");
watch(() => props.task.id, () => { open.value = false; comment.value = ""; password.value = ""; uncertain.value = false; error.value = ""; });
watch(open, value => { if (!value) password.value = ""; });

async function submit(): Promise<void> {
  if (saving.value || uncertain.value || !comment.value.trim() || !password.value) return;
  const taskId = props.task.id;
  const text = comment.value.trim();
  const secret = password.value;
  password.value = "";
  saving.value = true;
  error.value = "";
  try {
    await zentaoService.comment(taskId, text, secret);
    if (taskId !== props.task.id) return;
    comment.value = "";
    open.value = false;
    emit("commented");
    pushToast("评论已提交并核实", "success");
  } catch (reason) {
    if (taskId !== props.task.id) return;
    error.value = String(reason);
    uncertain.value = error.value.includes("勿重复提交");
  } finally { saving.value = false; }
}

async function check(): Promise<void> {
  const id = Number(props.task.externalId?.split(":").pop());
  if (!Number.isInteger(id) || id <= 0 || !props.task.externalType) return;
  checking.value = true;
  try {
    await zentaoService.detail(props.task.externalType, id, { force: true });
    emit("commented");
    error.value = "已刷新动态，请检查评论是否出现，再决定是否重新提交。";
    uncertain.value = false;
  } catch (reason) { error.value = String(reason); }
  finally { checking.value = false; }
}
</script>

<template>
  <UButton icon="i-lucide-message-square" size="sm" variant="outline" color="neutral" @click="open = true">评论</UButton>
  <UModal v-model:open="open" title="发表评论" :description="task.title" :dismissible="!saving" :close="saving ? false : undefined">
    <template #body>
      <form class="space-y-4" @submit.prevent="submit">
        <UAlert v-if="error" :color="uncertain ? 'warning' : 'error'" variant="subtle" :description="error" />
        <UFormField label="评论" required><UTextarea v-model="comment" :rows="4" :maxlength="4000" :disabled="saving || uncertain" class="w-full" /></UFormField>
        <UFormField label="禅道密码" required><UInput v-model="password" type="password" autocomplete="current-password" :disabled="saving || uncertain" class="w-full" /></UFormField>
        <p class="text-xs text-muted">评论使用禅道网页会话，密码仅用于本次登录，不会保存。</p>
        <div class="flex justify-end gap-2">
          <UButton color="neutral" variant="ghost" :disabled="saving" @click="open = false">取消</UButton>
          <UButton v-if="uncertain" color="neutral" variant="outline" :loading="checking" @click="check">刷新动态核实</UButton>
          <UButton v-else type="submit" :loading="saving" :disabled="!comment.trim() || !password">提交评论</UButton>
        </div>
      </form>
    </template>
  </UModal>
</template>
