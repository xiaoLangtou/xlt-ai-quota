<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
import { storeToRefs } from "pinia";
import type {
  ApplyPlanRequest,
  CatalogFilter,
  CatalogSource,
  McpPackage,
  McpPlan,
  McpServer,
  PlanRequest,
  SourceSaveRequest,
} from "@/types/mcp";
import { useSkillsStore } from "@/stores/skills";
import { mcpApi, mcpErrorMessage } from "@/api/mcp";
import { pushToast } from "@/composables/useToast";
import { formatDate, formatStars, sourceLabel } from "@/utils/mcp";
import McpPackageDrawer from "@/views/mcp/McpPackageDrawer.vue";
import McpServerDrawer from "@/views/mcp/McpServerDrawer.vue";
import McpPlanDialog from "@/views/mcp/McpPlanDialog.vue";

const skillsStore = useSkillsStore();
const { projects } = storeToRefs(skillsStore);
const projectPaths = computed(() => projects.value.map((project) => project.path));

const PAGE_SIZE = 24;

const sources = ref<CatalogSource[]>([]);
const packages = ref<McpPackage[]>([]);
const loading = ref(false);
const syncing = ref(false);
const error = ref("");
const visible = ref(PAGE_SIZE);
const lastSync = ref("");

const search = ref("");
const sourceFilter = ref("all");
const sortFilter = ref<"all" | "star" | "updated" | "favorites">("all");
const runtimeFilter = ref("all");
const showClients = ref(false);
const favoritesOnly = ref(false);

const selected = ref<McpPackage | null>(null);
const packageOpen = ref(false);

const serverDrawerOpen = ref(false);
const installServer = ref<McpServer | null>(null);
const plan = ref<McpPlan | null>(null);
const planOpen = ref(false);
const applying = ref(false);

const sourceManagerOpen = ref(false);
const newSource = reactive({ label: "", url: "" });

const sourceItems = computed(() => [
  { label: "全部来源", value: "all" },
  ...sources.value.map((source) => ({ label: source.label, value: source.id })),
]);
const sortItems = [
  { label: "综合排序", value: "all" },
  { label: "Star 最多", value: "star" },
  { label: "最近更新", value: "updated" },
  { label: "我的收藏", value: "favorites" },
];
const runtimeItems = [
  { label: "全部运行时", value: "all" },
  { label: "npx", value: "npx" },
  { label: "uvx", value: "uvx" },
  { label: "docker", value: "docker" },
  { label: "远程", value: "remote" },
];

const displayed = computed(() => packages.value.slice(0, visible.value));
const hasMore = computed(() => packages.value.length > visible.value);

const sourceErrors = computed(() => sources.value.filter((source) => source.lastError));

function currentFilter(): CatalogFilter {
  return {
    source: sourceFilter.value === "all" ? undefined : sourceFilter.value,
    query: search.value.trim() || undefined,
    sort: sortFilter.value,
    runtime: runtimeFilter.value === "all" ? undefined : runtimeFilter.value,
    showClients: showClients.value,
    favoritesOnly: favoritesOnly.value,
  };
}

async function loadSources() {
  try {
    sources.value = await mcpApi.listSources();
    const stamps = sources.value.map((source) => source.lastSyncAt).filter(Boolean) as string[];
    lastSync.value = stamps.sort().at(-1) ?? "";
  } catch (caught) {
    error.value = mcpErrorMessage(caught);
  }
}

async function loadPackages() {
  loading.value = true;
  error.value = "";
  try {
    packages.value = await mcpApi.listPackages(currentFilter(), projectPaths.value);
    visible.value = PAGE_SIZE;
  } catch (caught) {
    error.value = mcpErrorMessage(caught);
  } finally {
    loading.value = false;
  }
}

async function refresh() {
  syncing.value = true;
  try {
    await mcpApi.sync(sourceFilter.value === "all" ? undefined : sourceFilter.value);
    await loadSources();
    await loadPackages();
    pushToast("库数据已刷新", "success");
  } catch (caught) {
    pushToast(mcpErrorMessage(caught), "error");
    await loadSources();
    await loadPackages();
  } finally {
    syncing.value = false;
  }
}

onMounted(async () => {
  await loadSources();
  await loadPackages();
  if (!packages.value.length && !sources.value.some((source) => source.lastSyncAt)) {
    void refresh();
  }
});

let searchTimer: number | undefined;
watch(search, () => {
  window.clearTimeout(searchTimer);
  searchTimer = window.setTimeout(() => void loadPackages(), 500);
});
watch([sourceFilter, sortFilter, runtimeFilter, showClients, favoritesOnly], () => void loadPackages());

function openPackage(item: McpPackage) {
  selected.value = item;
  packageOpen.value = true;
}

async function toggleFavorite(item: McpPackage) {
  try {
    await mcpApi.favorite(item.id, !item.favorite);
    item.favorite = !item.favorite;
  } catch (caught) {
    pushToast(mcpErrorMessage(caught), "error");
  }
}

function onInstall(server: McpServer) {
  packageOpen.value = false;
  installServer.value = server;
  serverDrawerOpen.value = true;
}

function closeServerDrawer() {
  serverDrawerOpen.value = false;
  installServer.value = null;
}

async function onServerSubmit(request: PlanRequest) {
  closeServerDrawer();
  try {
    plan.value = await mcpApi.plan(request);
    planOpen.value = true;
  } catch (caught) {
    pushToast(mcpErrorMessage(caught), "error");
  }
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
      pushToast("安装完成，已写入配置", "success");
      await loadPackages();
    }
    planOpen.value = false;
    plan.value = null;
  } catch (caught) {
    pushToast(mcpErrorMessage(caught), "error");
  } finally {
    applying.value = false;
  }
}

async function addCustomSource() {
  if (!newSource.label.trim() || !newSource.url.trim()) {
    pushToast("请填写名称与 JSON URL", "warn");
    return;
  }
  const request: SourceSaveRequest = {
    label: newSource.label.trim(),
    kind: "custom",
    url: newSource.url.trim(),
    enabled: true,
  };
  try {
    await mcpApi.saveSource(request);
    newSource.label = "";
    newSource.url = "";
    await loadSources();
    pushToast("自定义源已添加", "success");
  } catch (caught) {
    pushToast(mcpErrorMessage(caught), "error");
  }
}

async function toggleSource(source: CatalogSource, enabled: boolean) {
  try {
    await mcpApi.saveSource({
      id: source.id,
      label: source.label,
      kind: source.kind,
      url: source.url,
      channelId: source.channelId,
      subChannelId: source.subChannelId,
      enabled,
    });
    await loadSources();
  } catch (caught) {
    pushToast(mcpErrorMessage(caught), "error");
  }
}

async function removeSource(source: CatalogSource) {
  try {
    await mcpApi.removeSource(source.id);
    await loadSources();
    await loadPackages();
  } catch (caught) {
    pushToast(mcpErrorMessage(caught), "error");
  }
}

const lastSyncLabel = computed(() => (lastSync.value ? formatDate(lastSync.value) : "从未同步"));
</script>

<template>
  <UDashboardPanel id="mcp-library">
    <template #header>
      <UDashboardNavbar title="MCP 库" description="发现并一键装进 MCP 服务" :ui="{ right: 'gap-1.5' }">
        <template #leading>
          <UDashboardSidebarCollapse />
        </template>
        <template #right>
          <UButton icon="i-lucide-server-cog" color="neutral" variant="ghost" @click="sourceManagerOpen = true">
            数据源
          </UButton>
          <UButton icon="i-lucide-refresh-cw" color="neutral" variant="ghost" :loading="syncing" @click="refresh">
            刷新
          </UButton>
        </template>
      </UDashboardNavbar>

      <UDashboardToolbar>
        <template #left>
          <UInput v-model="search" icon="i-lucide-search" placeholder="搜索 MCP 服务…" class="w-60" />
          <USelect v-model="sourceFilter" :items="sourceItems" icon="i-lucide-database" class="w-40" />
          <USelect v-model="runtimeFilter" :items="runtimeItems" icon="i-lucide-cpu" class="w-36" />
          <USelect v-model="sortFilter" :items="sortItems" icon="i-lucide-arrow-up-down" class="w-36" />
          <UCheckbox v-model="favoritesOnly" label="收藏" />
          <UCheckbox v-model="showClients" label="显示客户端类" />
        </template>
        <template #right>
          <span class="text-xs text-dimmed">上次同步 {{ lastSyncLabel }}</span>
        </template>
      </UDashboardToolbar>
    </template>

    <template #body>
      <UAlert
        v-for="source in sourceErrors"
        :key="source.id"
        color="warning"
        variant="subtle"
        icon="i-lucide-cloud-off"
        :title="`${source.label} 暂时不可用`"
        :description="`${source.lastError}；当前展示的是缓存数据。`"
        class="mb-3"
      />
      <UAlert v-if="error" color="error" variant="soft" icon="i-lucide-triangle-alert" :title="error" class="mb-3" />

      <div v-if="loading && !packages.length" class="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
        <USkeleton v-for="index in 6" :key="index" class="h-32 w-full" />
      </div>

      <div v-else-if="!displayed.length" class="flex flex-col items-center py-20">
        <span class="mb-4 flex size-14 items-center justify-center rounded-2xl bg-elevated ring ring-default">
          <UIcon name="i-lucide-library-big" class="size-7 text-dimmed" />
        </span>
        <p class="mb-1 text-sm font-medium text-highlighted">没有匹配的 MCP 条目</p>
        <p class="text-xs text-muted">尝试更换关键词、来源或点击「刷新」拉取最新数据。</p>
      </div>

      <div v-else class="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
        <UCard
          v-for="item in displayed"
          :key="item.id"
          :ui="{ body: 'p-4' }"
          class="cursor-pointer transition-shadow hover:shadow-md"
          @click="openPackage(item)"
        >
          <div class="flex h-full flex-col gap-2">
            <div class="flex items-start justify-between gap-2">
              <div class="min-w-0">
                <p class="truncate text-sm font-semibold text-highlighted">{{ item.name }}</p>
                <p class="truncate text-[11px] text-dimmed">{{ item.repo ?? item.homepage }}</p>
              </div>
              <button
                type="button"
                class="shrink-0 rounded p-0.5 text-dimmed transition-colors hover:text-primary cursor-pointer"
                @click.stop="toggleFavorite(item)"
              >
                <UIcon :name="item.favorite ? 'i-lucide-star' : 'i-lucide-star'" :class="item.favorite ? 'size-4 text-warning' : 'size-4'" />
              </button>
            </div>

            <p class="line-clamp-2 min-h-8 text-xs text-muted">{{ item.description || "（无描述）" }}</p>

            <div class="mt-auto flex flex-wrap items-center gap-1.5 pt-1">
              <UBadge color="neutral" variant="soft" size="sm">{{ sourceLabel(item.source) }}</UBadge>
              <UBadge v-if="item.mirror" color="neutral" variant="outline" size="sm">镜像</UBadge>
              <UBadge v-if="item.install.level === 'ready'" color="success" variant="subtle" size="sm">可一键安装</UBadge>
              <UBadge v-else color="neutral" variant="outline" size="sm">需手动配置</UBadge>
              <UBadge v-if="item.installedAgents.length" color="primary" variant="soft" size="sm">
                已安装 · {{ item.installedAgents.join("、") }}
              </UBadge>
              <span class="ml-auto inline-flex items-center gap-1 text-[11px] text-dimmed">
                <UIcon name="i-lucide-star" class="size-3" />{{ formatStars(item.stars) }}
              </span>
            </div>
          </div>
        </UCard>
      </div>

      <div v-if="hasMore" class="mt-5 flex justify-center">
        <UButton color="neutral" variant="soft" icon="i-lucide-chevron-down" @click="visible += PAGE_SIZE">
          加载更多
        </UButton>
      </div>
    </template>
  </UDashboardPanel>

  <McpPackageDrawer v-model:open="packageOpen" :item="selected" @install="onInstall" />

  <McpServerDrawer
    v-model:open="serverDrawerOpen"
    :initial-server="installServer"
    @submit="onServerSubmit"
  />

  <McpPlanDialog v-model:open="planOpen" :plan="plan" :busy="applying" @confirm="confirmApply" />

  <UModal
    v-model:open="sourceManagerOpen"
    title="数据源管理"
    description="内置源开箱可用；自定义源需填写符合约定格式的 JSON URL（仅 https）。"
    :ui="{ content: 'max-w-2xl' }"
  >
    <template #body>
      <div class="space-y-3">
        <div
          v-for="source in sources"
          :key="source.id"
          class="flex items-center justify-between gap-3 rounded-lg px-3 py-2 ring ring-default"
        >
          <div class="min-w-0">
            <div class="flex items-center gap-2">
              <span class="text-sm font-medium text-highlighted">{{ source.label }}</span>
              <UBadge color="neutral" variant="soft" size="sm">{{ source.kind }}</UBadge>
              <UBadge v-if="source.builtin" color="neutral" variant="outline" size="sm">内置</UBadge>
            </div>
            <p v-if="source.lastError" class="mt-0.5 truncate text-[11px] text-error">{{ source.lastError }}</p>
            <p v-else class="mt-0.5 text-[11px] text-dimmed">
              {{ source.lastSyncAt ? `上次同步 ${formatDate(source.lastSyncAt)}` : "尚未同步" }}
            </p>
          </div>
          <div class="flex shrink-0 items-center gap-2">
            <USwitch
              :model-value="source.enabled"
              @update:model-value="toggleSource(source, Boolean($event))"
            />
            <UButton
              v-if="!source.builtin"
              icon="i-lucide-trash-2"
              color="error"
              variant="ghost"
              size="sm"
              square
              @click="removeSource(source)"
            />
          </div>
        </div>

        <USeparator />

        <p class="text-xs font-semibold text-highlighted">添加自定义源</p>
        <div class="grid grid-cols-3 gap-2">
          <UInput v-model="newSource.label" placeholder="名称" class="col-span-1" />
          <UInput v-model="newSource.url" placeholder="https://example.com/mcp.json" class="col-span-2 font-mono" />
        </div>
      </div>
    </template>
    <template #footer>
      <div class="flex w-full justify-end gap-2">
        <UButton color="neutral" variant="ghost" @click="sourceManagerOpen = false">关闭</UButton>
        <UButton icon="i-lucide-plus" :disabled="!newSource.label.trim() || !newSource.url.trim()" @click="addCustomSource">
          添加
        </UButton>
      </div>
    </template>
  </UModal>
</template>
