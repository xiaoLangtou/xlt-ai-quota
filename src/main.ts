import { createApp } from "vue";
import App from "./App.vue";
import { initTheme } from "./composables/useTheme";
import "./style.css";

// 挂载前先写入 data-theme，避免首帧主题闪烁。
initTheme();

createApp(App).mount("#app");
