<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { storeToRefs } from 'pinia'
import type { AddSourceRequest, CatalogSkill, CommitPlanResponse, ConflictStrategy, InstallPlan, PrepareSourceResponse, ProjectAgentDir, RemoteSource, SkillRoot, SkillScope, SkillsShSearchItem, SkillSource } from '@/types/skills'
import { useSkillsStore } from '@/stores/skills'
import { skillsApi, pickDirectory, pickFile } from '@/api/skills'
import { formatBytes } from '@/utils/skills-markdown'

const store = useSkillsStore()
const { roots, projects } = storeToRefs(store)
const router = useRouter()
const route = useRoute()

const tab = ref<'remote' | 'local' | 'upload'>('remote')
// 安装目标：范围（全局/项目）+ 多选根目录（可同时装到多个 Agent 目录）
const targetScope = ref<SkillScope>('global')
const targetProjectId = ref('')
const selectedRootIds = ref<string[]>([])
const onConflict = ref<ConflictStrategy>('keep-both')
const busy = ref(false)
const error = ref('')
const okMsg = ref('')

const writableRoots = computed(() => roots.value.filter((r) => r.writable))
/** 全局范围下可选的目标目录 */
const scopeRoots = computed(() => writableRoots.value.filter((r) => r.scope === 'global'))
const projectOptions = computed(() => projects.value.map((p) => ({ label: p.label, value: p.id })))

// 项目级：选定项目后拉取已知 Agent 的候选 skill 目录（含尚未创建的）
const projectAgentDirs = ref<ProjectAgentDir[]>([])
const agentDirsLoading = ref(false)
const moreAgentsOpen = ref(false)
const pickingProject = ref(false)
const existingAgentDirs = computed(() => projectAgentDirs.value.filter((d) => d.exists))
const missingAgentDirs = computed(() => projectAgentDirs.value.filter((d) => !d.exists))

function agentDirDisplay(d: ProjectAgentDir): string {
  return d.agent ?? 'skills'
}

function rootDisplay(r: SkillRoot): string {
  return r.agent || r.label
}

function toggleRoot(id: string) {
  selectedRootIds.value = selectedRootIds.value.includes(id)
    ? selectedRootIds.value.filter((x) => x !== id)
    : [...selectedRootIds.value, id]
}

/** 拉取项目的候选 Agent 目录；默认选中首个已存在目录，无已存在时自动展开全部 */
async function loadAgentDirs() {
  projectAgentDirs.value = []
  selectedRootIds.value = []
  moreAgentsOpen.value = false
  if (!targetProjectId.value) return
  agentDirsLoading.value = true
  try {
    projectAgentDirs.value = await skillsApi.listProjectAgentDirs(targetProjectId.value)
    const first = projectAgentDirs.value.find((d) => d.exists && d.writable)
    selectedRootIds.value = first ? [first.rootId] : []
    moreAgentsOpen.value = !projectAgentDirs.value.some((d) => d.exists)
  } catch (e) {
    error.value = (e as Error).message
  } finally {
    agentDirsLoading.value = false
  }
}

// 切换范围/项目时重置选择；项目范围默认选中第一个项目并拉取候选
watch(targetScope, (scope) => {
  if (scope === 'project') {
    const firstProject = projects.value[0]?.id ?? ''
    if (!targetProjectId.value && firstProject) {
      targetProjectId.value = firstProject // 触发下方 watch 拉取候选
    } else {
      void loadAgentDirs()
    }
  } else {
    selectedRootIds.value = scopeRoots.value.length ? [scopeRoots.value[0]!.id] : []
  }
})
watch(targetProjectId, () => {
  if (targetScope.value === 'project') void loadAgentDirs()
})

/** 项目级：唤起访达选择项目目录，自动登记（同路径幂等）并选中 */
async function pickProjectDir() {
  pickingProject.value = true
  error.value = ''
  try {
    const path = await pickDirectory('选择项目根目录')
    if (!path) return
    const project = await skillsApi.addProject(path)
    await store.refresh()
    if (targetProjectId.value === project.id) void loadAgentDirs()
    else targetProjectId.value = project.id // 触发上方 watch 拉取候选
  } catch (e) {
    error.value = (e as Error).message
  } finally {
    pickingProject.value = false
  }
}
const conflictOptions = [
  { label: '保留两者（追加后缀）', value: 'keep-both' },
  { label: '覆盖已有', value: 'overwrite' },
  { label: '跳过（存在则报错）', value: 'skip' },
]

const tabItems = [
  { label: '远程安装', value: 'remote', icon: 'i-lucide-globe' },
  { label: '本地路径', value: 'local', icon: 'i-lucide-folder' },
  { label: '上传压缩包', value: 'upload', icon: 'i-lucide-upload' },
]

// 本地路径
const sourcePath = ref('')
const pickingSource = ref(false)
// 上传（桌面端经原生对话框选择本地归档路径）
const uploadPath = ref('')
const uploadName = computed(() => uploadPath.value.split(/[\\/]/).pop() || '')
// 远程
const remoteSource = ref<RemoteSource>('github')
const remoteRef = ref('')
// 技能目录（内置精选 + 已接入数据源索引）
const catalog = ref<CatalogSkill[]>([])
const catalogLoading = ref(true)
const catalogFilter = ref('')
const catalogGroup = ref('all')
const installingCatalogId = ref('')
const previewPanel = ref<HTMLElement | null>(null)
// 数据源管理
const sourcesOpen = ref(false)
const sources = ref<SkillSource[]>([])
const sourcesLoading = ref(false)
const sourceBusyId = ref('')
const sourceError = ref('')
const newSourceRepo = ref('')
const newSourceLabel = ref('')
const addSourceBusy = ref(false)
// 安装向导状态：来源准备 → 候选选择 → 计划确认 → 逐目标结果
const prep = ref<PrepareSourceResponse | null>(null)
const selectedSkillIds = ref<string[]>([])
const plan = ref<InstallPlan | null>(null)
const ackCodes = ref<string[]>([])
const commitResult = ref<CommitPlanResponse | null>(null)
const sourceItems = [
  { label: 'GitHub', value: 'github' },
  { label: 'skills.sh', value: 'skills-sh' },
  { label: '技能市场', value: 'market' },
  { label: '任意 URL', value: 'http' },
]

const remotePlaceholder = computed(() => {
  if (remoteSource.value === 'github') return '如：anthropics/skills/skills/pdf 或完整 GitHub URL'
  if (remoteSource.value === 'skills-sh') return '如：vercel-labs/agent-skills/vercel-react-best-practices'
  if (remoteSource.value === 'market') return '技能市场链接（返回 zip/tar 或 SKILL.md）'
  return '任意 http(s) 链接：.zip / .tar.gz / 或 raw SKILL.md'
})

function ensureRoot(): boolean {
  if (targetScope.value === 'global' && !selectedRootIds.value.length && scopeRoots.value.length) {
    selectedRootIds.value = [scopeRoots.value[0]!.id]
  }
  if (!selectedRootIds.value.length) {
    error.value = targetScope.value === 'project'
      ? '请选择要安装到的 Agent 目录（尚未创建的目录会在安装时自动创建）'
      : '没有可写的目标根目录'
    return false
  }
  return true
}

function reset() {
  error.value = ''
  okMsg.value = ''
}

function onTabChange() {
  reset()
  resetWizard()
}

async function chooseUpload() {
  const selected = await pickFile('选择 Skill 归档（.zip / .tar.gz）')
  if (!selected) return
  uploadPath.value = selected
  resetWizard()
}

/** 清空安装向导全部中间状态（不调后端） */
function resetWizard() {
  prep.value = null
  selectedSkillIds.value = []
  plan.value = null
  ackCodes.value = []
  commitResult.value = null
}

/** 取消向导：同步释放服务端 staging / 计划 */
async function cancelWizard() {
  const stagingId = prep.value?.stagingId
  const planId = plan.value?.id
  resetWizard()
  try {
    if (planId) await skillsApi.cancelInstallPlan(planId)
    if (stagingId) await skillsApi.removeStaging(stagingId)
  } catch { /* 服务端会自动过期清理，失败可忽略 */ }
}

/** 统一封装来源准备请求；成功后默认选中全部可安装候选 */
async function runPrepare(fn: () => Promise<PrepareSourceResponse>) {
  reset()
  resetWizard()
  busy.value = true
  try {
    const res = await fn()
    prep.value = res
    selectedSkillIds.value = res.skills.filter((s) => s.installable).map((s) => s.id)
  } catch (e) {
    error.value = (e as Error).message
  } finally {
    busy.value = false
  }
}

function previewRemote() {
  return runPrepare(() => skillsApi.prepareRemoteSource(remoteSource.value, remoteRef.value.trim()))
}

/** 来源筛选分组：全部 / 精选 / 各已接入源 */
const catalogGroups = computed(() => {
  const groups: Array<{ label: string; value: string }> = [{ label: '全部', value: 'all' }, { label: '精选', value: 'curated' }]
  const seen = new Set<string>()
  for (const c of catalog.value) {
    if (c.sourceId && !seen.has(c.sourceId)) {
      seen.add(c.sourceId)
      groups.push({ label: c.tags[0] || c.repo, value: c.sourceId })
    }
  }
  return groups
})

const filteredCatalog = computed(() => {
  let list = catalog.value
  if (catalogGroup.value === 'curated') list = list.filter((c) => !c.sourceId)
  else if (catalogGroup.value !== 'all') list = list.filter((c) => c.sourceId === catalogGroup.value)
  const q = catalogFilter.value.trim().toLowerCase()
  if (!q) return list
  return list.filter((c) =>
    c.name.toLowerCase().includes(q)
    || c.description.toLowerCase().includes(q)
    || c.repo.toLowerCase().includes(q)
    || c.tags.some((t) => t.toLowerCase().includes(q)))
})

function formatStars(n: number): string {
  return n >= 1000 ? `${(n / 1000).toFixed(1).replace(/\.0$/, '')}k` : String(n)
}

function formatSyncTime(iso?: string): string {
  return iso ? new Date(iso).toLocaleString() : '从未同步'
}

/** 拉取完整目录（首次同步源可能较慢，失败静默降级） */
async function loadCatalog() {
  catalogLoading.value = true
  try {
    catalog.value = await skillsApi.getCatalog()
  } catch {
    // 静默降级：仅隐藏区块
  } finally {
    catalogLoading.value = false
  }
}

// ---------- 数据源管理 ----------

async function loadSources() {
  sourcesLoading.value = true
  try {
    sources.value = await skillsApi.listSources()
  } catch (e) {
    sourceError.value = (e as Error).message
  } finally {
    sourcesLoading.value = false
  }
}

function openSources() {
  sourceError.value = ''
  sourcesOpen.value = true
  loadSources()
}

async function handleAddSource() {
  const repo = newSourceRepo.value.trim()
  if (!repo) return
  sourceError.value = ''
  addSourceBusy.value = true
  try {
    const body: AddSourceRequest = { repo, label: newSourceLabel.value.trim() || undefined }
    const created = await skillsApi.addSource(body)
    newSourceRepo.value = ''
    newSourceLabel.value = ''
    await loadSources()
    // 新源立即同步一次并刷新目录
    await handleSyncSource(created.id)
  } catch (e) {
    sourceError.value = (e as Error).message
  } finally {
    addSourceBusy.value = false
  }
}

async function handleSyncSource(id: string) {
  sourceError.value = ''
  sourceBusyId.value = id
  try {
    await skillsApi.syncSource(id)
    await loadSources()
    await loadCatalog()
  } catch (e) {
    sourceError.value = (e as Error).message
    await loadSources()
  } finally {
    sourceBusyId.value = ''
  }
}

async function handleRemoveSource(id: string) {
  sourceError.value = ''
  sourceBusyId.value = id
  try {
    await skillsApi.removeSource(id)
    if (catalogGroup.value === id) catalogGroup.value = 'all'
    await loadSources()
    await loadCatalog()
  } catch (e) {
    sourceError.value = (e as Error).message
  } finally {
    sourceBusyId.value = ''
  }
}

/** 从目录一键填入 ref 并拉取预览，后续走统一的确认安装流程 */
async function installFromCatalog(item: CatalogSkill) {
  remoteSource.value = 'github'
  remoteRef.value = item.installRef
  installingCatalogId.value = item.id
  try {
    await previewRemote()
  } finally {
    installingCatalogId.value = ''
  }
  if (prep.value) {
    await nextTick()
    previewPanel.value?.scrollIntoView({ behavior: 'smooth', block: 'start' })
  }
}

// ---------- skills.sh 全网搜索（与技能目录共用同一搜索框：本地即时过滤 + 全网防抖检索） ----------
const shResults = ref<SkillsShSearchItem[]>([])
const shLoading = ref(false)
const shSearched = ref(false)
const shError = ref('')
const installingShId = ref('')
let shTimer: ReturnType<typeof setTimeout> | undefined

async function runShSearch() {
  const q = catalogFilter.value.trim()
  if (q.length < 2) return
  shLoading.value = true
  shError.value = ''
  try {
    const results = await skillsApi.searchSkillsSh(q)
    // 丢弃过时响应（输入已变化）
    if (catalogFilter.value.trim() === q) {
      shResults.value = results
      shSearched.value = true
    }
  } catch (e) {
    shError.value = (e as Error).message
  } finally {
    shLoading.value = false
  }
}

watch(catalogFilter, (q) => {
  if (shTimer) clearTimeout(shTimer)
  if (q.trim().length < 2) {
    shResults.value = []
    shSearched.value = false
    shError.value = ''
    return
  }
  shTimer = setTimeout(runShSearch, 400)
})

onUnmounted(() => {
  if (shTimer) clearTimeout(shTimer)
})

function formatInstalls(n?: number): string {
  if (!n || n <= 0) return ''
  if (n >= 1e6) return `${(n / 1e6).toFixed(1).replace(/\.0$/, '')}M`
  if (n >= 1e3) return `${(n / 1e3).toFixed(1).replace(/\.0$/, '')}k`
  return String(n)
}

/** 从 skills.sh 搜索结果一键填入 ref 并预览，后续走统一确认安装流程 */
async function installFromSkillsSh(item: SkillsShSearchItem) {
  remoteSource.value = 'skills-sh'
  remoteRef.value = item.id
  installingShId.value = item.id
  try {
    await previewRemote()
  } finally {
    installingShId.value = ''
  }
  if (prep.value) {
    await nextTick()
    previewPanel.value?.scrollIntoView({ behavior: 'smooth', block: 'start' })
  }
}

// ---------- 远程技能详情页跳转（独立页面，与本地 skill 详情一致） ----------
function openCatalogDetail(item: CatalogSkill) {
  router.push({
    name: 'skills-remote',
    query: {
      source: 'github',
      ref: item.installRef,
      homepage: item.homepage,
      ...(item.hasScripts ? { hasScripts: '1' } : {}),
    },
  })
}

function openShDetail(item: SkillsShSearchItem) {
  router.push({
    name: 'skills-remote',
    query: { source: 'skills-sh', ref: item.id, homepage: item.repoUrl, detailUrl: item.detailUrl },
  })
}

/** 唤起系统原生目录选择窗口，回填本地 skill 目录路径 */
async function pickSourceDir() {
  pickingSource.value = true
  error.value = ''
  try {
    const path = await pickDirectory('选择包含 SKILL.md 的 skill 目录', sourcePath.value.trim() || undefined)
    if (path) sourcePath.value = path
  } catch (e) {
    error.value = (e as Error).message
  } finally {
    pickingSource.value = false
  }
}

function previewLocal() {
  return runPrepare(() => skillsApi.prepareLocalSource(sourcePath.value.trim()))
}

function previewUpload() {
  if (!uploadPath.value) {
    error.value = '请选择要上传的压缩包'
    return Promise.resolve()
  }
  return runPrepare(() => skillsApi.prepareUploadSource(uploadPath.value, uploadName.value))
}

// ---------- 计划与提交 ----------

function toggleSkill(id: string) {
  const s = prep.value?.skills.find((x) => x.id === id)
  if (!s?.installable) return
  selectedSkillIds.value = selectedSkillIds.value.includes(id)
    ? selectedSkillIds.value.filter((x) => x !== id)
    : [...selectedSkillIds.value, id]
}

// 候选 / 目标 / 冲突策略变化后，既有计划失效（需重新生成）
watch([selectedSkillIds, selectedRootIds, onConflict], () => {
  if (plan.value && !commitResult.value) {
    plan.value = null
    ackCodes.value = []
  }
})

/** 生成不可变安装计划：冲突与动作固化，warning 需显式确认 */
async function makePlan() {
  if (!prep.value) return
  if (!selectedSkillIds.value.length) {
    error.value = '请至少选择一个候选 Skill'
    return
  }
  if (!ensureRoot()) return
  reset()
  busy.value = true
  try {
    plan.value = await skillsApi.createInstallPlan({
      stagingId: prep.value.stagingId,
      skillIds: selectedSkillIds.value,
      targetRootIds: selectedRootIds.value,
      onConflict: onConflict.value,
    })
    ackCodes.value = []
  } catch (e) {
    error.value = (e as Error).message
  } finally {
    busy.value = false
  }
}

const planWarnings = computed(() => plan.value?.issues.filter((i) => i.level === 'warning') ?? [])
const allWarningsAcked = computed(() => planWarnings.value.every((i) => ackCodes.value.includes(i.code)))

function toggleAck(code: string) {
  ackCodes.value = ackCodes.value.includes(code)
    ? ackCodes.value.filter((c) => c !== code)
    : [...ackCodes.value, code]
}

function rootLabelOf(rootId: string): string {
  const r = roots.value.find((x) => x.id === rootId)
  if (r) return rootDisplay(r)
  const d = projectAgentDirs.value.find((x) => x.rootId === rootId)
  if (d) return agentDirDisplay(d)
  const t = plan.value?.targets.find((x) => x.rootId === rootId)
  return t?.label ?? rootId
}

const actionMeta: Record<string, { label: string; color: 'primary' | 'warning' | 'neutral' | 'info' }> = {
  'install': { label: '安装', color: 'primary' },
  'overwrite': { label: '覆盖', color: 'warning' },
  'skip': { label: '跳过', color: 'neutral' },
  'keep-both': { label: '保留两者', color: 'info' },
}

/** 提交计划：逐 Skill 逐目标事务安装，展示逐目标结果 */
async function commitPlan() {
  if (!plan.value) return
  reset()
  busy.value = true
  try {
    commitResult.value = await skillsApi.commitInstallPlan(plan.value.id, {
      confirmationHash: plan.value.confirmationHash,
      acknowledgedIssueCodes: ackCodes.value,
    })
    const flat = commitResult.value.outcomes.flatMap((o) => o.targets)
    const installed = flat.filter((t) => t.status === 'installed').length
    const failed = flat.filter((t) => t.status === 'failed').length
    const skipped = flat.filter((t) => t.status === 'skipped').length
    if (!failed) {
      okMsg.value = `安装完成：成功 ${installed} 项${skipped ? `，跳过 ${skipped} 项` : ''}`
    } else {
      error.value = `部分目标安装失败：成功 ${installed} 项，失败 ${failed} 项，可重新生成计划重试`
    }
    await store.refresh()
    // 安装可能新建了 Agent 目录，刷新候选状态（exists/skillCount）
    if (targetScope.value === 'project') void loadAgentDirs()
  } catch (e) {
    error.value = (e as Error).message
    // 计划可能已失效（过期/指纹不匹配），回到计划前重新生成
    if (/重新生成|过期/.test((e as Error).message)) plan.value = null
  } finally {
    busy.value = false
  }
}

/** 提交失败后重开计划（staging 仍保留） */
function retryPlan() {
  commitResult.value = null
  plan.value = null
  ackCodes.value = []
  reset()
}

/** 完成向导（全部成功后 staging 已被服务端清理） */
function finishWizard() {
  resetWizard()
}

function goList() {
  router.push({ name: 'skills-library' })
}

onMounted(async () => {
  // 目录加载不阻塞主流程，失败静默降级（仅隐藏区块）
  void loadCatalog()
  await store.refresh()
  if (!selectedRootIds.value.length && scopeRoots.value.length) {
    selectedRootIds.value = [scopeRoots.value[0]!.id]
  }
  // 从详情页「安装」跳转过来：自动填入并拉取预览
  const q = route.query
  if (q.auto === '1' && typeof q.ref === 'string' && q.ref) {
    remoteSource.value = (q.source as RemoteSource) || 'github'
    remoteRef.value = q.ref
    void router.replace({ query: {} })
    await previewRemote()
    if (prep.value) {
      await nextTick()
      previewPanel.value?.scrollIntoView({ behavior: 'smooth', block: 'start' })
    }
  }
})
</script>

<template>
  <UDashboardPanel id="install">
    <template #header>
      <UDashboardNavbar title="安装" description="Skills">
        <template #leading>
          <UDashboardSidebarCollapse />
        </template>
      </UDashboardNavbar>
    </template>

    <template #body>
      <div class="max-w-3xl w-full mx-auto space-y-5">
        <!-- Tab 切换（下划线风格） -->
        <UTabs
          v-model="tab"
          :items="tabItems"
          :content="false"
          variant="link"
          color="primary"
          @update:model-value="onTabChange"
        />

        <!-- 公共：安装目标（范围 + 多选 Agent 目录）+ 冲突策略 -->
        <UCard :ui="{ body: 'p-4 sm:p-5' }">
          <div class="flex items-center gap-1.5 mb-4 text-[11px] font-semibold text-dimmed uppercase tracking-wider">
            <UIcon name="i-lucide-settings-2" class="size-3.5" />
            安装设置
          </div>
          <div class="space-y-4">
            <UFormField label="安装范围">
              <div class="flex flex-wrap items-center gap-2">
                <button
                  type="button"
                  class="rounded-full px-3 py-1.5 text-xs transition-colors cursor-pointer inline-flex items-center gap-1.5"
                  :class="targetScope === 'global'
                    ? 'bg-primary/10 text-primary ring ring-primary/30 font-medium'
                    : 'ring ring-default bg-elevated/40 hover:bg-elevated text-default'"
                  @click="targetScope = 'global'"
                >
                  <UIcon name="i-lucide-globe" class="size-3.5" />全局（用户级）
                </button>
                <button
                  type="button"
                  class="rounded-full px-3 py-1.5 text-xs transition-colors cursor-pointer inline-flex items-center gap-1.5"
                  :class="targetScope === 'project'
                    ? 'bg-primary/10 text-primary ring ring-primary/30 font-medium'
                    : 'ring ring-default bg-elevated/40 hover:bg-elevated text-default'"
                  @click="targetScope = 'project'"
                >
                  <UIcon name="i-lucide-folder-git-2" class="size-3.5" />项目级
                </button>
                <USelect
                  v-if="targetScope === 'project' && projectOptions.length"
                  v-model="targetProjectId"
                  :items="projectOptions"
                  icon="i-lucide-folder"
                  size="sm"
                  class="w-full sm:w-56"
                />
                <UButton
                  v-if="targetScope === 'project'"
                  icon="i-lucide-folder-search"
                  color="neutral"
                  variant="outline"
                  size="sm"
                  :loading="pickingProject"
                  @click="pickProjectDir"
                >
                  选择项目…
                </UButton>
              </div>
            </UFormField>

            <UFormField v-if="targetScope === 'project' && !projectOptions.length">
              <div class="text-xs text-dimmed">
                还没有关联项目，点击上方「选择项目…」通过访达选择，或
                <UButton variant="link" size="xs" class="px-0" trailing-icon="i-lucide-arrow-right" @click="router.push({ name: 'skills-roots' })">去关联项目</UButton>
              </div>
            </UFormField>

            <UFormField v-else :label="`目标目录（可多选，已选 ${selectedRootIds.length}）`">
              <!-- 全局：已登记的可写根目录 -->
              <template v-if="targetScope === 'global'">
                <div v-if="scopeRoots.length" class="flex flex-wrap items-center gap-2">
                  <button
                    v-for="r in scopeRoots"
                    :key="r.id"
                    type="button"
                    :title="r.path"
                    class="rounded-full px-3 py-1.5 text-xs transition-colors cursor-pointer inline-flex items-center gap-1.5"
                    :class="selectedRootIds.includes(r.id)
                      ? 'bg-primary/10 text-primary ring ring-primary/30 font-medium'
                      : 'ring ring-default bg-elevated/40 hover:bg-elevated text-default'"
                    @click="toggleRoot(r.id)"
                  >
                    <UIcon :name="selectedRootIds.includes(r.id) ? 'i-lucide-check' : 'i-lucide-folder-tree'" class="size-3.5" />
                    {{ rootDisplay(r) }}
                    <span class="text-[10px] text-dimmed tabular-nums">{{ r.skillCount ?? 0 }}</span>
                  </button>
                </div>
                <div v-else class="text-xs text-dimmed">没有可写的全局根目录</div>
              </template>

              <!-- 项目级：已知 Agent 候选目录，已存在的优先，其余折叠展开 -->
              <template v-else>
                <div v-if="agentDirsLoading" class="flex items-center gap-2 py-1 text-xs text-dimmed">
                  <UIcon name="i-lucide-loader-circle" class="size-3.5 animate-spin" />探测项目内的 Agent 目录…
                </div>
                <template v-else-if="projectAgentDirs.length">
                  <div class="flex flex-wrap items-center gap-2">
                    <button
                      v-for="d in existingAgentDirs"
                      :key="d.rootId"
                      type="button"
                      :title="d.path"
                      :disabled="!d.writable"
                      class="rounded-full px-3 py-1.5 text-xs transition-colors cursor-pointer inline-flex items-center gap-1.5 disabled:opacity-50 disabled:cursor-not-allowed"
                      :class="selectedRootIds.includes(d.rootId)
                        ? 'bg-primary/10 text-primary ring ring-primary/30 font-medium'
                        : 'ring ring-default bg-elevated/40 hover:bg-elevated text-default'"
                      @click="toggleRoot(d.rootId)"
                    >
                      <UIcon :name="selectedRootIds.includes(d.rootId) ? 'i-lucide-check' : 'i-lucide-folder-tree'" class="size-3.5" />
                      {{ agentDirDisplay(d) }}
                      <span class="text-[10px] text-dimmed tabular-nums">{{ d.skillCount }}</span>
                    </button>
                    <template v-if="moreAgentsOpen">
                      <button
                        v-for="d in missingAgentDirs"
                        :key="d.rootId"
                        type="button"
                        :title="`安装时将创建：${d.path}`"
                        :disabled="!d.writable"
                        class="rounded-full px-3 py-1.5 text-xs transition-colors cursor-pointer inline-flex items-center gap-1.5 disabled:opacity-50 disabled:cursor-not-allowed"
                        :class="selectedRootIds.includes(d.rootId)
                          ? 'bg-primary/10 text-primary ring ring-primary/30 ring-dashed font-medium'
                          : 'ring ring-default ring-dashed bg-elevated/20 hover:bg-elevated text-dimmed hover:text-default'"
                        @click="toggleRoot(d.rootId)"
                      >
                        <UIcon :name="selectedRootIds.includes(d.rootId) ? 'i-lucide-check' : 'i-lucide-folder-plus'" class="size-3.5" />
                        {{ agentDirDisplay(d) }}
                      </button>
                    </template>
                    <button
                      v-if="missingAgentDirs.length && existingAgentDirs.length"
                      type="button"
                      class="rounded-full px-3 py-1.5 text-xs cursor-pointer inline-flex items-center gap-1 text-dimmed hover:text-default transition-colors"
                      @click="moreAgentsOpen = !moreAgentsOpen"
                    >
                      <UIcon :name="moreAgentsOpen ? 'i-lucide-chevron-up' : 'i-lucide-chevron-down'" class="size-3.5" />
                      {{ moreAgentsOpen ? '收起' : `更多 Agent（${missingAgentDirs.length}）` }}
                    </button>
                  </div>
                  <div v-if="moreAgentsOpen && missingAgentDirs.length" class="mt-1.5 text-[11px] text-dimmed">
                    虚线胶囊为尚未创建的 Agent 目录，选中安装时会自动创建
                  </div>
                </template>
                <div v-else class="text-xs text-dimmed">该项目不可用（目录不存在或无候选 Agent 目录）</div>
              </template>
            </UFormField>

            <UFormField label="同名冲突策略" class="sm:w-64">
              <USelect v-model="onConflict" :items="conflictOptions" icon="i-lucide-git-merge" class="w-full" />
            </UFormField>
          </div>
        </UCard>

        <!-- 远程 -->
        <UCard v-if="tab === 'remote'" :ui="{ body: 'p-4 sm:p-5' }">
          <div class="flex items-start gap-3 mb-5">
            <span class="flex size-9 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary ring-1 ring-primary/15 ring-inset">
              <UIcon name="i-lucide-globe" class="size-4.5" />
            </span>
            <div>
              <div class="text-sm font-semibold text-highlighted">远程安装</div>
              <div class="text-xs text-muted mt-0.5">从 GitHub、技能市场或任意 URL 拉取，先预览再确认安装。</div>
            </div>
          </div>

          <UFormField label="来源类型" class="mb-4">
            <div class="flex flex-wrap items-center gap-2">
              <span class="text-xs text-dimmed mr-1">来源</span>
              <button
                v-for="opt in sourceItems"
                :key="opt.value"
                type="button"
                class="rounded-full px-3 py-1.5 text-xs transition-colors cursor-pointer"
                :class="remoteSource === opt.value
                  ? 'bg-primary/10 text-primary ring ring-primary/30 font-medium'
                  : 'ring ring-default bg-elevated/40 hover:bg-elevated text-default'"
                @click="remoteSource = opt.value as RemoteSource"
              >
                {{ opt.label }}
              </button>
            </div>
          </UFormField>
          <UFormField label="来源地址">
            <div class="flex gap-2">
              <UInput
                v-model="remoteRef"
                icon="i-lucide-link-2"
                :placeholder="remotePlaceholder"
                class="flex-1 font-mono"
                @keyup.enter="previewRemote"
              />
              <UButton icon="i-lucide-eye" color="neutral" variant="outline" :loading="busy && !prep" :disabled="!remoteRef.trim()" @click="previewRemote">
                获取预览
              </UButton>
            </div>
          </UFormField>
        </UCard>

        <!-- 本地路径 -->
        <UCard v-else-if="tab === 'local'" :ui="{ body: 'p-4 sm:p-5' }">
          <div class="flex items-start gap-3 mb-5">
            <span class="flex size-9 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary ring-1 ring-primary/15 ring-inset">
              <UIcon name="i-lucide-folder" class="size-4.5" />
            </span>
            <div>
              <div class="text-sm font-semibold text-highlighted">从本机目录安装</div>
              <div class="text-xs text-muted mt-0.5">复制本机某个 skill 目录（需包含 SKILL.md）到目标根目录，先预览再确认。</div>
            </div>
          </div>

          <UFormField label="本机 skill 目录的绝对路径">
            <div class="flex gap-2">
              <UInput
                v-model="sourcePath"
                icon="i-lucide-folder-input"
                placeholder="/path/to/my-skill"
                class="flex-1 font-mono"
                @keyup.enter="previewLocal"
              />
              <UButton icon="i-lucide-folder-search" color="neutral" variant="outline" :loading="pickingSource" @click="pickSourceDir">
                浏览
              </UButton>
              <UButton icon="i-lucide-eye" color="neutral" variant="outline" :loading="busy && !prep" :disabled="!sourcePath.trim()" @click="previewLocal">
                获取预览
              </UButton>
            </div>
          </UFormField>
        </UCard>

        <!-- 上传压缩包 -->
        <UCard v-else :ui="{ body: 'p-4 sm:p-5' }">
          <div class="flex items-start gap-3 mb-5">
            <span class="flex size-9 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary ring-1 ring-primary/15 ring-inset">
              <UIcon name="i-lucide-upload" class="size-4.5" />
            </span>
            <div>
              <div class="text-sm font-semibold text-highlighted">上传压缩包</div>
              <div class="text-xs text-muted mt-0.5">上传包含 SKILL.md 的 .zip / .tar.gz 压缩包，先预览再确认安装。</div>
            </div>
          </div>

          <button
            type="button"
            class="flex flex-col items-center justify-center gap-2 w-full rounded-lg border-2 border-dashed py-10 px-4 text-center transition-colors"
            :class="uploadPath ? 'border-primary bg-primary/5' : 'border-accented hover:border-primary/50 hover:bg-elevated/50'"
            @click="chooseUpload"
          >
            <UIcon :name="uploadPath ? 'i-lucide-file-archive' : 'i-lucide-cloud-upload'" class="size-8" :class="uploadPath ? 'text-primary' : 'text-dimmed'" />
            <template v-if="uploadPath">
              <span class="text-sm font-medium text-highlighted break-all">{{ uploadName }}</span>
              <span class="text-xs text-dimmed">点击更换压缩包</span>
            </template>
            <template v-else>
              <span class="text-sm text-default">点击选择压缩包</span>
              <span class="text-xs text-dimmed">支持 .zip / .tar.gz / .tgz</span>
            </template>
          </button>

          <div class="flex justify-end mt-4">
            <UButton icon="i-lucide-eye" color="neutral" variant="outline" :loading="busy && !prep" :disabled="!uploadPath" @click="previewUpload">
              获取预览
            </UButton>
          </div>
        </UCard>

        <!-- 安装向导面板（三种来源统一）：候选选择 → 计划确认 → 逐目标结果 -->
        <div v-if="prep" ref="previewPanel" class="rounded-lg ring ring-default overflow-hidden">
          <div class="flex items-center gap-2 px-4 h-10 border-b border-default bg-elevated/50">
            <UIcon name="i-lucide-package-search" class="size-4 text-dimmed" />
            <span class="font-semibold text-sm text-highlighted truncate">
              {{ commitResult ? '安装结果' : plan ? '确认安装计划' : '选择要安装的 Skill' }}
            </span>
            <UBadge color="neutral" variant="subtle" size="sm">{{ prep.skills.length }} 个候选</UBadge>
            <UButton
              v-if="!commitResult" icon="i-lucide-x" color="neutral" variant="ghost" size="xs"
              class="ml-auto" :disabled="busy" @click="cancelWizard"
            >
              取消
            </UButton>
          </div>
          <div class="p-4 space-y-4">
            <template v-if="!commitResult">
              <!-- ① 候选选择（error 级问题的候选不可选） -->
              <div class="space-y-2">
                <button
                  v-for="s in prep.skills"
                  :key="s.id"
                  type="button"
                  :disabled="!s.installable"
                  class="w-full text-left rounded-lg ring p-3 transition-colors cursor-pointer disabled:cursor-not-allowed disabled:opacity-60"
                  :class="selectedSkillIds.includes(s.id) ? 'ring-primary/40 bg-primary/5' : 'ring-default hover:bg-elevated/50'"
                  @click="toggleSkill(s.id)"
                >
                  <div class="flex items-center gap-2">
                    <UIcon
                      :name="selectedSkillIds.includes(s.id) ? 'i-lucide-square-check-big' : 'i-lucide-square'"
                      class="size-4 shrink-0" :class="selectedSkillIds.includes(s.id) ? 'text-primary' : 'text-dimmed'"
                    />
                    <span class="text-sm font-medium text-highlighted truncate">{{ s.name }}</span>
                    <UBadge v-if="s.version" color="neutral" variant="subtle" size="sm" class="font-mono">v{{ s.version }}</UBadge>
                    <UBadge v-if="!s.installable" color="error" variant="subtle" size="sm" icon="i-lucide-ban">不可安装</UBadge>
                    <UBadge v-else-if="s.hasScripts" color="warning" variant="subtle" size="sm" icon="i-lucide-terminal">含脚本</UBadge>
                    <UBadge v-if="s.hasBinaryFiles" color="neutral" variant="subtle" size="sm" icon="i-lucide-file-digit">二进制</UBadge>
                    <span class="ml-auto text-xs text-dimmed tabular-nums shrink-0">{{ s.fileCount }} 文件 · {{ formatBytes(s.sizeBytes) }}</span>
                  </div>
                  <p v-if="s.description" class="text-xs text-muted mt-1 ml-6 line-clamp-2">{{ s.description }}</p>
                  <div v-if="s.issues.length" class="mt-1.5 ml-6 space-y-0.5">
                    <div
                      v-for="i in s.issues"
                      :key="i.code + i.message"
                      class="flex items-start gap-1.5 text-xs"
                      :class="i.level === 'error' ? 'text-error' : i.level === 'warning' ? 'text-warning' : 'text-dimmed'"
                    >
                      <UIcon
                        :name="i.level === 'error' ? 'i-lucide-circle-x' : i.level === 'warning' ? 'i-lucide-triangle-alert' : 'i-lucide-info'"
                        class="size-3.5 shrink-0 mt-0.5"
                      />{{ i.message }}
                    </div>
                  </div>
                </button>
              </div>

              <!-- ② 生成计划 -->
              <div v-if="!plan" class="flex justify-end">
                <UButton icon="i-lucide-clipboard-list" :loading="busy" :disabled="!selectedSkillIds.length" @click="makePlan">
                  生成安装计划
                </UButton>
              </div>

              <!-- ③ 计划确认：逐 Skill 逐目标动作 + warning 显式确认 -->
              <div v-else class="space-y-3">
                <div class="rounded-lg bg-elevated/50 p-3 space-y-2.5">
                  <div v-for="ps in plan.skills" :key="ps.skillId" class="space-y-1">
                    <div class="flex items-center gap-2 text-sm">
                      <UIcon name="i-lucide-package" class="size-4 text-dimmed shrink-0" />
                      <span class="font-medium text-highlighted truncate">{{ ps.name }}</span>
                      <UBadge v-if="ps.version" color="neutral" variant="subtle" size="sm" class="font-mono">v{{ ps.version }}</UBadge>
                      <span class="ml-auto text-xs text-dimmed tabular-nums shrink-0">{{ formatBytes(ps.sizeBytes) }}</span>
                    </div>
                    <div v-for="t in ps.targets" :key="t.rootId" class="flex items-center gap-2 ml-6 text-xs">
                      <UIcon name="i-lucide-corner-down-right" class="size-3.5 text-dimmed shrink-0" />
                      <span class="text-default">{{ rootLabelOf(t.rootId) }}</span>
                      <UBadge :color="actionMeta[t.action]?.color ?? 'neutral'" variant="subtle" size="sm">
                        {{ actionMeta[t.action]?.label ?? t.action }}
                      </UBadge>
                      <span class="font-mono text-dimmed truncate">{{ t.finalName }}</span>
                      <span v-if="t.existingVersion" class="text-dimmed shrink-0">当前 v{{ t.existingVersion }}</span>
                    </div>
                  </div>
                </div>
                <div v-if="plan.targets.some((t) => t.needsCreate)" class="flex items-start gap-1.5 text-xs text-dimmed">
                  <UIcon name="i-lucide-folder-plus" class="size-3.5 shrink-0 mt-0.5" />
                  部分目标目录尚不存在，安装时会自动创建
                </div>
                <div v-if="planWarnings.length" class="rounded-lg bg-warning/5 ring ring-warning/20 p-3 space-y-1.5">
                  <label
                    v-for="w in planWarnings"
                    :key="w.code"
                    class="flex items-start gap-2 text-xs text-warning cursor-pointer select-none"
                  >
                    <input
                      type="checkbox" class="mt-0.5 accent-warning"
                      :checked="ackCodes.includes(w.code)" @change="toggleAck(w.code)"
                    />
                    <span>{{ w.message }}</span>
                  </label>
                </div>
                <div class="flex justify-end gap-2">
                  <UButton color="neutral" variant="ghost" :disabled="busy" @click="plan = null">返回选择</UButton>
                  <UButton icon="i-lucide-check" :loading="busy" :disabled="!allWarningsAcked" @click="commitPlan">确认安装</UButton>
                </div>
              </div>
            </template>

            <!-- ④ 逐目标结果 -->
            <div v-else class="space-y-3">
              <div v-for="o in commitResult.outcomes" :key="o.skillId" class="rounded-lg bg-elevated/50 p-3 space-y-1">
                <div class="flex items-center gap-2 text-sm font-medium text-highlighted">
                  <UIcon name="i-lucide-package" class="size-4 text-dimmed shrink-0" />{{ o.name }}
                </div>
                <div v-for="t in o.targets" :key="t.rootId" class="ml-6 space-y-0.5">
                  <div class="flex items-center gap-2 text-xs">
                    <UIcon
                      :name="t.status === 'installed' ? 'i-lucide-circle-check' : t.status === 'skipped' ? 'i-lucide-circle-minus' : 'i-lucide-circle-x'"
                      class="size-3.5 shrink-0"
                      :class="t.status === 'installed' ? 'text-success' : t.status === 'skipped' ? 'text-dimmed' : 'text-error'"
                    />
                    <span class="text-default">{{ rootLabelOf(t.rootId) }}</span>
                    <span v-if="t.finalName" class="font-mono text-dimmed truncate">{{ t.finalName }}</span>
                    <span
                      class="ml-auto shrink-0"
                      :class="t.status === 'installed' ? 'text-success' : t.status === 'skipped' ? 'text-dimmed' : 'text-error'"
                    >
                      {{ t.status === 'installed' ? '已安装' : t.status === 'skipped' ? '已跳过' : '失败' }}
                    </span>
                  </div>
                  <div v-if="t.error" class="ml-5.5 text-xs text-error">{{ t.error.message }}</div>
                </div>
              </div>
              <div class="flex justify-end gap-2">
                <UButton
                  v-if="commitResult.outcomes.some((o) => o.targets.some((t) => t.status === 'failed'))"
                  icon="i-lucide-rotate-ccw" color="warning" variant="soft" @click="retryPlan"
                >
                  重新生成计划
                </UButton>
                <UButton icon="i-lucide-check" color="neutral" variant="outline" @click="finishWizard">完成</UButton>
              </div>
            </div>
          </div>
        </div>

        <!-- 反馈 -->
        <UAlert v-if="error" color="error" variant="soft" icon="i-lucide-triangle-alert" :title="error" />
        <UAlert v-if="okMsg" color="success" variant="soft" icon="i-lucide-circle-check" :title="okMsg">
          <template #description>
            <UButton color="success" variant="link" size="sm" trailing-icon="i-lucide-arrow-right" class="px-0" @click="goList">
              去列表查看
            </UButton>
          </template>
        </UAlert>

        <!-- 技能目录：内置精选 + 已接入数据源 + skills.sh 全网搜索（仅远程 tab；共用一个搜索框） -->
        <UCard v-if="tab === 'remote'" :ui="{ body: 'p-4 sm:p-5' }">
          <div class="flex flex-wrap items-center gap-3 mb-3">
            <div class="flex items-center gap-1.5 text-[11px] font-semibold text-dimmed uppercase tracking-wider">
              <UIcon name="i-lucide-flame" class="size-3.5 text-warning" />
              技能目录
              <span v-if="!catalogLoading" class="font-normal normal-case tracking-normal">（{{ filteredCatalog.length }}）</span>
            </div>
            <div class="ml-auto flex items-center gap-2 w-full sm:w-auto">
              <UInput
                v-model="catalogFilter"
                icon="i-lucide-search"
                placeholder="搜索目录 + 全网 60 万+ 技能"
                size="sm"
                class="flex-1 sm:w-64"
                :loading="shLoading"
                @keyup.enter="runShSearch"
              />
              <UButton size="sm" color="neutral" variant="outline" icon="i-lucide-database" @click="openSources">
                源管理
              </UButton>
            </div>
          </div>

          <!-- 来源筛选 -->
          <div class="flex flex-wrap items-center gap-2 mb-4">
            <button
              v-for="g in catalogGroups"
              :key="g.value"
              type="button"
              class="rounded-full px-3 py-1 text-xs transition-colors cursor-pointer"
              :class="catalogGroup === g.value
                ? 'bg-primary/10 text-primary ring ring-primary/30 font-medium'
                : 'ring ring-default bg-elevated/40 hover:bg-elevated text-default'"
              @click="catalogGroup = g.value"
            >
              {{ g.label }}
            </button>
          </div>

          <div v-if="catalogLoading" class="flex items-center gap-2 py-6 justify-center text-xs text-dimmed">
            <UIcon name="i-lucide-loader-circle" class="size-4 animate-spin" />
            加载技能目录（首次同步数据源可能需要几十秒）…
          </div>
          <div v-else-if="!filteredCatalog.length" class="py-6 text-center text-xs text-dimmed">
            目录中没有匹配的 skill{{ catalogFilter.trim().length >= 2 ? '，下方可查看全网结果' : '' }}
          </div>
          <div v-else class="grid grid-cols-1 sm:grid-cols-2 gap-3">
            <div
              v-for="item in filteredCatalog"
              :key="item.id"
              class="group rounded-lg ring ring-default bg-elevated/30 hover:bg-elevated/60 hover:ring-primary/30 transition-colors p-3 flex flex-col gap-2"
            >
              <div class="flex items-center gap-2 min-w-0">
                <span class="font-mono text-sm font-semibold text-highlighted truncate">{{ item.name }}</span>
                <span v-if="item.stars != null" class="inline-flex items-center gap-0.5 text-[11px] text-dimmed tabular-nums shrink-0">
                  <UIcon name="i-lucide-star" class="size-3 text-warning" />{{ formatStars(item.stars) }}
                </span>
                <UBadge v-if="item.hasScripts" color="warning" variant="subtle" size="sm" icon="i-lucide-terminal" class="shrink-0">含脚本</UBadge>
                <UBadge v-for="t in item.tags" :key="t" color="neutral" variant="subtle" size="sm" class="shrink-0 hidden sm:inline-flex">{{ t }}</UBadge>
              </div>
              <p class="text-xs text-muted line-clamp-2 flex-1">{{ item.description }}</p>
              <div class="flex items-center gap-2">
                <a
                  :href="item.homepage"
                  target="_blank"
                  rel="noopener"
                  class="inline-flex items-center gap-1 text-[11px] text-dimmed hover:text-primary font-mono truncate"
                >
                  <UIcon name="i-lucide-github" class="size-3 shrink-0" />{{ item.repo }}
                </a>
                <UButton
                  size="xs"
                  color="neutral"
                  variant="ghost"
                  icon="i-lucide-eye"
                  class="ml-auto shrink-0"
                  @click="openCatalogDetail(item)"
                >
                  详情
                </UButton>
                <UButton
                  size="xs"
                  color="primary"
                  variant="soft"
                  icon="i-lucide-download"
                  class="shrink-0"
                  :loading="installingCatalogId === item.id"
                  :disabled="busy && installingCatalogId !== item.id"
                  @click="installFromCatalog(item)"
                >
                  安装
                </UButton>
              </div>
            </div>
          </div>

          <!-- 全网结果：搜索词 ≥ 2 字符时自动检索 skills.sh，无搜索时不占版面 -->
          <template v-if="shSearched || shLoading || shError">
            <div class="flex items-center gap-1.5 mt-5 mb-3 pt-4 border-t border-default text-[11px] font-semibold text-dimmed uppercase tracking-wider">
              <UIcon name="i-lucide-telescope" class="size-3.5 text-primary" />
              全网结果（skills.sh）
              <span v-if="shSearched && !shLoading" class="font-normal normal-case tracking-normal">（{{ shResults.length }}）</span>
            </div>

            <UAlert v-if="shError" color="error" variant="soft" icon="i-lucide-triangle-alert" :title="shError" />
            <div v-else-if="shLoading && !shResults.length" class="flex items-center gap-2 py-4 justify-center text-xs text-dimmed">
              <UIcon name="i-lucide-loader-circle" class="size-4 animate-spin" />搜索中…
            </div>
            <div v-else-if="shSearched && !shResults.length" class="py-4 text-center text-xs text-dimmed">
              全网没有匹配的技能
            </div>
            <div v-else class="space-y-2">
              <div
                v-for="item in shResults"
                :key="item.id"
                class="group rounded-lg ring ring-default bg-elevated/30 hover:bg-elevated/60 hover:ring-primary/30 transition-colors p-3 flex items-center gap-3"
              >
                <div class="min-w-0 flex-1">
                  <div class="flex items-center gap-2 min-w-0">
                    <span class="font-mono text-sm font-semibold text-highlighted truncate">{{ item.name }}</span>
                    <span v-if="formatInstalls(item.installs)" class="inline-flex items-center gap-0.5 text-[11px] text-dimmed tabular-nums shrink-0">
                      <UIcon name="i-lucide-download" class="size-3" />{{ formatInstalls(item.installs) }}
                    </span>
                  </div>
                  <div class="flex items-center gap-3 mt-1">
                    <a
                      :href="item.repoUrl"
                      target="_blank"
                      rel="noopener"
                      class="inline-flex items-center gap-1 text-[11px] text-dimmed hover:text-primary font-mono truncate"
                    >
                      <UIcon name="i-lucide-github" class="size-3 shrink-0" />{{ item.source }}
                    </a>
                    <button
                      type="button"
                      class="inline-flex items-center gap-1 text-[11px] text-dimmed hover:text-primary shrink-0 cursor-pointer"
                      @click="openShDetail(item)"
                    >
                      <UIcon name="i-lucide-eye" class="size-3" />详情
                    </button>
                  </div>
                </div>
                <UButton
                  size="xs"
                  color="primary"
                  variant="soft"
                  icon="i-lucide-download"
                  class="shrink-0"
                  :loading="installingShId === item.id"
                  :disabled="busy && installingShId !== item.id"
                  @click="installFromSkillsSh(item)"
                >
                  安装
                </UButton>
              </div>
            </div>
          </template>
        </UCard>

        <!-- 数据源管理弹窗 -->
        <UModal v-model:open="sourcesOpen" title="数据源管理" description="接入 GitHub 仓库作为技能目录数据源，自动扫描其中含 SKILL.md 的目录">
          <template #body>
            <div class="space-y-4">
              <UAlert v-if="sourceError" color="error" variant="soft" icon="i-lucide-triangle-alert" :title="sourceError" />

              <div v-if="sourcesLoading" class="flex items-center gap-2 py-4 justify-center text-xs text-dimmed">
                <UIcon name="i-lucide-loader-circle" class="size-4 animate-spin" />加载中…
              </div>
              <div v-else class="space-y-2">
                <div
                  v-for="s in sources"
                  :key="s.id"
                  class="rounded-lg ring ring-default p-3 flex items-center gap-3"
                >
                  <div class="min-w-0 flex-1">
                    <div class="flex items-center gap-2">
                      <span class="text-sm font-medium text-highlighted truncate">{{ s.label }}</span>
                      <UBadge v-if="s.builtin" color="neutral" variant="subtle" size="sm">内置</UBadge>
                      <UBadge v-if="s.kind === 'search'" color="info" variant="subtle" size="sm" icon="i-lucide-telescope">搜索引擎</UBadge>
                      <UBadge v-if="s.lastSyncError" color="error" variant="subtle" size="sm" icon="i-lucide-triangle-alert">同步失败</UBadge>
                    </div>
                    <div class="text-[11px] text-dimmed font-mono truncate mt-0.5">{{ s.repo }}</div>
                    <div v-if="s.kind === 'search'" class="text-[11px] text-dimmed mt-0.5">{{ s.description }}</div>
                    <div v-else class="text-[11px] text-dimmed mt-0.5">
                      {{ s.skillCount }} 个技能 · {{ formatSyncTime(s.lastSyncAt) }}
                      <span v-if="s.lastSyncError" class="text-error">（{{ s.lastSyncError }}）</span>
                    </div>
                  </div>
                  <UButton
                    v-if="s.kind !== 'search'"
                    size="xs"
                    color="neutral"
                    variant="outline"
                    icon="i-lucide-refresh-cw"
                    :loading="sourceBusyId === s.id"
                    @click="handleSyncSource(s.id)"
                  >
                    同步
                  </UButton>
                  <UButton
                    v-if="!s.builtin"
                    size="xs"
                    color="error"
                    variant="ghost"
                    icon="i-lucide-trash-2"
                    :disabled="sourceBusyId === s.id"
                    @click="handleRemoveSource(s.id)"
                  />
                </div>
              </div>

              <div class="pt-2 border-t border-default space-y-2">
                <div class="text-xs font-medium text-default">新增数据源</div>
                <div class="flex flex-col sm:flex-row gap-2">
                  <UInput
                    v-model="newSourceRepo"
                    icon="i-lucide-github"
                    placeholder="owner/repo 或 GitHub 仓库 URL"
                    size="sm"
                    class="flex-1 font-mono"
                    @keyup.enter="handleAddSource"
                  />
                  <UInput v-model="newSourceLabel" placeholder="展示名（可选）" size="sm" class="sm:w-36" />
                  <UButton size="sm" icon="i-lucide-plus" :loading="addSourceBusy" :disabled="!newSourceRepo.trim()" @click="handleAddSource">
                    接入
                  </UButton>
                </div>
                <div class="text-[11px] text-dimmed">接入后会立即扫描一次；仅支持公开仓库。</div>
              </div>
            </div>
          </template>
        </UModal>
      </div>
    </template>
  </UDashboardPanel>
</template>
