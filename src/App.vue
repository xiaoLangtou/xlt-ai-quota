<script setup lang="ts">
import { onMounted, onUnmounted } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useRouter } from "vue-router";
import ToastHost from "@/components/ToastHost.vue";
import AppLayout from "@/layouts/AppLayout.vue";
import { useTranslateRequest } from "@/composables/useTranslateRequest";
import { isTauriDesktop } from "@/connectors/types";
import { clipboardService } from "@/services/clipboard-service";

const router = useRouter();
const { request } = useTranslateRequest();
const unlisteners: UnlistenFn[] = [];

onMounted(() => {
  if (!isTauriDesktop()) return;
  // 应用打开时：辅助功能权限未开启则弹出 macOS 系统授权对话框。
  void (async () => {
    try {
      const status = await clipboardService.status();
      if (!status.accessibilityGranted) await clipboardService.requestAccessibility();
    } catch {
      // 权限检查失败不影响看板主流程
    }
  })();

  // 快捷面板是独立窗口，用事件把文本交接给主窗口并跳转到翻译页。
  void listen<{ text: string; from: string; to: string }>("translate:open", async (event) => {
    request(event.payload);
    if (router.currentRoute.value.name !== "translate") await router.push("/translate");
  }).then((stop) => unlisteners.push(stop));

  // 面板未配置凭据时直接跳到设置页「翻译」分类。
  void listen("translate:settings", async () => {
    await router.push({ path: "/settings", query: { tab: "ai" } });
  }).then((stop) => unlisteners.push(stop));
});

onUnmounted(() => {
  for (const stop of unlisteners) stop();
});
</script>

<template>
  <UApp>
    <AppLayout />
  </UApp>
  <ToastHost />
</template>
