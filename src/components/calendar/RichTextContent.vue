<script setup lang="ts">
import { computed } from "vue";

// 渲染带【截图n】占位符的文本：文本与图片按原位置交替输出。
// 描述与历史里的富文本变更共用此组件；点击图片向外抛 preview 事件（由父组件开查看层）。
// imageMaxHeight 控制描述截图的显示高度上限：不传默认 300px，传 null 表示放开（跟随容器宽度等比放大）。
const props = defineProps<{
  text: string;
  images: string[];
  emptyTip?: string;
  imageMaxHeight?: number | null;
}>();

const emit = defineEmits<{
  (event: "preview", src: string): void;
}>();

/** 描述截图的最大显示高度；宽幅详情页可放开上限，让截图铺满容器宽度。 */
const effectiveImageMaxHeight = computed(() =>
  props.imageMaxHeight === undefined ? 300 : props.imageMaxHeight,
);
const imageStyle = computed(() => ({ maxHeight: effectiveImageMaxHeight.value === null ? "none" : `${effectiveImageMaxHeight.value}px` }));

type Part = { type: "text"; value: string } | { type: "image"; src: string };

const parts = computed<Part[]>(() => {
  const result: Part[] = [];
  const pattern = /【截图(\d+)】/g;
  const text = props.text ?? "";
  let last = 0;
  let match: RegExpExecArray | null;
  while ((match = pattern.exec(text))) {
    if (match.index > last) result.push({ type: "text", value: text.slice(last, match.index) });
    const src = props.images[Number(match[1]) - 1];
    if (src) result.push({ type: "image", src });
    last = pattern.lastIndex;
  }
  if (last < text.length) result.push({ type: "text", value: text.slice(last) });
  return result.filter((part) => part.type === "image" || part.value.trim().length > 0);
});
</script>

<template>
  <div v-if="parts.length" class="rich-text">
    <template v-for="(part, index) in parts" :key="index">
      <span v-if="part.type === 'text'" class="rich-text-seg">{{ part.value }}</span>
      <img
        v-else
        :src="part.src"
        class="rich-text-image"
        :style="imageStyle"
        title="点击查看大图"
        alt="截图"
        @click="emit('preview', part.src)"
      />
    </template>
  </div>
  <p v-else-if="emptyTip" class="rich-text-empty">{{ emptyTip }}</p>
</template>

<style scoped>
.rich-text {
  color: var(--text);
  font-size: 12.5px;
  line-height: 1.75;
  white-space: pre-wrap;
  word-break: break-word;
}
.rich-text-seg {
  white-space: pre-wrap;
}
.rich-text-image {
  display: block;
  width: 100%;
  max-width: 1200px;
  max-height: 300px;
  margin: 8px auto;
  object-fit: contain;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--surface-2);
  cursor: zoom-in;
}
/* imageMaxHeight=null 时由内联样式接管高度，这里兜底可被覆盖。 */
.rich-text-empty {
  margin: 0;
  color: var(--text-dimmed, var(--text-subtle));
  font-size: 12px;
}
</style>
