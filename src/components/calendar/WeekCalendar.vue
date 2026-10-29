<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import TimeGrid from "@/components/calendar/TimeGrid.vue";
import { useCalendarTodo } from "@/composables/useCalendarTodo";
import type { CalendarEvent } from "@/types/productivity";
import { threeDayDates, weekDates } from "@/utils/productivity";

const emit = defineEmits<{
  (event: "select-event", payload: CalendarEvent): void;
  (event: "move-event", payload: { id: string; startAt: string; endAt?: string }): void;
  (event: "create-event", payload: { date: string; time: string; endTime?: string }): void;
  (event: "quick-create", payload: { date: string; time: string; x: number; y: number }): void;
  (event: "open-day", payload: string): void;
}>();

const { anchor, visibleEvents } = useCalendarTodo();
const root = ref<HTMLElement | null>(null);
const compact = ref(false);
let observer: ResizeObserver | undefined;

// 内容区小于 820px 时改用三日窗口，避免七列被压缩到不可读。
function measure(): void {
  if (root.value) compact.value = root.value.clientWidth < 820;
}

onMounted(() => {
  measure();
  if (typeof ResizeObserver !== "undefined") {
    observer = new ResizeObserver(measure);
    if (root.value) observer.observe(root.value);
  }
});

onBeforeUnmount(() => observer?.disconnect());

const dates = computed(() => (compact.value ? threeDayDates(anchor.value) : weekDates(anchor.value)));
</script>

<template>
  <div ref="root" class="week-calendar">
    <TimeGrid
      :dates="dates"
      :events="visibleEvents"
      @select-event="emit('select-event', $event)"
      @move-event="emit('move-event', $event)"
      @create-event="emit('create-event', $event)"
      @quick-create="emit('quick-create', $event)"
      @open-day="emit('open-day', $event)"
    />
  </div>
</template>

<style scoped>
.week-calendar {
  min-width: 0;
  min-height: 0;
  height: 100%;
  display: flex;
}
.week-calendar > * {
  flex: 1;
}
</style>
