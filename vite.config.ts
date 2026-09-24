import { fileURLToPath, URL } from "node:url";
import { defineConfig, type UserConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import ui from "@nuxt/ui/vite";
import { localConnectorsDevPlugin } from "./vite-local-connectors";

// 在 dev 模式下，浏览器无法直接调用火山方舟控制面 usage API（需 Volc AK/SK 签名，
// 且 ark- 开头的 Bearer Key 只能用于数据面推理，无 usage 端点）。
// 故通过 Vite 中间件 spawn 本机 arkcli 复用其 SSO->签名调用链。
export default defineConfig(({ mode }): UserConfig => ({
  publicDir: mode === "diagnostic" ? false : "public",
  plugins: mode === "diagnostic" ? [] : [
    vue(),
    ui({
      ui: {
        colors: {
          primary: "brand",
          neutral: "slate",
        },
        // 页头去分隔线，并让出顶部自绘标题栏（36px），使内容不与窗口控制按钮重叠。
        dashboardNavbar: {
          slots: {
            root: "border-b-0 h-auto pt-[15px] pb-2",
          },
        },
      },
    }),
    localConnectorsDevPlugin(),
  ],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  server: { port: 1420 },
  build: {
    outDir: mode === "diagnostic" ? "dist-diagnostic" : "dist",
    rollupOptions: {
      // 多页入口：主看板、托盘面板与剪贴板快捷面板。
      input: mode === "diagnostic"
        ? { diagnostic: fileURLToPath(new URL("./diagnostic.html", import.meta.url)) }
        : {
          main: fileURLToPath(new URL("./index.html", import.meta.url)),
          "tray-panel": fileURLToPath(new URL("./tray-panel.html", import.meta.url)),
          "clipboard-panel": fileURLToPath(new URL("./clipboard-panel.html", import.meta.url)),
        },
      output: {
        manualChunks: mode === "diagnostic" ? undefined : {
          echarts: ["echarts", "vue-echarts"],
          vue: ["vue"],
        },
      },
    },
  },
}));
