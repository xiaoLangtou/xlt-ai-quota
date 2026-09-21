<script setup lang="ts">
import { computed, onMounted } from "vue";
import { storeToRefs } from "pinia";
import { useSkillsStore } from "@/stores/skills";
import { formatBytes } from "@/utils/skills-markdown";

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

function initial(name: string): string {
  return (name[0] ?? "?").toUpperCase();
}

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
  { label: "技能总数", value: String(skills.value.length), icon: "i-lucide-layers", to: "/skills/library" },
  { label: "校验通过", value: `${validCount.value}/${skills.value.length}`, icon: "i-lucide-circle-check", to: "/skills/library" },
  { label: "来源目录（可写）", value: `${writableCount.value}/${roots.value.length}`, icon: "i-lucide-folder-tree", to: "/skills/roots" },
]);

onMounted(() => store.refresh());
</script>

<template>
  <UDashboardPanel id="skills-dashboard">
    <template #header>
      <UDashboardNavbar title="概览" description="Skills">
        <template #leading>
          <UDashboardSidebarCollapse />
        </template>
        <template #right>
          <UTooltip text="重新扫描根目录">
            <UButton
              icon="i-lucide-refresh-cw"
              color="neutral"
              variant="ghost"
              square
              :loading="loading"
              @click="store.refresh()"
            />
          </UTooltip>
        </template>
      </UDashboardNavbar>
    </template>

    <template #body>
      <div class="max-w-5xl w-full mx-auto">
        <UAlert
          v-if="error"
          color="error"
          variant="soft"
          icon="i-lucide-triangle-alert"
          :title="error"
          class="mb-6"
        />

        <div class="pt-2 pb-6 border-b border-default">
          <h1 class="text-2xl font-bold text-highlighted mb-2">你好！</h1>
          <p class="text-sm text-muted">
            技能库中 {{ skills.length }} 个技能 · {{ roots.length }} 个来源目录
            <template v-if="symlinkCount"> · {{ symlinkCount }} 个软链接</template>
          </p>
        </div>

        <div class="grid grid-cols-1 sm:grid-cols-3 gap-4 mt-6">
          <RouterLink v-for="item in stats" :key="item.label" :to="item.to" class="group focus:outline-none">
            <UCard
              class="h-full transition-all duration-200 hover:ring-primary/40 group-focus-visible:ring-2 group-focus-visible:ring-primary"
              :ui="{ body: 'p-4 sm:p-5' }"
            >
              <div class="flex items-start justify-between gap-3">
                <div class="min-w-0">
                  <div class="text-[11px] font-semibold text-dimmed uppercase tracking-wider mb-2">{{ item.label }}</div>
                  <div class="text-2xl font-bold text-highlighted tabular-nums">{{ item.value }}</div>
                </div>
                <span class="flex size-10 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary ring-1 ring-primary/15 ring-inset">
                  <UIcon :name="item.icon" class="size-5" />
                </span>
              </div>
            </UCard>
          </RouterLink>
        </div>

        <div class="grid grid-cols-1 sm:grid-cols-2 gap-4 mt-5">
          <UButton icon="i-lucide-refresh-cw" size="lg" block :loading="loading" @click="store.refresh()">
            扫描本地 Skills
          </UButton>
          <UButton icon="i-lucide-layers" size="lg" block color="neutral" variant="outline" to="/skills/library">
            浏览技能库
          </UButton>
        </div>

        <div class="mt-8">
          <h2 class="text-sm font-semibold text-highlighted mb-3">近期动态</h2>

          <UCard v-if="loading && skills.length === 0" :ui="{ body: 'p-0' }">
            <div v-for="i in 4" :key="i" class="flex items-center gap-3 px-4 py-3.5 border-b border-default last:border-b-0">
              <USkeleton class="size-9 rounded-lg" />
              <div class="flex-1 space-y-2">
                <USkeleton class="h-3.5 w-1/3" />
                <USkeleton class="h-3 w-1/4" />
              </div>
            </div>
          </UCard>

          <UCard v-else-if="recent.length" :ui="{ body: 'p-0' }">
            <RouterLink
              v-for="item in recent"
              :key="item.id"
              :to="{ name: 'skills-detail', params: { rootId: item.rootId, name: item.name } }"
              class="group flex items-center gap-3 px-4 py-3.5 border-b border-default last:border-b-0 hover:bg-elevated/50 transition-colors focus:outline-none focus-visible:bg-elevated"
            >
              <span class="flex size-9 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary text-sm font-bold">
                {{ initial(item.name) }}
              </span>
              <div class="min-w-0 flex-1">
                <div class="flex items-center gap-2 min-w-0">
                  <span class="text-sm font-semibold text-highlighted truncate group-hover:text-primary transition-colors">
                    {{ item.name }}
                  </span>
                  <span class="shrink-0 rounded border border-default px-1.5 py-px text-[10px] text-dimmed">
                    {{ item.rootLabel }}
                  </span>
                  <UBadge v-if="item.isSymlink" color="info" variant="subtle" size="sm">软链接</UBadge>
                </div>
                <div class="text-xs text-muted mt-0.5">
                  更新于 {{ formatDate(item.mtime) }} · {{ formatBytes(item.sizeBytes) }}
                </div>
              </div>
              <UIcon name="i-lucide-chevron-right" class="size-4 text-dimmed shrink-0 opacity-0 group-hover:opacity-100 transition-opacity" />
            </RouterLink>
          </UCard>

          <div v-else class="flex flex-col items-center py-16 rounded-lg ring ring-default">
            <UIcon name="i-lucide-package-open" class="size-8 text-dimmed mb-3" />
            <p class="text-sm text-muted mb-4">还没有任何 skill</p>
            <UButton size="sm" icon="i-lucide-refresh-cw" @click="store.refresh()">重新扫描</UButton>
          </div>
        </div>
      </div>
    </template>
  </UDashboardPanel>
</template>
