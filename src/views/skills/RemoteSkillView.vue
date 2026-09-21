<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import type { RemoteSkillDetail, RemoteSource } from '@/types/skills'
import { skillsApi } from '@/api/skills'
import { formatBytes, renderMarkdown } from '@/utils/skills-markdown'

/**
 * 远程技能详情页（安装前查看，不落 staging）：
 * 布局与本地 SkillDetail 对齐——左文件列表、右 markdown 内容，导航栏返回 + 安装入口。
 * 入参走 query：source（github/skills-sh）、ref、homepage/detailUrl（可选外链）、hasScripts。
 */
const route = useRoute()
const router = useRouter()

const source = computed(() => (route.query.source as RemoteSource) || 'github')
const refStr = computed(() => (route.query.ref as string) || '')
const homepage = computed(() => (route.query.homepage as string) || '')
const detailUrl = computed(() => (route.query.detailUrl as string) || '')
const hasScripts = computed(() => route.query.hasScripts === '1')

const loading = ref(true)
const error = ref('')
const detail = ref<RemoteSkillDetail | null>(null)

// 文件预览：'' 表示 SKILL.md
const activeFile = ref('')
const fileLoading = ref(false)
const fileError = ref('')
const fileContent = ref('')
const fileCache = new Map<string, string>()

/** 可点击预览的文件（目录以 / 结尾，不可点） */
const fileEntries = computed(() =>
  (detail.value?.files ?? []).filter((f) => !/skill\.md$/i.test(f.path)))

const skillHtml = computed(() => (detail.value ? renderMarkdown(detail.value.body) : ''))
const fileHtml = computed(() => {
  if (!activeFile.value) return skillHtml.value
  // markdown 文件直接渲染，其他文本文件包代码块展示
  if (/\.(md|markdown)$/i.test(activeFile.value)) return renderMarkdown(fileContent.value)
  return renderMarkdown(`\`\`\`\n${fileContent.value}\n\`\`\``)
})

async function load() {
  loading.value = true
  error.value = ''
  detail.value = null
  activeFile.value = ''
  fileCache.clear()
  try {
    detail.value = await skillsApi.getRemoteDetail(source.value, refStr.value)
  } catch (e) {
    error.value = (e as Error).message
  } finally {
    loading.value = false
  }
}

async function selectFile(path: string) {
  if (path.endsWith('/')) return // 目录不可预览
  activeFile.value = path
  if (!path) return
  fileError.value = ''
  const cached = fileCache.get(path)
  if (cached !== undefined) {
    fileContent.value = cached
    return
  }
  fileLoading.value = true
  fileContent.value = ''
  try {
    const res = await skillsApi.getRemoteFile(source.value, refStr.value, path)
    fileCache.set(path, res.content)
    // 丢弃过时响应（用户已切换到其他文件）
    if (activeFile.value === path) fileContent.value = res.content
  } catch (e) {
    if (activeFile.value === path) fileError.value = (e as Error).message
  } finally {
    fileLoading.value = false
  }
}

/** 转入安装页并自动触发预览（统一走「预览 → 确认」流程） */
function goInstall() {
  router.push({
    name: 'skills-install',
    query: { source: source.value, ref: refStr.value, auto: '1' },
  })
}

function goBack() {
  if (window.history.length > 1) router.back()
  else router.push({ name: 'skills-install' })
}

watch([source, refStr], load, { immediate: true })
</script>

<template>
  <UDashboardPanel id="remote-skill">
    <template #header>
      <UDashboardNavbar :title="detail?.name || '技能详情'" :ui="{ right: 'gap-1.5' }">
        <template #leading>
          <UButton icon="i-lucide-arrow-left" color="neutral" variant="ghost" square @click="goBack" />
        </template>
        <template #trailing>
          <UBadge color="info" variant="subtle" size="sm" icon="i-lucide-cloud">远程</UBadge>
          <UBadge v-if="detail?.version" color="neutral" variant="subtle" size="sm" class="font-mono">v{{ detail.version }}</UBadge>
          <UBadge v-if="detail?.license" color="neutral" variant="subtle" size="sm">{{ detail.license }}</UBadge>
          <UBadge v-if="hasScripts" color="warning" variant="subtle" size="sm" icon="i-lucide-terminal">含脚本</UBadge>
        </template>
        <template #right>
          <UButton
            v-if="homepage"
            icon="i-lucide-github"
            color="neutral"
            variant="ghost"
            :to="homepage"
            target="_blank"
          >
            源仓库
          </UButton>
          <UButton
            v-if="detailUrl"
            icon="i-lucide-external-link"
            color="neutral"
            variant="ghost"
            :to="detailUrl"
            target="_blank"
          >
            skills.sh
          </UButton>
          <UButton icon="i-lucide-download" :disabled="loading || !!error" @click="goInstall">安装</UButton>
        </template>
      </UDashboardNavbar>
    </template>

    <template #body>
      <UAlert v-if="error" color="error" variant="soft" icon="i-lucide-triangle-alert" :title="error" class="mb-4" />

      <!-- 加载骨架 -->
      <div v-if="loading" class="space-y-5">
        <USkeleton class="h-4 w-64" />
        <div class="grid grid-cols-1 lg:grid-cols-[250px_1fr] gap-5">
          <USkeleton class="h-48 rounded-lg" />
          <USkeleton class="h-96 rounded-lg" />
        </div>
      </div>

      <template v-if="detail && !loading">
        <!-- 元信息 -->
        <div class="flex flex-wrap items-center gap-x-4 gap-y-1.5 mb-5 text-xs text-muted">
          <span class="inline-flex items-center gap-1.5 min-w-0 max-w-full">
            <UIcon name="i-lucide-link-2" class="size-3.5 text-dimmed shrink-0" />
            <span class="truncate font-mono">{{ refStr }}</span>
          </span>
          <span v-if="detail.files?.length" class="inline-flex items-center gap-1.5 tabular-nums">
            <UIcon name="i-lucide-files" class="size-3.5 text-dimmed" />{{ detail.files.length }} 项
          </span>
        </div>

        <p v-if="detail.description" class="text-sm text-muted mb-5">{{ detail.description }}</p>

        <div class="grid gap-5 grid-cols-1 lg:grid-cols-[250px_1fr] items-start">
          <!-- 左：文件列表 -->
          <UCard :ui="{ body: 'p-2 sm:p-2.5' }" class="self-start lg:sticky lg:top-0">
            <div class="flex items-center gap-1.5 px-2 pt-0.5 pb-2 text-[11px] font-semibold text-dimmed uppercase tracking-wider">
              <UIcon name="i-lucide-files" class="size-3.5" />
              文件
              <span class="ml-auto font-normal tabular-nums">{{ (detail.files?.length ?? 0) || 1 }}</span>
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
            <div
              v-for="f in fileEntries"
              :key="f.path"
              class="flex items-center gap-1.5 px-2 h-8 rounded-md transition-colors text-sm"
              :class="[
                f.path.endsWith('/') ? 'text-dimmed' : 'cursor-pointer',
                activeFile === f.path ? 'bg-primary/10 text-primary font-medium' : !f.path.endsWith('/') ? 'hover:bg-elevated text-default' : '',
              ]"
              @click="selectFile(f.path)"
            >
              <UIcon :name="f.path.endsWith('/') ? 'i-lucide-folder' : 'i-lucide-file'" class="size-4 shrink-0" />
              <span class="flex-1 truncate font-mono text-xs">{{ f.path }}</span>
              <span v-if="f.sizeBytes != null" class="text-[10px] text-dimmed tabular-nums shrink-0">{{ formatBytes(f.sizeBytes) }}</span>
            </div>
            <div v-if="!detail.files" class="px-2 py-1.5 text-[11px] text-dimmed">
              文件清单暂不可用（仅展示 SKILL.md）
            </div>
          </UCard>

          <!-- 右：内容 -->
          <section class="min-w-0">
            <div v-if="activeFile && fileLoading" class="flex items-center gap-2 py-10 justify-center text-xs text-dimmed">
              <UIcon name="i-lucide-loader-circle" class="size-4 animate-spin" />加载文件内容…
            </div>
            <UAlert v-else-if="activeFile && fileError" color="error" variant="soft" icon="i-lucide-triangle-alert" :title="fileError" />
            <UCard v-else :ui="{ body: 'p-5 sm:p-6' }"><div class="markdown-body" v-html="fileHtml" /></UCard>
          </section>
        </div>
      </template>
    </template>
  </UDashboardPanel>
</template>
