<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { storeToRefs } from 'pinia'
import type { AddSourceRequest, CatalogSkill, CommitPlanResponse, ConflictStrategy, InstallPlan, PrepareSourceResponse, ProjectAgentDir, RemoteSource, SkillRoot, SkillScope, SkillsShSearchItem, SkillSource } from '@/types/skills'
import { useSkillsStore } from '@/stores/skills'
import { skillsApi, pickDirectory, pickFile } from '@/api/skills'
import { formatBytes } from '@/utils/skills-markdown'
import AgentAvatar from '@/components/skills/AgentAvatar.vue'

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
const installSteps = ['选择来源', '预览与目标', '确认安装', '安装结果']
const currentStep = computed(() => commitResult.value ? 3 : plan.value ? 2 : prep.value ? 1 : 0)
const catalogLimit = ref(12)
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
  if (busy.value || prep.value) return
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
  if (busy.value || prep.value) return
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

const visibleCatalog = computed(() => filteredCatalog.value.slice(0, catalogLimit.value))
watch([catalogFilter, catalogGroup], () => { catalogLimit.value = 12 })

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
  if (busy.value || prep.value) return
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
  if (busy.value || prep.value) return
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
  if (busy.value || !prep.value) return
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
  if (busy.value || !plan.value || !allWarningsAcked.value) return
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
  reset()
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
  <UDashboardPanel id="install" :ui="{ body: 'p-0 sm:p-0 gap-0' }">
    <template #header>
      <UDashboardNavbar title="安装" description="Skills">
        <template #leading>
          <UDashboardSidebarCollapse />
        </template>
        <template #right>
          <UButton to="/skills/library" icon="i-lucide-library" color="neutral" variant="ghost">技能库</UButton>
        </template>
      </UDashboardNavbar>
    </template>

    <template #body>
      <div class="skills-install">
        <header class="install-heading">
          <h1>安装 Skills</h1>
          <p>从远程仓库或本地文件导入，检查内容后再安装到指定目录。</p>
        </header>
        <ol class="install-steps" aria-label="安装进度">
          <li v-for="(step, index) in installSteps" :key="step"
            :class="{ active: currentStep === index, done: currentStep > index }"
            :aria-current="currentStep === index ? 'step' : undefined">
            <span class="step-number"><UIcon v-if="currentStep > index" name="i-lucide-check" class="size-3.5" /><template v-else>{{ index + 1 }}</template></span>
            <span>{{ step }}</span>
          </li>
        </ol>

        <div class="install-grid">
        <fieldset class="source-section" :disabled="busy || !!prep">
          <legend class="sr-only">选择安装来源</legend>
          <!-- Tab 切换（下划线风格） -->
          <UTabs v-model="tab" :items="tabItems.map((item) => ({ ...item, disabled: busy || !!prep }))"
            :content="false" variant="link" color="primary" :ui="{ list: 'justify-start', trigger: 'flex-none px-4' }"
            @update:model-value="onTabChange" />

          <!-- 远程 -->
          <section v-if="tab === 'remote'" class="source-form">
            <div class="section-title"><h2>从链接导入</h2><p>支持 GitHub、skills.sh、技能市场与压缩包链接。</p></div>
            <UFormField label="来源地址">
              <div class="remote-address">
                <USelect v-model="remoteSource" :items="sourceItems" aria-label="来源类型" class="source-type" />
                <UInput v-model="remoteRef" icon="i-lucide-link-2" :placeholder="remotePlaceholder" class="source-input" @keyup.enter="remoteRef.trim() && previewRemote()" />
              </div>
            </UFormField>
            <div class="source-actions">
              <span><UIcon name="i-lucide-shield-check" class="size-3.5" />预览不会写入目标目录</span>
              <UButton icon="i-lucide-arrow-right" trailing :loading="busy && !prep" :disabled="!remoteRef.trim() || !!prep" @click="previewRemote">获取预览</UButton>
            </div>
          </section>

          <!-- 本地路径 -->
          <section v-else-if="tab === 'local'" class="source-form">
            <div class="section-title"><h2>从本机目录导入</h2><p>选择包含 SKILL.md 的技能目录，原始文件保持不变。</p></div>
            <UFormField label="技能目录">
              <div class="source-address">
                <UInput v-model="sourcePath" icon="i-lucide-folder-input" placeholder="/path/to/my-skill" class="source-input" @keyup.enter="sourcePath.trim() && previewLocal()" />
                <UButton icon="i-lucide-folder-search" color="neutral" variant="outline" :loading="pickingSource" @click="pickSourceDir">浏览</UButton>
              </div>
            </UFormField>
            <div class="source-actions">
              <span>先检查技能内容，再选择安装目标</span>
              <UButton icon="i-lucide-arrow-right" trailing :loading="busy && !prep" :disabled="!sourcePath.trim() || !!prep" @click="previewLocal">获取预览</UButton>
            </div>
          </section>

          <!-- 上传压缩包 -->
          <section v-else class="source-form">
            <div class="section-title"><h2>从压缩包导入</h2><p>选择本机归档文件，解压并检查其中的技能。</p></div>
            <button type="button" class="archive-picker" :class="{ selected: uploadPath }" @click="chooseUpload">
              <UIcon name="i-lucide-file-archive" class="size-6 shrink-0 text-dimmed" />
              <span><strong>{{ uploadName || '选择压缩包' }}</strong><small>{{ uploadPath ? '点击更换文件' : '支持 .zip / .tar.gz / .tgz' }}</small></span>
              <UIcon name="i-lucide-folder-open" class="size-4 shrink-0 text-dimmed" />
            </button>
            <div class="source-actions">
              <span>压缩包中需包含 SKILL.md</span>
              <UButton icon="i-lucide-arrow-right" trailing :loading="busy && !prep" :disabled="!uploadPath || !!prep" @click="previewUpload">获取预览</UButton>
            </div>
          </section>
        </fieldset>

        <!-- 公共：安装目标（范围 + 多选 Agent 目录）+ 冲突策略 -->
        <aside class="target-section">
          <fieldset :disabled="busy || !!commitResult" class="target-fields">
          <legend class="target-title">安装目标 <span>已选 {{ selectedRootIds.length }}</span></legend>
          <p class="target-caption">可同时安装到多个 Agent 目录</p>
          <div class="space-y-5">
            <UFormField label="安装范围">
              <div class="flex flex-wrap items-center gap-2">
                <button
                  type="button"
                  class="scope-option"
                  :class="{ selected: targetScope === 'global' }"
                  :aria-pressed="targetScope === 'global'"
                  @click="targetScope = 'global'"
                >
                  <UIcon name="i-lucide-globe" class="size-3.5" />全局（用户级）
                </button>
                <button
                  type="button"
                  class="scope-option"
                  :class="{ selected: targetScope === 'project' }"
                  :aria-pressed="targetScope === 'project'"
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
                  class="w-full"
                  aria-label="目标项目"
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

            <UFormField v-else label="Agent 目录">
              <!-- 全局：已登记的可写根目录 -->
              <template v-if="targetScope === 'global'">
                <div v-if="scopeRoots.length" class="target-grid">
                  <button
                    v-for="r in scopeRoots"
                    :key="r.id"
                    type="button"
                    :title="r.path"
                    class="target-option"
                    :class="{ selected: selectedRootIds.includes(r.id) }"
                    :aria-pressed="selectedRootIds.includes(r.id)"
                    @click="toggleRoot(r.id)"
                  >
                    <AgentAvatar :agent="r.agent || rootDisplay(r)" :size="20" />
                    <span>{{ rootDisplay(r) }}</span>
                    <UIcon :name="selectedRootIds.includes(r.id) ? 'i-lucide-square-check' : 'i-lucide-square'" class="size-3.5 shrink-0" />
                  </button>
                </div>
                <div v-else class="text-xs text-dimmed">没有可写的全局目录。<UButton to="/skills/roots" variant="link" size="xs">添加目录</UButton></div>
              </template>

              <!-- 项目级：已知 Agent 候选目录，已存在的优先，其余折叠展开 -->
              <template v-else>
                <div v-if="agentDirsLoading" class="flex items-center gap-2 py-1 text-xs text-dimmed">
                  <UIcon name="i-lucide-loader-circle" class="size-3.5 animate-spin" />探测项目内的 Agent 目录…
                </div>
                <template v-else-if="projectAgentDirs.length">
                  <div class="target-grid">
                    <button
                      v-for="d in existingAgentDirs"
                      :key="d.rootId"
                      type="button"
                      :title="d.path"
                      :disabled="!d.writable"
                      class="target-option"
                      :class="{ selected: selectedRootIds.includes(d.rootId) }"
                      :aria-pressed="selectedRootIds.includes(d.rootId)"
                      @click="toggleRoot(d.rootId)"
                    >
                      <AgentAvatar :agent="agentDirDisplay(d)" :size="20" />
                      <span>{{ agentDirDisplay(d) }}</span>
                      <UIcon :name="selectedRootIds.includes(d.rootId) ? 'i-lucide-square-check' : 'i-lucide-square'" class="size-3.5 shrink-0" />
                    </button>
                    <template v-if="moreAgentsOpen">
                      <button
                        v-for="d in missingAgentDirs"
                        :key="d.rootId"
                        type="button"
                        :title="`安装时将创建：${d.path}`"
                        :disabled="!d.writable"
                        class="target-option missing"
                        :class="{ selected: selectedRootIds.includes(d.rootId) }"
                        :aria-pressed="selectedRootIds.includes(d.rootId)"
                        @click="toggleRoot(d.rootId)"
                      >
                        <AgentAvatar :agent="agentDirDisplay(d)" :size="20" />
                        <span>{{ agentDirDisplay(d) }}</span>
                        <UIcon :name="selectedRootIds.includes(d.rootId) ? 'i-lucide-square-check' : 'i-lucide-plus'" class="size-3.5 shrink-0" />
                      </button>
                    </template>
                    <button
                      v-if="missingAgentDirs.length && existingAgentDirs.length"
                      type="button"
                      class="more-agents"
                      :aria-expanded="moreAgentsOpen"
                      @click="moreAgentsOpen = !moreAgentsOpen"
                    >
                      <UIcon :name="moreAgentsOpen ? 'i-lucide-chevron-up' : 'i-lucide-chevron-down'" class="size-3.5" />
                      {{ moreAgentsOpen ? '收起' : `更多 Agent（${missingAgentDirs.length}）` }}
                    </button>
                  </div>
                  <div v-if="moreAgentsOpen && missingAgentDirs.length" class="mt-1.5 text-[11px] text-dimmed">
                    带「+」的目录尚未创建，选中后将在安装时创建
                  </div>
                </template>
                <div v-else class="text-xs text-dimmed">该项目不可用（目录不存在或无候选 Agent 目录）</div>
              </template>
            </UFormField>

            <UFormField label="同名技能处理">
              <USelect v-model="onConflict" :items="conflictOptions" icon="i-lucide-git-merge" class="w-full" />
            </UFormField>
          </div>
          </fieldset>
        </aside>

        <!-- 安装向导面板（三种来源统一）：候选选择 → 计划确认 → 逐目标结果 -->
        <div v-if="prep" ref="previewPanel" class="wizard-section" aria-live="polite">
          <div class="wizard-heading">
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
          <div class="py-4 space-y-4">
            <p class="prepared-source" :title="prep.source.ref">来源：{{ prep.source.ref }}</p>
            <template v-if="!commitResult">
              <!-- ① 候选选择（error 级问题的候选不可选） -->
              <div v-if="!plan" class="space-y-2">
                <p v-if="!prep.skills.length" class="text-sm text-muted py-4">没有找到可安装的技能，请检查来源后重试。</p>
                <button
                  v-for="s in prep.skills"
                  :key="s.id"
                  type="button"
                  :disabled="busy || !s.installable"
                  :aria-pressed="selectedSkillIds.includes(s.id)"
                  class="candidate-option w-full text-left rounded-md ring p-3 cursor-pointer disabled:cursor-not-allowed disabled:opacity-60"
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
              <div v-if="!plan" class="wizard-actions">
                <span>已选 {{ selectedSkillIds.length }} 个技能 · {{ selectedRootIds.length }} 个目标</span>
                <UButton icon="i-lucide-clipboard-list" :loading="busy" :disabled="!selectedSkillIds.length || !selectedRootIds.length || agentDirsLoading" @click="makePlan">
                  生成安装计划
                </UButton>
              </div>

              <!-- ③ 计划确认：逐 Skill 逐目标动作 + warning 显式确认 -->
              <div v-else class="space-y-3">
                <div class="plan-targets">
                  <div v-for="target in plan.targets" :key="target.rootId">
                    <span>{{ target.label }}<small v-if="target.needsCreate"> · 将创建</small></span>
                    <code>{{ target.path }}</code>
                  </div>
                </div>
                <div class="rounded-md bg-elevated/30 p-3 space-y-2.5">
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
                      :checked="ackCodes.includes(w.code)" :disabled="busy" @change="toggleAck(w.code)"
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
        <div v-if="error || okMsg" class="install-feedback" aria-live="polite">
        <UAlert v-if="error" color="error" variant="soft" icon="i-lucide-triangle-alert" :title="error" />
        <UAlert v-if="okMsg" color="success" variant="soft" icon="i-lucide-circle-check" :title="okMsg">
          <template #description>
            <UButton color="success" variant="link" size="sm" trailing-icon="i-lucide-arrow-right" class="px-0" @click="goList">
              去列表查看
            </UButton>
          </template>
        </UAlert>
        </div>

        <!-- 技能目录：内置精选 + 已接入数据源 + skills.sh 全网搜索（仅远程 tab；共用一个搜索框） -->
        <section v-if="tab === 'remote' && !prep" class="catalog-section">
          <div class="catalog-toolbar">
            <div class="flex items-center gap-2 text-sm font-semibold text-highlighted">
              发现技能
              <span v-if="!catalogLoading" class="font-normal normal-case tracking-normal">（{{ filteredCatalog.length }}）</span>
            </div>
            <div class="ml-auto flex items-center gap-2 w-full sm:w-auto">
              <UInput
                v-model="catalogFilter"
                icon="i-lucide-search"
                placeholder="搜索技能名称、描述或仓库"
                aria-label="搜索技能目录与 skills.sh"
                size="sm"
                class="flex-1 sm:w-64"
                :loading="shLoading"
                @keyup.enter="runShSearch"
              />
              <UButton size="sm" color="neutral" variant="outline" icon="i-lucide-database" @click="openSources">
                数据源
              </UButton>
            </div>
          </div>

          <!-- 来源筛选 -->
          <div class="flex flex-wrap items-center gap-2 mb-4">
            <button
              v-for="g in catalogGroups"
              :key="g.value"
              type="button"
              class="catalog-filter"
              :class="{ selected: catalogGroup === g.value }"
              :aria-pressed="catalogGroup === g.value"
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
          <div v-else class="catalog-grid">
            <div
              v-for="item in visibleCatalog"
              :key="item.id"
              class="catalog-item"
            >
              <div class="flex items-center gap-2 min-w-0">
                <span class="font-mono text-sm font-semibold text-highlighted truncate">{{ item.name }}</span>
                <span v-if="item.stars != null" class="inline-flex items-center gap-0.5 text-[11px] text-dimmed tabular-nums shrink-0">
                  <UIcon name="i-lucide-star" class="size-3 text-warning" />{{ formatStars(item.stars) }}
                </span>
                <UBadge v-if="item.hasScripts" color="warning" variant="subtle" size="sm" icon="i-lucide-terminal" class="shrink-0">含脚本</UBadge>
              </div>
              <div v-if="item.tags.length" class="flex flex-wrap gap-1">
                <UBadge v-for="t in item.tags" :key="t" color="neutral" variant="subtle" size="sm">{{ t }}</UBadge>
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
                  预览安装
                </UButton>
              </div>
            </div>
          </div>

          <div v-if="filteredCatalog.length > visibleCatalog.length" class="catalog-more">
            <span>已展示 {{ visibleCatalog.length }} / {{ filteredCatalog.length }} 个技能</span>
            <UButton color="neutral" variant="outline" size="sm" @click="catalogLimit += 12">加载更多</UButton>
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
                class="search-result"
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
                  预览安装
                </UButton>
              </div>
            </div>
          </template>
        </section>

        </div>

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

<style scoped>
.skills-install { width: 100%; max-width: 1320px; margin: 0 auto; padding: 28px 32px 36px; container-type: inline-size; }
.install-heading h1 { margin: 0; font-size: 22px; font-weight: 650; letter-spacing: -.035em; color: var(--text); }
.install-heading p { margin: 6px 0 0; font-size: 13px; color: var(--text-muted); }
.install-steps { display: flex; flex-wrap: wrap; gap: 12px 28px; list-style: none; padding: 20px 0; margin: 0 0 24px; border-bottom: 1px solid var(--border); }
.install-steps li { display: flex; align-items: center; gap: 8px; color: var(--text-subtle); font-size: 12px; }
.step-number { display: flex; align-items: center; justify-content: center; width: 22px; height: 22px; border: 1px solid var(--border); border-radius: 50%; font: 11px var(--font-mono); }
.install-steps .active { color: var(--text); font-weight: 600; }
.active .step-number { border-color: var(--accent); background: var(--accent); color: var(--bg); }
.done .step-number { color: var(--accent); }
.install-grid { display: grid; grid-template-columns: minmax(0, 1fr) 284px; gap: 24px 32px; align-items: start; }
.source-section, .target-fields { min-width: 0; margin: 0; padding: 0; border: 0; }
.source-section, .wizard-section, .install-feedback, .catalog-section { grid-column: 1; min-width: 0; }
.source-form { padding: 20px 0 0; }
.section-title { margin-bottom: 20px; }
.section-title h2 { margin: 0; color: var(--text); font-size: 14px; font-weight: 600; }
.section-title p { margin: 6px 0 0; font-size: 12px; color: var(--text-muted); line-height: 1.6; }
.remote-address { display: grid; grid-template-columns: 120px minmax(0, 1fr); gap: 8px; }
.source-type, .source-input { width: 100%; min-width: 0; }
.source-address { display: flex; gap: 8px; }
.source-address .source-input { flex: 1; }
.source-actions, .wizard-actions { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 12px; margin-top: 16px; }
.source-actions > span, .wizard-actions > span { display: flex; align-items: center; gap: 6px; color: var(--text-subtle); font-size: 11px; }
.archive-picker { display: flex; width: 100%; align-items: center; gap: 14px; padding: 20px; border: 1px dashed var(--border-strong); border-radius: 8px; text-align: left; cursor: pointer; background: transparent; }
.archive-picker > span { display: flex; min-width: 0; flex: 1; flex-direction: column; gap: 6px; }
.archive-picker strong { overflow-wrap: anywhere; font-size: 13px; font-weight: 500; color: var(--text); }
.archive-picker small { font-size: 11px; color: var(--text-subtle); }
.archive-picker:hover, .archive-picker.selected { border-color: var(--accent); }
.target-section { grid-column: 2; grid-row: 1 / span 4; border-left: 1px solid var(--border); padding: 6px 0 16px 24px; }
.target-title { display: flex; align-items: center; justify-content: space-between; width: 100%; margin-bottom: 6px; color: var(--text); font-size: 13px; font-weight: 600; }
.target-title span { font-size: 11px; font-weight: 400; color: var(--text-subtle); }
.target-caption { font-size: 11px; line-height: 1.7; margin: 0 0 20px; color: var(--text-subtle); }
.scope-option { display: inline-flex; flex: 1; justify-content: center; align-items: center; gap: 6px; padding: 8px; border: 1px solid var(--border); border-radius: 6px; color: var(--text-muted); font-size: 11px; white-space: nowrap; cursor: pointer; }
.scope-option.selected { color: var(--accent); border-color: var(--accent); background: var(--accent-weak); }
.target-grid { display: grid; grid-template-columns: minmax(0, 1fr); gap: 4px; max-height: 360px; overflow-y: auto; padding: 2px; }
.target-option { display: flex; align-items: center; gap: 8px; min-width: 0; padding: 8px; border: 1px solid transparent; border-radius: 6px; color: var(--text-muted); cursor: pointer; }
.target-option > span:not(:last-child) { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1; text-align: left; font-size: 12px; }
.target-option:hover { background: var(--surface); }
.target-option.selected { color: var(--accent); background: var(--accent-weak); }
.target-option.missing { color: var(--text-subtle); }
.target-option:disabled { opacity: .45; cursor: not-allowed; }
.more-agents { display: flex; align-items: center; gap: 6px; padding: 8px; font-size: 11px; color: var(--text-muted); cursor: pointer; }
.wizard-section { scroll-margin-top: 20px; border-top: 1px solid var(--border); }
.wizard-heading { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; min-height: 48px; border-bottom: 1px solid var(--border); }
.prepared-source { font: 11px/1.6 var(--font-mono); color: var(--text-muted); overflow-wrap: anywhere; }
.candidate-option > div:first-child { flex-wrap: wrap; }
.plan-targets { display: grid; gap: 10px; padding: 12px 0; }
.plan-targets span { font-size: 12px; color: var(--text); }
.plan-targets code { display: block; font: 11px/1.6 var(--font-mono); color: var(--text-muted); overflow-wrap: anywhere; }
.install-feedback { display: grid; gap: 12px; }
.catalog-section { padding-top: 24px; border-top: 1px solid var(--border); }
.catalog-toolbar { display: flex; flex-wrap: wrap; align-items: center; gap: 12px; margin-bottom: 16px; }
.catalog-filter { padding: 5px 10px; border-radius: 5px; color: var(--text-muted); font-size: 11px; cursor: pointer; }
.catalog-filter:hover { color: var(--text); }
.catalog-filter.selected { color: var(--accent); background: var(--accent-weak); }
.catalog-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; }
.catalog-item { min-width: 0; display: flex; flex-direction: column; gap: 10px; padding: 14px; border: 1px solid var(--border); border-radius: 8px; }
.catalog-item:hover { border-color: var(--border-strong); }
.catalog-item > div:last-child { flex-wrap: wrap; }
.catalog-item > div:last-child > a { max-width: 100%; }
.catalog-more { display: flex; align-items: center; justify-content: center; flex-wrap: wrap; gap: 12px; padding-top: 20px; color: var(--text-subtle); font-size: 11px; }
.search-result { display: flex; align-items: center; gap: 12px; padding: 14px 0; border-bottom: 1px solid var(--border); }
.skills-install button:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
@container (max-width: 940px) { .catalog-grid { grid-template-columns: minmax(0, 1fr); } }
@container (max-width: 760px) {
  .install-grid { grid-template-columns: minmax(0, 1fr); gap: 24px; }
  .target-section { grid-column: 1; grid-row: auto; border-left: 0; padding: 20px 0 0; border-top: 1px solid var(--border); }
  .target-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); max-height: 260px; }
  .target-caption { margin-bottom: 16px; }
}
@container (max-width: 440px) {
  .install-steps { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; }
  .remote-address { grid-template-columns: minmax(0, 1fr); }
  .source-actions { align-items: flex-start; }
  .source-actions > span { flex-basis: 100%; }
  .target-grid { grid-template-columns: minmax(0, 1fr); }
}
@media (max-width: 640px) { .skills-install { padding: 20px 16px; } }
@media (prefers-reduced-motion: reduce) { .skills-install :deep(*) { animation: none !important; transition: none !important; scroll-behavior: auto !important; } }
</style>
