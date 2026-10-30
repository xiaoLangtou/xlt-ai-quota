<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import {
  Activity,
  Archive,
  AlertTriangle,
  CheckCircle2,
  Copy,
  ExternalLink,
  FileCode2,
  FileText,
  Pencil,
  Plus,
  RefreshCw,
  Route,
  Search,
  Terminal,
  Trash2,
  X,
} from "lucide-vue-next";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { DialogDescription, DialogTitle } from "reka-ui";
import { Input } from "@/components/ui/input";
import CodeEditor from "@/components/skills/CodeEditor.vue";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { pushToast } from "@/composables/useToast";
import { clipboardService } from "@/services/clipboard-service";
import { shellEnvService } from "@/services/shell-env-service";
import {
  SHELL_RELOAD_COMMAND,
  type AliasEntry,
  type EnvEntry,
  type ShellBackup,
  type ShellChangePlan,
  type ShellChangeRequest,
  type ShellCommitResult,
  type ShellPathReport,
  type ShellSnapshot,
  type ShellSourceMap,
  type ShellValidateResult,
} from "@/types/shell";

// ---------------------------------------------------------------------------
// 快照状态（UI 数据源）
// ---------------------------------------------------------------------------

const snapshot = ref<ShellSnapshot | null>(null);
const validation = ref<ShellValidateResult | null>(null);
const loading = ref(false);
const activeTab = ref<"alias" | "env" | "path" | "diagnostics" | "source">("env");

const aliasFile = computed(() => snapshot.value?.alias);
const zshrcFile = computed(() => snapshot.value?.zshrc);
const aliases = computed(() => snapshot.value?.alias.aliases ?? []);
const envs = computed(() => snapshot.value?.zshrc.envs ?? []);
const managedEnvs = computed(() => envs.value.filter((entry) => entry.managed));
const readonlyEnvs = computed(() => envs.value.filter((entry) => !entry.managed));
const aliasGroupCount = computed(() => new Set(aliases.value.map((entry) => entry.group ?? "")).size);

const aliasQuery = ref("");
const envQuery = ref("");
/** 当前 Tab 的搜索词（仅别名 / 变量两个表格有搜索框） */
const activeQuery = computed({
  get: () => (activeTab.value === "alias" ? aliasQuery.value : activeTab.value === "env" ? envQuery.value : ""),
  set: (value) => {
    if (activeTab.value === "alias") aliasQuery.value = value;
    else if (activeTab.value === "env") envQuery.value = value;
  },
});

const filteredAliases = computed(() => {
  const query = aliasQuery.value.trim().toLowerCase();
  if (!query) return aliases.value;
  return aliases.value.filter((entry) =>
    [entry.name, entry.value, entry.group ?? ""].some((field) => field.toLowerCase().includes(query)),
  );
});

const filteredManagedEnvs = computed(() => {
  const query = envQuery.value.trim().toLowerCase();
  if (!query) return managedEnvs.value;
  return managedEnvs.value.filter((entry) =>
    [entry.key, entry.value].some((field) => field.toLowerCase().includes(query)),
  );
});

const filteredReadonlyEnvs = computed(() => {
  const query = envQuery.value.trim().toLowerCase();
  if (!query) return readonlyEnvs.value;
  return readonlyEnvs.value.filter((entry) =>
    [entry.key, entry.value].some((field) => field.toLowerCase().includes(query)),
  );
});

async function loadAll(): Promise<void> {
  loading.value = true;
  try {
    snapshot.value = await shellEnvService.snapshot();
    validation.value = await shellEnvService.validate();
  } catch (reason) {
    pushToast(reason instanceof Error ? reason.message : String(reason), "error");
  } finally {
    loading.value = false;
  }
}

// ---------------------------------------------------------------------------
// R1 只读来源模型：来源地图 + PATH 报告（不产生任何写操作）
// ---------------------------------------------------------------------------

const sourceMap = ref<ShellSourceMap | null>(null);
const pathReport = ref<ShellPathReport | null>(null);
/** 用户主动勾选后才读取 .zshenv / .zprofile / /etc/paths / /etc/paths.d（全部只读） */
const includeExtended = ref(false);
const sourceModelLoading = ref(false);

async function loadSourceModel(): Promise<void> {
  sourceModelLoading.value = true;
  try {
    const [map, report] = await Promise.all([
      shellEnvService.sourceMap(includeExtended.value),
      shellEnvService.pathReport(includeExtended.value),
    ]);
    sourceMap.value = map;
    pathReport.value = report;
  } catch (reason) {
    pushToast(reason instanceof Error ? reason.message : String(reason), "error");
  } finally {
    sourceModelLoading.value = false;
  }
}

watch(includeExtended, loadSourceModel);

/** PATH 条目分类的中文标签 */
const PATH_KIND_LABELS: Record<string, string> = {
  literal: "字面量",
  "home-relative": "主目录相对",
  "variable-expression": "变量表达式",
  dynamic: "动态",
  relative: "相对路径",
  empty: "空条目",
  reference: "引用点",
  system: "系统",
};

/** PATH 修改模式的中文标签 */
const PATH_MODE_LABELS: Record<string, string> = {
  prepend: "前置",
  append: "追加",
  replace: "替换",
};

/** 上下文的中文标签 */
const CONTEXT_LABELS: Record<string, string> = {
  static: "静态",
  dynamic: "动态",
  system: "系统",
};

function pathKindLabel(kind: string): string {
  return PATH_KIND_LABELS[kind] ?? kind;
}

function pathModeLabel(mode: string): string {
  return PATH_MODE_LABELS[mode] ?? mode;
}

function contextLabel(context: string): string {
  return CONTEXT_LABELS[context] ?? context;
}

function ownerLabel(owner: string): string {
  return owner === "unknown" ? "未知" : owner;
}

/** 状态栏展示用短路径：/Users/<name> 归一为 ~（完整路径见 title） */
function shortPath(path: string | undefined): string {
  if (!path) return "";
  return path.replace(/^\/Users\/[^/]+(?=\/|$)/, "~");
}

/** 诊断总数（PATH 诊断 + 来源地图诊断） */
const diagnosticsCount = computed(
  () => (pathReport.value?.diagnostics.length ?? 0) + (sourceMap.value?.diagnostics.length ?? 0),
);

/** 打开来源位置：注册文件跳源码视图，其余用系统方式打开（只读） */
async function openSourceAt(location: string): Promise<void> {
  const separator = location.lastIndexOf(":");
  const path = location.slice(0, separator);
  const line = Number(location.slice(separator + 1));
  if (path === aliasFile.value?.path) jumpToSource("alias", line);
  else if (path === zshrcFile.value?.path) jumpToSource("zshrc", line);
  else {
    try {
      await shellEnvService.openFile(path);
    } catch (reason) {
      pushToast(reason instanceof Error ? reason.message : String(reason), "error");
    }
  }
}

onMounted(() => {
  void loadAll();
  void loadSourceModel();
});

// 页面离开主动取消未提交的计划，避免计划存储滞留至 TTL 过期
onBeforeUnmount(() => {
  const plan = pendingPlan.value;
  if (plan) void shellEnvService.cancelChange(plan.id).catch(() => {});
});

// ---------------------------------------------------------------------------
// 变更计划流程：preview → 差异确认 → commit（B03 / B09 / B10）
// ---------------------------------------------------------------------------

const pendingPlan = ref<ShellChangePlan | null>(null);
const confirmOpen = ref(false);
const committing = ref(false);
/** 本次变更的会话命令（保存后展示，用户手动复制；不默认自动复制） */
const lastSessionCommand = ref("");

/** 源码视图脏状态与目标文件互斥（B10）：脏源码期间禁止同文件其他写操作 */
function isSourceDirtyFor(kind: "alias" | "zshrc"): boolean {
  return sourceDirty.value && sourceFile.value === kind;
}

async function runChange(request: ShellChangeRequest): Promise<void> {
  // 目标文件类型：alias_* / .alias 备份 → alias，其余 → zshrc
  const kind =
    request.type === "restore_backup"
      ? request.backupId.startsWith(".alias.")
        ? "alias"
        : "zshrc"
      : request.type.startsWith("alias")
        ? "alias"
        : "zshrc";
  if (isSourceDirtyFor(kind)) {
    pushToast("源码有未保存修改，请先保存或重新加载后再执行该操作。", "warn");
    return;
  }
  let plan: ShellChangePlan;
  try {
    plan = await shellEnvService.previewChange(request);
  } catch (reason) {
    const message = reason instanceof Error ? reason.message : String(reason);
    if (message === "BLOCK_CREATE_CONFIRM_REQUIRED") {
      blockCreateOpen.value = true;
      return;
    }
    pushToast(message, "error");
    return;
  }
  pendingPlan.value = plan;
  confirmOpen.value = true;
}

function versionOf(kind: "alias" | "zshrc"): string {
  const file = kind === "alias" ? aliasFile.value : zshrcFile.value;
  if (!file) throw new Error("快照尚未加载");
  return file.version;
}

function applyCommitResult(result: ShellCommitResult): void {
  if (!snapshot.value) return;
  if (result.kind === "alias") snapshot.value.alias = result.snapshot;
  else snapshot.value.zshrc = result.snapshot;
  // 源码 Tab 同步到提交后的内容（清除脏状态）
  if (sourceFile.value === result.kind) {
    sourceSavedContent.value = result.content;
    sourceContent.value = result.content;
  }
  lastSessionCommand.value = result.sessionCommand;
}

async function confirmCommit(): Promise<void> {
  const plan = pendingPlan.value;
  if (!plan) return;
  committing.value = true;
  try {
    const result = await shellEnvService.commitChange(plan.id, versionOf(plan.kind));
    applyCommitResult(result);
    confirmOpen.value = false;
    pendingPlan.value = null;
    pushToast("已写入磁盘，新开终端自动生效。", "success");
    validation.value = await shellEnvService.validate();
  } catch (reason) {
    // 计划为单次消费：失败后需重新预览
    confirmOpen.value = false;
    pendingPlan.value = null;
    pushToast(reason instanceof Error ? reason.message : String(reason), "error", 6000);
  } finally {
    committing.value = false;
  }
}

function cancelConfirm(): void {
  const plan = pendingPlan.value;
  confirmOpen.value = false;
  pendingPlan.value = null;
  if (plan) void shellEnvService.cancelChange(plan.id).catch(() => {});
}

async function copyCommand(text: string | undefined): Promise<void> {
  if (!text) return;
  // R0-04：优先走 clipboardService（桌面端原生剪贴板）；纯浏览器 dev 环境回退 navigator
  try {
    await clipboardService.writeText(text);
    pushToast("命令已复制。", "success");
  } catch {
    try {
      await navigator.clipboard.writeText(text);
      pushToast("命令已复制。", "success");
    } catch {
      pushToast("复制失败，请手动选择文本复制。", "error");
    }
  }
}

// ---------------------------------------------------------------------------
// 别名
// ---------------------------------------------------------------------------

const aliasDialogOpen = ref(false);
const aliasEditing = ref<AliasEntry | null>(null);
const aliasRenameMode = ref(false);
const aliasForm = ref<{ name: string; value: string; group: string | null; description: string }>({
  name: "",
  value: "",
  group: null,
  description: "",
});
const aliasSaving = ref(false);
const aliasDeleteTarget = ref<AliasEntry | null>(null);

// 分组选择：输入过滤已有分组，点选填充；不匹配时可手输新分组（后端自动创建组头）
const groupPickerOpen = ref(false);
const existingGroups = computed(() =>
  [...new Set(aliases.value.map((entry) => entry.group ?? "").filter(Boolean))].sort((a, b) =>
    a.localeCompare(b, "zh-Hans-CN"),
  ),
);
const groupSuggestions = computed(() => {
  const query = aliasForm.value.group?.trim().toLowerCase() ?? "";
  if (!query) return existingGroups.value;
  return existingGroups.value.filter((group) => group.toLowerCase().includes(query));
});
const groupIsNew = computed(() => {
  const group = aliasForm.value.group?.trim();
  return !!group && !existingGroups.value.includes(group);
});

function pickGroup(group: string): void {
  aliasForm.value.group = group;
  groupPickerOpen.value = false;
}

function openCreateAlias(): void {
  aliasEditing.value = null;
  aliasRenameMode.value = false;
  aliasForm.value = {
    name: "",
    value: "",
    group: aliases.value.at(-1)?.group ?? null,
    description: "",
  };
  aliasDialogOpen.value = true;
}

function openEditAlias(entry: AliasEntry): void {
  aliasEditing.value = entry;
  aliasRenameMode.value = false;
  aliasForm.value = {
    name: entry.name,
    value: entry.value,
    group: entry.group,
    description: entry.description ?? "",
  };
  aliasDialogOpen.value = true;
}

function startAliasRename(): void {
  if (!aliasEditing.value) return;
  aliasRenameMode.value = true;
  aliasForm.value.name = aliasEditing.value.name;
}

async function saveAlias(): Promise<void> {
  aliasSaving.value = true;
  try {
    const description = aliasForm.value.description.trim() || null;
    let request: ShellChangeRequest;
    if (aliasEditing.value && aliasRenameMode.value) {
      request = { type: "alias_rename", entryId: aliasEditing.value.id, newName: aliasForm.value.name };
    } else if (aliasEditing.value) {
      request = {
        type: "alias_edit",
        entryId: aliasEditing.value.id,
        value: aliasForm.value.value,
        description,
      };
    } else {
      request = {
        type: "alias_add",
        name: aliasForm.value.name,
        value: aliasForm.value.value,
        group: aliasForm.value.group,
        description,
      };
    }
    aliasDialogOpen.value = false;
    await runChange(request);
  } finally {
    aliasSaving.value = false;
  }
}

function askDeleteAlias(entry: AliasEntry): void {
  aliasDeleteTarget.value = entry;
}

async function deleteAlias(): Promise<void> {
  const target = aliasDeleteTarget.value;
  if (!target) return;
  aliasDeleteTarget.value = null;
  await runChange({ type: "alias_delete", entryId: target.id });
}

// ---------------------------------------------------------------------------
// 环境变量
// ---------------------------------------------------------------------------

const envDialogOpen = ref(false);
const envEditing = ref<EnvEntry | null>(null);
const envForm = ref<{ key: string; value: string }>({ key: "", value: "" });
const envSaving = ref(false);
const envDeleteTarget = ref<EnvEntry | null>(null);
/** 首次创建托管区块的确认弹窗 */
const blockCreateOpen = ref(false);

function openCreateEnv(): void {
  envEditing.value = null;
  envForm.value = { key: "", value: "" };
  envDialogOpen.value = true;
}

function openEditEnv(entry: EnvEntry): void {
  envEditing.value = entry;
  envForm.value = { key: entry.key, value: entry.value };
  envDialogOpen.value = true;
}

async function saveEnv(): Promise<void> {
  envSaving.value = true;
  try {
    const request: ShellChangeRequest = envEditing.value
      ? { type: "env_set", key: envEditing.value.key, value: envForm.value.value }
      : { type: "env_set", key: envForm.value.key, value: envForm.value.value };
    envDialogOpen.value = false;
    await runChange(request);
  } finally {
    envSaving.value = false;
  }
}

/** 区块不存在时的确认弹窗 → 携带 confirmBlockCreate 重新预览 */
async function confirmBlockCreate(): Promise<void> {
  blockCreateOpen.value = false;
  await runChange({ type: "env_set", key: envForm.value.key, value: envForm.value.value, confirmBlockCreate: true });
}

function askDeleteEnv(entry: EnvEntry): void {
  envDeleteTarget.value = entry;
}

async function deleteEnv(): Promise<void> {
  const target = envDeleteTarget.value;
  if (!target) return;
  envDeleteTarget.value = null;
  await runChange({ type: "env_delete", entryId: target.id });
}

async function normalizeBlock(): Promise<void> {
  await runChange({ type: "block_normalize" });
}

// ---------------------------------------------------------------------------
// 源码视图
// ---------------------------------------------------------------------------

const sourceFile = ref<"alias" | "zshrc">("alias");
const sourceContent = ref("");
const sourceSavedContent = ref("");
const sourceSaving = ref(false);
const scrollTarget = ref<number | undefined>(undefined);
const sourceDirty = computed(() => sourceContent.value !== sourceSavedContent.value);

function syncSourceFromSnapshot(): void {
  const file = sourceFile.value === "alias" ? aliasFile.value : zshrcFile.value;
  sourceSavedContent.value = file?.content ?? "";
  sourceContent.value = file?.content ?? "";
}

// 每次切到源码 Tab 都从最新快照加载；有未保存修改时保留并提示
watch(activeTab, (tab) => {
  if (tab === "source") {
    if (sourceDirty.value) {
      pushToast("源码有未保存修改，已保留；点击「重新加载」可放弃修改。", "warn");
      return;
    }
    syncSourceFromSnapshot();
  }
});

async function reloadSource(): Promise<void> {
  await loadAll();
  syncSourceFromSnapshot();
  pushToast("源码已从磁盘重新加载。", "info");
}

async function switchSourceFile(file: "alias" | "zshrc"): Promise<void> {
  if (sourceFile.value === file) return;
  if (sourceDirty.value) {
    pushToast("源码有未保存修改，请先保存或重新加载后再切换文件。", "warn");
    return;
  }
  sourceFile.value = file;
  syncSourceFromSnapshot();
}

async function saveSource(): Promise<void> {
  sourceSaving.value = true;
  try {
    await runChange({ type: "source_write", fileKind: sourceFile.value, content: sourceContent.value });
  } finally {
    sourceSaving.value = false;
  }
}

/** 从表格跳转到源码对应行：切 Tab、必要时切换文件、滚动并选中该行 */
function jumpToSource(file: "alias" | "zshrc", line: number): void {
  if (sourceFile.value !== file) {
    if (sourceDirty.value) {
      pushToast("源码有未保存修改，请先保存或重新加载后再跳转其他文件。", "warn");
      return;
    }
    sourceFile.value = file;
    syncSourceFromSnapshot();
  }
  activeTab.value = "source";
  scrollTarget.value = undefined;
  void nextTick().then(() => {
    scrollTarget.value = line;
  });
}

// ---------------------------------------------------------------------------
// 通用动作
// ---------------------------------------------------------------------------

async function copyReload(): Promise<void> {
  await copyCommand(SHELL_RELOAD_COMMAND);
}

function openInEditor(kind?: "alias" | "zshrc"): void {
  const targets = kind
    ? [kind === "alias" ? aliasFile.value?.path : zshrcFile.value?.path]
    : [aliasFile.value?.path, zshrcFile.value?.path];
  const paths = targets.filter((path): path is string => !!path);
  if (!paths.length) return;
  Promise.all(paths.map((path) => shellEnvService.openFile(path)))
    .then(() => pushToast("已用系统文本编辑器打开，修改保存后请点击「重新加载」。", "info"))
    .catch((reason) => pushToast(reason instanceof Error ? reason.message : String(reason), "error"));
}

// 备份
const backupsOpen = ref(false);
const backups = ref<ShellBackup[]>([]);
const backupsLoading = ref(false);

async function openBackups(): Promise<void> {
  backupsOpen.value = true;
  backupsLoading.value = true;
  try {
    backups.value = await shellEnvService.listBackups();
  } catch (reason) {
    pushToast(reason instanceof Error ? reason.message : String(reason), "error");
  } finally {
    backupsLoading.value = false;
  }
}

async function restoreBackup(id: string): Promise<void> {
  backupsOpen.value = false;
  await runChange({ type: "restore_backup", backupId: id });
}

function formatSize(size: number): string {
  if (size < 1024) return `${size} B`;
  return `${(size / 1024).toFixed(1)} KB`;
}
</script>

<template>
  <section class="shell-env" aria-label="Shell 环境管理">
    <!-- 顶栏：状态 + 操作 -->
    <div class="se-toolbar">
      <div class="se-status">
        <Badge v-if="validation?.ok" class="se-badge-ok">zsh 语法校验通过</Badge>
        <Badge v-else-if="validation" class="se-badge-err">语法校验未通过</Badge>
        <Badge v-if="zshrcFile?.managedBlock" class="se-badge-ro">托管区块已创建</Badge>
        <span v-if="aliasFile" class="se-path" :title="aliasFile.path">{{ shortPath(aliasFile.path) }}</span>
        <span v-if="zshrcFile" class="se-path" :title="zshrcFile.path">{{ shortPath(zshrcFile.path) }}</span>
      </div>
      <div class="se-actions">
        <Button variant="ghost" size="sm" title="备份与恢复" @click="openBackups">
          <Archive :size="15" />备份
        </Button>
        <Button variant="ghost" size="sm" title="用系统文本编辑器打开两个配置文件" @click="openInEditor()">
          <ExternalLink :size="15" />打开文件
        </Button>
        <Button variant="outline" size="sm" title="复制 source 命令，让已打开的终端立即生效" @click="copyReload">
          <RefreshCw :size="15" />复制刷新命令
        </Button>
        <Button
          v-if="activeTab === 'alias' || activeTab === 'env'"
          size="sm"
          @click="activeTab === 'alias' ? openCreateAlias() : openCreateEnv()"
        >
          <Plus :size="15" />{{ activeTab === "alias" ? "新增别名" : "新增变量" }}
        </Button>
      </div>
    </div>

    <!-- 本次变更的会话命令（保存后出现，手动复制，不默认自动复制） -->
    <div v-if="lastSessionCommand" class="se-session-banner">
      <CheckCircle2 :size="15" />
      <span>已写入磁盘。已开终端执行：</span>
      <code>{{ lastSessionCommand }}</code>
      <Button variant="ghost" size="sm" @click="copyCommand(lastSessionCommand)">
        <Copy :size="14" />复制
      </Button>
      <Button variant="ghost" size="icon" aria-label="关闭提示" @click="lastSessionCommand = ''">
        <X :size="14" />
      </Button>
    </div>

    <Tabs v-model="activeTab" default-value="env">
      <div class="se-tabs-row">
        <div class="se-tabs-left">
          <TabsList>
            <TabsTrigger value="env"><FileCode2 :size="14" />变量 {{ managedEnvs.length }}/{{ envs.length }}</TabsTrigger>
            <TabsTrigger value="path"><Route :size="14" />PATH {{ pathReport?.summary.mergedCount ?? "…" }}</TabsTrigger>
            <TabsTrigger value="alias"><Terminal :size="14" />别名 {{ aliases.length }}</TabsTrigger>
            <TabsTrigger value="diagnostics"><Activity :size="14" />诊断 {{ diagnosticsCount }}</TabsTrigger>
            <TabsTrigger value="source"><FileText :size="14" />源码</TabsTrigger>
          </TabsList>
          <label
            v-if="activeTab === 'path' || activeTab === 'diagnostics'"
            class="se-extended-toggle"
            title="勾选后额外只读读取 ~/.zshenv、~/.zprofile、/etc/paths 与 /etc/paths.d（不做任何修改）"
          >
            <input v-model="includeExtended" type="checkbox" />
            扩展文件
          </label>
        </div>
        <label v-if="activeTab === 'alias' || activeTab === 'env'" class="se-search">
          <Search :size="15" />
          <Input v-model="activeQuery" :placeholder="activeTab === 'alias' ? '搜索别名或命令…' : '搜索变量名或值…'" />
          <Button
            v-if="activeQuery"
            type="button"
            variant="ghost"
            size="icon"
            aria-label="清除搜索"
            @click="activeQuery = ''"
          >
            <X :size="15" />
          </Button>
        </label>
      </div>

      <!-- 别名 Tab -->
      <TabsContent value="alias">
        <div class="se-table-wrap">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead class="w-40">名称</TableHead>
                <TableHead>命令</TableHead>
                <TableHead class="w-36">分组</TableHead>
                <TableHead class="w-16">行号</TableHead>
                <TableHead class="w-24 text-right">操作</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-if="loading">
                <TableCell colspan="5" class="se-empty">加载中…</TableCell>
              </TableRow>
              <TableRow v-else-if="!filteredAliases.length">
                <TableCell colspan="5" class="se-empty">
                  {{ aliasQuery ? "没有匹配的别名" : "~/.alias 为空，点击右上角「新增别名」创建第一条" }}
                </TableCell>
              </TableRow>
              <TableRow v-for="entry in filteredAliases" :key="entry.id">
                <TableCell class="se-mono se-name">{{ entry.name }}</TableCell>
                <TableCell class="se-mono se-value" :title="entry.raw">
                  <span class="se-value-main">{{ entry.value }}</span>
                  <Badge v-if="entry.valueKind === 'expression'" class="se-badge-expr" title="含 $ / 反引号展开语法，请在源码视图中编辑">表达式</Badge>
                  <span v-if="entry.description" class="se-desc">{{ entry.description }}</span>
                </TableCell>
                <TableCell class="se-group">{{ entry.group ?? "—" }}</TableCell>
                <TableCell>
                  <button class="se-line-btn" :title="`在源码中查看第 ${entry.line} 行`" @click="jumpToSource('alias', entry.line)">
                    {{ entry.line }}
                  </button>
                </TableCell>
                <TableCell class="text-right">
                  <Button
                    variant="ghost"
                    size="icon"
                    aria-label="编辑别名"
                    :title="entry.valueKind === 'expression' ? '含展开语法，请在源码视图中编辑' : '编辑别名'"
                    :disabled="entry.valueKind === 'expression'"
                    @click="openEditAlias(entry)"
                  >
                    <Pencil :size="15" />
                  </Button>
                  <Button variant="ghost" size="icon" aria-label="删除别名" @click="askDeleteAlias(entry)">
                    <Trash2 :size="15" />
                  </Button>
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </div>
        <p class="se-foot">共 {{ filteredAliases.length }} 条别名 · {{ aliasGroupCount }} 个分组</p>
      </TabsContent>

      <!-- 环境变量 Tab -->
      <TabsContent value="env">
        <!-- 区块结构问题横幅 -->
        <div v-if="zshrcFile?.blockIssues.length" class="se-issue-banner">
          <AlertTriangle :size="15" />
          <span>{{ zshrcFile.blockIssues.join("；") }}</span>
          <Button variant="outline" size="sm" @click="normalizeBlock">规范化区块标记</Button>
        </div>
        <div v-else-if="zshrcFile && !zshrcFile.managedBlock" class="se-issue-banner se-issue-info">
          <AlertTriangle :size="15" />
          <span>尚未创建托管区块；新增变量时会先在 ~/.zshrc 顶层末尾创建（需确认）。</span>
        </div>

        <div class="se-table-wrap">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead class="w-56">变量名</TableHead>
                <TableHead>值</TableHead>
                <TableHead class="w-28">来源</TableHead>
                <TableHead class="w-16">行号</TableHead>
                <TableHead class="w-24 text-right">操作</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-if="loading">
                <TableCell colspan="5" class="se-empty">加载中…</TableCell>
              </TableRow>
              <template v-else>
                <TableRow v-if="!filteredManagedEnvs.length && !filteredReadonlyEnvs.length">
                  <TableCell colspan="5" class="se-empty">
                    {{ envQuery ? "没有匹配的环境变量" : "暂无环境变量，点击右上角「新增变量」创建" }}
                  </TableCell>
                </TableRow>
                <TableRow v-for="entry in filteredManagedEnvs" :key="entry.id">
                  <TableCell class="se-mono se-name">{{ entry.key }}</TableCell>
                  <TableCell class="se-mono se-value" :title="entry.value">
                    {{ entry.value }}
                    <Badge v-if="entry.valueKind === 'expression'" class="se-badge-expr" title="含 $ / 反引号展开语法，请在源码视图中编辑">表达式</Badge>
                  </TableCell>
                  <TableCell><Badge class="se-badge-ok">托管</Badge></TableCell>
                  <TableCell>
                    <button class="se-line-btn" :title="`在源码中查看第 ${entry.line} 行`" @click="jumpToSource('zshrc', entry.line)">
                      {{ entry.line }}
                    </button>
                  </TableCell>
                  <TableCell class="text-right">
                    <Button
                      variant="ghost"
                      size="icon"
                      aria-label="编辑变量"
                      :title="entry.valueKind === 'expression' ? '含展开语法，请在源码视图中编辑' : '编辑变量'"
                      :disabled="entry.valueKind === 'expression'"
                      @click="openEditEnv(entry)"
                    >
                      <Pencil :size="15" />
                    </Button>
                    <Button variant="ghost" size="icon" aria-label="删除变量" @click="askDeleteEnv(entry)">
                      <Trash2 :size="15" />
                    </Button>
                  </TableCell>
                </TableRow>
                <TableRow v-for="entry in filteredReadonlyEnvs" :key="entry.id">
                  <TableCell class="se-mono se-name se-subtle">{{ entry.key }}</TableCell>
                  <TableCell class="se-mono se-value se-subtle" :title="entry.value">{{ entry.value }}</TableCell>
                  <TableCell>
                    <Badge class="se-badge-ro" title="非本应用管理，请在原文件中手动修改">只读</Badge>
                  </TableCell>
                  <TableCell>
                    <button class="se-line-btn" :title="`在源码中查看第 ${entry.line} 行`" @click="jumpToSource('zshrc', entry.line)">
                      {{ entry.line }}
                    </button>
                  </TableCell>
                  <TableCell class="text-right">
                    <Button
                      variant="ghost"
                      size="icon"
                      aria-label="打开 ~/.zshrc 手动修改"
                      title="非托管条目，打开原文件手动修改"
                      @click="openInEditor('zshrc')"
                    >
                      <ExternalLink :size="15" />
                    </Button>
                  </TableCell>
                </TableRow>
              </template>
            </TableBody>
          </Table>
        </div>
        <p class="se-foot">托管 {{ filteredManagedEnvs.length }} 条 · 区块外只读 {{ filteredReadonlyEnvs.length }} 条</p>
      </TabsContent>

      <!-- 源码 Tab -->
      <!-- PATH Tab：只读展示（分类 + 静态合并 + 诊断） -->
      <TabsContent value="path">
        <div v-if="!pathReport" class="se-empty">{{ sourceModelLoading ? "分析中…" : "暂无数据" }}</div>
        <template v-else>
          <p class="se-hint">
            扫描到 {{ pathReport.summary.contributionCount }} 处 PATH 修改语句<template v-if="pathReport.summary.dynamicCount">，其中 {{ pathReport.summary.dynamicCount }} 处在条件上下文、未纳入合并</template>。
            合并结果为静态推断的只读展示，不约束 NVM / mise 等工具的运行时修改。
          </p>

          <h3 class="se-section-title">合并结果（静态推断 · 只读）</h3>
          <div class="se-table-wrap">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead class="w-12">#</TableHead>
                  <TableHead>目录</TableHead>
                  <TableHead class="w-28">类型</TableHead>
                  <TableHead class="w-20">确定性</TableHead>
                  <TableHead class="w-56">添加位置</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                <TableRow v-if="!pathReport.merged.length">
                  <TableCell colspan="5" class="se-empty">未发现静态可确定的 PATH 条目</TableCell>
                </TableRow>
                <TableRow v-for="(entry, index) in pathReport.merged" :key="`${entry.via}:${index}`">
                  <TableCell class="se-mono">{{ index + 1 }}</TableCell>
                  <TableCell class="se-mono se-value" :title="entry.resolved ?? entry.text">{{ entry.text }}</TableCell>
                  <TableCell>
                    <Badge :class="entry.kind === 'relative' || entry.kind === 'empty' ? 'se-badge-expr' : 'se-badge-ro'">
                      {{ pathKindLabel(entry.kind) }}
                    </Badge>
                  </TableCell>
                  <TableCell>
                    <Badge :class="entry.certainty === 'system' ? 'se-badge-ro' : 'se-badge-ro'">
                      {{ contextLabel(entry.certainty) }}
                    </Badge>
                  </TableCell>
                  <TableCell>
                    <button class="se-line-btn" :title="`打开来源 ${entry.via}`" @click="openSourceAt(entry.via)">
                      {{ entry.via }}
                    </button>
                  </TableCell>
                </TableRow>
              </TableBody>
            </Table>
          </div>

          <h3 class="se-section-title">添加语句（{{ pathReport.contributions.length }} 处）</h3>
          <div class="se-table-wrap">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>语句</TableHead>
                  <TableHead class="w-16">模式</TableHead>
                  <TableHead class="w-36">条目分类</TableHead>
                  <TableHead class="w-16">上下文</TableHead>
                  <TableHead class="w-24">归属</TableHead>
                  <TableHead class="w-56">来源</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                <TableRow v-for="contribution in pathReport.contributions" :key="`${contribution.path}:${contribution.line}`">
                  <TableCell class="se-mono se-value" :title="contribution.raw">{{ contribution.raw }}</TableCell>
                  <TableCell>{{ pathModeLabel(contribution.mode) }}</TableCell>
                  <TableCell>
                    <Badge v-for="segment in contribution.segments" :key="segment.text" class="se-badge-ro se-seg-badge" :title="segment.resolved ?? segment.text">
                      {{ pathKindLabel(segment.kind) }}
                    </Badge>
                  </TableCell>
                  <TableCell>
                    <Badge :class="contribution.context === 'dynamic' ? 'se-badge-expr' : 'se-badge-ro'">
                      {{ contextLabel(contribution.context) }}
                    </Badge>
                  </TableCell>
                  <TableCell>{{ ownerLabel(contribution.owner) }}</TableCell>
                  <TableCell>
                    <button class="se-line-btn" @click="openSourceAt(`${contribution.path}:${contribution.line}`)">
                      {{ contribution.path }}:{{ contribution.line }}
                    </button>
                  </TableCell>
                </TableRow>
              </TableBody>
            </Table>
          </div>
        </template>
      </TabsContent>

      <!-- 诊断 Tab：PATH / 来源地图诊断 + 跨文件变量来源 -->
      <TabsContent value="diagnostics">
        <div v-if="!pathReport || !sourceMap" class="se-empty">{{ sourceModelLoading ? "分析中…" : "暂无数据" }}</div>
        <template v-else>
          <ul v-if="diagnosticsCount" class="se-diag-list">
            <li
              v-for="diagnostic in pathReport.diagnostics"
              :key="`path-${diagnostic.code}-${diagnostic.message}`"
              class="se-diag-item"
              :class="diagnostic.severity === 'warn' ? 'is-warn' : 'is-info'"
            >
              <AlertTriangle v-if="diagnostic.severity === 'warn'" :size="15" />
              <CheckCircle2 v-else :size="15" />
              <div class="se-diag-body">
                <span>{{ diagnostic.message }}</span>
                <span v-if="diagnostic.sources.length" class="se-diag-sources">
                  <button
                    v-for="source in diagnostic.sources"
                    :key="source"
                    class="se-line-btn"
                    @click="openSourceAt(source)"
                  >
                    {{ source }}
                  </button>
                </span>
              </div>
            </li>
            <li
              v-for="diagnostic in sourceMap.diagnostics"
              :key="`map-${diagnostic.code}-${diagnostic.message}`"
              class="se-diag-item is-warn"
            >
              <AlertTriangle :size="15" />
              <div class="se-diag-body"><span>{{ diagnostic.message }}</span></div>
            </li>
          </ul>
          <div v-else class="se-empty">当前扫描范围未发现需要关注的问题</div>

          <h3 class="se-section-title">变量来源（跨文件 · 只读）</h3>
          <div class="se-table-wrap">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead class="w-40">变量</TableHead>
                  <TableHead>值</TableHead>
                  <TableHead class="w-16">导出</TableHead>
                  <TableHead class="w-16">上下文</TableHead>
                  <TableHead class="w-24">归属</TableHead>
                  <TableHead class="w-56">来源</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                <TableRow v-if="!sourceMap.envs.length">
                  <TableCell colspan="6" class="se-empty">暂无定义</TableCell>
                </TableRow>
                <TableRow v-for="origin in sourceMap.envs" :key="`${origin.path}:${origin.line}`">
                  <TableCell class="se-mono se-name">
                    {{ origin.key }}
                    <Badge v-if="origin.managed" class="se-badge-ro">托管</Badge>
                  </TableCell>
                  <TableCell class="se-mono se-value" :title="origin.raw">{{ origin.value }}</TableCell>
                  <TableCell>{{ origin.exported ? "export" : "赋值" }}</TableCell>
                  <TableCell>
                    <Badge :class="origin.context === 'dynamic' ? 'se-badge-expr' : 'se-badge-ro'">
                      {{ contextLabel(origin.context) }}
                    </Badge>
                  </TableCell>
                  <TableCell :title="`依据：${origin.ownerEvidence === 'signature' ? '行内签名' : origin.ownerEvidence === 'marker' ? '区块标记' : '无已知标记'}`">
                    {{ ownerLabel(origin.owner) }}
                  </TableCell>
                  <TableCell>
                    <button class="se-line-btn" @click="openSourceAt(`${origin.path}:${origin.line}`)">
                      {{ origin.path }}:{{ origin.line }}
                    </button>
                  </TableCell>
                </TableRow>
              </TableBody>
            </Table>
          </div>

          <h3 class="se-section-title">来源文件</h3>
          <div class="se-table-wrap">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>路径</TableHead>
                  <TableHead class="w-32">角色</TableHead>
                  <TableHead class="w-24">状态</TableHead>
                  <TableHead class="w-40">纳入方式</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                <TableRow v-for="file in sourceMap.files" :key="file.id">
                  <TableCell class="se-mono se-value">{{ file.path }}</TableCell>
                  <TableCell>{{ file.role }}</TableCell>
                  <TableCell>
                    <Badge :class="file.included ? 'se-badge-ro' : 'se-badge-expr'">
                      {{ file.included ? "已读取" : file.exists ? "仅登记" : "不存在" }}
                    </Badge>
                  </TableCell>
                  <TableCell>
                    {{ file.via === "sourced-ref" && file.sourcedFrom ? `source 引用（${file.sourcedFrom}）` : file.via === "opt-in" ? "用户勾选" : "默认登记" }}
                  </TableCell>
                </TableRow>
              </TableBody>
            </Table>
          </div>
          <p class="se-foot">
            工具归属来自已知标记匹配（行内签名或 # >>> 区块标记），未知不代表没有工具；扩展文件仅在勾选后只读读取，不做任何修改。
          </p>
        </template>
      </TabsContent>

      <TabsContent value="source">
        <div class="se-source-bar">
          <div class="se-file-seg" role="group" aria-label="选择配置文件">
            <button :class="{ active: sourceFile === 'alias' }" type="button" @click="switchSourceFile('alias')">~/.alias</button>
            <button :class="{ active: sourceFile === 'zshrc' }" type="button" @click="switchSourceFile('zshrc')">~/.zshrc</button>
          </div>
          <span v-if="sourceDirty" class="se-dirty-hint">有未保存修改</span>
          <div class="se-source-actions">
            <Button variant="ghost" size="sm" title="放弃修改并从磁盘重新读取" @click="reloadSource">
              <RefreshCw :size="15" />重新加载
            </Button>
            <Button size="sm" :disabled="!sourceDirty || sourceSaving" @click="saveSource">
              {{ sourceSaving ? "准备中…" : "保存源码" }}
            </Button>
          </div>
        </div>
        <div class="se-source-editor">
          <div v-if="loading" class="se-empty">加载中…</div>
          <CodeEditor
            v-else
            :key="sourceFile"
            v-model="sourceContent"
            language="text"
            :scroll-to-line="scrollTarget"
          />
        </div>
        <p class="se-foot">
          直接编辑原始文件；保存经「预览 → 确认」流程，写入前自动备份并通过 zsh -n 语法校验，不规范化换行与末尾格式。
          源码有未保存修改期间，同文件的其他写操作会被暂停（防止互相覆盖）。
        </p>
      </TabsContent>
    </Tabs>

    <p class="se-hint se-hint-footer">
      修改统一走「预览 → 确认 → 写入」，写入前自动备份并做 zsh 语法校验；新开终端自动生效，已开终端执行会话 /
      刷新命令。变量仅管理 <code>~/.zshrc</code> 托管区块，区块外只读。
    </p>

    <!-- 别名编辑弹窗 -->
    <Dialog :open="aliasDialogOpen" content-class="se-dialog-content" @update:open="aliasDialogOpen = $event">
      <form class="se-dialog" @submit.prevent="saveAlias">
        <DialogTitle>{{ aliasEditing ? (aliasRenameMode ? "重命名别名" : "编辑别名") : "新增别名" }}</DialogTitle>
        <DialogDescription class="se-dialog-desc">
          <template v-if="aliasEditing && aliasRenameMode">按原身份一次提交重命名，写入 ~/.alias 前会展示差异确认。</template>
          <template v-else-if="aliasEditing">将替换 ~/.alias 第 {{ aliasEditing.line }} 行的 alias 定义；保存前展示差异确认。</template>
          <template v-else>写入 ~/.alias，保存前展示差异确认；新开终端自动生效。</template>
        </DialogDescription>
        <label class="se-field">
          <span>名称</span>
          <Input v-model="aliasForm.name" :disabled="!!aliasEditing && !aliasRenameMode" placeholder="如 gcm" required />
        </label>
        <label v-if="!aliasRenameMode" class="se-field">
          <span>命令</span>
          <Input v-model="aliasForm.value" placeholder="如 git commit -m" required />
        </label>
        <label v-if="!aliasRenameMode" class="se-field">
          <span>描述（可选）</span>
          <Input v-model="aliasForm.description" placeholder="如 快速提交；以行尾注释形式保存" maxlength="120" />
        </label>
        <label v-if="!aliasEditing && !aliasRenameMode" class="se-field">
          <span>分组（可选）</span>
          <div class="se-group-picker">
            <Input
              v-model="aliasForm.group"
              placeholder="如 git；可点选已有分组，新分组会自动创建组头"
              autocomplete="off"
              @focus="groupPickerOpen = true"
              @blur="groupPickerOpen = false"
            />
            <ul v-if="groupPickerOpen && (groupSuggestions.length || groupIsNew)" class="se-group-menu">
              <li v-for="group in groupSuggestions" :key="group">
                <button type="button" @mousedown.prevent="pickGroup(group)">{{ group }}</button>
              </li>
              <li v-if="groupIsNew" class="is-new">将创建新分组「{{ aliasForm.group?.trim() }}」</li>
            </ul>
          </div>
        </label>
        <div class="se-dialog-actions">
          <Button
            v-if="aliasEditing && !aliasRenameMode"
            type="button"
            variant="ghost"
            size="sm"
            @click="startAliasRename"
          >
            重命名
          </Button>
          <span class="se-dialog-spacer" />
          <Button type="button" variant="ghost" @click="aliasDialogOpen = false">取消</Button>
          <Button
            type="submit"
            :disabled="aliasSaving || !aliasForm.name || (!aliasRenameMode && !aliasForm.value)"
          >
            {{ aliasSaving ? "处理中…" : "预览变更" }}
          </Button>
        </div>
      </form>
    </Dialog>

    <!-- 环境变量编辑弹窗 -->
    <Dialog :open="envDialogOpen" content-class="se-dialog-content" @update:open="envDialogOpen = $event">
      <form class="se-dialog" @submit.prevent="saveEnv">
        <DialogTitle>{{ envEditing ? "编辑环境变量" : "新增环境变量" }}</DialogTitle>
        <DialogDescription class="se-dialog-desc">
          写入 ~/.zshrc 的 xlt-managed 托管区块，值按字面量语义保存；保存前展示差异确认。
        </DialogDescription>
        <label class="se-field">
          <span>变量名</span>
          <Input v-model="envForm.key" :disabled="!!envEditing" placeholder="如 HTTP_PROXY" required />
        </label>
        <label class="se-field">
          <span>值</span>
          <Input v-model="envForm.value" placeholder="如 http://127.0.0.1:8798" required />
        </label>
        <p v-if="envForm.value.includes('$')" class="se-dialog-desc se-warn-text">
          值包含 $：将按字面量保存（不展开）。如需展开语义，请改用源码视图。
        </p>
        <div class="se-dialog-actions">
          <Button type="button" variant="ghost" @click="envDialogOpen = false">取消</Button>
          <Button type="submit" :disabled="envSaving || !envForm.key || !envForm.value">
            {{ envSaving ? "处理中…" : "预览变更" }}
          </Button>
        </div>
      </form>
    </Dialog>

    <!-- 首次创建托管区块确认 -->
    <Dialog :open="blockCreateOpen" content-class="se-dialog-content" @update:open="blockCreateOpen = $event">
      <form class="se-dialog" @submit.prevent="confirmBlockCreate">
        <DialogTitle>创建托管区块</DialogTitle>
        <DialogDescription class="se-dialog-desc">
          将在 <code>~/.zshrc</code> 顶层末尾创建 <code># >>> xlt-managed env >>></code> 托管区块并写入
          <code>{{ envForm.key }}</code>。已有初始化区块（Kiro / SDKMAN 等）保持原顺序不变。
        </DialogDescription>
        <div class="se-dialog-actions">
          <Button type="button" variant="ghost" @click="blockCreateOpen = false">取消</Button>
          <Button type="submit">确认创建并预览</Button>
        </div>
      </form>
    </Dialog>

    <!-- 删除确认：别名 -->
    <Dialog :open="!!aliasDeleteTarget" content-class="se-dialog-content" @update:open="(open) => !open && (aliasDeleteTarget = null)">
      <form class="se-dialog" @submit.prevent="deleteAlias">
        <DialogTitle>删除别名</DialogTitle>
        <DialogDescription class="se-dialog-desc">
          确认删除 <code>{{ aliasDeleteTarget?.name }}</code>（第 {{ aliasDeleteTarget?.line }} 行）？
          保存前会展示差异确认并自动备份。已开终端需要执行 <code>unalias</code> 清理旧定义。
        </DialogDescription>
        <div class="se-dialog-actions">
          <Button type="button" variant="ghost" @click="aliasDeleteTarget = null">取消</Button>
          <Button type="submit" variant="destructive">预览删除</Button>
        </div>
      </form>
    </Dialog>

    <!-- 删除确认：环境变量 -->
    <Dialog :open="!!envDeleteTarget" content-class="se-dialog-content" @update:open="(open) => !open && (envDeleteTarget = null)">
      <form class="se-dialog" @submit.prevent="deleteEnv">
        <DialogTitle>删除环境变量</DialogTitle>
        <DialogDescription class="se-dialog-desc">
          确认从托管区块删除 <code>{{ envDeleteTarget?.key }}</code>？保存前会展示差异确认并自动备份。
        </DialogDescription>
        <div class="se-dialog-actions">
          <Button type="button" variant="ghost" @click="envDeleteTarget = null">取消</Button>
          <Button type="submit" variant="destructive">预览删除</Button>
        </div>
      </form>
    </Dialog>

    <!-- 变更差异确认（统一入口） -->
    <Dialog :open="confirmOpen" content-class="se-dialog-content se-dialog-plan" @update:open="(open) => !open && cancelConfirm()">
      <div class="se-dialog">
        <DialogTitle>确认变更</DialogTitle>
        <DialogDescription class="se-dialog-desc">
          以下变更将写入
          <code>{{ pendingPlan?.kind === "alias" ? "~/.alias" : "~/.zshrc" }}</code>；
          写入前自动备份并通过 zsh 语法校验。校验或版本比对失败时不会写入。
        </DialogDescription>
        <ul class="se-op-list">
          <li v-for="(op, index) in pendingPlan?.ops ?? []" :key="index">
            <b>{{ op.action }}</b>
            <span>{{ op.detail }}</span>
          </li>
        </ul>
        <div class="se-session-box">
          <span>已开终端生效命令（手动复制）：</span>
          <code>{{ pendingPlan?.sessionCommand }}</code>
        </div>
        <div class="se-dialog-actions">
          <Button type="button" variant="ghost" :disabled="committing" @click="cancelConfirm">取消</Button>
          <Button type="button" :disabled="committing" @click="confirmCommit">
            {{ committing ? "写入中…" : "确认写入" }}
          </Button>
        </div>
      </div>
    </Dialog>

    <!-- 备份与恢复 -->
    <Dialog :open="backupsOpen" content-class="se-dialog-wide-content" @update:open="backupsOpen = $event">
      <div class="se-dialog se-dialog-wide">
        <DialogTitle>备份与恢复</DialogTitle>
        <DialogDescription class="se-dialog-desc">
          每次写入前自动备份（UUID 命名），每类文件保留最近 5 份。恢复经同一「预览 → 确认」流程，后像在预览时固定。
        </DialogDescription>
        <div class="se-table-wrap se-backup-list">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>备份文件</TableHead>
                <TableHead class="w-28">类型</TableHead>
                <TableHead class="w-44">时间</TableHead>
                <TableHead class="w-20">大小</TableHead>
                <TableHead class="w-20 text-right">操作</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-if="backupsLoading">
                <TableCell colspan="5" class="se-empty">加载中…</TableCell>
              </TableRow>
              <TableRow v-else-if="!backups.length">
                <TableCell colspan="5" class="se-empty">暂无备份</TableCell>
              </TableRow>
              <TableRow v-for="backup in backups" :key="backup.id">
                <TableCell class="se-mono">{{ backup.id }}</TableCell>
                <TableCell>
                  <Badge>{{ backup.file === "alias" ? "~/.alias" : "~/.zshrc" }}</Badge>
                </TableCell>
                <TableCell class="se-subtle">{{ backup.createdAt }}</TableCell>
                <TableCell class="se-subtle">{{ formatSize(backup.size) }}</TableCell>
                <TableCell class="text-right">
                  <Button size="sm" variant="outline" @click="restoreBackup(backup.id)">恢复</Button>
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </div>
        <div class="se-dialog-actions">
          <Button type="button" variant="ghost" @click="backupsOpen = false">关闭</Button>
        </div>
      </div>
    </Dialog>
  </section>
</template>

<style scoped>
.shell-env {
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding-bottom: 24px;
}

.se-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
}

.se-status {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  min-width: 0;
}

.se-path {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 12px;
  color: var(--text-subtle);
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 2px 8px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 240px;
}

.se-badge-ok {
  background: var(--accent-weak);
  color: var(--accent);
}

.se-badge-err {
  background: rgba(182, 68, 68, 0.12);
  color: var(--u-crit);
}

.se-badge-ro {
  color: var(--text-subtle);
}

.se-badge-expr {
  margin-left: 6px;
  background: rgba(156, 122, 46, 0.14);
  color: var(--u-warn);
}

/* R1 只读面板：扩展文件开关 / 小节标题 / 诊断列表 */
.se-tabs-left {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
  flex-wrap: wrap;
}

.se-hint-footer {
  margin: 4px 2px 0;
}

.se-extended-toggle {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  font-size: 12px;
  color: var(--text-subtle);
  cursor: pointer;
  user-select: none;
  border: 1px solid var(--border);
  border-radius: 999px;
  padding: 5px 12px;
  background: var(--surface);
  white-space: nowrap;
  transition: border-color 0.15s ease, color 0.15s ease;
}

.se-extended-toggle:hover {
  color: var(--text);
  border-color: var(--accent, var(--border));
}

.se-extended-toggle input[type="checkbox"] {
  accent-color: var(--accent, currentColor);
  margin: 0;
}

.se-section-title {
  margin: 18px 0 8px;
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
}

.se-seg-badge {
  margin-right: 4px;
}

.se-diag-list {
  margin: 0;
  padding: 0;
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.se-diag-item {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 8px 12px;
  font-size: 12.5px;
  line-height: 1.6;
}

.se-diag-item.is-warn {
  border-color: rgba(156, 122, 46, 0.35);
  color: var(--u-warn);
}

.se-diag-item.is-info {
  border-color: var(--border);
  color: var(--text-subtle);
}

.se-diag-body {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}

.se-diag-sources {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 10px;
}

.se-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.se-session-banner {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  border: 1px solid var(--accent-ring, rgba(79, 157, 105, 0.2));
  background: var(--accent-weak);
  border-radius: 10px;
  padding: 8px 12px;
  color: var(--accent);
  font-size: 12.5px;
}

.se-session-banner code {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  color: var(--text);
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 2px 8px;
}

.se-hint {
  margin: 2px 0 12px;
  font-size: 12px;
  color: var(--text-subtle);
  line-height: 1.6;
}

.se-hint code {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  background: var(--surface-3);
  border-radius: 4px;
  padding: 1px 5px;
}

.se-tabs-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
  margin-bottom: 12px;
}

/* Tailwind preflight 将 svg 设为 block，图标会被挤到文字上方；
   trigger 显式 inline-flex 保证图标与文字同行、尺寸正常。 */
.se-tabs-row :deep(.cn-tabs-trigger) {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  white-space: nowrap;
}

.se-search {
  display: flex;
  align-items: center;
  gap: 8px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--surface);
  padding: 0 8px;
  min-width: 240px;
  color: var(--text-subtle);
}

.se-search :deep(input) {
  border: none;
  background: transparent;
  height: 32px;
  box-shadow: none;
}

.se-table-wrap {
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--surface);
  overflow: hidden;
}

/* 表头高亮 */
.se-table-wrap :deep(thead tr) {
  background: var(--surface-3);
}

.se-table-wrap :deep(thead th:first-child) {
  border-top-left-radius: 10px;
}

.se-table-wrap :deep(thead th:last-child) {
  border-top-right-radius: 10px;
}

.se-mono {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 12.5px;
}

.se-name {
  color: var(--accent);
  font-weight: 600;
}

.se-value {
  color: var(--text);
  max-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.se-desc {
  display: block;
  font-family: system-ui, sans-serif;
  font-size: 11.5px;
  color: var(--text-subtle);
  margin-top: 2px;
}

/* 分组选择下拉 */
.se-group-picker {
  position: relative;
}

.se-group-menu {
  position: absolute;
  z-index: 60;
  top: calc(100% + 4px);
  left: 0;
  right: 0;
  margin: 0;
  padding: 4px;
  list-style: none;
  background: var(--popover);
  border: 1px solid var(--border);
  border-radius: 8px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.14);
  max-height: 180px;
  overflow: auto;
}

.se-group-menu button {
  display: block;
  width: 100%;
  text-align: left;
  padding: 6px 8px;
  border: none;
  border-radius: 6px;
  background: none;
  font-size: 12.5px;
  color: var(--text);
  cursor: pointer;
}

.se-group-menu button:hover {
  background: var(--accent-weak);
}

.se-group-menu .is-new {
  padding: 6px 8px;
  font-size: 12px;
  color: var(--text-subtle);
}

.se-group,
.se-subtle {
  color: var(--text-subtle);
  font-size: 12px;
}

.se-empty {
  text-align: center;
  color: var(--text-subtle);
  padding: 28px 0;
  font-size: 13px;
}

.se-foot {
  margin: 8px 2px 0;
  font-size: 12px;
  color: var(--text-subtle);
}

.se-issue-banner {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  border: 1px solid rgba(156, 122, 46, 0.35);
  background: rgba(156, 122, 46, 0.1);
  border-radius: 10px;
  padding: 8px 12px;
  color: var(--u-warn);
  font-size: 12.5px;
  margin-bottom: 10px;
}

.se-issue-info {
  border-color: var(--border);
  background: var(--surface-2);
  color: var(--text-muted);
}

.se-dialog {
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 22px;
  text-align: left;
}

/* 弹窗经 portal 渲染在 body 下，scoped 样式无法命中，
   宽度通过 Dialog 的 content-class + :global 控制。 */
:global(.cn-dialog-content.se-dialog-content) {
  width: min(460px, calc(100vw - 32px));
}

:global(.cn-dialog-content.se-dialog-plan) {
  width: min(560px, calc(100vw - 32px));
}

:global(.cn-dialog-content.se-dialog-wide-content) {
  width: min(720px, calc(100vw - 32px));
}

.se-dialog :deep(h2) {
  margin: 0;
  color: var(--text);
  font-size: 16px;
  font-weight: 650;
}

.se-dialog :deep(p) {
  margin: 0;
  color: var(--text-muted);
  font-size: 12.5px;
  line-height: 1.6;
}

.se-dialog-desc {
  font-size: 12.5px;
  color: var(--text-subtle);
  margin: 0;
}

.se-dialog-desc code {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  background: var(--surface-3);
  border-radius: 4px;
  padding: 1px 5px;
}

.se-warn-text {
  color: var(--u-warn);
}

.se-op-list {
  margin: 0;
  padding: 10px 14px;
  list-style: none;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--surface-2);
  max-height: 200px;
  overflow: auto;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.se-op-list li {
  display: flex;
  flex-direction: column;
  gap: 2px;
  font-size: 12.5px;
}

.se-op-list b {
  color: var(--text);
  font-weight: 650;
}

.se-op-list span {
  color: var(--text-muted);
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 12px;
  word-break: break-all;
}

.se-session-box {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  font-size: 12px;
  color: var(--text-subtle);
}

.se-session-box code {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  color: var(--text);
  background: var(--surface-3);
  border-radius: 6px;
  padding: 2px 8px;
}

.se-field {
  display: flex;
  flex-direction: column;
  gap: 6px;
  font-size: 12.5px;
  color: var(--text-muted);
}

.se-dialog-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 4px;
}

.se-dialog-spacer {
  flex: 1;
}

.se-backup-list {
  max-height: 46vh;
  overflow: auto;
}

/* 源码视图 */
.se-source-bar {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
  margin-bottom: 10px;
}

.se-file-seg {
  display: inline-flex;
  gap: 2px;
  padding: 3px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--surface-2);
}

.se-file-seg button {
  padding: 5px 12px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--text-muted);
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 12px;
  cursor: pointer;
}

.se-file-seg button:hover {
  color: var(--text);
}

.se-file-seg button.active {
  background: var(--surface);
  box-shadow: var(--shadow-sm);
  color: var(--accent);
  font-weight: 600;
}

.se-dirty-hint {
  font-size: 12px;
  color: var(--u-warn);
}

.se-source-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-left: auto;
}

.se-source-editor {
  height: min(62vh, 640px);
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--surface);
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.se-source-editor :deep(.cm-editor) {
  height: 100%;
  font-size: 12.5px;
}

.se-source-editor :deep(.cm-scroller) {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
}

/* 行号跳转按钮 */
.se-line-btn {
  padding: 2px 7px;
  border: 0;
  border-radius: 5px;
  background: transparent;
  color: var(--text-subtle);
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  cursor: pointer;
}

.se-line-btn:hover {
  background: var(--accent-weak);
  color: var(--accent);
}
</style>
