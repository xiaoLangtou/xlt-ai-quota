<script setup lang="ts">
import type { Component } from "vue";

/**
 * 工具页统一外壳：Nuxt UI 面板 + 页头 + 独立滚动区。
 * 工具页路由直接指向各自的 View，由此外壳补齐页面骨架，
 * 避免所有工具挤在 DashboardView 里按 route.name 分派。
 */
withDefaults(
  defineProps<{
    /** UDashboardPanel id，用于布局持久化 */
    id: string;
    title: string;
    description?: string;
    view: Component;
    /** 是否渲染统一页头（页面自带标题时关闭） */
    nav?: boolean;
    /** 滚动区变体 */
    variant?: "default" | "snippet" | "clipboard";
  }>(),
  { nav: true, variant: "default" },
);

const VARIANT_CLASS: Record<string, string> = {
  default: "",
  snippet: "snippet-workspace",
  clipboard: "clipboard-workspace-shell",
};
</script>

<template>
  <UDashboardPanel :id="id">
    <template #header>
      <UDashboardNavbar v-if="nav" :title="title" :description="description">
        <template #leading>
          <UDashboardSidebarCollapse />
        </template>
        <template #right>
          <slot name="actions" />
        </template>
      </UDashboardNavbar>
    </template>

    <template #body>
      <div class="workspace-main" :class="VARIANT_CLASS[variant]">
        <div class="page-scroll">
          <component :is="view" />
        </div>
      </div>
    </template>
  </UDashboardPanel>
</template>

<style scoped>
.workspace-main {
  display: flex;
  min-width: 0;
  min-height: 0;
  flex: 1;
  flex-direction: column;
  gap: 12px;
}
.page-scroll {
  min-height: 0;
  flex: 1;
  overflow-y: auto;
  padding: 0 4px 10px 0;
  -ms-overflow-style: none;
  scrollbar-width: none;
}
.page-scroll::-webkit-scrollbar {
  display: none;
}
.snippet-workspace .page-scroll {
  padding: 0;
}
.clipboard-workspace-shell .page-scroll {
  overflow: hidden;
}
</style>
