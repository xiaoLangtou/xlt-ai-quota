<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { storeToRefs } from "pinia";
import type { McpServer, McpService, McpTransport, ParsedServer, PlanRequest, WriteTarget } from "@/types/mcp";
import { useMcpStore } from "@/stores/mcp";
import { useSkillsStore } from "@/stores/skills";
import { mcpApi, mcpErrorMessage } from "@/api/mcp";
import { pickDirectory, skillsApi } from "@/api/skills";
import { formatKeyValues, parseKeyValues, splitArgs } from "@/utils/mcp";
import AgentAvatar from "@/components/skills/AgentAvatar.vue";

const props = defineProps<{
  open: boolean;
  editing?: McpService | null;
  initialServer?: McpServer | null;
}>();

const emit = defineEmits<{
  "update:open": [value: boolean];
  submit: [request: PlanRequest];
}>();

const mcpStore = useMcpStore();
const skillsStore = useSkillsStore();
const { agents } = storeToRefs(mcpStore);
const { projects } = storeToRefs(skillsStore);

const isEditing = computed(() => Boolean(props.editing));

const tab = ref<"paste" | "manual">("paste");
const pasteText = ref("");
const parsed = ref<ParsedServer[]>([]);
const parsedIndex = ref(0);
const parseError = ref("");
const parsing = ref(false);

const form = reactive({
  name: "",
  transport: "stdio" as McpTransport,
  command: "",
  argsText: "",
  envText: "",
  url: "",
  headersText: "",
});

const selected = ref<Set<string>>(new Set());
const targetScope = ref<"global" | "project">("global");
const targetProjectId = ref("");
const pickingProject = ref(false);
const targetError = ref("");

const transportItems = [
  { label: "stdio（本地命令）", value: "stdio" },
  { label: "HTTP（远程）", value: "http" },
  { label: "SSE（远程）", value: "sse" },
];

function globalKey(agent: string): string {
  return `g|${agent}`;
}
function projectKey(agent: string, projectPath: string): string {
  return `p|${agent}|${projectPath}`;
}

function toggle(key: string, value: boolean) {
  const next = new Set(selected.value);
  if (value) next.add(key);
  else next.delete(key);
  selected.value = next;
}
function isSelected(key: string): boolean {
  return selected.value.has(key);
}

const projectTargets = computed(() =>
  projects.value.flatMap((project) =>
    agents.value
      .filter((agent) => agent.supportsProject)
      .map((agent) => ({
        key: projectKey(agent.id, project.path),
        agentLabel: agent.label,
        projectLabel: project.label,
      })),
  ),
);

const projectItems = computed(() =>
  projects.value.map((project) => ({
    label: project.label,
    value: project.id,
    icon: "i-lucide-folder-git-2",
  })),
);
const selectedProject = computed(() =>
  projects.value.find((project) => project.id === targetProjectId.value),
);
const availableTargetAgents = computed(() =>
  targetScope.value === "project"
    ? agents.value.filter((agent) => agent.supportsProject)
    : agents.value,
);

function setTargetScope(scope: "global" | "project") {
  if (targetScope.value === scope) return;
  targetScope.value = scope;
  targetProjectId.value = "";
  selected.value = new Set();
  targetError.value = "";
}

function setTargetProject(id: string) {
  targetProjectId.value = id;
  selected.value = new Set();
  targetError.value = "";
}

function newTargetKey(agent: string): string {
  if (targetScope.value === "global") return globalKey(agent);
  return selectedProject.value ? projectKey(agent, selectedProject.value.path) : "";
}

async function pickProject() {
  pickingProject.value = true;
  targetError.value = "";
  try {
    const path = await pickDirectory("选择项目根目录");
    if (!path) return;
    const project = await skillsApi.addProject(path);
    await skillsStore.refresh();
    setTargetProject(project.id);
  } catch (caught) {
    targetError.value = mcpErrorMessage(caught);
  } finally {
    pickingProject.value = false;
  }
}

function reset() {
  tab.value = "paste";
  pasteText.value = "";
  parsed.value = [];
  parsedIndex.value = 0;
  parseError.value = "";
  form.name = "";
  form.transport = "stdio";
  form.command = "";
  form.argsText = "";
  form.envText = "";
  form.url = "";
  form.headersText = "";
  targetScope.value = "global";
  targetProjectId.value = "";
  targetError.value = "";
  const next = new Set<string>();
  if (props.editing) {
    for (const instance of props.editing.instances) {
      if (instance.scope === "project" && instance.projectPath) {
        next.add(projectKey(instance.agent, instance.projectPath));
      } else {
        next.add(globalKey(instance.agent));
      }
    }
    const firstProjectInstance = props.editing.instances.find(
      (instance) => instance.scope === "project" && instance.projectPath,
    );
    if (firstProjectInstance && !props.editing.instances.some((instance) => instance.scope === "global")) {
      targetScope.value = "project";
      targetProjectId.value =
        projects.value.find((project) => project.path === firstProjectInstance.projectPath)?.id ?? "";
    }
  } else if (props.initialServer) {
    form.transport = props.initialServer.transport;
    for (const agent of agents.value.filter((item) => item.available)) next.add(globalKey(agent.id));
  } else {
    const first = agents.value.find((agent) => agent.available) ?? agents.value[0];
    if (first) next.add(globalKey(first.id));
  }
  selected.value = next;
}

function fillFromServer(server: McpServer) {
  form.name = server.name;
  form.transport = server.transport;
  form.command = server.command ?? "";
  form.argsText = server.args.join(" ");
  form.envText = formatKeyValues(server.env);
  form.url = server.url ?? "";
  form.headersText = formatKeyValues(server.headers);
  if (props.initialServer) {
    for (const agent of agents.value.filter((item) => item.available)) {
      selected.value = new Set(selected.value).add(globalKey(agent.id));
    }
  }
}

watch(
  () => props.open,
  async (open) => {
    if (!open) return;
    await Promise.all([
      mcpStore.loadAgents(),
      projects.value.length ? Promise.resolve() : skillsStore.refresh(),
    ]);
    reset();
    if (props.editing) {
      tab.value = "manual";
      fillFromServer({
        name: props.editing.name,
        transport: props.editing.transport,
        command: props.editing.command,
        args: props.editing.args,
        env: props.editing.env,
        url: props.editing.url,
        headers: props.editing.headers,
        extra: {},
      });
    } else if (props.initialServer) {
      tab.value = "manual";
      fillFromServer(props.initialServer);
    }
  },
);

async function runParse() {
  parsing.value = true;
  parseError.value = "";
  try {
    const result = await mcpApi.parsePaste(pasteText.value);
    parsed.value = result;
    parsedIndex.value = 0;
    if (!result.length) parseError.value = "未能识别任何 MCP 配置，请检查粘贴内容。";
  } catch (caught) {
    parseError.value = mcpErrorMessage(caught);
  } finally {
    parsing.value = false;
  }
}

function manualServer(): McpServer {
  const remote = form.transport !== "stdio";
  return {
    name: form.name.trim(),
    transport: form.transport,
    command: remote ? undefined : form.command.trim() || undefined,
    args: remote ? [] : splitArgs(form.argsText),
    env: remote ? {} : parseKeyValues(form.envText),
    url: remote ? form.url.trim() || undefined : undefined,
    headers: remote ? parseKeyValues(form.headersText) : {},
    extra: {},
  };
}

const activeServer = computed<McpServer | null>(() => {
  if (tab.value === "paste") return parsed.value[parsedIndex.value]?.server ?? null;
  return manualServer();
});

const targets = computed<WriteTarget[]>(() => {
  const list: WriteTarget[] = [];
  for (const key of selected.value) {
    const [kind, agent, projectPath] = key.split("|");
    if (!agent) continue;
    if (!isEditing.value) {
      if (targetScope.value === "global" && kind !== "g") continue;
      if (
        targetScope.value === "project" &&
        (kind !== "p" || projectPath !== selectedProject.value?.path)
      ) continue;
    }
    if (kind === "g") list.push({ agent, scope: "global" });
    else list.push({ agent, scope: "project", projectPath });
  }
  return list;
});

const validationError = computed(() => {
  const server = activeServer.value;
  if (!server) return "请先解析或填写服务配置";
  if (!server.name.trim()) return "服务名不能为空";
  if (server.transport === "stdio" && !server.command?.trim()) return "stdio 服务必须填写命令";
  if (server.transport !== "stdio" && !server.url?.trim()) return "远程服务必须填写 URL";
  if (!isEditing.value && targetScope.value === "project" && !selectedProject.value) return "请先选择项目";
  if (!targets.value.length) return "至少选择一个安装目标";
  return "";
});

const commandPreview = computed(() => {
  const server = activeServer.value;
  if (!server) return "";
  if (server.transport !== "stdio") return server.url ?? "";
  return [server.command, ...server.args].filter(Boolean).join(" ");
});

function close() {
  emit("update:open", false);
}

function submit() {
  const server = activeServer.value;
  if (!server || validationError.value) return;
  emit("submit", {
    kind: "upsert",
    server,
    name: server.name,
    targets: targets.value,
  });
}
</script>

<template>
  <USlideover
    :open="open"
    :title="isEditing ? `编辑 ${editing?.name}` : '添加 MCP 服务'"
    :description="isEditing ? '修改后需选择写入范围并预览确认。' : '粘贴 JSON / 命令，或手动填写；写入前会展示完整命令行。'"
    :ui="{ content: 'max-w-2xl' }"
    @update:open="!$event && close()"
  >
    <template #body>
      <div class="space-y-5">
        <div v-if="!isEditing" class="inline-flex w-full items-center gap-0.5 rounded-lg bg-elevated/40 p-0.5 ring ring-default">
          <button
            type="button"
            class="flex-1 rounded-md px-3 py-1.5 text-xs transition-colors cursor-pointer"
            :class="tab === 'paste' ? 'bg-default text-highlighted font-medium shadow-sm' : 'text-muted hover:text-default'"
            @click="tab = 'paste'"
          >
            粘贴 JSON / 命令
          </button>
          <button
            type="button"
            class="flex-1 rounded-md px-3 py-1.5 text-xs transition-colors cursor-pointer"
            :class="tab === 'manual' ? 'bg-default text-highlighted font-medium shadow-sm' : 'text-muted hover:text-default'"
            @click="tab = 'manual'"
          >
            手动填写
          </button>
        </div>

        <div v-if="tab === 'paste'" class="space-y-3">
          <UTextarea
            v-model="pasteText"
            :rows="7"
            class="w-full font-mono text-xs"
            placeholder='{"mcpServers":{"demo":{"command":"npx","args":["-y","pkg"]}}} 或 claude mcp add demo -- npx -y pkg'
          />
          <div class="flex items-center gap-2">
            <UButton size="sm" icon="i-lucide-wand-2" :loading="parsing" :disabled="!pasteText.trim()" @click="runParse">
              解析
            </UButton>
            <span class="text-xs text-muted">不会自动执行任何命令，仅解析为配置。</span>
          </div>
          <UAlert v-if="parseError" color="error" variant="soft" icon="i-lucide-triangle-alert" :title="parseError" />
          <div v-if="parsed.length" class="space-y-1.5">
            <p class="text-xs font-medium text-muted">识别到 {{ parsed.length }} 个服务</p>
            <button
              v-for="(item, index) in parsed"
              :key="`${item.server.name}-${index}`"
              type="button"
              class="flex w-full items-center justify-between gap-2 rounded-lg px-3 py-2 text-left ring ring-default transition-colors cursor-pointer"
              :class="index === parsedIndex ? 'bg-primary/10 ring-primary/40' : 'bg-elevated/30 hover:bg-elevated/60'"
              @click="parsedIndex = index"
            >
              <span class="min-w-0">
                <span class="block truncate text-sm font-medium text-highlighted">{{ item.server.name }}</span>
                <span class="block truncate font-mono text-[11px] text-dimmed">
                  {{ [item.server.command, ...item.server.args].filter(Boolean).join(" ") || item.server.url }}
                </span>
              </span>
              <UBadge color="neutral" variant="soft" size="sm">{{ item.source }}</UBadge>
            </button>
          </div>
        </div>

        <div v-else class="space-y-4">
          <div class="grid grid-cols-2 gap-3">
            <UFormField label="名称" required>
              <UInput
                v-model="form.name"
                :disabled="isEditing"
                placeholder="my-mcp"
                class="w-full font-mono"
              />
            </UFormField>
            <UFormField label="类型">
              <USelect v-model="form.transport" :items="transportItems" class="w-full" />
            </UFormField>
          </div>

          <template v-if="form.transport === 'stdio'">
            <UFormField label="命令" required>
              <UInput v-model="form.command" placeholder="npx / uvx / docker" class="w-full font-mono" />
            </UFormField>
            <UFormField label="参数" help="空格分隔，含空格的参数用引号包裹">
              <UInput v-model="form.argsText" placeholder="-y @modelcontextprotocol/server-filesystem /path" class="w-full font-mono" />
            </UFormField>
            <UFormField label="环境变量" help="每行 KEY=VALUE；值会以明文写入配置文件">
              <UTextarea v-model="form.envText" :rows="3" placeholder="API_KEY=xxxx" class="w-full font-mono text-xs" />
            </UFormField>
          </template>

          <template v-else>
            <UFormField label="URL" required>
              <UInput v-model="form.url" placeholder="https://example.com/mcp" class="w-full font-mono" />
            </UFormField>
            <UFormField label="请求头" help="每行 KEY=VALUE；值会以明文写入配置文件">
              <UTextarea v-model="form.headersText" :rows="3" placeholder="Authorization=Bearer xxxx" class="w-full font-mono text-xs" />
            </UFormField>
          </template>
        </div>

        <USeparator />

        <div class="space-y-3">
          <p class="text-xs font-semibold text-highlighted">安装目标</p>
          <template v-if="!isEditing">
            <div class="grid grid-cols-2 gap-2">
              <button
                type="button"
                class="flex items-center gap-2 rounded-lg px-3 py-2.5 text-left ring ring-default transition-colors cursor-pointer"
                :class="targetScope === 'global' ? 'bg-primary/10 ring-primary/40' : 'bg-elevated/30 hover:bg-elevated/60'"
                @click="setTargetScope('global')"
              >
                <UIcon name="i-lucide-user-round" class="size-4 shrink-0" />
                <span>
                  <span class="block text-sm font-medium text-highlighted">全局</span>
                  <span class="block text-[11px] text-dimmed">写入用户级配置</span>
                </span>
              </button>
              <button
                type="button"
                class="flex items-center gap-2 rounded-lg px-3 py-2.5 text-left ring ring-default transition-colors cursor-pointer"
                :class="targetScope === 'project' ? 'bg-primary/10 ring-primary/40' : 'bg-elevated/30 hover:bg-elevated/60'"
                @click="setTargetScope('project')"
              >
                <UIcon name="i-lucide-folder-git-2" class="size-4 shrink-0" />
                <span>
                  <span class="block text-sm font-medium text-highlighted">项目级</span>
                  <span class="block text-[11px] text-dimmed">写入指定项目</span>
                </span>
              </button>
            </div>

            <div v-if="targetScope === 'project'" class="space-y-2 rounded-lg bg-elevated/25 p-3 ring ring-default">
              <div class="flex items-center justify-between gap-2">
                <p class="text-xs font-medium text-highlighted">1. 选择项目</p>
                <span class="text-[11px] text-dimmed">也可从访达添加</span>
              </div>
              <div class="flex items-center gap-2">
                <USelect
                  :model-value="targetProjectId"
                  :items="projectItems"
                  placeholder="选择已登记项目"
                  class="min-w-0 flex-1"
                  @update:model-value="setTargetProject(String($event ?? ''))"
                />
                <UButton
                  icon="i-lucide-folder-open"
                  color="neutral"
                  variant="soft"
                  :loading="pickingProject"
                  class="shrink-0"
                  @click="pickProject"
                >
                  浏览选择
                </UButton>
              </div>
              <p v-if="selectedProject" class="truncate font-mono text-[11px] text-dimmed" :title="selectedProject.path">
                {{ selectedProject.path }}
              </p>
            </div>

            <div class="space-y-2">
              <div class="flex items-center justify-between gap-2">
                <p class="text-xs font-medium text-highlighted">
                  {{ targetScope === 'project' ? '2.' : '1.' }} 选择 AI 工具
                </p>
                <span class="text-[11px] text-dimmed">可多选</span>
              </div>
              <div
                v-if="targetScope === 'project' && !selectedProject"
                class="rounded-lg border border-dashed border-default px-3 py-5 text-center text-xs text-dimmed"
              >
                选择项目后即可选择支持项目级配置的 AI 工具
              </div>
              <div v-else class="grid grid-cols-1 gap-1.5 sm:grid-cols-2">
                <label
                  v-for="agent in availableTargetAgents"
                  :key="agent.id"
                  class="flex items-center gap-2.5 rounded-lg px-3 py-2 ring ring-default cursor-pointer"
                >
                  <AgentAvatar :agent="agent.label" :size="24" />
                  <span class="min-w-0 flex-1">
                    <span class="block truncate text-sm text-highlighted">{{ agent.label }}</span>
                    <span class="block truncate text-[11px] text-dimmed">
                      {{ targetScope === 'global' ? agent.globalPath : '项目级配置' }}
                    </span>
                  </span>
                  <UCheckbox
                    :model-value="isSelected(newTargetKey(agent.id))"
                    @update:model-value="toggle(newTargetKey(agent.id), Boolean($event))"
                  />
                </label>
              </div>
            </div>
          </template>

          <div v-else class="space-y-1.5">
            <label
              v-for="agent in agents"
              :key="agent.id"
              class="flex items-center justify-between gap-3 rounded-lg px-3 py-2 ring ring-default"
            >
              <span class="flex min-w-0 items-center gap-2.5">
                <AgentAvatar :agent="agent.label" :size="24" />
                <span class="min-w-0">
                  <span class="block text-sm text-highlighted">{{ agent.label }}</span>
                  <span class="block truncate font-mono text-[11px] text-dimmed">{{ agent.globalPath }}</span>
                </span>
              </span>
              <UCheckbox
                :model-value="isSelected(globalKey(agent.id))"
                label="全局"
                @update:model-value="toggle(globalKey(agent.id), Boolean($event))"
              />
            </label>
          </div>

          <UCollapsible v-if="isEditing && projectTargets.length">
            <UButton label="项目级目标" color="neutral" variant="ghost" size="xs" trailing-icon="i-lucide-chevron-down" />
            <template #content>
              <div class="mt-2 grid grid-cols-2 gap-1.5">
                <label
                  v-for="target in projectTargets"
                  :key="target.key"
                  class="flex items-center gap-2 rounded-md px-2 py-1.5 text-xs ring ring-default"
                >
                  <UCheckbox
                    :model-value="isSelected(target.key)"
                    @update:model-value="toggle(target.key, Boolean($event))"
                  />
                  <AgentAvatar :agent="target.agentLabel" :size="18" />
                  <span class="truncate">{{ target.agentLabel }} · {{ target.projectLabel }}</span>
                </label>
              </div>
            </template>
          </UCollapsible>
          <UAlert v-if="targetError" color="error" variant="soft" icon="i-lucide-triangle-alert" :title="targetError" />
        </div>

        <UAlert
          v-if="commandPreview"
          color="warning"
          variant="subtle"
          icon="i-lucide-shield-alert"
          title="将执行第三方代码"
        >
          <p class="mt-1 break-all font-mono text-xs">{{ commandPreview }}</p>
          <p class="mt-1 text-xs text-muted">env / headers 会以明文写入 Agent 配置文件。</p>
        </UAlert>

        <UAlert v-if="validationError" color="error" variant="soft" icon="i-lucide-triangle-alert" :title="validationError" />
      </div>
    </template>

    <template #footer>
      <div class="flex w-full justify-end gap-2">
        <UButton color="neutral" variant="ghost" @click="close">取消</UButton>
        <UButton icon="i-lucide-file-diff" :disabled="Boolean(validationError)" @click="submit">
          生成变更预览
        </UButton>
      </div>
    </template>
  </USlideover>
</template>
