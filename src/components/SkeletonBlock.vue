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
  <span class="skeleton" :style="{ height, width, borderRadius: radius }" aria-hidden="true" />
</template>

<style scoped>
.skeleton {
  display: block;
  background: linear-gradient(
    100deg,
    var(--surface-3) 30%,
    color-mix(in srgb, var(--surface-3) 55%, var(--surface)) 50%,
    var(--surface-3) 70%
  );
  background-size: 200% 100%;
  animation: skeleton-shimmer 1.3s ease-in-out infinite;
}
@keyframes skeleton-shimmer {
  0% {
    background-position: 200% 0;
  }
  100% {
    background-position: -200% 0;
  }
}
@media (prefers-reduced-motion: reduce) {
  .skeleton {
    animation: none;
  }
}
</style>
