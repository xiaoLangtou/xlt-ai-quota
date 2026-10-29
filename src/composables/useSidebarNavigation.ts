import { computed, readonly } from "vue";
import { createGlobalState, useLocalStorage } from "@vueuse/core";
import { NAV_GROUPS } from "@/config/navigation";

/** 仅控制导航入口的可见性，路由与业务数据保持独立。 */
export const useSidebarNavigation = createGlobalState(() => {
  const hiddenPaths = useLocalStorage<string[]>("xlt.sidebar.hidden-menus", []);
  const visibleGroups = computed(() => NAV_GROUPS
    .map((group) => ({
      ...group,
      items: group.items.filter((item) => !hiddenPaths.value.includes(item.to)),
    }))
    .filter((group) => group.items.length > 0));

  function setMenuVisible(path: string, visible: boolean): void {
    if (!NAV_GROUPS.some((group) => group.items.some((item) => item.to === path))) return;
    const paths = hiddenPaths.value.filter((item) => item !== path);
    hiddenPaths.value = visible ? paths : [...paths, path];
  }

  function showAllMenus(): void {
    hiddenPaths.value = [];
  }

  return { hiddenPaths: readonly(hiddenPaths), visibleGroups, setMenuVisible, showAllMenus };
});
