<script setup lang="ts">
import { nextTick, ref } from "vue";

const emit = defineEmits<{
  (event: "full"): void;
  (event: "quick", text: string): void;
}>();

const open = ref(false);
const text = ref("");
const input = ref<HTMLElement | null>(null);

function openFull(): void {
  open.value = false;
  emit("full");
}

function submitQuick(): void {
  const value = text.value.trim();
  if (!value) return;
  emit("quick", value);
  text.value = "";
  open.value = false;
}

function onOpenChange(value: boolean): void {
  open.value = value;
  if (value) void nextTick(() => input.value?.querySelector("input")?.focus());
}
</script>

<template>
  <UPopover :open="open" :content="{ side: 'bottom', align: 'end' }" @update:open="onOpenChange">
    <UButton icon="i-lucide-plus" size="sm">新建</UButton>

    <template #content>
      <div class="new-task-menu">
        <UButton
          icon="i-lucide-list-plus"
          color="neutral"
          variant="ghost"
          block
          class="justify-start"
          @click="openFull"
        >
          新建待办
          <template #trailing><UKbd value="n" variant="soft" class="ms-auto" /></template>
        </UButton>

        <USeparator class="my-2" />

        <form @submit.prevent="submitQuick">
          <UFormField label="快速创建" class="px-1 pb-1">
            <div ref="input">
              <UInput
                v-model="text"
                placeholder="明天下午3点 评审拆解结果"
                :maxlength="120"
                class="w-full"
              />
            </div>
          </UFormField>
        </form>
      </div>
    </template>
  </UPopover>
</template>

<style scoped>
.new-task-menu {
  width: 292px;
  padding: 8px;
}
</style>
