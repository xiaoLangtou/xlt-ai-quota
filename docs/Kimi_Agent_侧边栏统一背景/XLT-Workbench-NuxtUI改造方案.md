# XLT Workbench 界面改造方案（Nuxt UI v3）

> 目标：将已确认的 HTML 原型（版本 `95e590a`）落地到 Tauri 项目的 Nuxt UI v3 前端。
> 范围：**纯 UI 改造**，不动数据层；页面继续消费现有 mock / 接口数据。
> 方式：以 Nuxt UI 组件为骨架，设计细节通过设计令牌（CSS 变量）+ `app.config.ts` 组件主题覆盖实现。

---

## 1. 总体策略

三层定制，优先级从高到低，**能用上层解决就不碰下层**：

| 层级 | 位置 | 负责什么 |
|---|---|---|
| ① 设计令牌 | `app/assets/css/main.css` | 颜色、背景层级、文字、边框、圆角、字体 —— 原型 90% 的质感来自这里 |
| ② 组件主题 | `app/app.config.ts` | 全局调整 Nuxt UI 组件的 slots / variants（按钮圆角、表格密度、卡片边框等） |
| ③ 定制组件 | `app/components/` | Nuxt UI 没有、原型里手写的部分：统计卡、SVG 图表、额度条、噪点层等 |

**核心原则**：不要给 Nuxt UI 组件写覆盖类去"硬改"内部样式，优先改它暴露的 CSS 变量和 slots 配置，这样升级组件库时不会碎。

---

## 2. 前置检查（开工前确认）

| 项 | 要求 | 说明 |
|---|---|---|
| 渲染模式 | `ssr: false` | Tauri 桌面应用按 SPA 跑 |
| 路由 history | hash 模式 | Tauri 的 `tauri://` / `file://` 协议下 history 模式刷新会 404；在 `app/router.options.ts` 用 `createWebHashHistory` |
| CSP | 允许 `img-src data:` | 噪点层是内联 SVG data URI |
| color-mode | 随 `@nuxt/ui` 自动注册，无需另装 | 配置 `preference: 'dark'`、`storageKey: 'xlt-theme'` |
| 图标 | `@nuxt/icon`（lucide） | 原型手绘图标映射到 `i-lucide-*`，见 §6 |

---

## 3. 设计令牌映射（原型 → Nuxt UI v3）

### 3.1 背景层级映射

原型有 4 层背景，映射到 Nuxt UI 的背景变量：

| 原型变量 | 含义 | Nuxt UI 变量 | 深色值 | 浅色值 |
|---|---|---|---|---|
| `--bg` | 窗口外底（侧栏融入其中） | `--ui-bg` | `#000000` | `#f3f3f3` |
| `--frame` | 内容框（圆角内嵌面板） | `--ui-bg-muted` | `#0e0e0e` | `#ffffff` |
| `--raise` | 卡片层 | `--ui-bg-elevated` | `#161616` | `#fbfbfb` |
| `--sunken` | 下沉层（表头/输入框） | `--ui-bg-accented` | `#0c0c0c` | `#f2f2f2` |

### 3.2 品牌绿色阶

以原型主色 `#4F9D69` 为 500 锚点生成完整色阶，浅色主题用 600、深色主题用 400（与原型的 `#3d8156` / `#63c183` 一致）：

```
50:#f0f7f2  100:#dcefe3  200:#badfc8  300:#8fc7a6  400:#63c183
500:#4f9d69  600:#3d8156  700:#336847  800:#2b533b  900:#244532  950:#12251b
```

### 3.3 `app/assets/css/main.css`（可直接粘贴）

```css
@import "tailwindcss";
@import "@nuxt/ui";

@theme static {
  --font-sans: -apple-system, BlinkMacSystemFont, "SF Pro Text",
    "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", "Segoe UI", sans-serif;
  --font-mono: ui-monospace, "SF Mono", "JetBrains Mono", Menlo,
    Consolas, "PingFang SC", monospace;

  /* 品牌绿 */
  --color-brand-50:  #f0f7f2;
  --color-brand-100: #dcefe3;
  --color-brand-200: #badfc8;
  --color-brand-300: #8fc7a6;
  --color-brand-400: #63c183;
  --color-brand-500: #4f9d69;
  --color-brand-600: #3d8156;
  --color-brand-700: #336847;
  --color-brand-800: #2b533b;
  --color-brand-900: #244532;
  --color-brand-950: #12251b;
}

/* ---------- 浅色 ---------- */
:root {
  --ui-radius: 0.625rem;          /* 10px，卡片/控件统一圆角 */
  --ui-container: 73.75rem;       /* 1180px 内容最大宽 */

  --ui-primary: var(--color-brand-600);
  --ui-bg: #f3f3f3;               /* 窗口外底 */
  --ui-bg-muted: #ffffff;         /* 内容框 */
  --ui-bg-elevated: #fbfbfb;      /* 卡片 */
  --ui-bg-accented: #f2f2f2;      /* 下沉层 */
  --ui-text: #0a0a0a;
  --ui-text-muted: rgba(0, 0, 0, .58);
  --ui-text-dimmed: rgba(0, 0, 0, .36);
  --ui-border: rgba(0, 0, 0, .09);
  --ui-border-accented: rgba(0, 0, 0, .18);
}

/* ---------- 深色 ---------- */
.dark {
  --ui-primary: var(--color-brand-400);
  --ui-bg: #000000;
  --ui-bg-muted: #0e0e0e;
  --ui-bg-elevated: #161616;
  --ui-bg-accented: #0c0c0c;
  --ui-text: #ffffff;
  --ui-text-muted: rgba(255, 255, 255, .60);
  --ui-text-dimmed: rgba(255, 255, 255, .36);
  --ui-border: rgba(255, 255, 255, .08);
  --ui-border-accented: rgba(255, 255, 255, .16);
}

/* ---------- 主题切换过渡（0.55s 全局渐变） ---------- */
body, .xlt-frame, .xlt-card {
  transition: background-color .55s cubic-bezier(.4,0,.2,1),
              color .55s cubic-bezier(.4,0,.2,1),
              border-color .55s cubic-bezier(.4,0,.2,1);
}

/* ---------- 等宽数字：只给数字展示用，不要全局 ---------- */
.tnum { font-feature-settings: "tnum"; }
```

### 3.4 `app/app.config.ts`

```ts
export default defineAppConfig({
  ui: {
    colors: {
      primary: 'brand',
      neutral: 'neutral'   // Tailwind 纯灰，表面色已被 §3.3 的变量接管
    },
    button: {
      slots: { base: 'font-medium' }
    },
    card: {
      slots: {
        root: 'bg-elevated ring-1 ring-default rounded-[10px] shadow-none',
        body: 'p-5'
      }
    },
    table: {
      slots: {
        th: 'bg-accented/60 text-muted text-xs font-medium',
        td: 'text-sm'
      }
    }
  }
})
```

---

## 4. 应用骨架：侧栏融合 + 内容框布局

这是原型最标志性的结构，用布局文件一次实现，所有页面自动继承：

```vue
<!-- app/layouts/default.vue -->
<template>
  <div class="flex h-screen bg-default">
    <!-- 侧栏：透明背景，直接融入窗口外底 -->
    <aside class="w-[232px] shrink-0 flex flex-col">
      <AppBrand />
      <UNavigationMenu
        orientation="vertical"
        :items="navItems"
        class="flex-1 overflow-y-auto px-3"
      />
      <AppSidebarFooter /> <!-- 设置入口 + 主题切换 -->
    </aside>

    <!-- 内容框：外边距 + 圆角 + 边框，滚动限制在框内 -->
    <main class="xlt-frame flex-1 my-3 mr-3 rounded-2xl border border-default
                 bg-muted overflow-y-auto">
      <div class="max-w-[1180px] mx-auto px-10 pt-9 pb-20">
        <slot />
      </div>
    </main>
  </div>
</template>
```

要点：

- **侧栏不要加任何背景色和右边框**，导航选中态用 `UNavigationMenu` 的 active 样式（在 `app.config.ts` 里把 active 定制为 `bg-primary/10 text-primary` 的 pill）。
- **快捷键**：在布局里用 `defineShortcuts` 注册 `meta_1` ~ `meta_7` 跳页面、`meta_,` 进设置，与原型一致。
- **主题切换**：侧栏底部放 `UColorModeSwitch`（或按原型样式自制 switch + `useColorMode()`）。
- **首屏防闪烁**：`nuxt.config.ts` 里 `colorMode: { preference: 'dark', fallback: 'dark', storageKey: 'xlt-theme' }`；SPA 模式下 color-mode 仍会向 HTML 注入内联脚本，无需手写原型里那段 head script。

---

## 5. 页面改造映射表

| 页面 | 原型元素 | Nuxt UI v3 组件 | 定制点 |
|---|---|---|---|
| **概览** | 额度告警条 | `UAlert`（color=error, variant=subtle） | 右侧"查看额度 →"用 `UButton` ghost |
| | 4 张统计卡 | 定制 `StatCard.vue`（§6） | 大数字 `font-mono tnum` |
| | 套餐额度卡 | `UCard` + 定制 `MeterRow.vue` | 进度条用 `UProgress`，100% 换 error 色 |
| | 油价/今日工作/剪贴板小组件 | `UCard` | 标题行 + 紧凑列表 |
| **用量分析** | 时间范围切换 | `UTabs`（variant=pill，size=sm） | 今天/7天/30天/90天 |
| | 趋势图/堆叠柱/环图/热力图 | 定制 SVG 组件（§6） | 直接移植原型算法 |
| | 按工具用量表 | `UTable` | 占比列内嵌 `UProgress` |
| | Token/费用切换 | `UTabs` | — |
| **费用中心** | 三页签 | `UTabs` | 套餐额度/订阅列表/账单流水 |
| | 订阅与账单表 | `UTable` + `UBadge` | 金额列 `tnum` 右对齐 |
| **Git 报告** | 工具栏 | `USelectMenu` + `UButton` | 生成中 `loading` 态 |
| | 报告正文 | `UCard` + prose 排版 | 空状态用 `UEmpty` |
| | 提交记录 | 列表行 + `UBadge`（分支） | hash 用 `font-mono` |
| **剪贴板历史** | 类型筛选 | `UTabs` 或 pill 按钮组 | 全部/文本/图片/文件/已收藏 |
| | 双栏列表+预览 | 自定义 grid + `UInput` 搜索 | 选中行高亮 |
| **代码片段** | 片段卡片 | `UCard` 列表 | 代码块 `font-mono` |
| **密钥库** | 解锁屏 | `UCard` + `UInput`（type=password） | 锁定/解锁状态切换 |
| **设置** | 页签 | `UTabs` | 通用/平台连接/剪贴板/高级 |
| | 开关/勾选/下拉 | `USwitch` / `UCheckbox` / `USelect` | 主题切换卡复用侧栏逻辑 |

---

## 6. 需要手写的定制组件（从原型移植）

Nuxt UI 没有对应物的部分，原型里已有完整实现，**逐个移植为 Vue SFC 即可，算法不变**：

| 组件 | 来源 | 说明 |
|---|---|---|
| `StatCard.vue` | 原型 `statCard()` | label + delta 徽标 + 大数字 + 迷你走势；delta 存在时 sparkline 下移（原型已修） |
| `charts/Sparkline.vue` | `sparkline()` | props: points, color |
| `charts/AreaChart.vue` | `areaChart()` | 平滑贝塞尔 + 渐变填充 + 首尾标签 start/end 锚点 |
| `charts/StackedBars.vue` | `stackedBars()` | 按工具堆叠 |
| `charts/DonutChart.vue` | `donut()` | stroke-dasharray 弧段 + 中心总数 |
| `charts/HeatmapGrid.vue` | `heatmap()` | 13 周 × 7 天，5 级色阶用 `--color-brand-*` |
| `MeterRow.vue` | `meterRow()` | 额度行：标签/重置时间/百分比/进度条 |
| `SvcBadge.vue` | `svcBadge()` | 字母徽标，`color-mix` 底色 |
| `NoiseOverlay.vue` | `.noise` | fixed 全层、pointer-events-none、SVG feTurbulence data URI，深浅主题不同透明度 |
| `RevealOnMount.vue` | `.rv` | 挂载时按 `--i` 交错上浮（55ms 阶梯），用 `TransitionGroup` 或纯 CSS animation |

**图表不引第三方库**：原型 SVG 是手绘的，无依赖、体积小、和双主题变量天然联动，视觉能和原型 1:1。

图标映射（原型手绘 → lucide）：概览 `i-lucide-layout-grid`、用量 `i-lucide-trending-up`、费用 `i-lucide-wallet`、Git `i-lucide-git-branch`、剪贴板 `i-lucide-clipboard`、片段 `i-lucide-code`、密钥 `i-lucide-key-round`、设置 `i-lucide-settings`、同步 `i-lucide-refresh-cw`、告警 `i-lucide-triangle-alert`。

---

## 7. 分阶段实施计划

| 阶段 | 内容 | 预估 | 验收标准 |
|---|---|---|---|
| **P0 准备** | 切分支 `feat/ui-redesign`；现有页面截图存档作对照基线；确认 §2 前置项 | 0.5 天 | 基线截图归档 |
| **P1 设计系统 + 骨架** | §3 令牌落地；§4 布局、侧栏、主题切换、快捷键、噪点层 | 1–2 天 | 空壳框架双主题切换无闪烁，与原型骨架截图一致 |
| **P2 洞察组** | 概览、用量分析（图表移植是大头）、费用中心 | 2–3 天 | 三页双主题与原型逐区对照一致 |
| **P3 工作 + 工具箱** | Git 报告、剪贴板、代码片段、密钥库 | 1–2 天 | 同上 |
| **P4 设置 + 打磨** | 设置页、空状态、页面切换过渡、动效复查 | 1 天 | 同上 |
| **P5 验收** | 按 §8 清单走查，Tauri 窗口实测 | 0.5 天 | 清单全过 |

**建议顺序的原因**：P1 是全部页面的地基；P2 包含你的高频模块（用量+费用），先交付可以先用起来；图表组件集中在 P2 移植，P3/P4 纯组装会快很多。

---

## 8. 验收清单

- [ ] 8 个页面在深色/浅色下与原型截图逐区一致
- [ ] 主题切换 0.55s 平滑过渡，无闪烁、无首屏白闪
- [ ] 侧栏无独立底色，内容区为圆角内嵌框，框内独立滚动
- [ ] ⌘1–7 / ⌘, 快捷键生效
- [ ] 所有金额、Token 数字为等宽 tnum 且右对齐
- [ ] Tauri 打包后 hash 路由直达任意页面不 404
- [ ] 无任何 Nuxt UI 内部类名被硬覆盖（grep 检查）

---

## 9. 风险与注意点

1. **不要硬覆盖组件内部类**。Nuxt UI v3 用 Tailwind Variants，定制走 `app.config.ts` 的 slots/variants 或组件 `ui` prop，会自动合并类。
2. **中性色阶仍会被部分组件引用**（如 hover 态的 `neutral-*`）。§3.3 已覆盖表面变量，若个别组件 hover 色偏蓝灰，把 `neutral` 换成 Tailwind 的 `neutral`（纯灰）而非 `slate`（蓝灰）。
3. **tnum 不要全局开**，只加在统计卡、表格金额列等数字展示处，否则中文排版会受影响。
4. **Nuxt UI v4 已发布**且全部组件免费，API 与本方案基本一致；本方案按你当前 v3 编写，将来升级 v4 时 §3–§5 基本无需改动。
5. **数据接口隔离**：每个页面的 mock 数据集中放 `app/utils/mock.ts`（直接抄原型数据），后续接 Tauri command 时只换这一层，页面代码不动。
