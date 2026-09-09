<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { CalendarDays, ChevronLeft, ChevronRight } from "lucide-vue-next";
import { PopoverContent, PopoverPortal, PopoverRoot, PopoverTrigger } from "reka-ui";
import { Button } from "@/components/ui/button";

const props = defineProps<{ modelValue: string; placeholder?: string }>();
const emit = defineEmits<{ (e: "update:modelValue", value: string): void }>();

const open = ref(false);
const modelYear = computed(() => Number(props.modelValue.slice(0, 4)) || new Date().getFullYear());
const modelMonth = computed(() => Number(props.modelValue.slice(5, 7)) || 1);
const displayedYear = ref(modelYear.value);
const monthNames = ["1 月", "2 月", "3 月", "4 月", "5 月", "6 月", "7 月", "8 月", "9 月", "10 月", "11 月", "12 月"];
const label = computed(() => props.modelValue
  ? `${modelYear.value} 年 ${modelMonth.value} 月`
  : props.placeholder ?? "选择月份");

function selectMonth(month: number): void {
  emit("update:modelValue", `${displayedYear.value}-${String(month).padStart(2, "0")}`);
  open.value = false;
}

watch(open, (isOpen) => {
  if (isOpen) displayedYear.value = modelYear.value;
});
</script>

<template>
  <PopoverRoot v-model:open="open">
    <PopoverTrigger as-child>
      <Button type="button" variant="outline" class="cn-date-picker-trigger cn-month-picker-trigger">
        <CalendarDays :size="15" />
        <span>{{ label }}</span>
      </Button>
    </PopoverTrigger>
    <PopoverPortal>
      <PopoverContent class="cn-date-picker-content cn-month-picker-content" :side-offset="6">
        <div class="cn-calendar-header">
          <button type="button" class="cn-calendar-nav" aria-label="上一年" @click="displayedYear -= 1"><ChevronLeft :size="16" /></button>
          <strong class="cn-calendar-heading">{{ displayedYear }} 年</strong>
          <button type="button" class="cn-calendar-nav" aria-label="下一年" @click="displayedYear += 1"><ChevronRight :size="16" /></button>
        </div>
        <div class="cn-month-picker-grid">
          <button
            v-for="(name, index) in monthNames"
            :key="name"
            type="button"
            class="cn-month-picker-item"
            :data-selected="displayedYear === modelYear && index + 1 === modelMonth ? '' : undefined"
            @click="selectMonth(index + 1)"
          >{{ name }}</button>
        </div>
      </PopoverContent>
    </PopoverPortal>
  </PopoverRoot>
</template>
