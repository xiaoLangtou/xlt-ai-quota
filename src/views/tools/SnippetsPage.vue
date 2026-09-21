<script setup lang="ts">
import WorkspacePage from "@/layouts/WorkspacePage.vue";
import SnippetLibraryView from "@/views/SnippetLibraryView.vue";
</script>

<template>
  <WorkspacePage
    id="snippets"
    title="代码片段"
    description="效率工具"
    variant="snippet"
    :nav="false"
    :view="SnippetLibraryView"
  />
</template>
