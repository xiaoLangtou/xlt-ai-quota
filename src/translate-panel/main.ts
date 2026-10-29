import { createApp } from "vue";
import TranslateQuickPanel from "./TranslateQuickPanel.vue";
import { initTheme } from "@/composables/useTheme";
import "@/style.css";

initTheme();
createApp(TranslateQuickPanel).mount("#app");
