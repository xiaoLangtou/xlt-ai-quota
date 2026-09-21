<script setup lang="ts">
import WorkspacePage from "@/layouts/WorkspacePage.vue";
import VaultView from "@/views/VaultView.vue";
</script>

<template>
  <WorkspacePage id="vault" title="密钥库" description="效率工具" :view="VaultView" />
</template>
