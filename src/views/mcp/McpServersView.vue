<script setup lang="ts">
import { ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import WorkspaceFilter from "@/components/WorkspaceFilter.vue";

/**
 * MCP 服务页（预留）。
 * 导航先占位，后续在此接入各 Agent 的 MCP 配置：列表 + 启停 + 编辑，
 * 「添加」走页内抽屉；配置异常以行内状态标签展示，不另开问题中心。
 * 写入策略：只读检测 → 备份 → 写入并保留未知字段；环境变量 / Token 引用密钥库。
 */
const route = useRoute();
const router = useRouter();

const workspace = ref("");
const search = ref("");
const showCreate = ref(route.name === "mcp-new");

const WRITE_POLICY = [
  "先做只读检测：解析配置并探测命令是否存在，确认可写后再落盘",
  "写入前备份原文件，保留未识别的字段，避免覆盖用户自定义内容",
  "环境变量与 Token 引用密钥库，避免明文落盘",
];

watch(
  () => route.name,
  (name) => {
    showCreate.value = name === "mcp-new";
  },
);

function closeCreate() {
  showCreate.value = false;
  if (route.name === "mcp-new") void router.replace({ name: "mcp" });
}
</script>

<template>
  <UDashboardPanel id="mcp">
    <template #header>
      <UDashboardNavbar title="MCP 服务" description="MCP" :ui="{ right: 'gap-1.5' }">
        <template #leading>
          <UDashboardSidebarCollapse />
        </template>
        <template #right>
          <UButton icon="i-lucide-plus" @click="showCreate = true">添加 MCP</UButton>
        </template>
      </UDashboardNavbar>

      <UDashboardToolbar>
        <template #left>
          <UInput v-model="search" icon="i-lucide-search" placeholder="搜索 MCP 服务…" class="w-56" />
          <WorkspaceFilter v-model="workspace" placeholder="全部工作区" />
        </template>
      </UDashboardToolbar>
    </template>

    <template #body>
      <UAlert
        color="neutral"
        variant="subtle"
        icon="i-lucide-construction"
        title="MCP 服务管理已预留"
        description="当前为只读占位：检测与写入能力接入后，这里会按工作区列出各 Agent 的 MCP 服务，支持启停与编辑。"
        class="mb-6"
      />

      <div class="flex flex-col items-center py-20">
        <span class="flex size-14 items-center justify-center rounded-2xl bg-elevated ring ring-default mb-4">
          <UIcon name="i-lucide-plug" class="size-7 text-dimmed" />
        </span>
        <p class="text-sm font-medium text-highlighted mb-1">尚未检测到 MCP 服务</p>
        <p class="text-xs text-muted mb-5">选定工作区后即可查看该范围内各 Agent 的 MCP 配置。</p>
        <UButton size="sm" icon="i-lucide-plus" @click="showCreate = true">添加 MCP</UButton>
      </div>
    </template>
  </UDashboardPanel>

  <UModal
    :open="showCreate"
    title="添加 MCP"
    description="MCP 写入能力尚未接入，先确认写入策略。"
    @update:open="!$event && closeCreate()"
  >
    <template #body>
      <div class="space-y-4">
        <UAlert
          color="warning"
          variant="subtle"
          icon="i-lucide-shield-alert"
          title="写入策略"
          description="以下约束会在接入时生效，避免破坏现有配置。"
        />
        <ul class="space-y-2">
          <li v-for="rule in WRITE_POLICY" :key="rule" class="flex items-start gap-2 text-sm text-muted">
            <UIcon name="i-lucide-check" class="size-4 shrink-0 mt-0.5 text-primary" />
            <span>{{ rule }}</span>
          </li>
        </ul>
      </div>
    </template>
    <template #footer>
      <div class="flex justify-end gap-2 w-full">
        <UButton color="neutral" variant="ghost" @click="closeCreate">关闭</UButton>
        <UButton disabled>保存</UButton>
      </div>
    </template>
  </UModal>
</template>
