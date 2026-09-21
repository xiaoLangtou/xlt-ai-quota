import { computed, reactive } from "vue";

export type ThemePref = "light" | "dark" | "system";
export type ResolvedTheme = "light" | "dark";

const STORAGE_KEY = "xlt-ui-theme";

interface ThemeState {
  pref: ThemePref;
  resolved: ResolvedTheme;
}

function systemPrefersDark(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-color-scheme: dark)").matches
  );
}

function readStoredPref(): ThemePref {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw === "light" || raw === "dark" || raw === "system") return raw;
  } catch {
    /* localStorage 不可用时回退到默认 */
  }
  return "system";
}

function resolve(pref: ThemePref): ResolvedTheme {
  if (pref === "system") return systemPrefersDark() ? "dark" : "light";
  return pref;
}

const state = reactive<ThemeState>({
  pref: "system",
  resolved: "light",
});

let initialized = false;

function apply(resolved: ResolvedTheme): void {
  state.resolved = resolved;
  if (typeof document !== "undefined") {
    document.documentElement.dataset.theme = resolved;
    document.documentElement.style.colorScheme = resolved;
    // 同步 Nuxt UI 的暗色类，让两侧主题（data-theme 与 .dark）保持一致。
    document.documentElement.classList.toggle("dark", resolved === "dark");
  }
}

/** 在应用挂载前调用一次，尽早写入 data-theme，避免首帧闪烁。 */
export function initTheme(): void {
  if (initialized) return;
  initialized = true;
  state.pref = readStoredPref();
  apply(resolve(state.pref));

  if (typeof window !== "undefined" && typeof window.matchMedia === "function") {
    const mql = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = (): void => {
      if (state.pref === "system") apply(resolve("system"));
    };
    // Safari <14 仅支持 addListener
    if (typeof mql.addEventListener === "function") mql.addEventListener("change", onChange);
    else mql.addListener(onChange);
  }
}

function setPref(pref: ThemePref): void {
  state.pref = pref;
  try {
    localStorage.setItem(STORAGE_KEY, pref);
  } catch {
    /* 忽略存储失败 */
  }
  apply(resolve(pref));
}

/** 在 亮 → 暗 → 跟随系统 之间循环切换。 */
function cycle(): void {
  const next: Record<ThemePref, ThemePref> = {
    light: "dark",
    dark: "system",
    system: "light",
  };
  setPref(next[state.pref]);
}

export function useTheme() {
  initTheme();
  return {
    pref: computed(() => state.pref),
    resolved: computed(() => state.resolved),
    isDark: computed(() => state.resolved === "dark"),
    setPref,
    cycle,
  };
}
