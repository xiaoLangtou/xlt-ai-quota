<script setup lang="ts">
import type { SkillFileNode } from "@/types/skills";
import { formatBytes } from "@/utils/skills-markdown";

defineProps<{ nodes: SkillFileNode[]; activePath?: string }>();
const emit = defineEmits<{ (e: "select", node: SkillFileNode): void }>();
</script>

<template>
  <ul class="text-sm">
    <li v-for="node in nodes" :key="node.path">
      <div
        v-if="node.type === 'file'"
        class="flex items-center gap-1.5 px-2 h-8 rounded-md transition-colors"
        :class="[
          activePath === node.path ? 'bg-primary/10 text-primary font-medium' : 'text-default',
          node.editable ? 'cursor-pointer hover:bg-elevated' : 'opacity-45 cursor-not-allowed',
        ]"
        :title="node.editable ? node.path : `${node.path}（二进制/不可编辑）`"
        @click="node.editable && emit('select', node)"
      >
        <UIcon
          name="i-lucide-file"
          class="size-4 shrink-0"
          :class="activePath === node.path ? '' : 'text-dimmed'"
        />
        <span class="flex-1 truncate">{{ node.name }}</span>
        <UIcon v-if="activePath === node.path" name="i-lucide-check" class="size-3.5 shrink-0" />
        <span v-else class="text-[11px] text-dimmed tabular-nums">{{ formatBytes(node.sizeBytes ?? 0) }}</span>
      </div>
      <div v-else class="flex items-center gap-1.5 px-2 h-8 text-muted">
        <UIcon name="i-lucide-folder" class="size-4 shrink-0 text-dimmed" />
        <span class="truncate text-xs font-medium">{{ node.name }}</span>
      </div>
      <div v-if="node.children?.length" class="ml-3.5 pl-2 border-l border-default">
        <FileTree :nodes="node.children" :active-path="activePath" @select="emit('select', $event)" />
      </div>
    </li>
  </ul>
</template>
