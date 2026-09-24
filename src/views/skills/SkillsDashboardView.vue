<script setup lang="ts">
import { computed, onMounted } from "vue";
import { storeToRefs } from "pinia";
import { useSkillsStore } from "@/stores/skills";
import { formatBytes } from "@/utils/skills-markdown";
import AgentAvatar from "@/components/skills/AgentAvatar.vue";

const store = useSkillsStore();
const { roots, skills, loading, error } = storeToRefs(store);

const writableCount = computed(() => roots.value.filter((item) => item.writable).length);
const symlinkCount = computed(() => skills.value.filter((item) => item.isSymlink).length);
const validCount = computed(() => skills.value.filter((item) => item.valid).length);

const recent = computed(() =>
  [...skills.value]
    .sort((a, b) => new Date(b.mtime).getTime() - new Date(a.mtime).getTime())
    .slice(0, 6),
);

const directories = computed(() => roots.value.map((root) => ({
  ...root,
  displayName: root.agent || root.projectLabel || root.label.split(/[\\/]/).filter(Boolean).pop() || root.label,
  count: skills.value.filter((skill) => skill.rootId === root.id).length,
})).sort((a, b) => b.count - a.count).slice(0, 5));
const initialLoading = computed(() => loading.value && !skills.value.length);
const invalidCount = computed(() => skills.value.length - validCount.value);

function formatDate(iso: string): string {
  const date = new Date(iso);
  const diff = Date.now() - date.getTime();
  const day = 24 * 60 * 60 * 1000;
  if (diff < 60 * 60 * 1000) return `${Math.max(1, Math.floor(diff / 60000))} 分钟前`;
  if (diff < day) return `${Math.floor(diff / (60 * 60 * 1000))} 小时前`;
  if (diff < 30 * day) return `${Math.floor(diff / day)} 天前`;
  return date.toLocaleDateString();
}

const stats = computed(() => [
  { label: "技能总数", value: skills.value.length, hint: symlinkCount.value ? `含 ${symlinkCount.value} 个软链接` : "来自已登记的本地目录", icon: "i-lucide-layers", to: "/skills/library" },
  { label: "校验通过", value: validCount.value, hint: invalidCount.value ? `${invalidCount.value} 个未通过校验` : "SKILL.md 结构校验", icon: "i-lucide-circle-check", to: invalidCount.value ? "/skills/library?filter=issues" : "/skills/library" },
  { label: "来源目录", value: roots.value.length, hint: `${writableCount.value} 个可写目录`, icon: "i-lucide-folder-tree", to: "/skills/roots" },
]);

onMounted(() => store.refresh());
</script>

<template>
  <UDashboardPanel id="skills-dashboard" :ui="{ body: 'p-0 sm:p-0 gap-0' }">
    <template #header>
      <UDashboardNavbar title="概览" description="Skills">
        <template #leading>
          <UDashboardSidebarCollapse />
        </template>
        <template #right>
          <UTooltip text="重新扫描本地 Skills">
            <UButton
              icon="i-lucide-refresh-cw"
              color="neutral"
              variant="ghost"
              label="重新扫描"
              :loading="loading"
              @click="store.refresh()"
            />
          </UTooltip>
        </template>
      </UDashboardNavbar>
    </template>

    <template #body>
      <div class="skills-overview">
        <UAlert
          v-if="error"
          color="error"
          variant="soft"
          icon="i-lucide-triangle-alert"
          :title="error"
          class="mb-6"
        />

        <header class="overview-heading">
          <div>
            <h1>本地 Skills</h1>
            <p>查看技能状态，继续最近的工作。</p>
          </div>
          <UButton icon="i-lucide-plus" to="/skills/install">安装技能</UButton>
        </header>

        <div class="overview-stats" aria-label="技能统计" :aria-busy="loading">
          <RouterLink v-for="item in stats" :key="item.label" :to="item.to" class="overview-stat">
            <span class="stat-label"><UIcon :name="item.icon" class="size-4" />{{ item.label }}</span>
            <strong>{{ initialLoading || (error && !skills.length) ? '—' : item.value }}</strong>
            <span class="stat-hint">{{ initialLoading ? '正在扫描…' : item.hint }}</span>
          </RouterLink>
        </div>

        <div class="overview-content">
        <section class="recent-section">
          <div class="section-heading">
            <h2>最近更新</h2>
            <UButton to="/skills/library" trailing-icon="i-lucide-arrow-right" color="neutral" variant="link" size="sm">查看全部</UButton>
          </div>

          <div v-if="initialLoading" class="recent-list" aria-label="正在读取技能">
            <div v-for="i in 4" :key="i" class="flex items-center gap-3 px-4 py-3.5 border-b border-default last:border-b-0">
              <USkeleton class="size-9 rounded-lg" />
              <div class="flex-1 space-y-2">
                <USkeleton class="h-3.5 w-1/3" />
                <USkeleton class="h-3 w-1/4" />
              </div>
            </div>
          </div>

          <div v-else-if="recent.length" class="recent-list">
            <RouterLink
              v-for="item in recent"
              :key="item.id"
              :to="{ name: 'skills-detail', params: { rootId: item.rootId, name: item.name } }"
              class="recent-item"
            >
              <UIcon name="i-lucide-file-text" class="size-5 shrink-0 text-dimmed" />
              <div class="recent-info">
                <div class="recent-name">
                  <strong :title="item.name">{{ item.name }}</strong>
                  <UIcon v-if="item.isSymlink" name="i-lucide-link" class="size-3.5 shrink-0 text-dimmed" title="软链接" />
                  <UIcon v-if="!item.valid" name="i-lucide-triangle-alert" class="size-3.5 shrink-0 text-warning" title="未通过校验" />
                </div>
                <span class="recent-path" :title="item.dirPath">{{ item.rootLabel }}</span>
              </div>
              <div class="recent-meta">
                <time :datetime="item.mtime" :title="new Date(item.mtime).toLocaleString()">{{ formatDate(item.mtime) }}</time>
                <span>{{ formatBytes(item.sizeBytes) }}</span>
              </div>
              <UIcon name="i-lucide-chevron-right" class="size-3.5 shrink-0 text-dimmed" />
            </RouterLink>
          </div>

          <div v-else class="overview-empty">
            <UIcon name="i-lucide-package-open" class="size-7 text-dimmed" />
            <p>{{ error ? '未能读取本地技能' : '还没有发现本地技能' }}</p>
            <span>{{ error ? '请检查来源目录后重新扫描。' : '安装一个技能，或添加已有技能的来源目录。' }}</span>
            <UButton v-if="error" size="sm" color="neutral" variant="outline" :loading="loading" @click="store.refresh()">重新扫描</UButton>
            <UButton v-else to="/skills/roots" size="sm" color="neutral" variant="outline">添加来源目录</UButton>
          </div>
        </section>

        <aside class="directories-section">
          <div class="section-heading">
            <h2>来源目录</h2>
            <UButton to="/skills/roots" color="neutral" variant="link" size="sm">管理</UButton>
          </div>
          <p class="directory-caption">按收录技能数量排序</p>
          <div v-if="initialLoading" class="space-y-5 py-4">
            <USkeleton v-for="i in 3" :key="i" class="h-10 w-full" />
          </div>
          <RouterLink v-for="root in directories" v-else :key="root.id"
            :to="{ name: 'skills-library', query: { root: root.id } }" class="directory-item">
            <AgentAvatar :agent="root.agent || root.displayName" :size="28" />
            <div class="directory-info">
              <strong :title="root.label">{{ root.displayName }}</strong>
              <span :title="root.path">{{ root.scope === 'project' ? root.projectLabel || '项目级' : '全局' }} · {{ root.writable ? '可写' : '只读' }}</span>
            </div>
            <span class="directory-count">{{ root.count }}</span>
          </RouterLink>
          <p v-if="!initialLoading && !directories.length" class="directory-caption py-4">尚未登记来源目录</p>
          <RouterLink v-if="roots.length > 5" to="/skills/roots" class="directory-more">查看全部 {{ roots.length }} 个目录 →</RouterLink>
        </aside>
        </div>
      </div>
    </template>
  </UDashboardPanel>
</template>

<style scoped>
.skills-overview { width: 100%; max-width: 1240px; margin: 0 auto; padding: 28px 32px 36px; container-type: inline-size; }
.overview-heading { display: flex; align-items: center; justify-content: space-between; gap: 16px; }
.overview-heading h1 { margin: 0; font-size: 22px; font-weight: 650; letter-spacing: -.035em; color: var(--text); }
.overview-heading p { margin: 6px 0 0; font-size: 13px; color: var(--text-muted); }
.overview-stats { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); margin: 26px 0 30px; padding: 18px 0; border-block: 1px solid var(--border); }
.overview-stat { display: flex; flex-direction: column; gap: 7px; padding: 0 24px; text-decoration: none; }
.overview-stat:first-child { padding-left: 0; }
.overview-stat + .overview-stat { border-left: 1px solid var(--border); }
.stat-label { display: flex; align-items: center; gap: 7px; color: var(--text-muted); font-size: 12px; }
.overview-stat strong { color: var(--text); font: 600 28px/1.3 var(--font-mono); letter-spacing: -.04em; }
.stat-hint { font-size: 11px; color: var(--text-subtle); }
.overview-stat:hover strong { color: var(--accent); }
.overview-content { display: grid; grid-template-columns: minmax(0, 1fr) 260px; gap: 32px; }
.recent-section, .directories-section { min-width: 0; }
.section-heading { display: flex; align-items: center; justify-content: space-between; gap: 12px; min-height: 32px; margin-bottom: 12px; }
.section-heading h2 { font-size: 13px; font-weight: 600; color: var(--text); }
.recent-item { display: flex; align-items: center; gap: 12px; min-height: 74px; padding: 14px 4px; border-bottom: 1px solid var(--border); text-decoration: none; }
.recent-item:last-child { border-bottom: 0; }
.recent-item:hover, .directory-item:hover { background: var(--surface); }
.recent-info { flex: 1; min-width: 0; }
.recent-name { display: flex; align-items: center; gap: 6px; min-width: 0; }
.recent-name strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13px; font-weight: 550; color: var(--text); }
.recent-path { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; margin-top: 5px; color: var(--text-subtle); font-size: 11px; }
.recent-meta { display: flex; flex-direction: column; align-items: flex-end; gap: 5px; flex: 0 0 auto; color: var(--text-muted); font-size: 11px; font-variant-numeric: tabular-nums; }
.recent-meta span { color: var(--text-subtle); }
.directories-section { padding-left: 26px; border-left: 1px solid var(--border); }
.directory-caption { margin: 0; color: var(--text-subtle); font-size: 11px; }
.directory-item { display: flex; align-items: center; gap: 10px; padding: 14px 0; text-decoration: none; }
.directory-info { display: flex; flex: 1; min-width: 0; flex-direction: column; gap: 4px; }
.directory-info strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; font-weight: 500; color: var(--text); }
.directory-info span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 11px; color: var(--text-subtle); }
.directory-count { font: 12px var(--font-mono); color: var(--text-muted); }
.directory-more { display: inline-block; margin-top: 8px; font-size: 11px; color: var(--text-muted); }
.overview-empty { display: flex; flex-direction: column; align-items: center; gap: 12px; padding: 48px 12px; text-align: center; }
.overview-empty p { margin: 0; font-size: 14px; color: var(--text); }
.overview-empty > span { font-size: 12px; color: var(--text-muted); }
.skills-overview a:focus-visible { outline: 2px solid var(--accent); outline-offset: 4px; border-radius: 4px; }
@container (max-width: 780px) {
  .overview-content { grid-template-columns: minmax(0, 1fr); gap: 24px; }
  .directories-section { border-left: 0; padding-left: 0; border-top: 1px solid var(--border); padding-top: 20px; }
}
@container (max-width: 480px) {
  .overview-heading { align-items: flex-start; flex-wrap: wrap; }
  .overview-stats { gap: 18px; grid-template-columns: 1fr; }
  .overview-stat { padding: 0; display: grid; grid-template-columns: 1fr auto; }
  .overview-stat + .overview-stat { border-left: 0; }
  .overview-stat strong { grid-column: 2; grid-row: 1 / 3; align-self: center; font-size: 24px; }
  .recent-item { gap: 8px; }
}
@media (max-width: 640px) { .skills-overview { padding: 20px 16px; } }
</style>
