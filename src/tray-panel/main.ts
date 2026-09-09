import { createApp } from "vue";
import TrayPanel from "./TrayPanel.vue";
import { initTheme } from "@/composables/useTheme";
import "@/style.css";

// 托盘面板与主窗口同源，共享 localStorage 主题；挂载前写入 data-theme 避免首帧闪烁。
initTheme();

createApp(TrayPanel).mount("#app");
