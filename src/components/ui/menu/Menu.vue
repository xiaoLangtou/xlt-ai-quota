<script setup lang="ts">
import {
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuPortal,
  DropdownMenuRoot,
  DropdownMenuTrigger,
} from "reka-ui";

type MenuItem = { label: string; value: string; danger?: boolean };
defineProps<{ items: MenuItem[]; ariaLabel?: string }>();
const emit = defineEmits<{ (e: "select", value: string): void }>();
</script>

<template>
  <DropdownMenuRoot>
    <DropdownMenuTrigger class="cn-menu-trigger" :aria-label="ariaLabel ?? '更多操作'">
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <circle cx="12" cy="5" r="1.6" />
        <circle cx="12" cy="12" r="1.6" />
        <circle cx="12" cy="19" r="1.6" />
      </svg>
    </DropdownMenuTrigger>
    <DropdownMenuPortal>
      <DropdownMenuContent class="cn-menu-content" align="end" :side-offset="4">
        <DropdownMenuItem
          v-for="item in items"
          :key="item.value"
          class="cn-menu-item"
          :class="{ danger: item.danger }"
          @select="emit('select', item.value)"
        >
          {{ item.label }}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenuPortal>
  </DropdownMenuRoot>
</template>
