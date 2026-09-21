<script setup lang="ts">
import WorkspacePage from "@/layouts/WorkspacePage.vue";
import DailyReportView from "@/views/DailyReportView.vue";
</script>

<template>
  <WorkspacePage id="daily-report" title="Git 报告" description="效率工具" :view="DailyReportView" />
</template>
