<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import type { McpPackage, McpServer, ReadmeResult, Runtime } from "@/types/mcp";
import { mcpApi, mcpErrorMessage } from "@/api/mcp";
import { pushToast } from "@/composables/useToast";
import { formatDate, formatStars, packageRiskLabel, serverFromRuntime, sourceLabel } from "@/utils/mcp";

const props = defineProps<{
  open: boolean;
  item: McpPackage | null;
}>();

const emit = defineEmits<{
  "update:open": [value: boolean];
  install: [server: McpServer];
}>();

const runtimeIndex = ref(0);
const envValues = reactive<Record<string, string>>({});
const readme = ref<ReadmeResult | null>(null);
const readmeLoading = ref(false);

const install = computed(() => props.item?.install ?? { level: "manual" as const });
const runtimes = computed<Runtime[]>(() =>
  install.value.level === "ready" ? install.value.runtimes : [],
);
const envSpec = computed(() =>
  install.value.level === "ready" ? install.value.envSpec : [],
);
const currentRuntime = computed<Runtime | null>(() => runtimes.value[runtimeIndex.value] ?? null);

const missingEnv = computed(() =>
  envSpec.value.filter((variable) => variable.required && !envValues[variable.name]?.trim()),
);

async function loadReadme() {
  if (!props.item) return;
  readmeLoading.value = true;
  try {
    readme.value = await mcpApi.getReadme(props.item.id);
  } catch (caught) {
    readme.value = {
      available: false,
      text: "",
      snippets: [],
      error: mcpErrorMessage(caught),
    };
  } finally {
    readmeLoading.value = false;
  }
}

watch(
  () => props.open,
  (open) => {
    if (!open || !props.item) return;
    runtimeIndex.value = 0;
    for (const key of Object.keys(envValues)) delete envValues[key];
    if (props.item.install.level === "ready") {
      for (const variable of props.item.install.envSpec) envValues[variable.name] = "";
    }
    readme.value = null;
    if (props.item.install.level === "manual") void loadReadme();
  },
);

function close() {
  emit("update:open", false);
}

function doInstall() {
  if (!props.item || !currentRuntime.value) return;
  if (missingEnv.value.length) {
    pushToast(`请填写：${missingEnv.value.map((item) => item.name).join("、")}`, "warn");
    return;
  }
  const env: Record<string, string> = {};
  for (const [key, value] of Object.entries(envValues)) {
    if (value.trim()) env[key] = value.trim();
  }
  emit("install", serverFromRuntime(props.item.name, currentRuntime.value, env));
}

function installSnippet(server: McpServer) {
  emit("install", server);
}

function runtimeLabel(runtime: Runtime): string {
  if (runtime.type === "remote") return "远程连接";
  return `${runtime.type} · ${[runtime.command, ...(runtime.args ?? [])].filter(Boolean).join(" ")}`;
}
</script>

<template>
  <USlideover
    :open="open"
    :title="item?.name ?? 'MCP 详情'"
    :description="item?.description"
    :ui="{ content: 'max-w-2xl' }"
    @update:open="!$event && close()"
  >
    <template #body>
      <div v-if="item" class="space-y-5">
        <div class="flex flex-wrap items-center gap-1.5">
          <UBadge color="primary" variant="subtle" size="sm">{{ sourceLabel(item.source) }}</UBadge>
          <UBadge v-if="item.verified" color="success" variant="subtle" size="sm" icon="i-lucide-badge-check">已验证</UBadge>
          <UBadge v-if="item.mirror" color="neutral" variant="soft" size="sm">镜像</UBadge>
          <UBadge v-if="item.language" color="neutral" variant="soft" size="sm">{{ item.language.name }}</UBadge>
          <UBadge v-if="item.license" color="neutral" variant="outline" size="sm">{{ item.license }}</UBadge>
          <span class="ml-auto inline-flex items-center gap-2 text-xs text-muted">
            <span class="inline-flex items-center gap-1"><UIcon name="i-lucide-star" class="size-3.5" />{{ formatStars(item.stars) }}</span>
            <span>{{ formatDate(item.updatedAt) }}</span>
          </span>
        </div>

        <div class="flex flex-wrap gap-1.5">
          <UBadge v-for="tag in item.tags" :key="tag" color="neutral" variant="soft" size="sm">{{ tag }}</UBadge>
          <UBadge v-for="platform in item.platforms" :key="platform" color="neutral" variant="outline" size="sm">{{ platform }}</UBadge>
        </div>

        <UAlert
          color="warning"
          variant="subtle"
          icon="i-lucide-shield-alert"
          title="安装 MCP = 执行第三方代码"
          :description="`来源等级：${packageRiskLabel(item)}。命令会完整展示，需你确认后才会写入配置；应用永不自动执行。`"
        />

        <a
          v-if="item.homepage"
          :href="item.homepage"
          target="_blank"
          rel="noreferrer noopener"
          class="inline-flex items-center gap-1.5 text-xs text-primary hover:underline"
        >
          <UIcon name="i-lucide-external-link" class="size-3.5" />
          {{ item.homepage }}
        </a>

        <USeparator />

        <template v-if="install.level === 'ready'">
          <div class="space-y-3">
            <p class="text-xs font-semibold text-highlighted">选择运行方式</p>
            <label
              v-for="(runtime, index) in runtimes"
              :key="index"
              class="flex cursor-pointer items-start gap-2.5 rounded-lg px-3 py-2.5 ring ring-default transition-colors"
              :class="index === runtimeIndex ? 'bg-primary/10 ring-primary/40' : 'bg-elevated/30 hover:bg-elevated/60'"
            >
              <input v-model.number="runtimeIndex" type="radio" :value="index" class="mt-1" />
              <span class="min-w-0">
                <span class="block text-sm text-highlighted">{{ runtime.type }}</span>
                <span class="block break-all font-mono text-[11px] text-dimmed">{{ runtimeLabel(runtime) }}</span>
              </span>
            </label>
          </div>

          <div v-if="envSpec.length" class="space-y-3">
            <p class="text-xs font-semibold text-highlighted">需要的环境变量</p>
            <UFormField
              v-for="variable in envSpec"
              :key="variable.name"
              :label="variable.name"
              :help="variable.description"
              :required="variable.required"
            >
              <UInput
                v-model="envValues[variable.name]"
                :type="variable.secret ? 'password' : 'text'"
                class="w-full font-mono"
                :placeholder="variable.secret ? '值会以明文写入配置文件' : ''"
              />
            </UFormField>
          </div>
        </template>

        <template v-else>
          <div class="space-y-3">
            <div class="flex items-center justify-between">
              <p class="text-xs font-semibold text-highlighted">README 中的安装方式</p>
              <UButton size="xs" color="neutral" variant="ghost" :loading="readmeLoading" icon="i-lucide-refresh-cw" @click="loadReadme">
                重新解析
              </UButton>
            </div>
            <div v-if="readmeLoading" class="flex items-center gap-2 text-xs text-muted">
              <UIcon name="i-lucide-loader-circle" class="size-4 animate-spin" /> 正在获取 README…
            </div>
            <template v-else-if="readme">
              <UAlert
                v-if="!readme.available"
                color="neutral"
                variant="subtle"
                icon="i-lucide-file-question"
                :title="readme.error ?? 'README 不可用'"
              />
              <template v-else>
                <div v-if="readme.snippets.length" class="space-y-1.5">
                  <button
                    v-for="(snippet, index) in readme.snippets"
                    :key="index"
                    type="button"
                    class="flex w-full items-center justify-between gap-2 rounded-lg px-3 py-2 text-left ring ring-default transition-colors cursor-pointer hover:bg-elevated/60"
                    @click="installSnippet(snippet.server)"
                  >
                    <span class="min-w-0">
                      <span class="block text-sm font-medium text-highlighted">{{ snippet.server.name }}</span>
                      <span class="block truncate font-mono text-[11px] text-dimmed">
                        {{ [snippet.server.command, ...snippet.server.args].filter(Boolean).join(" ") || snippet.server.url }}
                      </span>
                    </span>
                    <UBadge color="primary" variant="soft" size="sm">送到添加抽屉</UBadge>
                  </button>
                </div>
                <p v-else class="text-xs text-muted">未从 README 中识别出安装命令，请手动配置。</p>

                <div class="max-h-72 overflow-auto rounded-lg bg-elevated/30 p-3">
                  <pre class="whitespace-pre-wrap text-[11px] leading-relaxed text-muted">{{ readme.text }}</pre>
                </div>
              </template>
            </template>
          </div>
        </template>
      </div>
    </template>

    <template #footer>
      <div class="flex w-full justify-end gap-2">
        <UButton color="neutral" variant="ghost" @click="close">关闭</UButton>
        <UButton
          v-if="install.level === 'ready'"
          icon="i-lucide-download"
          :disabled="!currentRuntime"
          @click="doInstall"
        >
          安装到…
        </UButton>
      </div>
    </template>
  </USlideover>
</template>
