<script setup lang="ts">
import WorkspacePage from "@/layouts/WorkspacePage.vue";
import ClipboardHistoryView from "@/views/ClipboardHistoryView.vue";
</script>

<template>
  <WorkspacePage
    id="clipboard"
    title="剪贴板历史"
    description="效率工具"
    variant="clipboard"
    :view="ClipboardHistoryView"
  />
</template>
