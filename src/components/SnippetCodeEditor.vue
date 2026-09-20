<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import HighlightedCode from "@/components/HighlightedCode.vue";

const props = withDefaults(defineProps<{ modelValue: string; language: string | null; placeholder?: string }>(), {
  placeholder: "粘贴或输入可复用内容",
});
const emit = defineEmits<{ (event: "update:modelValue", value: string): void }>();
const input = ref<HTMLElement | null>(null);
const highlight = ref<HTMLElement | null>(null);
const lineCount = computed(() => Math.max(1, props.modelValue.split("\n").length));

function update(): void {
  if (input.value) emit("update:modelValue", input.value.innerText);
}
function insertText(text: string): void {
  const selection = window.getSelection();
  if (!selection || !selection.rangeCount) return;
  const range = selection.getRangeAt(0);
  range.deleteContents();
  const node = document.createTextNode(text);
  range.insertNode(node);
  range.setStartAfter(node);
  selection.removeAllRanges();
  selection.addRange(range);
  update();
}
function handleKeydown(event: KeyboardEvent): void {
  if (event.key !== "Tab") return;
  event.preventDefault();
  insertText("  ");
}
function handlePaste(event: ClipboardEvent): void {
  event.preventDefault();
  insertText(event.clipboardData?.getData("text/plain") ?? "");
}
function syncScroll(): void {
  const pre = highlight.value?.querySelector("pre");
  if (!input.value || !pre) return;
  pre.scrollTop = input.value.scrollTop;
  pre.scrollLeft = input.value.scrollLeft;
}

watch(() => props.modelValue, (value) => {
  if (input.value && document.activeElement !== input.value) input.value.textContent = value;
});
onMounted(() => { if (input.value) input.value.textContent = props.modelValue; });
</script>

<template>
  <div class="snippet-editor">
    <div class="line-numbers" aria-hidden="true"><span v-for="line in lineCount" :key="line">{{ line }}</span></div>
    <div class="editor-stack">
      <div ref="highlight" class="editor-highlight" aria-hidden="true"><HighlightedCode :code="modelValue" :language="language" /></div>
      <pre ref="input" class="editor-input" contenteditable="true" spellcheck="false" role="textbox" aria-multiline="true" :aria-label="placeholder" :data-placeholder="placeholder" @input="update" @keydown="handleKeydown" @paste="handlePaste" @scroll="syncScroll" />
    </div>
  </div>
</template>

<style scoped>
.snippet-editor{display:grid;grid-template-columns:42px minmax(0,1fr);min-height:220px;overflow:hidden;border:1px solid var(--border-strong);border-radius:var(--r-sm);background:#24292f;font-family:var(--font-mono);font-size:12px;line-height:1.65}.snippet-editor:focus-within{border-color:var(--accent);box-shadow:0 0 0 3px var(--accent-weak)}.line-numbers{display:flex;align-items:flex-end;flex-direction:column;gap:0;overflow:hidden;padding:12px 9px;background:#1d2328;color:#68727c;line-height:1.65;text-align:right;user-select:none}.editor-stack{position:relative;min-width:0}.editor-highlight,.editor-input{position:absolute;inset:0;width:100%;height:100%;margin:0;padding:12px;overflow:auto;white-space:pre-wrap;word-break:break-word}.editor-highlight{pointer-events:none}.editor-highlight :deep(.highlighted-code),.editor-highlight :deep(pre.shiki){min-height:100%;padding:0;overflow:hidden}.editor-input{z-index:1;border:0;outline:0;background:transparent;color:transparent;caret-color:#f0f6fc;font:inherit;line-height:inherit;resize:none}.editor-input::selection{background:rgba(113,171,255,.28)}.editor-input:empty::before{color:#7c8792;content:attr(data-placeholder);pointer-events:none}
</style>
