<script setup lang="ts">
import { computed, defineAsyncComponent, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { onBeforeRouteLeave, useRouter } from "vue-router";
import type { InstallRecord, InstallUpdateInfo, SkillDetail, SkillFileNode, UpdateDiffPreview, UpdateStatus } from "@/types/skills";
import { skillsApi, skillsErrorMessage } from "@/api/skills";
import { formatBytes, renderMarkdown } from "@/utils/skills-markdown";
import FileTree from "@/components/skills/FileTree.vue";

// 编辑器依赖（codemirror / md-editor-v3）体积大，异步加载：仅进入编辑态才拉取。
const CodeEditor = defineAsyncComponent(() => import("@/components/skills/CodeEditor.vue"));
const MarkdownEditor = defineAsyncComponent(() => import("@/components/skills/MarkdownEditor.vue"));

const props = defineProps<{ rootId: string; name: string }>();
const router = useRouter();

const detail = ref<SkillDetail | null>(null);
const loading = ref(false);
const error = ref("");
const editing = ref(false);
const saving = ref(false);
const metaOpen = ref(false);
const filesCollapsed = ref(false);

const form = ref({ name: "", version: "", description: "", extraYaml: "", body: "" });
const fileEdits = ref<Record<string, string>>({});
const fileOriginals = ref<Record<string, string>>({});
const activeFile = ref("");

// 来源与更新
const installRecord = ref<InstallRecord | null>(null);
const updateInfo = ref<InstallUpdateInfo | null>(null);
const updateDiff = ref<UpdateDiffPreview | null>(null);
const updateBusy = ref(false);

const updateStatusMeta: Record<
  UpdateStatus,
  { label: string; color: "success" | "info" | "warning" | "error" | "neutral"; icon: string }
> = {
  "up-to-date": { label: "已是最新", color: "success", icon: "i-lucide-circle-check" },
  "remote-changed": { label: "上游有更新", color: "info", icon: "i-lucide-circle-arrow-down" },
  "local-changed": { label: "本地已修改", color: "warning", icon: "i-lucide-pencil" },
  "both-changed": { label: "本地与上游均有变化", color: "error", icon: "i-lucide-git-merge" },
  unknown: { label: "无法判断", color: "neutral", icon: "i-lucide-circle-help" },
};

const canPreviewUpdate = computed(() => {
  const status = updateInfo.value?.status;
  return status === "remote-changed" || status === "both-changed";
});

async function loadInstallRecord() {
  installRecord.value = null;
  updateInfo.value = null;
  updateDiff.value = null;
  try {
    const records = await skillsApi.listInstalls();
    installRecord.value =
      records.find((item) => item.rootId === props.rootId && item.name === props.name) ?? null;
  } catch {
    // 无安装记录时静默降级，不影响详情页主流程
  }
}

async function checkUpdate() {
  if (!installRecord.value) return;
  updateBusy.value = true;
  error.value = "";
  try {
    updateDiff.value = null;
    updateInfo.value = await skillsApi.checkInstallUpdate(installRecord.value.id);
  } catch (caught) {
    error.value = skillsErrorMessage(caught);
  } finally {
    updateBusy.value = false;
  }
}

async function previewUpdate() {
  if (!installRecord.value) return;
  updateBusy.value = true;
  error.value = "";
  try {
    updateDiff.value = await skillsApi.previewInstallUpdate(installRecord.value.id);
  } catch (caught) {
    error.value = skillsErrorMessage(caught);
  } finally {
    updateBusy.value = false;
  }
}

function applyUpdate() {
  const diff = updateDiff.value;
  if (!diff) return;
  const localTouched =
    updateInfo.value?.status === "both-changed" || updateInfo.value?.status === "local-changed";
  openConfirm({
    title: "应用更新",
    description: localTouched
      ? "检测到本地修改，更新会用上游新版本覆盖这些改动（旧版本保留为备份，可回滚）。确认继续？"
      : "将替换为上游新版本内容（旧版本保留为备份，可回滚）。确认继续？",
    confirmLabel: "应用更新",
    confirmIcon: "i-lucide-circle-arrow-down",
    onConfirm: () => void doApplyUpdate(diff),
  });
}

async function doApplyUpdate(diff: UpdateDiffPreview) {
  updateBusy.value = true;
  error.value = "";
  try {
    const result = await skillsApi.applyInstallUpdate(diff.recordId, {
      stagingId: diff.stagingId,
      skillId: diff.skillId,
    });
    if (result.status === "failed") {
      throw new Error(result.error?.message ?? "更新失败，已恢复更新前内容");
    }
    await load();
  } catch (caught) {
    error.value = skillsErrorMessage(caught);
  } finally {
    updateBusy.value = false;
  }
}

function rollback(backupId: string) {
  const record = installRecord.value;
  if (!record) return;
  openConfirm({
    title: "回滚到更新前版本",
    description: "将丢弃当前内容并恢复更新前的备份，此操作不可撤销。确认回滚？",
    confirmLabel: "回滚",
    confirmColor: "error",
    confirmIcon: "i-lucide-undo-2",
    onConfirm: () => void doRollback(record.id, backupId),
  });
}

async function doRollback(recordId: string, backupId: string) {
  updateBusy.value = true;
  error.value = "";
  try {
    await skillsApi.rollbackInstall(recordId, backupId);
    await load();
  } catch (caught) {
    error.value = skillsErrorMessage(caught);
  } finally {
    updateBusy.value = false;
  }
}

const isReadonly = computed(() => {
  const record = detail.value;
  return !record || record.isSymlink;
});

async function load() {
  loading.value = true;
  error.value = "";
  try {
    const result = await skillsApi.getSkill(props.rootId, props.name);
    detail.value = result;
    resetForm(result);
  } catch (caught) {
    error.value = skillsErrorMessage(caught);
  } finally {
    loading.value = false;
  }
  void loadInstallRecord();
}

function resetForm(value: SkillDetail) {
  form.value = {
    name: value.name,
    version: value.version ?? "",
    description: value.description ?? "",
    extraYaml: value.frontmatterYaml,
    body: value.body,
  };
  fileEdits.value = {};
  fileOriginals.value = {};
  activeFile.value = "";
}

const dirty = computed(() => {
  if (!editing.value || !detail.value) return false;
  const record = detail.value;
  if (form.value.name !== record.name) return true;
  if (form.value.version !== (record.version ?? "")) return true;
  if (form.value.description !== (record.description ?? "")) return true;
  if (form.value.extraYaml !== record.frontmatterYaml) return true;
  if (form.value.body !== record.body) return true;
  return Object.keys(fileEdits.value).some((path) => fileEdits.value[path] !== fileOriginals.value[path]);
});

const confirmState = reactive({
  open: false,
  title: "",
  description: "",
  confirmLabel: "确定",
  confirmColor: "primary" as "primary" | "error",
  confirmIcon: "",
  onConfirm: () => {},
});

function openConfirm(options: {
  title: string;
  description: string;
  confirmLabel?: string;
  confirmColor?: "primary" | "error";
  confirmIcon?: string;
  onConfirm: () => void;
}) {
  confirmState.title = options.title;
  confirmState.description = options.description;
  confirmState.confirmLabel = options.confirmLabel ?? "确定";
  confirmState.confirmColor = options.confirmColor ?? "primary";
  confirmState.confirmIcon = options.confirmIcon ?? "";
  confirmState.onConfirm = options.onConfirm;
  confirmState.open = true;
}

function handleConfirm() {
  confirmState.open = false;
  confirmState.onConfirm();
}

const forceEdit = ref(false);

function enterEdit() {
  if (detail.value?.isSymlink && !forceEdit.value) {
    openConfirm({
      title: "编辑软链接 skill",
      description: "这是一个软链接 skill，编辑将直接修改链接指向的源文件。确认继续？",
      confirmLabel: "继续编辑",
      confirmIcon: "i-lucide-pencil",
      onConfirm: () => {
        forceEdit.value = true;
        editing.value = true;
        filesCollapsed.value = true;
      },
    });
    return;
  }
  editing.value = true;
  filesCollapsed.value = true;
}

function cancelEdit() {
  editing.value = false;
  if (detail.value) resetForm(detail.value);
}

async function selectFile(node: SkillFileNode) {
  if (node.path === "SKILL.md") {
    activeFile.value = "";
    return;
  }
  if (fileEdits.value[node.path] === undefined) {
    const { content } = await skillsApi.getSkillFile(props.rootId, props.name, node.path);
    fileEdits.value[node.path] = content;
    fileOriginals.value[node.path] = content;
  }
  activeFile.value = node.path;
}

const currentText = computed({
  get() {
    return activeFile.value ? fileEdits.value[activeFile.value] ?? "" : form.value.body;
  },
  set(value: string) {
    if (activeFile.value) fileEdits.value[activeFile.value] = value;
    else form.value.body = value;
  },
});

const previewHtml = computed(() =>
  renderMarkdown(activeFile.value ? currentText.value : form.value.body),
);

const isMarkdownFile = computed(() => activeFile.value === "" || activeFile.value.endsWith(".md"));

async function save() {
  if (!detail.value) return;
  saving.value = true;
  error.value = "";
  try {
    const updated = await skillsApi.saveSkill(props.rootId, props.name, {
      name: form.value.name.trim(),
      version: form.value.version.trim() || undefined,
      description: form.value.description,
      extraYaml: form.value.extraYaml,
      body: form.value.body,
      files: Object.keys(fileEdits.value).length ? fileEdits.value : undefined,
    });
    detail.value = updated;
    resetForm(updated);
    editing.value = false;
  } catch (caught) {
    error.value = skillsErrorMessage(caught);
  } finally {
    saving.value = false;
  }
}

function remove() {
  if (!detail.value) return;
  const isLink = detail.value.isSymlink;
  openConfirm({
    title: isLink ? "删除软链接" : "删除 skill",
    description: isLink
      ? "确认删除此软链接？（只删除链接，不影响源文件）"
      : `确认删除 skill「${detail.value.name}」？（将移入回收站，可恢复）`,
    confirmLabel: "删除",
    confirmColor: "error",
    confirmIcon: "i-lucide-trash-2",
    onConfirm: doRemove,
  });
}

async function doRemove() {
  try {
    await skillsApi.deleteSkill(props.rootId, props.name);
    void router.push({ name: "skills-library" });
  } catch (caught) {
    error.value = skillsErrorMessage(caught);
  }
}

const renameState = reactive({ open: false, value: "", busy: false });

function openRename() {
  if (!detail.value) return;
  renameState.value = detail.value.name;
  renameState.open = true;
}

async function doRename() {
  if (!detail.value) return;
  const newName = renameState.value.trim();
  if (!newName || newName === detail.value.name) {
    renameState.open = false;
    return;
  }
  renameState.busy = true;
  error.value = "";
  try {
    const updated = await skillsApi.renameSkill(props.rootId, props.name, newName);
    renameState.open = false;
    void router.replace({ name: "skills-detail", params: { rootId: props.rootId, name: updated.name } });
  } catch (caught) {
    error.value = skillsErrorMessage(caught);
  } finally {
    renameState.busy = false;
  }
}

function handleBeforeUnload(event: BeforeUnloadEvent) {
  if (dirty.value) {
    event.preventDefault();
    event.returnValue = "";
  }
}
onMounted(() => window.addEventListener("beforeunload", handleBeforeUnload));
onBeforeUnmount(() => window.removeEventListener("beforeunload", handleBeforeUnload));
onBeforeRouteLeave(() => {
  if (dirty.value) {
    return window.confirm("有未保存的修改，确定要离开吗？");
  }
  return true;
});

watch(() => [props.rootId, props.name], load, { immediate: true });
</script>

<template>
  <UDashboardPanel id="skills-detail">
    <template #header>
      <UDashboardNavbar :title="detail?.name || 'Skill'" :ui="{ right: 'gap-1.5' }">
        <template #leading>
          <UButton
            icon="i-lucide-arrow-left"
            color="neutral"
            variant="ghost"
            square
            @click="router.push({ name: 'skills-library' })"
          />
        </template>
        <template #trailing>
          <UBadge v-if="detail?.version" color="neutral" variant="subtle" size="sm" class="font-mono">
            v{{ detail.version }}
          </UBadge>
          <UBadge v-if="detail?.isSymlink" color="info" variant="subtle" size="sm" icon="i-lucide-link">
            软链接
          </UBadge>
        </template>
        <template #right>
          <template v-if="detail && !editing">
            <UButton icon="i-lucide-pencil" color="neutral" variant="outline" @click="enterEdit">编辑</UButton>
            <UButton icon="i-lucide-text-cursor-input" color="neutral" variant="ghost" @click="openRename">重命名</UButton>
            <UButton icon="i-lucide-trash-2" color="error" variant="soft" @click="remove">删除</UButton>
          </template>
          <template v-else-if="detail">
            <UButton color="neutral" variant="ghost" @click="cancelEdit">取消</UButton>
            <UButton icon="i-lucide-check" :loading="saving" @click="save">保存</UButton>
          </template>
        </template>
      </UDashboardNavbar>
    </template>

    <template #body>
      <UAlert v-if="error" color="error" variant="soft" icon="i-lucide-triangle-alert" :title="error" class="mb-4" />

      <div v-if="loading && !detail" class="space-y-5">
        <USkeleton class="h-4 w-64" />
        <div class="grid grid-cols-1 lg:grid-cols-[250px_1fr] gap-5">
          <USkeleton class="h-48 rounded-lg" />
          <USkeleton class="h-96 rounded-lg" />
        </div>
      </div>

      <template v-if="detail">
        <div v-if="!editing" class="flex flex-wrap items-center gap-x-4 gap-y-1.5 mb-5 text-xs text-muted">
          <span class="inline-flex items-center gap-1.5">
            <UIcon name="i-lucide-folder" class="size-3.5 text-dimmed" />{{ detail.rootLabel }}
          </span>
          <span class="inline-flex items-center gap-1.5 tabular-nums">
            <UIcon name="i-lucide-hard-drive" class="size-3.5 text-dimmed" />{{ formatBytes(detail.sizeBytes) }}
          </span>
          <span v-if="detail.isSymlink" class="inline-flex items-center gap-1.5 min-w-0 max-w-full">
            <UIcon name="i-lucide-corner-down-right" class="size-3.5 text-dimmed shrink-0" />
            <span class="truncate font-mono">{{ detail.symlinkTarget }}</span>
          </span>
        </div>

        <UAlert
          v-if="detail.warnings.length && !editing"
          color="warning"
          variant="soft"
          icon="i-lucide-triangle-alert"
          title="校验提醒"
          class="mb-5"
        >
          <template #description>
            <div v-for="warning in detail.warnings" :key="warning">{{ warning }}</div>
          </template>
        </UAlert>

        <!-- 来源与更新：仅对有安装记录的 skill 展示 -->
        <UCard v-if="installRecord && !editing" class="mb-5" :ui="{ body: 'p-4 sm:p-5' }">
          <div class="flex flex-wrap items-center gap-2">
            <UIcon name="i-lucide-package-search" class="size-4 text-dimmed" />
            <span class="text-sm font-semibold">来源与更新</span>
            <UBadge
              v-if="updateInfo"
              :color="updateStatusMeta[updateInfo.status].color"
              variant="subtle"
              size="sm"
              :icon="updateStatusMeta[updateInfo.status].icon"
            >
              {{ updateStatusMeta[updateInfo.status].label }}
            </UBadge>
            <div class="ml-auto flex items-center gap-1.5">
              <UButton
                size="xs"
                color="neutral"
                variant="outline"
                icon="i-lucide-refresh-cw"
                :loading="updateBusy"
                @click="checkUpdate"
              >
                检查更新
              </UButton>
              <UButton
                v-if="canPreviewUpdate && !updateDiff"
                size="xs"
                icon="i-lucide-file-diff"
                :loading="updateBusy"
                @click="previewUpdate"
              >
                预览更新
              </UButton>
            </div>
          </div>

          <div class="flex flex-wrap items-center gap-x-4 gap-y-1 mt-2.5 text-xs text-muted">
            <span class="inline-flex items-center gap-1.5">
              <UIcon name="i-lucide-git-branch" class="size-3.5 text-dimmed" />{{ installRecord.source.type }}
            </span>
            <span class="font-mono truncate max-w-72" :title="installRecord.source.ref">{{ installRecord.source.ref }}</span>
            <span v-if="installRecord.source.revision" class="font-mono">
              rev {{ installRecord.source.revision.slice(0, 10) }}
            </span>
            <span>安装于 {{ new Date(installRecord.installedAt).toLocaleString() }}</span>
            <span v-if="installRecord.updatedAt">更新于 {{ new Date(installRecord.updatedAt).toLocaleString() }}</span>
          </div>

          <p v-if="updateInfo?.message" class="mt-2 text-xs text-muted">{{ updateInfo.message }}</p>

          <div v-if="updateDiff" class="mt-3 border border-default rounded-md overflow-hidden">
            <div class="flex items-center gap-2 px-3 h-8 bg-elevated/50 text-[11px] font-semibold text-dimmed uppercase tracking-wider">
              文件变化
              <span class="font-normal tabular-nums">{{ updateDiff.entries.length }}</span>
              <UButton
                size="xs"
                class="ml-auto"
                icon="i-lucide-circle-arrow-down"
                :loading="updateBusy"
                @click="applyUpdate"
              >
                应用更新
              </UButton>
            </div>
            <div v-if="!updateDiff.entries.length" class="px-3 py-2.5 text-xs text-muted">
              内容一致，无文件变化。
            </div>
            <div
              v-for="entry in updateDiff.entries"
              :key="entry.path"
              class="flex items-center gap-2 px-3 py-1.5 border-t border-default text-xs"
            >
              <UBadge
                :color="entry.change === 'added' ? 'success' : entry.change === 'removed' ? 'error' : 'warning'"
                variant="subtle"
                size="sm"
                class="w-14 justify-center"
              >
                {{ entry.change === "added" ? "新增" : entry.change === "removed" ? "删除" : "修改" }}
              </UBadge>
              <span class="font-mono truncate">{{ entry.path }}</span>
            </div>
          </div>

          <div v-if="installRecord.backupIds.length" class="flex flex-wrap items-center gap-2 mt-3">
            <span class="text-xs text-muted">更新前备份：</span>
            <UButton
              v-for="backupId in installRecord.backupIds"
              :key="backupId"
              size="xs"
              color="error"
              variant="soft"
              icon="i-lucide-undo-2"
              :loading="updateBusy"
              @click="rollback(backupId)"
            >
              回滚 {{ backupId.replace(/^backup-/, "").slice(0, 8) }}
            </UButton>
          </div>
        </UCard>

        <div
          class="grid gap-5"
          :class="[
            editing && filesCollapsed ? 'grid-cols-1' : 'grid-cols-1 lg:grid-cols-[250px_1fr]',
            editing ? 'flex-1 min-h-0 grid-rows-[auto_1fr] lg:grid-rows-none' : 'items-start',
          ]"
        >
          <UCard
            v-show="!(editing && filesCollapsed)"
            :ui="{ body: 'p-2 sm:p-2.5' }"
            class="self-start lg:sticky lg:top-0"
          >
            <div class="flex items-center gap-1.5 px-2 pt-0.5 pb-2 text-[11px] font-semibold text-dimmed uppercase tracking-wider">
              <UIcon name="i-lucide-files" class="size-3.5" />
              文件
              <span class="ml-auto font-normal tabular-nums">{{ detail.files.length }}</span>
              <UButton
                v-if="editing"
                icon="i-lucide-panel-left-close"
                color="neutral"
                variant="ghost"
                size="xs"
                :ui="{ base: 'p-0.5' }"
                title="隐藏文件列表"
                @click="filesCollapsed = true"
              />
            </div>
            <div
              class="flex items-center gap-1.5 px-2 h-8 rounded-md cursor-pointer transition-colors text-sm"
              :class="activeFile === '' ? 'bg-primary/10 text-primary font-medium' : 'hover:bg-elevated text-default'"
              @click="activeFile = ''"
            >
              <UIcon name="i-lucide-file-text" class="size-4 shrink-0" />
              <span class="flex-1 truncate">SKILL.md</span>
              <UIcon v-if="activeFile === ''" name="i-lucide-check" class="size-3.5 shrink-0" />
            </div>
            <FileTree
              :nodes="detail.files.filter((file) => file.path !== 'SKILL.md')"
              :active-path="activeFile"
              @select="selectFile"
            />
          </UCard>

          <section class="min-w-0" :class="editing ? 'flex flex-col min-h-0' : ''">
            <template v-if="!editing">
              <div v-if="activeFile === ''" class="space-y-4">
                <UCard v-if="Object.keys(detail.frontmatter).length" class="overflow-hidden" :ui="{ body: 'p-0 sm:p-0' }">
                  <div class="flex items-center gap-1.5 px-4 h-9 border-b border-default bg-elevated/50 text-[11px] font-semibold text-dimmed uppercase tracking-wider">
                    <UIcon name="i-lucide-braces" class="size-3.5" />
                    Frontmatter
                  </div>
                  <table class="text-sm w-full">
                    <tbody class="divide-y divide-default">
                      <tr v-for="(value, key) in detail.frontmatter" :key="key">
                        <td class="px-4 py-2.5 align-top w-36 font-mono text-xs text-muted">{{ key }}</td>
                        <td class="px-4 py-2.5 whitespace-pre-wrap break-all text-default">
                          {{ typeof value === "object" ? JSON.stringify(value) : value }}
                        </td>
                      </tr>
                    </tbody>
                  </table>
                </UCard>
                <UCard :ui="{ body: 'p-5 sm:p-6' }"><div class="markdown-body" v-html="renderMarkdown(detail.body)" /></UCard>
              </div>
              <UCard v-else :ui="{ body: 'p-5 sm:p-6' }"><div class="markdown-body" v-html="previewHtml" /></UCard>
            </template>

            <template v-else>
              <div class="flex flex-col gap-3 flex-1 min-h-0">
                <UAlert
                  v-if="isReadonly && !forceEdit"
                  color="warning"
                  variant="soft"
                  icon="i-lucide-link"
                  title="该 skill 为软链接，保存将直接修改源文件。"
                  class="shrink-0"
                />

                <UCollapsible v-if="activeFile === ''" v-model:open="metaOpen" class="shrink-0">
                  <UButton
                    class="w-full justify-between"
                    color="neutral"
                    variant="soft"
                    block
                    :trailing-icon="metaOpen ? 'i-lucide-chevron-up' : 'i-lucide-chevron-down'"
                  >
                    <span class="inline-flex items-center gap-1.5 text-xs font-semibold text-dimmed uppercase tracking-wider">
                      <UIcon name="i-lucide-braces" class="size-3.5" />
                      元信息（名称 / 版本 / 描述 / frontmatter）
                    </span>
                  </UButton>
                  <template #content>
                    <div class="mt-2 rounded-lg ring ring-default bg-default p-4 sm:p-5">
                      <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                        <UFormField label="名称" required>
                          <UInput v-model="form.name" class="w-full font-mono" />
                        </UFormField>
                        <UFormField label="版本">
                          <UInput v-model="form.version" placeholder="1.0.0" class="w-full font-mono" />
                        </UFormField>
                        <UFormField label="描述" class="md:col-span-2">
                          <UTextarea v-model="form.description" :rows="2" class="w-full" />
                        </UFormField>
                        <UFormField label="其余 frontmatter（YAML）" class="md:col-span-2">
                          <UTextarea v-model="form.extraYaml" :rows="3" class="w-full font-mono" />
                        </UFormField>
                      </div>
                    </div>
                  </template>
                </UCollapsible>

                <div class="flex flex-col flex-1 min-h-[26rem] md:min-h-0 rounded-lg ring ring-default bg-default overflow-hidden">
                  <div class="flex items-center gap-1.5 px-3 h-9 shrink-0 border-b border-default bg-elevated/50 text-xs font-medium text-muted">
                    <UButton
                      v-if="filesCollapsed"
                      icon="i-lucide-panel-left-open"
                      color="neutral"
                      variant="ghost"
                      size="xs"
                      :ui="{ base: 'p-0.5' }"
                      title="显示文件列表"
                      @click="filesCollapsed = false"
                    />
                    <UIcon name="i-lucide-pencil-line" class="size-3.5 shrink-0" />
                    <span class="truncate font-mono">{{ activeFile === "" ? "SKILL.md" : activeFile }}</span>
                  </div>
                  <div class="flex-1 min-h-0 overflow-hidden">
                    <MarkdownEditor v-if="isMarkdownFile" v-model="currentText" />
                    <CodeEditor v-else v-model="currentText" language="text" />
                  </div>
                </div>
              </div>
            </template>
          </section>
        </div>
      </template>
    </template>
  </UDashboardPanel>

  <UModal v-model:open="confirmState.open" :title="confirmState.title">
    <template #body>
      <p class="text-sm text-muted leading-relaxed">{{ confirmState.description }}</p>
    </template>
    <template #footer>
      <div class="flex justify-end gap-2 w-full">
        <UButton color="neutral" variant="ghost" @click="confirmState.open = false">取消</UButton>
        <UButton :color="confirmState.confirmColor" :icon="confirmState.confirmIcon || undefined" @click="handleConfirm">
          {{ confirmState.confirmLabel }}
        </UButton>
      </div>
    </template>
  </UModal>

  <UModal v-model:open="renameState.open" title="重命名 skill">
    <template #body>
      <div class="space-y-3">
        <p class="text-sm text-muted leading-relaxed">
          将同步重命名目录名与 frontmatter 中的 name 字段。软链接仅重命名链接本身。
        </p>
        <UFormField label="新名称" required>
          <UInput
            v-model="renameState.value"
            class="w-full font-mono"
            placeholder="my-skill"
            autofocus
            @keydown.enter="doRename"
          />
        </UFormField>
      </div>
    </template>
    <template #footer>
      <div class="flex justify-end gap-2 w-full">
        <UButton color="neutral" variant="ghost" @click="renameState.open = false">取消</UButton>
        <UButton icon="i-lucide-check" :loading="renameState.busy" @click="doRename">确定</UButton>
      </div>
    </template>
  </UModal>
</template>
