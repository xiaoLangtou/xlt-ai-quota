<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { storeToRefs } from "pinia";
import type { ApplyPlanRequest, McpPlan, McpServer, McpService, PlanRequest, ProbeResult, WriteTarget } from "@/types/mcp";
import { useMcpStore } from "@/stores/mcp";
import { useSkillsStore } from "@/stores/skills";
import { mcpApi, mcpErrorMessage } from "@/api/mcp";
import { pushToast } from "@/composables/useToast";
import { serverTargetText, statusMeta, transportLabel } from "@/utils/mcp";
import McpServerDrawer from "@/views/mcp/McpServerDrawer.vue";
import McpPlanDialog from "@/views/mcp/McpPlanDialog.vue";

const route = useRoute();
const router = useRouter();
const store = useMcpStore();
const skillsStore = useSkillsStore();
const { services, issues, agents, loading, error, scannedAt } = storeToRefs(store);
const { projects } = storeToRefs(skillsStore);

const projectPaths = computed(() => projects.value.map((project) => project.path));

const search = ref("");
const agentFilter = ref("all");
const statusFilter = ref("all");
const scopeFilter = ref("all");

const drawerOpen = ref(false);
const editing = ref<McpService | null>(null);
const initialServer = ref<McpServer | null>(null);
const plan = ref<McpPlan | null>(null);
const planOpen = ref(false);
const applying = ref(false);

const probeTarget = ref<McpService | null>(null);
const probeResult = ref<ProbeResult | null>(null);
const probing = ref(false);

const agentItems = computed(() => [
  { label: "全部 Agent", value: "all" },
  ...agents.value.map((agent) => ({ label: agent.label, value: agent.id })),
]);
const statusItems = [
  { label: "全部状态", value: "all" },
  { label: "正常", value: "ok" },
  { label: "已停用", value: "disabled" },
  { label: "异常", value: "error" },
];
const scopeItems = [
  { label: "全部范围", value: "all" },
  { label: "全局", value: "global" },
  { label: "项目", value: "project" },
];

const filtered = computed(() => {
  const needle = search.value.trim().toLowerCase();
  return services.value.filter((service) => {
    if (needle) {
      const haystack = `${service.name} ${serverTargetText(service)} ${service.url ?? ""}`.toLowerCase();
      if (!haystack.includes(needle)) return false;
    }
    if (agentFilter.value !== "all" && !service.agents.includes(agentFilter.value)) return false;
    if (scopeFilter.value !== "all" && !service.scopes.includes(scopeFilter.value)) return false;
    if (statusFilter.value === "ok" && service.status !== "ok") return false;
    if (statusFilter.value === "disabled" && service.status !== "disabled") return false;
    if (
      statusFilter.value === "error" &&
      service.status !== "config_error" &&
      service.status !== "command_missing"
    ) {
      return false;
    }
    return true;
  });
});

function agentLabel(id: string): string {
  return agents.value.find((agent) => agent.id === id)?.label ?? id;
}

function serviceToServer(service: McpService): McpServer {
  return {
    name: service.name,
    transport: service.transport,
    command: service.command,
    args: service.args,
    env: service.env,
    url: service.url,
    headers: service.headers,
    extra: {},
  };
}

function targetsFor(service: McpService): WriteTarget[] {
  return service.instances.map((instance) => ({
    agent: instance.agent,
    scope: instance.scope,
    projectPath: instance.projectPath,
  }));
}

async function refresh() {
  await store.refresh(projectPaths.value);
}

onMounted(async () => {
  await Promise.all([store.loadAgents(), refresh()]);
  if (route.name === "mcp-new") drawerOpen.value = true;
});

watch(
  () => route.name,
  (name) => {
    if (name === "mcp-new") drawerOpen.value = true;
  },
);

watch(projectPaths, () => {
  void refresh();
});

function openAdd() {
  editing.value = null;
  initialServer.value = null;
  drawerOpen.value = true;
  if (route.name === "mcp-new") return;
  void router.replace({ name: "mcp" });
}

function openEdit(service: McpService) {
  editing.value = service;
  initialServer.value = null;
  drawerOpen.value = true;
}

function syncTo(service: McpService) {
  editing.value = null;
  initialServer.value = serviceToServer(service);
  drawerOpen.value = true;
}

function closeDrawer() {
  drawerOpen.value = false;
  editing.value = null;
  initialServer.value = null;
  if (route.name === "mcp-new") void router.replace({ name: "mcp" });
}

async function requestPlan(request: PlanRequest) {
  try {
    plan.value = await mcpApi.plan(request);
    planOpen.value = true;
  } catch (caught) {
    pushToast(mcpErrorMessage(caught), "error");
  }
}

function onDrawerSubmit(request: PlanRequest) {
  closeDrawer();
  void requestPlan(request);
}

function applyRequest(request: PlanRequest) {
  void requestPlan(request);
}

function toggleService(service: McpService) {
  const enable = !service.enabled;
  applyRequest({
    kind: enable ? "enable" : "disable",
    server: serviceToServer(service),
    name: service.name,
    targets: targetsFor(service),
  });
}

function removeService(service: McpService) {
  applyRequest({ kind: "remove", name: service.name, targets: targetsFor(service) });
}

async function confirmApply() {
  if (!plan.value) return;
  applying.value = true;
  try {
    const request: ApplyPlanRequest = { expectedMtimes: {} };
    const result = await mcpApi.apply(plan.value.id, request);
    if (result.error) {
      pushToast(result.error, result.rolledBack ? "warn" : "error");
    } else {
      pushToast("已写入并重新扫描", "success");
    }
    planOpen.value = false;
    plan.value = null;
    await refresh();
  } catch (caught) {
    pushToast(mcpErrorMessage(caught), "error");
  } finally {
    applying.value = false;
  }
}

async function runProbe(service: McpService) {
  probeTarget.value = service;
  probeResult.value = null;
  probing.value = true;
  try {
    probeResult.value = await mcpApi.probe(serviceToServer(service));
  } catch (caught) {
    probeResult.value = {
      ok: false,
      transport: service.transport,
      tools: [],
      error: mcpErrorMessage(caught),
      durationMs: 0,
    };
  } finally {
    probing.value = false;
  }
}

const scannedLabel = computed(() => {
  if (!scannedAt.value) return "";
  return new Date(scannedAt.value).toLocaleTimeString("zh-CN");
});
</script>

<template>
  <UDashboardPanel id="mcp">
    <template #header>
      <UDashboardNavbar title="MCP 服务" description="一处管理，多处生效" :ui="{ right: 'gap-1.5' }">
        <template #leading>
          <UDashboardSidebarCollapse />
        </template>
        <template #right>
          <UButton
            icon="i-lucide-refresh-cw"
            color="neutral"
            variant="ghost"
            :loading="loading"
            @click="refresh"
          >
            重新扫描
          </UButton>
          <UButton icon="i-lucide-plus" @click="openAdd">添加 MCP</UButton>
        </template>
      </UDashboardNavbar>

      <UDashboardToolbar>
        <template #left>
          <UInput v-model="search" icon="i-lucide-search" placeholder="搜索名称 / 命令 / URL…" class="w-60" />
          <USelect v-model="agentFilter" :items="agentItems" icon="i-lucide-bot" class="w-36" />
          <USelect v-model="statusFilter" :items="statusItems" icon="i-lucide-activity" class="w-32" />
          <USelect v-model="scopeFilter" :items="scopeItems" icon="i-lucide-layers" class="w-32" />
        </template>
        <template #right>
          <span v-if="scannedLabel" class="text-xs text-dimmed">上次扫描 {{ scannedLabel }}</span>
        </template>
      </UDashboardToolbar>
    </template>

    <template #body>
      <UAlert v-if="error" color="error" variant="soft" icon="i-lucide-triangle-alert" :title="error" class="mb-4" />

      <UAlert
        v-for="issue in issues"
        :key="issue.configFile + issue.message"
        color="warning"
        variant="subtle"
        icon="i-lucide-file-warning"
        :title="`${issue.agentLabel} 配置读取失败`"
        :description="`${issue.configFile}：${issue.message}`"
        class="mb-3"
      />

      <div v-if="loading && !services.length" class="space-y-3">
        <USkeleton v-for="index in 4" :key="index" class="h-20 w-full" />
      </div>

      <div v-else-if="!filtered.length" class="flex flex-col items-center py-20">
        <span class="mb-4 flex size-14 items-center justify-center rounded-2xl bg-elevated ring ring-default">
          <UIcon name="i-lucide-plug" class="size-7 text-dimmed" />
        </span>
        <p class="mb-1 text-sm font-medium text-highlighted">
          {{ services.length ? "没有符合筛选条件的服务" : "尚未检测到 MCP 服务" }}
        </p>
        <p class="mb-5 text-xs text-muted">
          {{ services.length ? "调整筛选条件或重新扫描。" : "从 MCP 库安装，或手动添加一个服务。" }}
        </p>
        <div class="flex gap-2">
          <UButton size="sm" icon="i-lucide-library-big" to="/mcp/library" color="neutral" variant="soft">浏览 MCP 库</UButton>
          <UButton size="sm" icon="i-lucide-plus" @click="openAdd">添加 MCP</UButton>
        </div>
      </div>

      <div v-else class="space-y-2.5">
        <UCard
          v-for="service in filtered"
          :key="service.key"
          :ui="{ body: 'p-4' }"
        >
          <div class="flex items-start justify-between gap-4">
            <div class="min-w-0 space-y-2">
              <div class="flex flex-wrap items-center gap-2">
                <span class="text-sm font-semibold text-highlighted">{{ service.name }}</span>
                <UBadge color="neutral" variant="soft" size="sm">{{ transportLabel(service.transport) }}</UBadge>
                <UBadge
                  :color="statusMeta(service.status).color"
                  variant="subtle"
                  size="sm"
                  :icon="statusMeta(service.status).icon"
                >
                  {{ statusMeta(service.status).label }}
                </UBadge>
                <UBadge v-if="service.sourceId" color="primary" variant="soft" size="sm" icon="i-lucide-package">
                  来自 MCP 库
                </UBadge>
              </div>
              <p class="truncate font-mono text-xs text-muted" :title="serverTargetText(service)">
                {{ serverTargetText(service) || "（无命令或 URL）" }}
              </p>
              <div class="flex flex-wrap items-center gap-1.5">
                <UBadge
                  v-for="agent in service.agents"
                  :key="agent"
                  color="neutral"
                  variant="outline"
                  size="sm"
                >
                  {{ agentLabel(agent) }}
                </UBadge>
                <UBadge
                  v-for="scope in service.scopes"
                  :key="scope"
                  color="neutral"
                  variant="soft"
                  size="sm"
                >
                  {{ scope === "global" ? "全局" : "项目" }}
                </UBadge>
                <span class="text-[11px] text-dimmed">{{ service.instances.length }} 处配置</span>
              </div>
            </div>

            <div class="flex shrink-0 flex-wrap items-center justify-end gap-1">
              <UTooltip text="测试连接">
                <UButton
                  icon="i-lucide-zap"
                  color="neutral"
                  variant="ghost"
                  size="sm"
                  square
                  @click="runProbe(service)"
                />
              </UTooltip>
              <UTooltip :text="service.enabled ? '停用' : '启用'">
                <UButton
                  :icon="service.enabled ? 'i-lucide-pause' : 'i-lucide-play'"
                  color="neutral"
                  variant="ghost"
                  size="sm"
                  square
                  @click="toggleService(service)"
                />
              </UTooltip>
              <UTooltip text="同步到其他 Agent">
                <UButton icon="i-lucide-copy-plus" color="neutral" variant="ghost" size="sm" square @click="syncTo(service)" />
              </UTooltip>
              <UTooltip text="编辑">
                <UButton icon="i-lucide-pencil" color="neutral" variant="ghost" size="sm" square @click="openEdit(service)" />
              </UTooltip>
              <UTooltip text="删除">
                <UButton icon="i-lucide-trash-2" color="error" variant="ghost" size="sm" square @click="removeService(service)" />
              </UTooltip>
            </div>
          </div>

          <div class="mt-3 flex flex-wrap gap-1.5 border-t border-default pt-3">
            <span
              v-for="instance in service.instances"
              :key="instance.configFile + instance.agent + instance.scope"
              class="inline-flex items-center gap-1.5 rounded-md bg-elevated/40 px-2 py-1 text-[11px] text-dimmed"
            >
              <UIcon :name="statusMeta(instance.status).icon" :class="`size-3`" />
              <span class="font-mono">{{ instance.configFile }}</span>
            </span>
          </div>
        </UCard>
      </div>
    </template>
  </UDashboardPanel>

  <McpServerDrawer
    v-model:open="drawerOpen"
    :editing="editing"
    :initial-server="initialServer"
    @submit="onDrawerSubmit"
  />

  <McpPlanDialog v-model:open="planOpen" :plan="plan" :busy="applying" @confirm="confirmApply" />

  <UModal
    :open="Boolean(probeTarget)"
    :title="`测试连接：${probeTarget?.name ?? ''}`"
    description="启动进程或连接 URL，发送 initialize + tools/list，10 秒超时，用完即关。"
    @update:open="!$event && (probeTarget = null)"
  >
    <template #body>
      <div class="space-y-3">
        <div v-if="probing" class="flex items-center gap-2 text-sm text-muted">
          <UIcon name="i-lucide-loader-circle" class="size-4 animate-spin" />
          正在连接…
        </div>
        <template v-else-if="probeResult">
          <UAlert
            :color="probeResult.ok ? 'success' : 'error'"
            variant="subtle"
            :icon="probeResult.ok ? 'i-lucide-circle-check' : 'i-lucide-circle-alert'"
            :title="probeResult.ok ? `连接成功（${probeResult.durationMs}ms）` : '连接失败'"
            :description="probeResult.error"
          />
          <p v-if="probeResult.serverInfo" class="text-xs text-muted">服务：{{ probeResult.serverInfo }}</p>
          <p v-if="probeResult.ok" class="text-xs text-muted">
            工具列表（{{ probeResult.tools.length }}）
          </p>
          <div v-if="probeResult.tools.length" class="max-h-64 space-y-1 overflow-auto">
            <div
              v-for="tool in probeResult.tools"
              :key="tool.name"
              class="rounded-md bg-elevated/40 px-3 py-2"
            >
              <p class="font-mono text-xs text-highlighted">{{ tool.name }}</p>
              <p v-if="tool.description" class="mt-0.5 text-[11px] text-muted">{{ tool.description }}</p>
            </div>
          </div>
        </template>
      </div>
    </template>
    <template #footer>
      <div class="flex w-full justify-end">
        <UButton color="neutral" variant="ghost" @click="probeTarget = null">关闭</UButton>
      </div>
    </template>
  </UModal>
</template>
