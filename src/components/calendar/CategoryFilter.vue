<script setup lang="ts">
import { computed } from "vue";
import { useCalendarTodo } from "@/composables/useCalendarTodo";
import { CATEGORY_LABELS, TODO_CATEGORIES, type TodoCategory } from "@/types/productivity";
import { CATEGORY_DOT_CLASS } from "@/utils/productivity";

// 参考模板的多日历可见性开关：这里按分类控制日历事件的显示/隐藏。
const { hiddenCategories, toggleCategory, events, anchor } = useCalendarTodo();

const counts = computed(() => {
  const month = anchor.value.slice(0, 7);
  const result: Record<TodoCategory, number> = { work: 0, planning: 0, personal: 0 };
  for (const item of events.value) {
    if (item.startAt.slice(0, 7) === month) result[item.category] += 1;
  }
  return result;
});

function isVisible(category: TodoCategory): boolean {
  return !hiddenCategories.value.includes(category);
}
</script>

<template>
  <div class="category-filter" role="group" aria-label="日历分类可见性">
    <button
      v-for="category in TODO_CATEGORIES"
      :key="category"
      type="button"
      class="category-chip"
      :class="{ off: !isVisible(category) }"
      :aria-pressed="isVisible(category)"
      @click="toggleCategory(category)"
    >
      <span class="dot" :class="CATEGORY_DOT_CLASS[category]" />
      <span>{{ CATEGORY_LABELS[category] }}</span>
      <span class="count">{{ counts[category] }}</span>
    </button>
  </div>
</template>

<style scoped>
.category-filter {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}
.category-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 28px;
  padding: 0 9px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--surface);
  color: var(--text-muted);
  font-size: 11px;
  font-weight: 600;
  cursor: pointer;
}
.category-chip:hover {
  border-color: var(--border-strong);
  color: var(--text);
}
.category-chip.off {
  opacity: 0.5;
  background: var(--surface-2);
}
.category-chip.off .dot {
  background: var(--border-strong);
}
.category-chip .count {
  color: var(--text-subtle);
  font-family: var(--font-mono);
  font-size: 9.5px;
}
.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
}
.dot-work {
  background: var(--u-ok);
}
.dot-planning {
  background: var(--u-warn);
}
.dot-personal {
  background: #7c5bb7;
}
</style>
