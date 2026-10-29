<script setup lang="ts">
import TimeGrid from "@/components/calendar/TimeGrid.vue";
import { useCalendarTodo } from "@/composables/useCalendarTodo";
import type { CalendarEvent } from "@/types/productivity";

const emit = defineEmits<{
  (event: "select-event", payload: CalendarEvent): void;
  (event: "move-event", payload: { id: string; startAt: string; endAt?: string }): void;
  (event: "create-event", payload: { date: string; time: string; endTime?: string }): void;
  (event: "quick-create", payload: { date: string; time: string; x: number; y: number }): void;
  (event: "open-day", payload: string): void;
}>();

const { anchor, visibleEvents } = useCalendarTodo();
</script>

<template>
  <div class="day-calendar">
    <TimeGrid
      :dates="[anchor]"
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
.day-calendar {
  min-width: 0;
  min-height: 0;
  height: 100%;
  display: flex;
}
.day-calendar > * {
  flex: 1;
}
</style>
