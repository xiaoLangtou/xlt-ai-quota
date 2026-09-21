<script setup lang="ts">
withDefaults(
  defineProps<{
    /** 高度（px 或任意 CSS 长度） */
    height?: string;
    /** 圆角 */
    radius?: string;
    /** 宽度 */
    width?: string;
  }>(),
  { height: "16px", radius: "8px", width: "100%" },
);
</script>

<template>
  <USkeleton class="block" :style="{ height, width, borderRadius: radius }" aria-hidden="true" />
</template>
