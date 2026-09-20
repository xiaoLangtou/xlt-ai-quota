import { createApp } from "vue";
import ClipboardQuickPanel from "./ClipboardQuickPanel.vue";
import { initTheme } from "@/composables/useTheme";
import "@/style.css";

initTheme();
createApp(ClipboardQuickPanel).mount("#app");
