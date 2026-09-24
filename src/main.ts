import { createApp } from "vue";
import { createPinia } from "pinia";
import ui from "@nuxt/ui/vue-plugin";
import App from "./App.vue";
import { createAppRouter } from "./router";
import { initTheme } from "./composables/useTheme";
import "./style.css";

// 挂载前先写入 data-theme，避免首帧主题闪烁。
initTheme();

createApp(App)
  .use(createPinia())
  .use(createAppRouter())
  .use(ui)
  .mount("#app");
