import { fileURLToPath, URL } from "node:url";
import { defineConfig } from "vite";
import tailwindcss from "@tailwindcss/vite";
import vue from "@vitejs/plugin-vue";
import { localConnectorsDevPlugin } from "./vite-local-connectors";

// 在 dev 模式下，浏览器无法直接调用火山方舟控制面 usage API（需 Volc AK/SK 签名，
// 且 ark- 开头的 Bearer Key 只能用于数据面推理，无 usage 端点）。
// 故通过 Vite 中间件 spawn 本机 arkcli 复用其 SSO->签名调用链。
export default defineConfig({
  plugins: [vue(), tailwindcss(), localConnectorsDevPlugin()],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  server: { port: 1420 },
  build: {
    rollupOptions: {
      // 多页入口：主看板、托盘面板与剪贴板快捷面板。
      input: {
        main: fileURLToPath(new URL("./index.html", import.meta.url)),
        "tray-panel": fileURLToPath(new URL("./tray-panel.html", import.meta.url)),
        "clipboard-panel": fileURLToPath(new URL("./clipboard-panel.html", import.meta.url)),
      },
      output: {
        manualChunks: {
          echarts: ["echarts", "vue-echarts"],
          vue: ["vue"],
        },
      },
    },
  },
});
