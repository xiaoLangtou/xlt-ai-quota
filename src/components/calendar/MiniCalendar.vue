<script setup lang="ts">
import { computed } from "vue";
import { CalendarDate, parseDate } from "@internationalized/date";
import { useCalendarTodo } from "@/composables/useCalendarTodo";

const { anchor, setAnchor } = useCalendarTodo();

const model = computed<CalendarDate>({
  get: () => parseDate(anchor.value),
  set: (value) => {
    if (value) setAnchor(value.toString());
  },
});
</script>

<template>
  <UCalendar v-model="model" :week-starts-on="1" :year-controls="false" size="xs" fixed-weeks />
</template>
