<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useColorMode } from "@vueuse/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useRouter } from "vue-router";
import { useSkillsStore } from "@/stores/skills";
import { useMcpStore } from "@/stores/mcp";
import { useTheme } from "@/composables/useTheme";
import { isTauriDesktop } from "@/connectors/types";
import { NAV_GROUPS } from "@/config/navigation";
import { skillsMcp as skillsMcpEnabled } from "@/config/features.json";
import AboutDialog from "@/components/AboutDialog.vue";

const isDesktop = isTauriDesktop();
const router = useRouter();
const colorMode = useColorMode();
const { pref: themePref, cycle: cycleTheme } = useTheme();
const aboutOpen = ref(false);

// 只迁移侧栏旧宽度一次，保留折叠状态及之后的手动拖拽设置。
if (isDesktop) {
  try {
    const key = "dashboard-sidebar-app";
    const saved = JSON.parse(localStorage.getItem(key) || "null");
    if (saved && typeof saved === "object" && saved.layoutVersion !== 2) {
      localStorage.setItem(key, JSON.stringify({
        ...saved,
        size: typeof saved.size === "number" ? Math.min(14.5, Math.max(13, saved.size)) : 14.5,
        layoutVersion: 2,
      }));
    }
  } catch {
    // 存储不可用时仍使用组件默认尺寸。
  }
}

// 打通 workbench data-theme 与 Nuxt UI color mode，并同步原生窗口外观
// （macOS 透明标题栏 / 关闭按钮的底色随窗口 theme 变化）。
watch(
  themePref,
  (value) => {
    colorMode.value = value === "system" ? "auto" : value;
    if (!isDesktop) return;
    void getCurrentWindow().setTheme(value === "system" ? null : value);
  },
  { immediate: true },
);
const isDark = computed(() => themePref.value === "dark");
const themeLabel = computed(() =>
  themePref.value === "system" ? "跟随系统" : isDark.value ? "深色模式" : "浅色模式",
);
const themeIcon = computed(() =>
  themePref.value === "system" ? "i-lucide-monitor-smartphone" : isDark.value ? "i-lucide-moon" : "i-lucide-sun",
);

// 小屏（< lg）不做抽屉：侧栏常驻并收成图标态。
const isNarrow = ref(false);
const NARROW_QUERY = "(max-width: 1023px)";

/** 图标态 = 用户折叠 或 小屏。 */
function iconOnly(collapsed: boolean): boolean {
  return collapsed || isNarrow.value;
}

/** 导航分组来自 src/config/navigation.ts，侧栏只负责渲染。 */
const groups = computed(() =>
  NAV_GROUPS.map((group) => ({
    title: group.title,
    items: group.items.map((item) => ({
      ...item,
      badge: item.badge?.() ?? 0,
    })),
  })),
);

// 原型快捷键：⌘1–⌘7 跳页面、⌘, 进设置。
defineShortcuts({
  meta_1: () => router.push("/dashboard"),
  meta_2: () => router.push("/analytics"),
  meta_3: () => router.push("/subscriptions"),
  meta_4: () => router.push("/daily-report"),
  meta_5: () => router.push("/clipboard"),
  meta_6: () => router.push("/snippets"),
  meta_7: () => router.push("/vault"),
  "meta_,": () => router.push("/settings"),
});

let narrowQuery: MediaQueryList | undefined;

function syncNarrow(event?: MediaQueryListEvent): void {
  isNarrow.value = event ? event.matches : narrowQuery?.matches ?? false;
}

onMounted(() => {
  if (typeof window !== "undefined" && typeof window.matchMedia === "function") {
    narrowQuery = window.matchMedia(NARROW_QUERY);
    syncNarrow();
    narrowQuery.addEventListener("change", syncNarrow);
  }

  // 排查期间连 store 初始化也跳过，避免隐藏入口后仍进行后台扫描。
  if (!isDesktop || !skillsMcpEnabled) return;
  void useSkillsStore().refresh();
  void useMcpStore().refresh();
});

onUnmounted(() => {
  narrowQuery?.removeEventListener("change", syncNarrow);
});
</script>

<template>
  <div v-if="!isDesktop" class="flex min-h-screen items-center justify-center p-8">
    <UAlert
      class="max-w-md"
      color="warning"
      variant="subtle"
      icon="i-lucide-monitor"
      title="请在桌面应用中打开"
      description="本应用依赖本机 CLI、文件系统与原生能力，请在 Tauri 桌面应用中运行。"
    />
  </div>

  <UDashboardGroup v-else unit="rem" storage="local">
    <UDashboardSidebar
      id="app"
      collapsible
      resizable
      :toggle="false"
      :min-size="13"
      :default-size="14.5"
      :max-size="18"
      :ui="{
        root: 'min-w-20 flex max-lg:w-20 max-lg:min-w-20',
        header: 'h-auto px-3 pt-9 pb-3 max-lg:px-2',
        body: 'min-h-0 px-3 max-lg:px-2',
        footer: 'px-3 py-3 max-lg:px-2',
        content: 'hidden',
        overlay: 'hidden',
      }"
    >
      <template #header="{ collapsed }">
        <div class="brand" :class="{ collapsed: iconOnly(collapsed) }" data-tauri-drag-region>
          <UIcon name="i-lucide-terminal" class="brand-mark" />
          <div v-if="!iconOnly(collapsed)" class="brand-text">
            <p class="brand-name">XLT Workbench</p>
            <p class="brand-sub">
              <span class="brand-dot" />
              {{ skillsMcpEnabled ? "本地工作台" : "Skills / MCP 已停用" }}
            </p>
          </div>
        </div>
      </template>

      <template #default="{ collapsed }">
        <nav class="nav">
          <div v-for="group in groups" :key="group.title" class="nav-group">
            <p v-if="!iconOnly(collapsed)" class="nav-group-label">{{ group.title }}</p>
            <RouterLink
              v-for="item in group.items"
              :key="item.to"
              :to="item.to"
              class="nav-item"
              :class="{ collapsed: iconOnly(collapsed) }"
              active-class="active"
              :title="iconOnly(collapsed) ? item.label : undefined"
            >
              <UIcon :name="item.icon" class="nav-icon" />
              <template v-if="!iconOnly(collapsed)">
                <span class="nav-label">{{ item.label }}</span>
                <span v-if="item.badge > 0" class="nav-badge">{{ item.badge }}</span>
                <span v-else-if="item.kbd" class="nav-kbd">{{ item.kbd }}</span>
              </template>
            </RouterLink>
          </div>
        </nav>
      </template>

      <template #footer="{ collapsed }">
        <div class="sidebar-foot">
          <RouterLink
            to="/settings"
            class="nav-item"
            :class="{ collapsed: iconOnly(collapsed) }"
            active-class="active"
            :title="iconOnly(collapsed) ? '设置' : undefined"
          >
            <UIcon name="i-lucide-settings" class="nav-icon" />
            <template v-if="!iconOnly(collapsed)">
              <span class="nav-label">设置</span>
              <span class="nav-kbd">⌘,</span>
            </template>
          </RouterLink>

          <button
            type="button"
            class="theme-toggle"
            :class="{ collapsed: iconOnly(collapsed) }"
            :title="`主题：${themeLabel}`"
            @click="cycleTheme"
          >
            <UIcon :name="themeIcon" class="nav-icon" />
            <template v-if="!iconOnly(collapsed)">
              <span class="nav-label">{{ themeLabel }}</span>
              <span class="theme-pill" :class="{ on: !isDark }">
                <span class="theme-knob" />
              </span>
            </template>
          </button>

          <button
            v-if="!iconOnly(collapsed)"
            type="button"
            class="about-link"
            @click="aboutOpen = true"
          >
            <UIcon name="i-lucide-info" class="nav-icon" />
            <span class="nav-label">关于</span>
          </button>
        </div>
      </template>
    </UDashboardSidebar>

    <RouterView />
  </UDashboardGroup>

  <!-- Overlay 标题栏：系统条覆盖在内容之上，这条提供窗口拖拽与双击最大化。 -->
  <div v-if="isDesktop" class="window-drag" data-tauri-drag-region="deep" />

  <AboutDialog v-model:open="aboutOpen" />
</template>

<style scoped>
/* ===== Overlay 标题栏拖拽区（无按钮，系统交通灯在上一层） ===== */
.window-drag {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  z-index: 50;
  height: 15px;
}

/* ===== 品牌区 ===== */
.brand {
  display: flex;
  align-items: center;
  gap: 11px;
  width: 100%;
  min-width: 0;
  padding: 0 2px;
}
.brand.collapsed {
  justify-content: center;
  padding: 0;
}
.brand-mark {
  flex-shrink: 0;
  width: 24px;
  height: 24px;
  color: var(--accent);
}
.brand-text {
  min-width: 0;
}
.brand-name {
  margin: 0;
  overflow: hidden;
  color: var(--text);
  font-size: 14.5px;
  font-weight: 600;
  letter-spacing: -0.01em;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.brand-sub {
  display: flex;
  align-items: center;
  gap: 5px;
  margin: 1px 0 0;
  overflow: hidden;
  color: var(--text-subtle);
  font-size: 11px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.brand-dot {
  width: 5px;
  height: 5px;
  flex-shrink: 0;
  border-radius: 50%;
  background: var(--accent);
}

/* ===== 导航 ===== */
.nav {
  display: flex;
  flex-direction: column;
  width: 100%;
}
.nav-group {
  margin-top: 16px;
}
.nav-group:first-child {
  margin-top: 4px;
}
.nav-group-label {
  margin: 0;
  padding: 0 10px 6px;
  color: var(--text-subtle);
  font-size: 10.5px;
  font-weight: 600;
  letter-spacing: 0.09em;
  text-transform: uppercase;
}
.nav-item {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  margin-bottom: 1px;
  padding: 7.5px 10px;
  border-radius: 8px;
  color: var(--text-muted);
  font-size: 13.5px;
  font-weight: 450;
  text-decoration: none;
  cursor: pointer;
  user-select: none;
  transition: background-color 0.18s ease, color 0.18s ease;
}
.nav-item.collapsed {
  justify-content: center;
  padding: 8px;
}
.nav-icon {
  flex-shrink: 0;
  width: 16px;
  height: 16px;
  opacity: 0.8;
  transition: opacity 0.18s ease;
}
.nav-item:hover {
  background: var(--surface-2);
  color: var(--text);
}
.nav-item:hover .nav-icon {
  opacity: 1;
}
.nav-item.active {
  background: var(--accent-weak);
  color: var(--accent);
  font-weight: 550;
}
.nav-item.active .nav-icon {
  opacity: 1;
}
.nav-label {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.nav-kbd {
  font-family: var(--font-mono);
  font-size: 10.5px;
  color: var(--text-subtle);
}
.nav-badge {
  min-width: 18px;
  padding: 0 6px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--u-crit) 16%, transparent);
  color: var(--u-crit);
  font-family: var(--font-mono);
  font-size: 10.5px;
  font-weight: 650;
  line-height: 16px;
  text-align: center;
}

/* ===== 底部 ===== */
.sidebar-foot {
  display: flex;
  flex-direction: column;
  width: 100%;
}
.theme-toggle {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  margin-top: 4px;
  padding: 7.5px 10px;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--text-muted);
  font: inherit;
  font-size: 13px;
  text-align: left;
  cursor: pointer;
  transition: background-color 0.18s ease, color 0.18s ease;
}
.theme-toggle.collapsed {
  justify-content: center;
  padding: 8px;
}
.theme-toggle:hover {
  background: var(--surface-2);
  color: var(--text);
}
.theme-pill {
  position: relative;
  width: 34px;
  height: 19px;
  margin-left: auto;
  border-radius: 10px;
  background: var(--surface-3);
  transition: background-color 0.25s ease;
}
.theme-knob {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 15px;
  height: 15px;
  border-radius: 50%;
  background: var(--text-subtle);
  transition: transform 0.3s cubic-bezier(0.16, 1, 0.3, 1), background-color 0.25s ease;
}
.theme-pill.on {
  background: var(--accent-weak);
}
.theme-pill.on .theme-knob {
  background: var(--accent);
  transform: translateX(15px);
}
.about-link {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  margin-top: 1px;
  padding: 7.5px 10px;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--text-muted);
  font: inherit;
  font-size: 13px;
  text-align: left;
  cursor: pointer;
  transition: background-color 0.18s ease, color 0.18s ease;
}
.about-link:hover {
  background: var(--surface-2);
  color: var(--text);
}
</style>
