<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useColorMode } from "@vueuse/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { NavigationMenuItem } from "@nuxt/ui";
import { useSkillsStore } from "@/stores/skills";
import { useTheme } from "@/composables/useTheme";
import { isTauriDesktop } from "@/connectors/types";
import { NAV_GROUPS } from "@/config/navigation";
import AboutDialog from "@/components/AboutDialog.vue";

const isDesktop = isTauriDesktop();
const colorMode = useColorMode();
const { pref: themePref, cycle: cycleTheme } = useTheme();
const aboutOpen = ref(false);

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
  themePref.value === "system" ? "跟随系统" : isDark.value ? "暗色模式" : "亮色模式",
);
const themeIcon = computed(() =>
  themePref.value === "system" ? "i-lucide-monitor-smartphone" : isDark.value ? "i-lucide-moon" : "i-lucide-sun",
);

// 导航由 src/config/navigation.ts 提供，布局只负责渲染。
// 徽章为 0 时不渲染，避免常驻数字。
const links = computed<NavigationMenuItem[]>(() =>
  NAV_GROUPS.flatMap((group) => [
    { type: "label" as const, label: group.title },
    ...group.items.map((item) => {
      const count = item.badge?.() ?? 0;
      return {
        label: item.label,
        icon: item.icon,
        to: item.to,
        badge: count > 0
          ? { label: String(count), color: "error" as const, variant: "subtle" as const }
          : undefined,
      };
    }),
  ]),
);

const store = useSkillsStore();

onMounted(() => {
  if (isDesktop) void store.refresh();
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
      :min-size="15"
      :default-size="17"
      :max-size="22"
      :ui="{
        header: 'h-auto py-3.5',
        body: 'min-h-0 gap-1',
        footer: 'lg:border-t lg:border-default',
      }"
    >
      <template #header="{ collapsed }">
        <div class="flex items-center gap-2.5 w-full min-w-0 px-0.5">
          <span
            class="size-9 shrink-0 rounded-xl bg-gradient-to-br from-primary to-primary/70 text-inverted flex items-center justify-center shadow-sm shadow-primary/30"
          >
            <UIcon name="i-lucide-terminal" class="size-4.5" />
          </span>
          <div v-if="!collapsed" class="min-w-0">
            <p class="truncate text-[15px] font-semibold tracking-tight text-highlighted">XLT Workbench</p>
            <p class="truncate text-[11px] text-dimmed">本地工作台</p>
          </div>
        </div>
      </template>

      <template #default="{ collapsed }">
        <UNavigationMenu
          :collapsed="collapsed"
          :items="links"
          orientation="vertical"
          highlight
          tooltip
          :ui="{
            list: 'flex flex-col gap-1',
            label: 'px-2.5 pt-4 pb-1 text-[11px] font-semibold text-dimmed',
            link: 'py-2',
          }"
        />
      </template>

      <template #footer="{ collapsed }">
        <div
          class="flex w-full items-center gap-1"
          :class="collapsed ? 'flex-col' : 'justify-between'"
        >
          <UButton
            to="/settings"
            icon="i-lucide-settings"
            :label="collapsed ? undefined : '设置'"
            color="neutral"
            variant="ghost"
            size="sm"
            :square="collapsed"
            class="text-muted"
          />
          <UTooltip :text="`主题：${themeLabel}`">
            <UButton
              :icon="themeIcon"
              color="neutral"
              variant="ghost"
              size="sm"
              square
              class="text-muted"
              @click="cycleTheme"
            />
          </UTooltip>
          <UButton
            icon="i-lucide-info"
            :label="collapsed ? undefined : '关于'"
            color="neutral"
            variant="ghost"
            size="sm"
            :square="collapsed"
            class="text-muted"
            @click="aboutOpen = true"
          />
        </div>
      </template>
    </UDashboardSidebar>

    <RouterView />
  </UDashboardGroup>

  <AboutDialog v-model:open="aboutOpen" />
</template>
