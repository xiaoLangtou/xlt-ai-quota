<script setup lang="ts">
import WorkspacePage from "@/layouts/WorkspacePage.vue";
import ShellEnvView from "@/views/ShellEnvView.vue";
</script>

<template>
  <WorkspacePage
    id="shell-env"
    title="Shell 环境"
    description="别名与环境变量管理"
    :view="ShellEnvView"
  />
</template>
