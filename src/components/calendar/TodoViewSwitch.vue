<script setup lang="ts">
import { useCalendarTodo, type TodoViewMode } from "@/composables/useCalendarTodo";

const { todoView, setTodoView } = useCalendarTodo();

const items: { value: TodoViewMode; label: string; icon: string }[] = [
  { value: "list", label: "列表", icon: "i-lucide-list" },
  { value: "board", label: "看板", icon: "i-lucide-kanban" },
];
</script>

<template>
  <div class="flex shrink-0 items-center gap-0.5 whitespace-nowrap rounded-lg bg-elevated p-0.5">
    <UButton
      v-for="item in items"
      :key="item.value"
      size="xs"
      :icon="item.icon"
      :color="todoView === item.value ? 'primary' : 'neutral'"
      :variant="todoView === item.value ? 'soft' : 'ghost'"
      @click="setTodoView(item.value)"
    >
      {{ item.label }}
    </UButton>
  </div>
</template>
