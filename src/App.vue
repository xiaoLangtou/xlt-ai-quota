<script setup lang="ts">
import { onMounted } from "vue";
import ToastHost from "@/components/ToastHost.vue";
import AppLayout from "@/layouts/AppLayout.vue";
import { isTauriDesktop } from "@/connectors/types";
import { clipboardService } from "@/services/clipboard-service";

// 应用打开时：辅助功能权限未开启则弹出 macOS 系统授权对话框。
onMounted(() => {
  if (!isTauriDesktop()) return;
  void (async () => {
    try {
      const status = await clipboardService.status();
      if (!status.accessibilityGranted) await clipboardService.requestAccessibility();
    } catch {
      // 权限检查失败不影响看板主流程
    }
  })();
});
</script>

<template>
  <UApp>
    <AppLayout />
  </UApp>
  <ToastHost />
</template>
