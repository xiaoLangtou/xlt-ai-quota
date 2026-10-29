import { fileURLToPath, URL } from "node:url";
import { defineConfig, type UserConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import ui from "@nuxt/ui/vite";

// 所有本地连接器统一走 Tauri command（src-tauri/src/connectors.rs）；
// 浏览器开发模式不再内置中间件，用量页仅展示已有本地数据。
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
        // 页头不透明，滚动内容在其下方齐边裁切；渐隐由 .is-scrolled 的遮罩处理。
        dashboardNavbar: {
          slots: {
            root: "relative z-10 h-auto pt-[15px] pb-3 bg-muted",
          },
        },
        // 右侧内容区做成圆角内嵌框（原型 frame）：外边距 + 描边 + 圆角，框内独立滚动。
        dashboardPanel: {
          slots: {
            root: "relative flex flex-col min-w-0 min-h-0 flex-1 my-3 me-3 ms-0 rounded-[10px] border border-default bg-muted overflow-hidden",
            body: "flex flex-col gap-4 sm:gap-6 flex-1 overflow-y-auto px-4 sm:px-6 pt-2 sm:pt-2 pb-6 sm:pb-6",
          },
        },
      },
    }),
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
          "translate-panel": fileURLToPath(new URL("./translate-panel.html", import.meta.url)),
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
