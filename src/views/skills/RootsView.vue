<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { storeToRefs } from 'pinia'
import type { DirScanProject, DirScanResult, SkillProject, SkillRoot } from '@/types/skills'
import { useSkillsStore } from '@/stores/skills'
import { skillsApi, pickDirectory } from '@/api/skills'
import AgentAvatar from '@/components/skills/AgentAvatar.vue'

const store = useSkillsStore()
const { roots, projects } = storeToRefs(store)

const busy = ref(false)
const error = ref('')
const okMsg = ref('')

// 全局根目录
const globalRoots = computed(() => roots.value.filter((r) => r.scope === 'global'))
// 某项目探测到的 skill 根目录
function rootsOfProject(projectId: string): SkillRoot[] {
  return roots.value.filter((r) => r.scope === 'project' && r.projectId === projectId)
}

// 添加全局根目录
const newPath = ref('')
// 添加项目
const newProjectPath = ref('')
const newProjectLabel = ref('')
const pickingProject = ref(false)
// 重命名（内联编辑）
const editingId = ref('')
const editLabel = ref('')

function feedback(msg: string) {
  okMsg.value = msg
  error.value = ''
}

async function run(fn: () => Promise<unknown>, ok: string) {
  busy.value = true
  error.value = ''
  okMsg.value = ''
  try {
    await fn()
    await store.refresh()
    feedback(ok)
  } catch (e) {
    error.value = (e as Error).message
  } finally {
    busy.value = false
  }
}

async function addRoot() {
  const p = newPath.value.trim()
  if (!p) return
  await run(() => skillsApi.addRoot(p), '已添加根目录')
  newPath.value = ''
}

async function addProject() {
  const p = newProjectPath.value.trim()
  if (!p) return
  await run(() => skillsApi.addProject(p, newProjectLabel.value.trim() || undefined), '已添加项目')
  newProjectPath.value = ''
  newProjectLabel.value = ''
}

async function pickProjectDir() {
  pickingProject.value = true
  error.value = ''
  try {
    const path = await pickDirectory('选择项目目录', newProjectPath.value.trim() || undefined)
    if (path) newProjectPath.value = path
  } catch (e) {
    error.value = (e as Error).message
  } finally {
    pickingProject.value = false
  }
}

async function removeProject(p: SkillProject) {
  await run(() => skillsApi.removeProject(p.id), '已移除项目（仅登记，未删除文件）')
}

function startEdit(r: SkillRoot) {
  editingId.value = r.id
  editLabel.value = r.label
}

function cancelEdit() {
  editingId.value = ''
  editLabel.value = ''
}

async function saveLabel(r: SkillRoot) {
  const label = editLabel.value.trim()
  if (!label) return
  await run(() => skillsApi.updateRootLabel(r.id, label), '已更新展示名')
  cancelEdit()
}

async function initDir(r: SkillRoot) {
  await run(() => skillsApi.initRoot(r.id), `已创建目录：${r.path}`)
}

async function removeRoot(r: SkillRoot) {
  await run(() => skillsApi.removeRoot(r.id), '已移除根目录（仅登记，未删除文件）')
}

// ============ 扫描发现（服务端递归扫描，结果可一键登记为项目） ============
const scanning = ref(false)
const scanError = ref('')
const scanResult = ref<DirScanResult | null>(null)

/** 选目录后立即扫描 */
async function pickAndScan() {
  scanning.value = true
  scanError.value = ''
  try {
    const path = await pickDirectory('选择要扫描的目录')
    if (path) await runScan(path)
  } catch (e) {
    scanError.value = (e as Error).message
  } finally {
    scanning.value = false
  }
}

async function runScan(path: string) {
  scanning.value = true
  scanError.value = ''
  scanResult.value = null
  try {
    scanResult.value = await skillsApi.scanDir(path)
  } catch (e) {
    scanError.value = (e as Error).message
  } finally {
    scanning.value = false
  }
}

function clearScan() {
  scanResult.value = null
  scanError.value = ''
}

async function registerScanned(p: DirScanProject) {
  await run(() => skillsApi.addProject(p.path), `已登记项目：${p.label}`)
  if (!error.value) p.registered = true
}

async function registerAllScanned() {
  const targets = scanResult.value?.projects.filter((p) => !p.registered) ?? []
  if (!targets.length) return
  await run(async () => {
    for (const p of targets) await skillsApi.addProject(p.path)
  }, `已登记 ${targets.length} 个项目`)
  if (!error.value) for (const p of targets) p.registered = true
}

onMounted(() => store.refresh())
</script>

<template>
  <UDashboardPanel id="roots">
    <template #header>
      <UDashboardNavbar title="来源目录">
        <template #leading>
          <UDashboardSidebarCollapse />
        </template>
      </UDashboardNavbar>
    </template>

    <template #body>
      <div class="max-w-3xl w-full mx-auto space-y-8">
        <!-- ============ 全局（用户级） ============ -->
        <section class="space-y-3">
          <div class="flex items-center gap-2 text-sm font-semibold text-highlighted">
            <UIcon name="i-lucide-globe" class="size-4 text-primary" />
            全局根目录
            <span class="text-xs font-normal text-dimmed">用户级，通常在 ~ 目录下</span>
          </div>

          <!-- 添加根目录 -->
          <UCard :ui="{ body: 'p-4 sm:p-5' }">
            <div class="flex gap-2">
              <UInput
                v-model="newPath"
                icon="i-lucide-folder-input"
                placeholder="skill 根目录的绝对路径，如 ~/.qoder/skills"
                class="flex-1 font-mono"
                @keyup.enter="addRoot"
              />
              <UButton icon="i-lucide-plus" :loading="busy" :disabled="!newPath.trim()" @click="addRoot">
                添加
              </UButton>
            </div>
          </UCard>

          <!-- 全局根目录列表 -->
          <div class="space-y-2.5">
            <div
              v-for="r in globalRoots"
              :key="r.id"
              class="rounded-lg ring ring-default bg-default p-4"
            >
              <div class="flex items-start gap-3">
                <AgentAvatar :agent="r.agent" :size="36" :class="!r.exists ? 'opacity-50' : ''" />

                <div class="min-w-0 flex-1">
                  <div v-if="editingId === r.id" class="flex items-center gap-2">
                    <UInput v-model="editLabel" size="sm" class="flex-1" @keyup.enter="saveLabel(r)" @keyup.esc="cancelEdit" />
                    <UButton icon="i-lucide-check" size="xs" :loading="busy" @click="saveLabel(r)" />
                    <UButton icon="i-lucide-x" size="xs" color="neutral" variant="ghost" @click="cancelEdit" />
                  </div>
                  <div v-else class="flex items-center gap-2 flex-wrap">
                    <span class="text-sm font-semibold text-highlighted truncate">{{ r.agent ?? r.label }}</span>
                    <UBadge v-if="r.isDefault" color="neutral" variant="subtle" size="sm">默认</UBadge>
                    <UBadge v-if="!r.writable" color="warning" variant="subtle" size="sm" icon="i-lucide-lock">只读</UBadge>
                    <UBadge v-if="!r.exists" color="error" variant="subtle" size="sm">不存在</UBadge>
                  </div>

                  <div class="mt-1 flex items-center gap-2 text-xs text-dimmed font-mono">
                    <span class="truncate">{{ r.path }}</span>
                  </div>
                  <div class="mt-1 text-xs text-muted">
                    {{ r.exists ? `${r.skillCount ?? 0} 个 skill` : '目录尚未创建' }}
                  </div>
                </div>

                <div v-if="editingId !== r.id" class="flex items-center gap-1 shrink-0">
                  <UButton
                    v-if="!r.exists && r.writable"
                    icon="i-lucide-folder-plus"
                    size="xs"
                    color="primary"
                    variant="soft"
                    :loading="busy"
                    @click="initDir(r)"
                  >
                    创建目录
                  </UButton>
                  <UButton icon="i-lucide-pencil" size="xs" color="neutral" variant="ghost" title="重命名展示名" @click="startEdit(r)" />
                  <UButton
                    v-if="!r.isDefault"
                    icon="i-lucide-trash-2"
                    size="xs"
                    color="error"
                    variant="ghost"
                    title="移除登记（不删除文件）"
                    :loading="busy"
                    @click="removeRoot(r)"
                  />
                </div>
              </div>
            </div>
          </div>
        </section>

        <!-- ============ 项目级 ============ -->
        <section class="space-y-3">
          <div class="flex items-center gap-2 text-sm font-semibold text-highlighted">
            <UIcon name="i-lucide-folder-git-2" class="size-4 text-primary" />
            项目
            <span class="text-xs font-normal text-dimmed">登记项目目录，自动探测其中的 skill（如 .claude/skills、.qoder/skills）</span>
          </div>

          <!-- 添加项目：手动输入 / 浏览选择 / 扫描发现 -->
          <UCard :ui="{ body: 'p-4 sm:p-5' }">
            <div class="flex flex-col gap-2 sm:flex-row">
              <UInput
                v-model="newProjectPath"
                icon="i-lucide-folder-input"
                placeholder="项目根目录的绝对路径，如 ~/work/my-app"
                class="flex-1 font-mono"
                @keyup.enter="addProject"
              />
              <UButton icon="i-lucide-folder-search" color="neutral" variant="outline" :loading="pickingProject" @click="pickProjectDir">
                浏览
              </UButton>
              <UInput
                v-model="newProjectLabel"
                icon="i-lucide-tag"
                placeholder="展示名（可选）"
                class="sm:w-44"
                @keyup.enter="addProject"
              />
              <UButton icon="i-lucide-plus" :loading="busy" :disabled="!newProjectPath.trim()" @click="addProject">
                添加
              </UButton>
            </div>
            <USeparator class="my-3" />
            <div class="flex items-center gap-2 flex-wrap">
              <UButton
                icon="i-lucide-scan-search"
                color="primary"
                variant="soft"
                :loading="scanning"
                @click="pickAndScan"
              >
                扫描目录发现项目
              </UButton>
              <span class="text-xs text-dimmed">选一个目录，递归找出所有含 agent skill 的项目，一键登记</span>
            </div>
            <UAlert v-if="scanError" class="mt-3" color="error" variant="soft" icon="i-lucide-triangle-alert" :title="scanError" />
          </UCard>

          <!-- 扫描结果：发现的项目可直接登记 -->
          <UCard v-if="scanResult" :ui="{ body: 'p-4 sm:p-5' }">
            <div class="flex items-center gap-2 flex-wrap">
              <UIcon name="i-lucide-scan-search" class="size-4 text-primary shrink-0" />
              <span class="text-xs text-muted">
                <span class="font-mono text-default">{{ scanResult.root }}</span>
                ：发现 {{ scanResult.skillDirCount }} 个 skill 目录，{{ scanResult.projects.length }} 个含 agent 的项目
              </span>
              <div class="flex items-center gap-1 ml-auto">
                <UButton
                  v-if="scanResult.projects.some((p) => !p.registered)"
                  icon="i-lucide-bookmark-plus"
                  size="xs"
                  color="primary"
                  variant="soft"
                  :loading="busy"
                  @click="registerAllScanned"
                >
                  全部登记
                </UButton>
                <UButton icon="i-lucide-x" size="xs" color="neutral" variant="ghost" @click="clearScan">
                  清空
                </UButton>
              </div>
            </div>

            <div v-if="scanResult.projects.length" class="mt-3 space-y-2.5">
              <div
                v-for="proj in scanResult.projects"
                :key="proj.path"
                class="rounded-lg ring ring-default bg-default p-4"
              >
                <div class="flex items-center gap-2 flex-wrap">
                  <UIcon name="i-lucide-folder-git-2" class="size-4 text-primary shrink-0" />
                  <span class="text-sm font-semibold text-highlighted truncate">{{ proj.label }}</span>
                  <UBadge color="neutral" variant="subtle" size="sm">{{ proj.total }} 个 skill</UBadge>
                  <UBadge v-if="proj.registered" color="success" variant="subtle" size="sm" icon="i-lucide-check">已登记</UBadge>
                  <UButton
                    v-else
                    icon="i-lucide-bookmark-plus"
                    size="xs"
                    color="primary"
                    variant="ghost"
                    :loading="busy"
                    @click="registerScanned(proj)"
                  >
                    登记为项目
                  </UButton>
                  <div class="flex items-center -space-x-1.5 ml-auto">
                    <UTooltip v-for="a in proj.agents" :key="a.agent" :text="`${a.agent} · ${a.skills.length} 个`">
                      <AgentAvatar :agent="a.agent" :size="22" />
                    </UTooltip>
                  </div>
                </div>
                <div class="mt-1.5 text-xs text-dimmed font-mono truncate">{{ proj.path }}</div>
                <div class="mt-3 space-y-1.5 border-t border-default pt-3">
                  <div v-for="a in proj.agents" :key="a.agent" class="flex items-start gap-2 text-xs">
                    <AgentAvatar :agent="a.agent" :size="18" class="mt-0.5" />
                    <span class="font-medium text-default shrink-0 w-20 truncate">{{ a.agent }}</span>
                    <span class="text-dimmed">{{ a.skills.join('、') }}</span>
                  </div>
                </div>
              </div>
            </div>
            <div v-else class="mt-3 rounded-lg border border-dashed border-default px-4 py-6 text-center text-xs text-dimmed">
              该目录下未发现含 agent skill 的项目
            </div>
          </UCard>

          <!-- 项目列表 -->
          <div v-if="projects.length" class="space-y-2.5">
            <div
              v-for="p in projects"
              :key="p.id"
              class="rounded-lg ring ring-default bg-default p-4"
            >
              <div class="flex items-start gap-3">
                <span
                  class="flex size-9 shrink-0 items-center justify-center rounded-lg"
                  :class="p.exists ? 'bg-primary/10 text-primary ring-1 ring-primary/15 ring-inset' : 'bg-elevated text-dimmed ring-1 ring-default ring-inset'"
                >
                  <UIcon :name="p.exists ? 'i-lucide-folder-git-2' : 'i-lucide-folder-x'" class="size-4.5" />
                </span>

                <div class="min-w-0 flex-1">
                  <div class="flex items-center gap-2 flex-wrap">
                    <span class="text-sm font-semibold text-highlighted truncate">{{ p.label }}</span>
                    <UBadge color="primary" variant="subtle" size="sm">项目</UBadge>
                    <UBadge v-if="!p.exists" color="error" variant="subtle" size="sm">不存在</UBadge>
                  </div>
                  <div class="mt-1 flex items-center gap-2 text-xs text-dimmed font-mono">
                    <span class="truncate">{{ p.path }}</span>
                  </div>
                  <div class="mt-1 text-xs text-muted">
                    {{ p.exists ? `${p.rootCount} 个 skill 目录 · ${p.skillCount} 个 skill` : '目录不存在' }}
                  </div>
                </div>

                <div class="flex items-center gap-1 shrink-0">
                  <UButton
                    icon="i-lucide-trash-2"
                    size="xs"
                    color="error"
                    variant="ghost"
                    title="移除项目登记（不删除文件）"
                    :loading="busy"
                    @click="removeProject(p)"
                  />
                </div>
              </div>

              <!-- 项目内探测到的 skill 根目录 -->
              <div v-if="rootsOfProject(p.id).length" class="mt-3 space-y-1.5 border-t border-default pt-3">
                <div
                  v-for="r in rootsOfProject(p.id)"
                  :key="r.id"
                  class="flex items-center gap-2 text-xs"
                >
                  <UIcon name="i-lucide-corner-down-right" class="size-3.5 text-dimmed shrink-0" />
                  <AgentAvatar :agent="r.agent" :size="18" />
                  <span class="font-medium text-default shrink-0">{{ r.agent ?? r.label }}</span>
                  <span class="font-mono text-dimmed truncate flex-1">{{ r.path }}</span>
                  <span class="text-muted tabular-nums shrink-0">{{ r.skillCount ?? 0 }} 个</span>
                </div>
              </div>
              <div v-else-if="p.exists" class="mt-3 border-t border-default pt-3 text-xs text-dimmed">
                未在该项目下发现 skill 目录（.claude/skills、.qoder/skills、skills 等）
              </div>
            </div>
          </div>
          <div v-else class="rounded-lg border border-dashed border-default px-4 py-6 text-center text-xs text-dimmed">
            还没有登记任何项目。添加一个项目目录，即可管理其中的项目级 skill。
          </div>
        </section>

        <UAlert v-if="error" color="error" variant="soft" icon="i-lucide-triangle-alert" :title="error" />
        <UAlert v-if="okMsg" color="success" variant="soft" icon="i-lucide-circle-check" :title="okMsg" />
      </div>
    </template>
  </UDashboardPanel>
</template>
