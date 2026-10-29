<script setup lang="ts">
import {
  CATEGORY_LABELS,
  PRIORITY_LABELS,
  TODO_CATEGORIES,
  TODO_PRIORITIES,
  type DecompositionDraft,
} from "@/types/productivity";

const props = defineProps<{
  draft: DecompositionDraft;
  dependencyTitles: Record<string, string>;
}>();

const priorityOptions = TODO_PRIORITIES.map((value) => ({ value, label: PRIORITY_LABELS[value] }));
const categoryOptions = TODO_CATEGORIES.map((value) => ({ value, label: CATEGORY_LABELS[value] }));
const estimateOptions = [
  { value: 0, label: "未估" },
  { value: 30, label: "30m" },
  { value: 60, label: "1h" },
  { value: 90, label: "1.5h" },
  { value: 120, label: "2h" },
  { value: 180, label: "3h" },
  { value: 240, label: "4h" },
  { value: 480, label: "1d" },
];

function addCriterion(): void {
  if (props.draft.acceptanceCriteria.length >= 6) return;
  props.draft.acceptanceCriteria.push("");
}

function removeCriterion(index: number): void {
  props.draft.acceptanceCriteria.splice(index, 1);
}
</script>

<template>
  <article class="draft-card" :class="{ selected: draft.selected }">
    <UCheckbox v-model="draft.selected" class="draft-check" />
    <div class="draft-main">
      <UInput v-model="draft.title" placeholder="任务标题" class="w-full draft-title" />
      <UTextarea
        v-model="draft.description"
        :rows="1"
        placeholder="实现要点（可选）"
        class="w-full"
      />
      <div class="field-row">
        <USelect v-model="draft.priority" :items="priorityOptions" size="sm" aria-label="优先级" />
        <USelect v-model="draft.category" :items="categoryOptions" size="sm" aria-label="分类" />
        <UInput v-model="draft.plannedDate" type="date" size="sm" aria-label="日期" />
        <UInput v-model="draft.plannedTime" type="time" size="sm" aria-label="时间" />
        <USelect v-model="draft.estimateMinutes" :items="estimateOptions" size="sm" aria-label="预估" />
      </div>
      <div class="criteria">
        <div v-for="(_, index) in draft.acceptanceCriteria" :key="index" class="criterion">
          <UIcon name="i-lucide-circle-check" class="criterion-icon" />
          <UInput v-model="draft.acceptanceCriteria[index]" size="sm" placeholder="验收标准" class="w-full" />
          <UButton
            icon="i-lucide-x"
            color="neutral"
            variant="ghost"
            square
            size="xs"
            aria-label="移除验收标准"
            @click="removeCriterion(index)"
          />
        </div>
        <UButton
          v-if="draft.acceptanceCriteria.length < 6"
          icon="i-lucide-plus"
          color="neutral"
          variant="ghost"
          size="xs"
          @click="addCriterion"
        >
          添加验收标准
        </UButton>
      </div>
      <div v-if="draft.dependencyIds.length" class="deps">
        <UIcon name="i-lucide-git-branch" />
        <span>依赖</span>
        <UBadge
          v-for="id in draft.dependencyIds"
          :key="id"
          color="neutral"
          variant="subtle"
          size="sm"
        >
          {{ dependencyTitles[id] ?? id }}
        </UBadge>
      </div>
    </div>
  </article>
</template>

<style scoped>
.draft-card {
  display: grid;
  grid-template-columns: 18px minmax(0, 1fr);
  gap: 9px;
  padding: 11px;
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--surface-2);
}
.draft-card.selected {
  border-color: var(--accent-ring);
  background: color-mix(in srgb, var(--accent) 4%, var(--surface-2));
}
.draft-check {
  margin-top: 4px;
}
.draft-main {
  display: grid;
  gap: 7px;
  min-width: 0;
}
.draft-title :deep(input) {
  font-weight: 600;
}
.field-row {
  display: grid;
  grid-template-columns: repeat(5, minmax(0, 1fr));
  gap: 6px;
}
.criteria {
  display: grid;
  gap: 5px;
}
.criterion {
  display: grid;
  grid-template-columns: 16px minmax(0, 1fr) auto;
  align-items: center;
  gap: 6px;
}
.criterion-icon {
  width: 13px;
  height: 13px;
  color: var(--accent);
}
.deps {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
  color: var(--text-subtle);
  font-size: 10.5px;
}
.deps :deep(svg) {
  width: 13px;
  height: 13px;
}
@media (max-width: 1080px) {
  .field-row {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
</style>
