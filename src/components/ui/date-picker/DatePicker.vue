<script setup lang="ts">
import { computed, ref } from "vue";
import { CalendarDate, parseDate } from "@internationalized/date";
import type { DateValue } from "@internationalized/date";
import { CalendarDays, ChevronLeft, ChevronRight } from "lucide-vue-next";
import {
  CalendarCell,
  CalendarCellTrigger,
  CalendarGrid,
  CalendarGridBody,
  CalendarGridHead,
  CalendarGridRow,
  CalendarHeadCell,
  CalendarHeader,
  CalendarHeading,
  CalendarNext,
  CalendarPrev,
  CalendarRoot,
  PopoverContent,
  PopoverPortal,
  PopoverRoot,
  PopoverTrigger,
} from "reka-ui";
import { Button } from "@/components/ui/button";

const props = defineProps<{ modelValue?: string; placeholder?: string }>();
const emit = defineEmits<{ (e: "update:modelValue", value: string): void }>();
const open = ref(false);
const selected = computed(() => props.modelValue ? parseDate(props.modelValue) : undefined);
const placeholder = computed(() => selected.value ?? new CalendarDate(new Date().getFullYear(), new Date().getMonth() + 1, 1));
const label = computed(() => selected.value
  ? new Intl.DateTimeFormat("zh-CN", { year: "numeric", month: "2-digit", day: "2-digit" }).format(new Date(selected.value.year, selected.value.month - 1, selected.value.day))
  : props.placeholder ?? "选择日期");

function select(date: DateValue | undefined): void {
  if (!date) return;
  emit("update:modelValue", date.toString());
  open.value = false;
}
</script>

<template>
  <PopoverRoot v-model:open="open">
    <PopoverTrigger as-child><Button type="button" variant="outline" class="cn-date-picker-trigger"><CalendarDays :size="15" /><span>{{ label }}</span></Button></PopoverTrigger>
    <PopoverPortal><PopoverContent class="cn-date-picker-content" :side-offset="6">
      <CalendarRoot v-slot="{ grid, weekDays }" :model-value="selected" :placeholder="placeholder" locale="zh-CN" :week-starts-on="1" @update:model-value="select">
        <CalendarHeader class="cn-calendar-header"><CalendarPrev class="cn-calendar-nav"><ChevronLeft :size="16" /></CalendarPrev><CalendarHeading class="cn-calendar-heading" /><CalendarNext class="cn-calendar-nav"><ChevronRight :size="16" /></CalendarNext></CalendarHeader>
        <CalendarGrid v-for="month in grid" :key="month.value.toString()" class="cn-calendar-grid"><CalendarGridHead><CalendarGridRow><CalendarHeadCell v-for="day in weekDays" :key="day" class="cn-calendar-weekday">{{ day }}</CalendarHeadCell></CalendarGridRow></CalendarGridHead><CalendarGridBody><CalendarGridRow v-for="(week, index) in month.rows" :key="index"><CalendarCell v-for="date in week" :key="date.toString()" :date="date"><CalendarCellTrigger :day="date" :month="month.value" class="cn-calendar-day" /></CalendarCell></CalendarGridRow></CalendarGridBody></CalendarGrid>
      </CalendarRoot>
    </PopoverContent></PopoverPortal>
  </PopoverRoot>
</template>
