<script setup lang="ts">
import { computed, ref, watch } from "vue";
import type { ZentaoSyncItem, ZentaoSyncResult } from "@/types/productivity";
import { PRIORITY_LABELS, STATUS_LABELS, type TodoPriority, type TodoStatus } from "@/types/productivity";
import { openExternal } from "@/services/zentao-service";
import { formatEstimate } from "@/utils/productivity";

// 同步完成后的明细弹窗：仿禅道任务列表，按 任务 / Bug 分页展示每条写入结果。
const props = defineProps<{
  open: boolean;
  result: ZentaoSyncResult | null;
}>();
const emit = defineEmits<{
  (event: "update:open", value: boolean): void;
}>();

const activeTab = ref<"task" | "bug">("task");
watch(
  () => props.open,
  (open) => {
    if (open) activeTab.value = "task";
  },
);

const items = computed(() => props.result?.items ?? []);
const taskItems = computed(() => items.value.filter((item) => item.externalType !== "bug"));
const bugItems = computed(() => items.value.filter((item) => item.externalType === "bug"));
const currentItems = computed(() => (activeTab.value === "bug" ? bugItems.value : taskItems.value));

const ACTION_LABELS: Record<string, string> = {
  created: "新增",
  updated: "更新",
  unchanged: "无变化",
  closed: "关闭",
};

function priorityLabel(priority: string): string {
  return PRIORITY_LABELS[priority as TodoPriority] ?? priority;
}

function statusLabel(status: string): string {
  return STATUS_LABELS[status as TodoStatus] ?? status;
}

function actionColor(action: string): string {
  if (action === "created") return "success";
  if (action === "updated") return "info";
  if (action === "closed") return "neutral";
  return "neutral";
}

/** 编号列已展示禅道 ID，名称里去掉重复的【#ID】前缀。 */
function displayTitle(item: ZentaoSyncItem): string {
  return item.title.replace(/^【#(\d+)】/, "").trim() || item.title;
}

function openUrl(item: ZentaoSyncItem): void {
  if (!item.externalUrl) return;
  openExternal(item.externalUrl).catch(() => {});
}
</script>

<template>
  <UModal
    :open="open"
    title="禅道同步明细"
    :ui="{ content: 'w-[92vw] max-w-[92vw] sm:max-w-5xl', footer: 'justify-end' }"
    @update:open="emit('update:open', $event)"
  >
    <template #body>
      <div v-if="result" class="sync-detail">
        <div class="toolbar-row">
          <div class="tabs" role="tablist" aria-label="条目类型">
            <button :class="{ active: activeTab === 'task' }" type="button" @click="activeTab = 'task'">
              任务 <em>{{ taskItems.length }}</em>
            </button>
            <button :class="{ active: activeTab === 'bug' }" type="button" @click="activeTab = 'bug'">
              Bug <em>{{ bugItems.length }}</em>
            </button>
          </div>
          <span class="elapsed">耗时 {{ (result.elapsedMs / 1000).toFixed(1) }}s<span v-if="result.requestCount != null"> · {{ result.requestCount }} 次请求</span><span v-if="result.historyDeferred"> · {{ result.historyDeferred }} 条已处理历史未到复查时间</span></span>
        </div>

        <div class="table-wrap">
          <table v-if="currentItems.length" class="detail-table">
            <thead>
              <tr>
                <th class="col-id">编号</th>
                <th>名称</th>
                <th class="col-p">P</th>
                <th class="col-status">状态</th>
                <th class="col-action">变更</th>
                <th class="col-plan">计划 / 估时</th>
                <th class="col-link" />
              </tr>
            </thead>
            <tbody>
              <tr v-for="item in currentItems" :key="`${item.externalType}-${item.zentaoId}`">
                <td class="col-id">#{{ item.zentaoId }}</td>
                <td class="col-title" :title="displayTitle(item)">{{ displayTitle(item) }}</td>
                <td class="col-p">
                  <UBadge
                    :color="item.priority === 'high' ? 'error' : item.priority === 'medium' ? 'warning' : 'neutral'"
                    variant="subtle"
                    size="sm"
                  >
                    {{ priorityLabel(item.priority) }}
                  </UBadge>
                </td>
                <td class="col-status">{{ statusLabel(item.status) }}</td>
                <td class="col-action">
                  <UBadge :color="actionColor(item.action)" variant="subtle" size="sm">
                    {{ ACTION_LABELS[item.action] ?? item.action }}
                  </UBadge>
                </td>
                <td class="col-plan">
                  <template v-if="item.due">{{ item.due }}</template>
                  <template v-if="item.estimateMinutes"> · {{ formatEstimate(item.estimateMinutes) }}</template>
                  <span v-if="!item.due && !item.estimateMinutes">—</span>
                </td>
                <td class="col-link">
                  <UButton
                    v-if="item.externalUrl"
                    icon="i-lucide-external-link"
                    color="neutral"
                    variant="ghost"
                    size="xs"
                    square
                    title="在禅道中打开"
                    @click="openUrl(item)"
                  />
                </td>
              </tr>
            </tbody>
          </table>
          <p v-else class="empty-hint">该类型下没有需要展示的条目。</p>
        </div>

        <p v-if="result.skipped" class="skipped-hint">部分范围未同步：{{ result.skipped }}</p>
      </div>
    </template>

    <template #footer>
      <UButton color="neutral" variant="ghost" size="sm" @click="emit('update:open', false)">关闭</UButton>
    </template>
  </UModal>
</template>

<style scoped>
.sync-detail {
  display: grid;
  gap: 12px;
}
.toolbar-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}
.elapsed {
  color: var(--text-subtle);
  font-size: 11px;
}
.tabs {
  display: inline-flex;
  gap: 2px;
  padding: 3px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--surface-2);
  width: fit-content;
}
.tabs button {
  padding: 4px 12px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--text-muted);
  font-size: 12px;
  font-weight: 600;
}
.tabs button.active {
  background: var(--accent-weak, var(--surface-3));
  color: var(--accent, var(--text));
}
.tabs em {
  margin-left: 3px;
  font-style: normal;
  color: var(--text-subtle);
  font-size: 10.5px;
}
.table-wrap {
  max-height: 60vh;
  overflow: auto;
  border: 1px solid var(--border);
  border-radius: 8px;
  scrollbar-width: thin;
}
.detail-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
}
.detail-table th {
  position: sticky;
  top: 0;
  padding: 7px 10px;
  border-bottom: 1px solid var(--border);
  background: var(--surface-2);
  color: var(--text-muted);
  font-size: 10.5px;
  font-weight: 650;
  text-align: left;
  white-space: nowrap;
}
.detail-table td {
  padding: 7px 10px;
  border-bottom: 1px solid var(--border);
  color: var(--text);
  vertical-align: middle;
}
.detail-table tr:last-child td {
  border-bottom: 0;
}
.col-id {
  width: 64px;
  color: var(--text-muted);
  font-family: var(--font-mono);
  font-size: 11px;
}
.col-title {
  min-width: 0;
  max-width: 340px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.col-p,
.col-action {
  width: 76px;
  white-space: nowrap;
}
.col-status {
  width: 64px;
  white-space: nowrap;
}
.col-plan {
  width: 150px;
  color: var(--text-muted);
  font-size: 11px;
  white-space: nowrap;
}
.col-link {
  width: 40px;
  text-align: right;
}
.empty-hint {
  margin: 0;
  padding: 22px 0;
  color: var(--text-subtle);
  font-size: 12px;
  text-align: center;
}
.skipped-hint {
  margin: 0;
  color: var(--u-warn, #b7791f);
  font-size: 11px;
}
</style>
