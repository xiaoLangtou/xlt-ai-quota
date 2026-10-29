<script setup lang="ts">
import { nextTick, ref, watch } from "vue";
import { CATEGORY_LABELS, TODO_CATEGORIES, type TodoCategory } from "@/types/productivity";
import { formatDateLabel } from "@/utils/productivity";

const props = defineProps<{ date: string; time?: string; x: number; y: number }>();
const emit = defineEmits<{
  (event: "create", payload: { title: string; time: string; category: TodoCategory }): void;
  (event: "full", payload: { date: string; time: string }): void;
  (event: "close"): void;
}>();

const open = ref(true);
const title = ref("");
const time = ref(props.time ?? "09:00");
const category = ref<TodoCategory>("work");
const inputWrap = ref<HTMLElement | null>(null);

const CATEGORY_OPTIONS = TODO_CATEGORIES.map((value) => ({ value, label: CATEGORY_LABELS[value] }));

// 虚拟锚点：让 UPopover 挂在鼠标点击位置，替代旧的 fixed 定位 + 自管理 outside-click。
const virtualAnchor = {
  getBoundingClientRect: () => ({
    x: props.x,
    y: props.y,
    top: props.y,
    left: props.x,
    bottom: props.y,
    right: props.x,
    width: 0,
    height: 0,
    toJSON: () => ({}),
  }),
};

watch(open, (value) => {
  if (!value) emit("close");
});

watch(
  () => open.value,
  async (value) => {
    if (!value) return;
    await nextTick();
    inputWrap.value?.querySelector("input")?.focus();
  },
  { immediate: true },
);

function submit(): void {
  const value = title.value.trim();
  if (!value) return;
  emit("create", { title: value, time: time.value, category: category.value });
}

function openFull(): void {
  emit("full", { date: props.date, time: time.value });
}
</script>

<template>
  <UPopover
    :open="open"
    :reference="virtualAnchor as unknown as HTMLElement"
    :content="{ side: 'right', align: 'start' }"
    @update:open="open = $event"
  >
    <template #content>
      <form class="quick-create" @submit.prevent="submit">
        <div class="quick-create-head">
          <strong>快速创建待办</strong>
          <span>{{ formatDateLabel(date) }}</span>
        </div>
        <div ref="inputWrap">
          <UInput v-model="title" placeholder="输入待办标题…" autocomplete="off" class="w-full" />
        </div>
        <div class="quick-create-fields">
          <UInput v-model="time" type="time" aria-label="时间" />
          <USelect v-model="category" :items="CATEGORY_OPTIONS" aria-label="分类" class="w-full" />
        </div>
        <div class="quick-create-actions">
          <UButton type="button" color="neutral" variant="ghost" size="sm" @click="openFull">
            更多选项
          </UButton>
          <div class="spacer" />
          <UButton type="submit" size="sm" icon="i-lucide-plus" :disabled="!title.trim()">创建待办</UButton>
        </div>
      </form>
    </template>
  </UPopover>
</template>

<style scoped>
.quick-create {
  width: 292px;
  padding: 13px;
  display: grid;
  gap: 8px;
}
.quick-create-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.quick-create-head strong {
  font-size: 12px;
}
.quick-create-head span {
  color: var(--text-subtle);
  font-family: var(--font-mono);
  font-size: 9.5px;
}
.quick-create-fields {
  display: grid;
  grid-template-columns: 1fr 1.2fr;
  gap: 7px;
}
.quick-create-actions {
  display: flex;
  align-items: center;
  gap: 7px;
  margin-top: 2px;
}
.quick-create-actions .spacer {
  flex: 1;
}
</style>
