<script setup lang="ts">
import WorkspacePage from "@/layouts/WorkspacePage.vue";
import TranslateView from "@/views/TranslateView.vue";
</script>

<template>
  <WorkspacePage id="translate" title="翻译" description="文章翻译与快捷面板" :view="TranslateView" />
</template>
