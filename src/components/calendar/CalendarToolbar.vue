<script setup lang="ts">
import { computed } from "vue";
import { useCalendarTodo } from "@/composables/useCalendarTodo";
import { VIEW_LABELS, type CalendarViewMode } from "@/types/productivity";
import { dayTitle, monthTitle, weekTitle } from "@/utils/productivity";

const { view, anchor, setView, navigate, goToday } = useCalendarTodo();

const VIEWS: CalendarViewMode[] = ["day", "week", "month"];

const heading = computed(() => {
  if (view.value === "day") return dayTitle(anchor.value);
  if (view.value === "week") return weekTitle(anchor.value);
  return monthTitle(anchor.value);
});
</script>

<template>
  <div class="calendar-toolbar">
    <div class="date-nav">
      <UButton
        icon="i-lucide-chevron-left"
        color="neutral"
        variant="outline"
        square
        size="sm"
        aria-label="上一段"
        @click="navigate(-1)"
      />
      <UButton
        icon="i-lucide-chevron-right"
        color="neutral"
        variant="outline"
        square
        size="sm"
        aria-label="下一段"
        @click="navigate(1)"
      />
      <UButton color="neutral" variant="outline" size="sm" @click="goToday">今天</UButton>
    </div>

    <h2 class="calendar-heading">{{ heading }}</h2>

    <div class="view-switch" role="group" aria-label="日历视图">
      <button
        v-for="item in VIEWS"
        :key="item"
        type="button"
        :class="{ active: view === item }"
        @click="setView(item)"
      >
        {{ VIEW_LABELS[item] }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.calendar-toolbar {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px 12px;
  min-width: 0;
}
.date-nav {
  display: flex;
  align-items: center;
  gap: 6px;
  flex: none;
}
.date-nav :deep(button),
.view-switch button {
  white-space: nowrap;
}
.calendar-heading {
  margin: 0;
  flex: 0 1 auto;
  min-width: 0;
  overflow: hidden;
  color: var(--text);
  font-size: 18px;
  font-weight: 700;
  letter-spacing: -0.02em;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.view-switch {
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 3px;
  border: 1px solid var(--border);
  border-radius: 9px;
  background: var(--surface-3);
  flex: none;
}
.view-switch button {
  min-width: 42px;
  height: 26px;
  padding: 0 10px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--text-muted);
  font-size: 11px;
  font-weight: 600;
  cursor: pointer;
}
.view-switch button.active {
  background: var(--surface);
  color: var(--accent);
  box-shadow: var(--shadow-sm);
}
@media (max-width: 1080px) {
  .calendar-heading {
    font-size: 16px;
  }
}
</style>
