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
      output: {
        manualChunks: {
          echarts: ["echarts", "vue-echarts"],
          vue: ["vue"],
        },
      },
    },
  },
});
