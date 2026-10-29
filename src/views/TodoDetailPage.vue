<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import {
  CATEGORY_LABELS,
  EXTERNAL_TYPE_LABELS,
  PRIORITY_LABELS,
  STATUS_LABELS,
  type TodoTask,
} from "@/types/productivity";
import type { ZentaoDetail } from "@/types/productivity";
import { openExternal, zentaoService } from "@/services/zentao-service";
import { useCalendarTodo } from "@/composables/useCalendarTodo";
import { pushToast } from "@/composables/useToast";
import TodoEditModal from "@/components/calendar/TodoEditModal.vue";
import ZentaoActions from "@/components/calendar/ZentaoActions.vue";
import ZentaoComment from "@/components/calendar/ZentaoComment.vue";
import ZentaoSummary from "@/components/calendar/ZentaoSummary.vue";
import ZentaoDetailContent from "@/components/calendar/ZentaoDetailContent.vue";
import { clipboardService } from "@/services/clipboard-service";
import { zentaoCopyText } from "@/utils/todo-workspace";
import { formatEstimate } from "@/utils/productivity";

// 待办详情独立页面：禅道条目（Bug / 任务）从待办页跳转进来，
// 大屏下以「左侧信息栏 + 右侧宽幅正文」布局替代抽屉，阅读截图与历史更从容。
const props = defineProps<{ taskId: string }>();
const router = useRouter();
const { loadTask, findTask } = useCalendarTodo();

const task = ref<TodoTask | null>(null);
const detail = ref<ZentaoDetail | null>(null);
const detailLoading = ref(false);
const detailRefreshing = ref(false);
const detailFetchedAt = ref<number | null>(null);
const editOpen = ref(false);

// 详情页通栏宽幅：描述截图放开高度上限（按容器宽度自适应），抽屉仍用默认 300px。
const imageMaxHeight = ref<number | null>(null);

const isZentao = computed(() => task.value?.sourceType === "zentao");
const externalLink = computed(() => task.value?.externalUrl ?? "");

let taskGeneration = 0;
let detailGeneration = 0;
onBeforeUnmount(() => { taskGeneration++; detailGeneration++; });
async function copyIdentity(): Promise<void> {
  if (!task.value) return;
  try { await clipboardService.writeText(zentaoCopyText(task.value)); pushToast("编号与标题已复制"); }
  catch (reason) { pushToast(String(reason), "error"); }
}
async function resolveTask(): Promise<boolean> {
  const request = ++taskGeneration;
  const loaded = await loadTask(props.taskId);
  if (request !== taskGeneration) return false;
  if (!loaded) {
    pushToast("待办不存在或已删除", "error");
    await router.replace({ name: "todos" });
    return false;
  }
  task.value = loaded;
  return true;
}

async function loadDetail(force = false): Promise<void> {
  const request = ++detailGeneration;
  const current = task.value;
  if (!current || !isZentao.value || !current.externalId) return;
  const externalId = Number(current.externalId.split(":").pop());
  if (!Number.isFinite(externalId)) return;
  if (force) detailRefreshing.value = true;
  else detailLoading.value = true;
  try {
    const cached = await zentaoService.detail(current.externalType ?? "task", externalId, { force });
    if (request !== detailGeneration || current.id !== props.taskId) return;
    detail.value = cached.detail;
    detailFetchedAt.value = cached.fetchedAt;
  } catch (reason) {
    if (request !== detailGeneration || current.id !== props.taskId) return;
    if (force) {
      pushToast(reason instanceof Error ? reason.message : String(reason), "error");
    } else {
      // 首次拉取失败不阻塞页面，仍展示本地已同步的描述。
      detail.value = null;
    }
  } finally {
    if (request === detailGeneration) {
      detailLoading.value = false;
      detailRefreshing.value = false;
    }
  }
}

/** 详情缓存时间，展示为「HH:MM 缓存」。 */
const detailCachedLabel = computed(() => {
  if (!detailFetchedAt.value) return "";
  const time = new Date(detailFetchedAt.value).toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" });
  return `${time} 缓存`;
});

/** 描述优先取禅道实时详情，失败时回退本地同步副本。 */
const description = computed(() => {
  const text = detail.value?.description || task.value?.description || "";
  return text.trim();
});

/** 信息栏键值对，集中维护避免模板重复。 */
const metaRows = computed(() => {
  const current = task.value;
  if (!current) return [];
  const rows: { label: string; value: string }[] = [
    { label: "分类", value: CATEGORY_LABELS[current.category] },
    {
      label: "来源",
      value: isZentao.value
        ? `禅道 · ${EXTERNAL_TYPE_LABELS[current.externalType ?? ""] ?? "条目"}`
        : (current.sourceName ?? "本地"),
    },
  ];
  if (current.plannedDate) {
    rows.push({ label: "计划", value: `${current.plannedDate}${current.plannedTime ? ` ${current.plannedTime}` : ""}` });
  }
  if (current.estimateMinutes) rows.push({ label: "估时", value: formatEstimate(current.estimateMinutes) });
  rows.push({ label: "创建于", value: new Date(current.createdAt).toLocaleDateString("zh-CN") });
  if (isZentao.value && detailCachedLabel.value) rows.push({ label: "详情", value: detailCachedLabel.value });
  return rows;
});

onMounted(async () => {
  if (await resolveTask()) void loadDetail();
});

watch(() => props.taskId, async () => {
  detailGeneration++;
  detail.value = null;
  detailFetchedAt.value = null;
  if (await resolveTask()) void loadDetail();
});

async function onEdited(): Promise<void> {
  // 编辑弹窗已刷新任务库，这里从缓存取回最新数据。
  task.value = findTask(props.taskId) ?? task.value;
}

function onDeleted(): void {
  pushToast("待办已删除");
  void router.replace({ name: "todos" });
}

async function onZentaoUpdated(updated: TodoTask): Promise<void> {
  task.value = updated;
  await loadDetail(true);
}

function goBack(): void {
  // 常规入口从待办页跳入，返回上一页可保留看板/列表的浏览位置。
  if (window.history.state?.back) router.back();
  else void router.replace({ name: "todos" });
}

function openInZentao(): void {
  if (!externalLink.value) return;
  openExternal(externalLink.value).catch((reason) => {
    pushToast(reason instanceof Error ? reason.message : String(reason), "error");
  });
}
</script>

<template>
  <UDashboardPanel id="todo-detail" :ui="{ body: 'p-0 sm:p-0 gap-0 overflow-hidden' }">
    <template #header>
      <UDashboardNavbar title="待办详情">
        <template #leading>
          <UButton icon="i-lucide-arrow-left" color="neutral" variant="ghost" size="sm" @click="goBack">
            返回
          </UButton>
        </template>
      </UDashboardNavbar>
    </template>

    <template #body>
      <div v-if="task" class="detail-page">
        <!-- 居中阅读区：标题、元信息与正文共享最大宽度。 -->
        <header class="head-card">
          <h1 class="detail-title"><button v-if="isZentao" type="button" class="inline whitespace-nowrap hover:text-primary" title="复制编号与标题" @click="copyIdentity">【#{{ task.externalId?.split(':').pop() }}】</button>{{ isZentao ? task.title.replace(/^【#\d+】\s*/, '') : task.title }}</h1>
          <div class="meta-chips">
            <UBadge :color="task.status === 'done' ? 'success' : task.status === 'doing' ? 'info' : 'neutral'" variant="subtle" size="sm">
              {{ STATUS_LABELS[task.status] }}
            </UBadge>
            <UBadge :color="task.priority === 'high' ? 'error' : task.priority === 'medium' ? 'warning' : 'neutral'" variant="subtle" size="sm">
              {{ PRIORITY_LABELS[task.priority] }}
            </UBadge>
          </div>
          <dl class="meta-rows">
            <div v-for="row in metaRows" :key="row.label" class="meta-row">
              <dt>{{ row.label }}</dt>
              <dd>{{ row.value }}</dd>
            </div>
          </dl>
        </header>
        <main class="detail-main">
          <ZentaoSummary v-if="task && task.id === taskId && detail && !detailLoading && !detailRefreshing" :task="task" :detail="detail" :fetched-at="detailFetchedAt" />
          <ZentaoDetailContent :detail="detail" :loading="detailLoading" :description="description" :image-max-height="imageMaxHeight" />
        </main>
      </div>
      <p v-else class="detail-placeholder">正在加载待办…</p>
    </template>
  </UDashboardPanel>

  <!-- 底部悬浮操作岛（Dock）：滚动时常驻，毛玻璃胶囊样式。 -->
  <div v-if="task" class="action-dock">
    <UButton
      v-if="isZentao"
      icon="i-lucide-refresh-cw"
      :loading="detailRefreshing"
      color="neutral"
      variant="ghost"
      size="sm"
      title="重新从禅道拉取详情"
      @click="loadDetail(true)"
    />
    <UButton
      v-if="isZentao && externalLink"
      icon="i-lucide-external-link"
      color="neutral"
      variant="ghost"
      size="sm"
      title="在禅道中打开"
      @click="openInZentao"
    />
    <span v-if="isZentao" class="dock-divider" />
    <ZentaoComment v-if="isZentao" :key="task.id" :task="task" @commented="loadDetail(true)" />
    <ZentaoActions v-if="isZentao" :key="task.id" :task="task" @updated="onZentaoUpdated" />
    <span v-if="isZentao" class="dock-divider" />
    <UButton icon="i-lucide-pencil" size="sm" @click="editOpen = true">编辑</UButton>
  </div>

  <TodoEditModal v-model:open="editOpen" :task="task" @saved="onEdited" @deleted="onDeleted" />
</template>

<style scoped>
.detail-page {
  display: flex;
  flex-direction: column;
  gap: 18px;
  width: min(100%, 1280px);
  margin-inline: auto;
  height: 100%;
  overflow-y: auto;
  /* 底部预留 Dock 高度，滚动到底时正文不被遮挡。 */
  padding: 16px 16px 96px;
  scrollbar-width: none;
}
.detail-page::-webkit-scrollbar {
  display: none;
}
.head-card {
  display: grid;
  gap: 10px;
  padding: 16px 18px;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--surface-2);
}
.detail-title {
  margin: 0;
  color: var(--text-highlighted, var(--text));
  font-size: 17px;
  font-weight: 700;
  line-height: 1.5;
  word-break: break-word;
}
.meta-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.meta-rows {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 20px;
  margin: 0;
}
.meta-row {
  display: flex;
  gap: 6px;
  font-size: 11.5px;
  line-height: 1.5;
}
.meta-row dt {
  color: var(--text-subtle, var(--text-dimmed));
}
.meta-row dd {
  margin: 0;
  color: var(--text);
  word-break: break-word;
}
.detail-main {
  min-width: 0;
}
.detail-placeholder {
  margin: 0;
  padding: 32px 0;
  color: var(--text-subtle, var(--text-dimmed));
  font-size: 12px;
  text-align: center;
}
/* 悬浮操作岛：底部居中胶囊，毛玻璃 + 投影，高于正文低于全屏查看层。 */
.action-dock {
  position: fixed;
  bottom: 20px;
  left: 50%;
  z-index: 30;
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 7px 12px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: color-mix(in srgb, var(--surface-2, var(--surface, #fff)) 88%, transparent);
  backdrop-filter: blur(16px);
  -webkit-backdrop-filter: blur(16px);
  box-shadow:
    0 12px 32px rgb(0 0 0 / 16%),
    0 2px 8px rgb(0 0 0 / 10%);
  transform: translateX(-50%);
}
.dock-divider {
  width: 1px;
  height: 18px;
  margin: 0 4px;
  background: var(--border);
}
</style>
