<script setup lang="ts">
import { computed } from "vue";
import { storeToRefs } from "pinia";
import { useSkillsStore } from "@/stores/skills";

/**
 * 工作区筛选下拉：全局根目录 / 项目 / Agent 三类入口。
 * Skills 与 MCP 共用；侧边栏不再承载任何动态列表。
 * 选中值编码为 `root:<id>` / `project:<id>` / `agent:<name>`，空串表示全部。
 * 按计数降序排列，计数为 0 的项折叠进「更多」分组。
 */

const props = withDefaults(
  defineProps<{
    modelValue: string;
    /** 单个根目录的计数；默认取 store 中该根目录下的 skill 数 */
    rootCount?: (rootId: string) => number;
    placeholder?: string;
    class?: string;
  }>(),
  { placeholder: "全部工作区" },
);

const emit = defineEmits<{ "update:modelValue": [value: string] }>();

const store = useSkillsStore();
const { roots, projects } = storeToRefs(store);

function countRoot(rootId: string): number {
  return props.rootCount ? props.rootCount(rootId) : store.skills.filter((s) => s.rootId === rootId).length;
}

function countProject(projectId: string): number {
  return roots.value
    .filter((item) => item.projectId === projectId)
    .reduce((total, item) => total + countRoot(item.id), 0);
}

function countAgent(agent: string): number {
  return roots.value
    .filter((item) => item.agent === agent)
    .reduce((total, item) => total + countRoot(item.id), 0);
}

interface FilterOption {
  label: string;
  value: string;
  type?: "label";
  icon?: string;
}

interface WorkspaceEntry {
  label: string;
  value: string;
  count: number;
}

const items = computed<FilterOption[]>(() => {
  const list: FilterOption[] = [{ label: props.placeholder, value: "", icon: "i-lucide-boxes" }];

  function pushGroup(title: string, entries: WorkspaceEntry[]) {
    if (!entries.length) return;
    const active = entries.filter((entry) => entry.count > 0).sort((a, b) => b.count - a.count);
    const empty = entries.filter((entry) => entry.count === 0).sort((a, b) => a.label.localeCompare(b.label));
    list.push({ type: "label", label: title, value: `__group__${title}` });
    list.push(...active.map((entry) => ({ label: `${entry.label} · ${entry.count}`, value: entry.value })));
    if (empty.length) {
      list.push({ type: "label", label: `更多（${empty.length} 项无内容）`, value: `__more__${title}` });
      list.push(...empty.map((entry) => ({ label: entry.label, value: entry.value })));
    }
  }

  pushGroup(
    "全局工作区",
    roots.value
      .filter((item) => item.scope === "global")
      .map((item) => ({
        label: item.agent ?? item.label,
        value: `root:${item.id}`,
        count: countRoot(item.id),
      })),
  );

  pushGroup(
    "项目工作区",
    projects.value.map((item) => ({
      label: item.label,
      value: `project:${item.id}`,
      count: countProject(item.id),
    })),
  );

  const agentIds = [...new Set(roots.value.map((item) => item.agent).filter((agent): agent is string => !!agent))];
  pushGroup(
    "Agent",
    agentIds.map((agent) => ({
      label: agent,
      value: `agent:${agent}`,
      count: countAgent(agent),
    })),
  );

  return list;
});
</script>

<template>
  <USelect
    :model-value="modelValue"
    :items="items"
    icon="i-lucide-boxes"
    :placeholder="placeholder"
    :class="props.class ?? 'w-56'"
    @update:model-value="emit('update:modelValue', String($event ?? ''))"
  />
</template>
