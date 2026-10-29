<script setup lang="ts">
import { computed } from "vue";
import { Boxes } from "lucide-vue-next";
import { resolveToolLogo } from "@/utils/tool-logos";

const props = withDefaults(
  defineProps<{
    platform: string;
    size?: number;
  }>(),
  { size: 18 },
);

const logo = computed(() => resolveToolLogo(props.platform));
</script>

<template>
  <img
    v-if="logo"
    class="tool-logo"
    :class="{ monochrome: logo.monochrome }"
    :src="logo.src"
    :width="size"
    :height="size"
    :style="{ width: `${size}px`, height: `${size}px` }"
    alt=""
    aria-hidden="true"
  />
  <Boxes
    v-else
    class="tool-logo default-logo"
    :size="size"
    :stroke-width="1.8"
    aria-hidden="true"
  />
</template>

<style scoped>
.tool-logo {
  display: block;
  max-width: none;
  flex: 0 0 auto;
  object-fit: contain;
}

.default-logo {
  color: var(--text-muted);
}
</style>
