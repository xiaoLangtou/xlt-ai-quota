<script setup lang="ts">
import { computed } from "vue";
import type { McpPlan } from "@/types/mcp";

const props = defineProps<{
  open: boolean;
  plan: McpPlan | null;
  busy?: boolean;
}>();

const emit = defineEmits<{
  "update:open": [value: boolean];
  confirm: [];
}>();

const changedTargets = computed(
  () => props.plan?.targets.filter((target) => target.diff.trim().length > 0) ?? [],
);

function diffLines(diff: string): { id: number; text: string; tone: string }[] {
  return diff
    .replace(/\n$/, "")
    .split("\n")
    .map((text, id) => ({
      id,
      text,
      tone: text.startsWith("+") ? "text-success" : text.startsWith("-") ? "text-error" : "text-muted",
    }));
}

function close() {
  emit("update:open", false);
}
</script>

<template>
  <UModal
    :open="open"
    :title="plan?.summary ?? '变更预览'"
    description="确认前请检查每个目标文件的改动；写入前会自动备份，失败自动回滚。"
    :ui="{ content: 'max-w-3xl' }"
    @update:open="!$event && close()"
  >
    <template #body>
      <div class="space-y-4">
        <UAlert
          v-if="plan?.warnings.length"
          color="warning"
          variant="subtle"
          icon="i-lucide-triangle-alert"
          title="注意"
        >
          <ul class="mt-1 list-disc space-y-1 pl-4 text-xs">
            <li v-for="warning in plan.warnings" :key="warning">{{ warning }}</li>
          </ul>
        </UAlert>

        <div v-if="!changedTargets.length" class="rounded-lg bg-elevated/50 p-4 text-sm text-muted">
          没有需要写入的改动。
        </div>

        <div
          v-for="target in changedTargets"
          :key="target.configFile + target.agent"
          class="overflow-hidden rounded-lg ring ring-default"
        >
          <div class="flex items-center justify-between gap-2 border-b border-default bg-elevated/40 px-3 py-2">
            <div class="flex min-w-0 items-center gap-2">
              <UBadge color="primary" variant="subtle" size="sm">{{ target.agentLabel }}</UBadge>
              <UBadge color="neutral" variant="soft" size="sm">
                {{ target.scope === "global" ? "全局" : target.projectPath }}
              </UBadge>
            </div>
            <div class="flex items-center gap-2">
              <UBadge v-if="!target.exists" color="warning" variant="soft" size="sm">新建文件</UBadge>
              <span class="truncate font-mono text-[11px] text-dimmed" :title="target.configFile">
                {{ target.configFile }}
              </span>
            </div>
          </div>
          <div class="max-h-64 overflow-auto bg-default px-3 py-2 font-mono text-[11px] leading-relaxed"><div
            v-for="line in diffLines(target.diff)"
            :key="line.id"
            :class="line.tone"
          >{{ line.text || " " }}</div></div>
        </div>
      </div>
    </template>

    <template #footer>
      <div class="flex w-full justify-end gap-2">
        <UButton color="neutral" variant="ghost" :disabled="busy" @click="close">取消</UButton>
        <UButton icon="i-lucide-save" :loading="busy" :disabled="!changedTargets.length" @click="emit('confirm')">
          确认写入
        </UButton>
      </div>
    </template>
  </UModal>
</template>
