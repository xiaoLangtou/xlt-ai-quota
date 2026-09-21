<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { storeToRefs } from "pinia";
import type { ScanFailure, SkillScope, SkillSummary } from "@/types/skills";
import { useSkillsStore } from "@/stores/skills";
import { skillsApi, skillsErrorMessage } from "@/api/skills";
import { agentStyle } from "@/utils/skills-agents";
import AgentAvatar from "@/components/skills/AgentAvatar.vue";
import WorkspaceFilter from "@/components/WorkspaceFilter.vue";

const store = useSkillsStore();
const { roots, skills, loading, error } = storeToRefs(store);
const router = useRouter();
const route = useRoute();

const ALL_ROOTS = "__all__";
const search = ref("");
const rootFilter = ref((route.query.root as string) || ALL_ROOTS);
const projectFilter = ref((route.query.project as string) || "");
const agentFilter = ref((route.query.agent as string) || "");
const scopeFilter = ref<"all" | "global" | "project">("all");
// 「有问题」筛选取代原问题中心入口：/skills/issues 重定向到 ?filter=issues
const issuesOnly = ref(route.query.filter === "issues");
const failures = ref<ScanFailure[]>([]);

const scopeItems = [
  { label: "全部", value: "all" as const },
  { label: "全局", value: "global" as const },
  { label: "项目", value: "project" as const },
];

watch(
  () => route.query.root,
  (value) => {
    rootFilter.value = (value as string) || ALL_ROOTS;
  },
);
watch(
  () => route.query.project,
  (value) => {
    projectFilter.value = (value as string) || "";
  },
);
watch(
  () => route.query.agent,
  (value) => {
    agentFilter.value = (value as string) || "";
  },
);
watch(
  () => route.query.filter,
  (value) => {
    issuesOnly.value = value === "issues";
  },
);

const projectRootIds = computed(
  () => new Set(roots.value.filter((item) => item.projectId === projectFilter.value).map((item) => item.id)),
);

/** 工作区下拉与 ?root= / ?project= / ?agent= 的双向绑定。 */
const workspaceValue = computed(() => {
  if (rootFilter.value !== ALL_ROOTS) return `root:${rootFilter.value}`;
  if (projectFilter.value) return `project:${projectFilter.value}`;
  if (agentFilter.value) return `agent:${agentFilter.value}`;
  return "";
});

function syncQuery() {
  const query: Record<string, string> = {};
  if (rootFilter.value !== ALL_ROOTS) query.root = rootFilter.value;
  if (projectFilter.value) query.project = projectFilter.value;
  if (agentFilter.value) query.agent = agentFilter.value;
  if (issuesOnly.value) query.filter = "issues";
  void router.replace({ path: "/skills/library", query });
}

function setWorkspace(value: string) {
  rootFilter.value = ALL_ROOTS;
  projectFilter.value = "";
  agentFilter.value = "";
  const index = value.indexOf(":");
  const kind = index === -1 ? "" : value.slice(0, index);
  const id = index === -1 ? "" : value.slice(index + 1);
  if (kind === "root") rootFilter.value = id;
  else if (kind === "project") projectFilter.value = id;
  else if (kind === "agent") agentFilter.value = id;
  syncQuery();
}

function setIssuesOnly(value: boolean) {
  issuesOnly.value = value;
  syncQuery();
}

interface SkillOrigin {
  agent?: string;
  rootId: string;
  rootLabel: string;
  scope: SkillScope;
  projectLabel?: string;
}

interface SkillGroup {
  key: string;
  name: string;
  version?: string;
  description?: string;
  valid: boolean;
  isSymlink: boolean;
  origins: SkillOrigin[];
  rep: SkillSummary;
}

const groups = computed<SkillGroup[]>(() => {
  const map = new Map<string, SkillGroup>();
  for (const skill of skills.value) {
    const key = `${skill.name}\u0000${skill.contentHash}`;
    let group = map.get(key);
    if (!group) {
      group = {
        key,
        name: skill.name,
        version: skill.version,
        description: skill.description,
        valid: skill.valid,
        isSymlink: skill.isSymlink,
        origins: [],
        rep: skill,
      };
      map.set(key, group);
    }
    if (!group.origins.some((origin) => origin.rootId === skill.rootId)) {
      group.origins.push({
        agent: skill.agent,
        rootId: skill.rootId,
        rootLabel: skill.rootLabel,
        scope: skill.scope,
        projectLabel: skill.projectLabel,
      });
    }
  }
  const list = [...map.values()];
  for (const group of list) {
    group.origins.sort((a, b) => (a.agent ?? a.rootLabel).localeCompare(b.agent ?? b.rootLabel));
  }
  return list.sort((a, b) => a.name.localeCompare(b.name));
});

function groupAgents(group: SkillGroup): Array<{ agent?: string; label: string }> {
  const seen = new Set<string>();
  const list: Array<{ agent?: string; label: string }> = [];
  for (const origin of group.origins) {
    const label = agentStyle(origin.agent).label;
    if (seen.has(label)) continue;
    seen.add(label);
    list.push({ agent: origin.agent, label });
  }
  return list;
}

function isProblem(group: SkillGroup): boolean {
  return !group.rep.valid || group.rep.warnings.length > 0;
}

const problemCount = computed(() => groups.value.filter(isProblem).length);

const filteredGroups = computed(() => {
  const keyword = search.value.trim().toLowerCase();
  return groups.value.filter((group) => {
    if (issuesOnly.value && !isProblem(group)) return false;
    if (scopeFilter.value !== "all" && !group.origins.some((origin) => origin.scope === scopeFilter.value)) return false;
    if (rootFilter.value !== ALL_ROOTS && !group.origins.some((origin) => origin.rootId === rootFilter.value)) return false;
    if (projectFilter.value && !group.origins.some((origin) => projectRootIds.value.has(origin.rootId))) return false;
    if (agentFilter.value && !group.origins.some((origin) => origin.agent === agentFilter.value)) return false;
    if (!keyword) return true;
    return group.name.toLowerCase().includes(keyword) || (group.description ?? "").toLowerCase().includes(keyword);
  });
});

function rootGroupCount(rootId: string): number {
  return groups.value.filter((group) => group.origins.some((origin) => origin.rootId === rootId)).length;
}

const hasFilter = computed(
  () =>
    search.value.trim() !== "" ||
    rootFilter.value !== ALL_ROOTS ||
    scopeFilter.value !== "all" ||
    projectFilter.value !== "" ||
    agentFilter.value !== "" ||
    issuesOnly.value,
);

function clearFilter() {
  search.value = "";
  rootFilter.value = ALL_ROOTS;
  scopeFilter.value = "all";
  projectFilter.value = "";
  agentFilter.value = "";
  issuesOnly.value = false;
  void router.replace({ path: "/skills/library" });
}

// 新建 skill
const showCreate = ref(false);
const createForm = reactive({ name: "", description: "", rootId: "" });
const createError = ref("");
const writableRoots = computed(() => roots.value.filter((item) => item.writable));
const writableOptions = computed(() => writableRoots.value.map((item) => ({ label: item.label, value: item.id })));

function openCreate() {
  createError.value = "";
  createForm.name = "";
  createForm.description = "";
  createForm.rootId = writableRoots.value[0]?.id ?? "";
  showCreate.value = true;
}

async function submitCreate() {
  createError.value = "";
  try {
    const detail = await skillsApi.createSkill({
      name: createForm.name.trim(),
      description: createForm.description.trim(),
      rootId: createForm.rootId || writableRoots.value[0]?.id,
    });
    showCreate.value = false;
    await store.refresh();
    void router.push({ name: "skills-detail", params: { rootId: detail.rootId, name: detail.name } });
  } catch (caught) {
    createError.value = skillsErrorMessage(caught);
  }
}

async function reload() {
  await store.refresh();
  try {
    failures.value = await skillsApi.listIssues();
  } catch {
    failures.value = [];
  }
}

onMounted(reload);
</script>

<template>
  <UDashboardPanel id="skills-library">
    <template #header>
      <UDashboardNavbar title="技能库" :ui="{ right: 'gap-1.5' }">
        <template #leading>
          <UDashboardSidebarCollapse />
        </template>
        <template #trailing>
          <UBadge :label="String(groups.length)" color="neutral" variant="subtle" size="sm" class="rounded-full" />
        </template>
        <template #right>
          <UTooltip text="重新扫描根目录">
            <UButton
              icon="i-lucide-refresh-cw"
              color="neutral"
              variant="ghost"
              square
              :loading="loading"
              @click="reload"
            />
          </UTooltip>
          <UButton icon="i-lucide-folder-tree" color="neutral" variant="outline" to="/skills/roots">
            来源目录
          </UButton>
          <UButton icon="i-lucide-trash-2" color="neutral" variant="outline" to="/skills/trash">
            回收站
          </UButton>
          <UButton icon="i-lucide-plus" @click="openCreate">新建 Skill</UButton>
        </template>
      </UDashboardNavbar>

      <UDashboardToolbar>
        <template #left>
          <UInput v-model="search" icon="i-lucide-search" placeholder="搜索名称或描述…" class="w-56" />
          <WorkspaceFilter
            :model-value="workspaceValue"
            :root-count="rootGroupCount"
            @update:model-value="setWorkspace"
          />
          <USelect v-model="scopeFilter" :items="scopeItems" icon="i-lucide-layers" class="w-28" />
          <div class="inline-flex items-center gap-0.5 rounded-lg ring ring-default bg-elevated/40 p-0.5">
            <button
              type="button"
              class="rounded-md px-3 py-1 text-xs transition-colors cursor-pointer"
              :class="!issuesOnly ? 'bg-default text-highlighted font-medium shadow-sm' : 'text-muted hover:text-default'"
              @click="setIssuesOnly(false)"
            >
              全部
            </button>
            <button
              type="button"
              class="rounded-md px-3 py-1 text-xs transition-colors cursor-pointer"
              :class="issuesOnly ? 'bg-default text-highlighted font-medium shadow-sm' : 'text-muted hover:text-default'"
              @click="setIssuesOnly(true)"
            >
              有问题
              <span v-if="problemCount" :class="issuesOnly ? 'text-error' : 'text-dimmed'">{{ problemCount }}</span>
            </button>
          </div>
        </template>
        <template #right>
          <span class="hidden sm:inline text-xs text-dimmed tabular-nums">
            {{ filteredGroups.length }} / {{ groups.length }} 个技能
          </span>
        </template>
      </UDashboardToolbar>
    </template>

    <template #body>
      <UAlert v-if="error" color="error" variant="soft" icon="i-lucide-triangle-alert" :title="error" class="mb-4" />

      <section v-if="issuesOnly && failures.length" class="mb-6 space-y-2.5">
        <div class="flex items-center gap-1.5 text-[11px] font-semibold text-dimmed uppercase tracking-wider">
          <UIcon name="i-lucide-file-x" class="size-3.5" />
          无法解析的目录（{{ failures.length }}）
        </div>
        <div
          v-for="(failure, index) in failures"
          :key="`${failure.rootId}-${failure.dirName}-${index}`"
          class="rounded-lg ring ring-default bg-default p-4"
        >
          <div class="flex items-start gap-3">
            <span class="flex size-9 shrink-0 items-center justify-center rounded-lg bg-error/10 text-error ring-1 ring-error/15 ring-inset">
              <UIcon name="i-lucide-file-warning" class="size-4.5" />
            </span>
            <div class="min-w-0 flex-1">
              <div class="flex items-center gap-2 flex-wrap">
                <span class="text-sm font-semibold text-highlighted truncate font-mono">{{ failure.dirName }}</span>
                <UBadge color="neutral" variant="subtle" size="sm">{{ failure.rootLabel }}</UBadge>
              </div>
              <div class="mt-1 text-xs text-error">{{ failure.reason }}</div>
            </div>
          </div>
        </div>
      </section>

      <div v-if="loading && groups.length === 0" class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
        <UCard v-for="i in 6" :key="i" :ui="{ body: 'p-4 sm:p-5' }">
          <div class="flex items-center gap-3 mb-4">
            <USkeleton class="size-10 rounded-xl" />
            <div class="flex-1 space-y-2">
              <USkeleton class="h-3.5 w-1/2" />
              <USkeleton class="h-3 w-1/3" />
            </div>
          </div>
          <USkeleton class="h-3 w-full mb-2" />
          <USkeleton class="h-3 w-2/3" />
        </UCard>
      </div>

      <div v-else class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
        <RouterLink
          v-for="group in filteredGroups"
          :key="group.key"
          :to="{ name: 'skills-detail', params: { rootId: group.rep.rootId, name: group.rep.name } }"
          class="group focus:outline-none"
        >
          <UCard
            class="h-full transition-all duration-200 hover:shadow-lg hover:shadow-neutral-950/5 hover:ring-primary/40 group-focus-visible:ring-2 group-focus-visible:ring-primary"
            :ui="{ body: 'flex flex-col h-full p-4 sm:p-5' }"
          >
            <div class="flex items-start justify-between gap-2 mb-3">
              <div class="flex items-center gap-3 min-w-0">
                <span class="flex size-10 shrink-0 items-center justify-center rounded-xl bg-primary/10 text-primary ring-1 ring-primary/15 ring-inset">
                  <UIcon :name="group.isSymlink ? 'i-lucide-link' : 'i-lucide-puzzle'" class="size-5" />
                </span>
                <div class="min-w-0">
                  <div class="flex items-center gap-1.5 min-w-0">
                    <span class="font-semibold text-highlighted truncate group-hover:text-primary transition-colors">
                      {{ group.name }}
                    </span>
                    <UTooltip v-if="!group.valid" text="SKILL.md 校验未通过">
                      <UIcon name="i-lucide-circle-x" class="text-error size-4 shrink-0" />
                    </UTooltip>
                    <UTooltip v-else-if="group.rep.warnings.length" text="存在告警">
                      <UIcon name="i-lucide-triangle-alert" class="text-warning size-4 shrink-0" />
                    </UTooltip>
                  </div>
                  <div class="flex items-center gap-1.5 mt-1">
                    <UBadge v-if="group.version" color="neutral" variant="subtle" size="sm" class="font-mono">
                      v{{ group.version }}
                    </UBadge>
                    <UBadge color="neutral" variant="subtle" size="sm" icon="i-lucide-users">
                      {{ group.origins.length }} 处引用
                    </UBadge>
                    <UBadge v-if="group.isSymlink" color="info" variant="subtle" size="sm">软链接</UBadge>
                    <UBadge v-if="!group.valid" color="error" variant="subtle" size="sm">错误</UBadge>
                    <UBadge v-else-if="group.rep.warnings.length" color="warning" variant="subtle" size="sm">
                      告警 {{ group.rep.warnings.length }}
                    </UBadge>
                  </div>
                </div>
              </div>
              <UIcon
                name="i-lucide-arrow-up-right"
                class="size-4 text-dimmed shrink-0 opacity-0 -translate-x-1 translate-y-1 group-hover:opacity-100 group-hover:translate-x-0 group-hover:translate-y-0 transition-all duration-200"
              />
            </div>

            <p class="text-sm text-muted leading-relaxed line-clamp-2 flex-1">
              {{ group.description || "（无描述）" }}
            </p>

            <div class="mt-4 pt-3 border-t border-default flex items-center justify-between gap-2">
              <div class="flex items-center -space-x-1.5">
                <UTooltip v-for="a in groupAgents(group).slice(0, 6)" :key="a.label" :text="a.label">
                  <AgentAvatar :agent="a.agent" :size="24" />
                </UTooltip>
                <span
                  v-if="groupAgents(group).length > 6"
                  class="inline-flex size-6 items-center justify-center rounded-full bg-elevated ring-1 ring-inset ring-default text-[10px] font-medium text-dimmed"
                >
                  +{{ groupAgents(group).length - 6 }}
                </span>
              </div>
              <span class="font-medium text-primary text-xs opacity-0 group-hover:opacity-100 transition-opacity shrink-0">查看</span>
            </div>
          </UCard>
        </RouterLink>
      </div>

      <div v-if="!loading && filteredGroups.length === 0" class="flex flex-col items-center py-24">
        <span class="flex size-14 items-center justify-center rounded-2xl bg-elevated ring ring-default mb-4">
          <UIcon
            :name="issuesOnly ? 'i-lucide-shield-check' : hasFilter ? 'i-lucide-search-x' : 'i-lucide-package-open'"
            class="size-7 text-dimmed"
          />
        </span>
        <p class="text-sm font-medium text-highlighted mb-1">
          {{ issuesOnly ? "没有发现问题的技能" : hasFilter ? "没有匹配的技能" : "还没有任何技能" }}
        </p>
        <p class="text-xs text-muted mb-5">
          {{
            issuesOnly
              ? "所有 skill 均可正常解析且通过结构校验。"
              : hasFilter
                ? "试试调整搜索关键词或切换工作区。"
                : "请先在根目录下创建 skill，或稍后从远程仓库安装。"
          }}
        </p>
        <UButton v-if="hasFilter" color="neutral" variant="outline" size="sm" icon="i-lucide-x" @click="clearFilter">
          清空筛选
        </UButton>
        <template v-else>
          <UButton size="sm" icon="i-lucide-plus" @click="openCreate">新建 Skill</UButton>
          <UButton size="sm" color="neutral" variant="outline" icon="i-lucide-refresh-cw" @click="reload">
            重新扫描
          </UButton>
        </template>
      </div>
    </template>
  </UDashboardPanel>

  <UModal v-model:open="showCreate" title="新建 Skill" description="在可写根目录下创建一个带 SKILL.md 的空白 skill。">
    <template #body>
      <div class="space-y-4">
        <UFormField label="名称" required help="仅限字母、数字、- 和 _">
          <UInput v-model="createForm.name" placeholder="my-skill" class="w-full font-mono" @keyup.enter="submitCreate" />
        </UFormField>
        <UFormField label="描述">
          <UTextarea v-model="createForm.description" :rows="3" placeholder="这个 skill 是做什么的…" class="w-full" />
        </UFormField>
        <UFormField label="安装到根目录">
          <USelect v-model="createForm.rootId" :items="writableOptions" icon="i-lucide-folder-tree" class="w-full" />
        </UFormField>
        <UAlert v-if="createError" color="error" variant="soft" icon="i-lucide-triangle-alert" :title="createError" />
      </div>
    </template>
    <template #footer>
      <div class="flex justify-end gap-2 w-full">
        <UButton color="neutral" variant="ghost" @click="showCreate = false">取消</UButton>
        <UButton icon="i-lucide-plus" :disabled="!createForm.name.trim()" @click="submitCreate">创建</UButton>
      </div>
    </template>
  </UModal>
</template>
