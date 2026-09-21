<script setup lang="ts">
import { onMounted, ref } from "vue";
import type { TrashItem } from "@/types/skills";
import { useSkillsStore } from "@/stores/skills";
import { skillsApi, skillsErrorMessage } from "@/api/skills";
import { formatBytes } from "@/utils/skills-markdown";

const store = useSkillsStore();

const items = ref<TrashItem[]>([]);
const loading = ref(false);
const busy = ref("");
const error = ref("");
const okMsg = ref("");

async function load() {
  loading.value = true;
  error.value = "";
  try {
    items.value = await skillsApi.listTrash();
  } catch (caught) {
    error.value = skillsErrorMessage(caught);
  } finally {
    loading.value = false;
  }
}

async function restore(item: TrashItem) {
  busy.value = item.id;
  error.value = "";
  okMsg.value = "";
  try {
    await skillsApi.restoreTrash(item.id);
    okMsg.value = `已恢复：${item.name}`;
    await load();
    await store.refresh();
  } catch (caught) {
    error.value = skillsErrorMessage(caught);
  } finally {
    busy.value = "";
  }
}

async function purge(item: TrashItem) {
  busy.value = item.id;
  error.value = "";
  okMsg.value = "";
  try {
    await skillsApi.purgeTrash(item.id);
    okMsg.value = `已永久删除：${item.name}`;
    await load();
  } catch (caught) {
    error.value = skillsErrorMessage(caught);
  } finally {
    busy.value = "";
  }
}

function formatDate(iso: string): string {
  try {
    return new Date(iso).toLocaleString();
  } catch {
    return iso;
  }
}

onMounted(load);
</script>

<template>
  <UDashboardPanel id="skills-trash">
    <template #header>
      <UDashboardNavbar title="回收站">
        <template #leading>
          <UDashboardSidebarCollapse />
        </template>
        <template #right>
          <UButton icon="i-lucide-refresh-cw" color="neutral" variant="ghost" :loading="loading" @click="load">
            刷新
          </UButton>
        </template>
      </UDashboardNavbar>
    </template>

    <template #body>
      <div class="max-w-3xl w-full mx-auto space-y-4">
        <UAlert v-if="error" color="error" variant="soft" icon="i-lucide-triangle-alert" :title="error" />
        <UAlert v-if="okMsg" color="success" variant="soft" icon="i-lucide-circle-check" :title="okMsg" />

        <div v-if="!loading && !items.length" class="flex flex-col items-center justify-center py-20 text-center">
          <UIcon name="i-lucide-trash-2" class="size-10 text-dimmed mb-3" />
          <div class="text-sm text-muted">回收站是空的</div>
          <div class="text-xs text-dimmed mt-1">删除的 skill 会先移到这里，可随时恢复</div>
        </div>

        <div class="space-y-2.5">
          <div v-for="item in items" :key="item.id" class="rounded-lg ring ring-default bg-default p-4">
            <div class="flex items-start gap-3">
              <span class="flex size-9 shrink-0 items-center justify-center rounded-lg bg-elevated text-dimmed ring-1 ring-default ring-inset">
                <UIcon :name="item.wasSymlink ? 'i-lucide-link' : 'i-lucide-package'" class="size-4.5" />
              </span>
              <div class="min-w-0 flex-1">
                <div class="flex items-center gap-2 flex-wrap">
                  <span class="text-sm font-semibold text-highlighted truncate">{{ item.name }}</span>
                  <UBadge color="neutral" variant="subtle" size="sm">{{ item.rootLabel }}</UBadge>
                  <UBadge v-if="item.wasSymlink" color="info" variant="subtle" size="sm" icon="i-lucide-link">软链接</UBadge>
                </div>
                <div class="mt-1 text-xs text-dimmed font-mono truncate">{{ item.originalPath }}</div>
                <div class="mt-1 text-xs text-muted">
                  删除于 {{ formatDate(item.deletedAt) }}
                  <span v-if="!item.wasSymlink"> · {{ formatBytes(item.sizeBytes) }}</span>
                </div>
              </div>
              <div class="flex items-center gap-1 shrink-0">
                <UButton
                  icon="i-lucide-undo-2"
                  size="xs"
                  color="primary"
                  variant="soft"
                  :loading="busy === item.id"
                  @click="restore(item)"
                >
                  恢复
                </UButton>
                <UButton
                  icon="i-lucide-trash-2"
                  size="xs"
                  color="error"
                  variant="ghost"
                  title="永久删除"
                  :loading="busy === item.id"
                  @click="purge(item)"
                />
              </div>
            </div>
          </div>
        </div>
      </div>
    </template>
  </UDashboardPanel>
</template>
