<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { EditorState } from "@codemirror/state";
import { EditorView, keymap, lineNumbers, highlightActiveLine } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { markdown } from "@codemirror/lang-markdown";

const props = defineProps<{ modelValue: string; language?: "markdown" | "text"; scrollToLine?: number }>();
const emit = defineEmits<{ (e: "update:modelValue", value: string): void }>();

const host = ref<HTMLDivElement>();
let view: EditorView | null = null;

function createState(doc: string) {
  const extensions = [
    lineNumbers(),
    highlightActiveLine(),
    history(),
    keymap.of([...defaultKeymap, ...historyKeymap]),
    EditorView.lineWrapping,
    EditorView.updateListener.of((update) => {
      if (update.docChanged) emit("update:modelValue", update.state.doc.toString());
    }),
  ];
  if (props.language !== "text") extensions.push(markdown());
  return EditorState.create({ doc, extensions });
}

onMounted(() => {
  view = new EditorView({ state: createState(props.modelValue), parent: host.value! });
});

watch(
  () => props.modelValue,
  (value) => {
    if (view && value !== view.state.doc.toString()) {
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: value } });
    }
  },
);

// 可选：滚动到指定行并选中（用于从表格跳转到源码对应行）
watch(
  () => props.scrollToLine,
  (line) => {
    if (!view || !line) return;
    const clamped = Math.min(Math.max(1, line), view.state.doc.lines);
    const target = view.state.doc.line(clamped);
    view.dispatch({
      selection: { anchor: target.from },
      effects: EditorView.scrollIntoView(target.from, { y: "center" }),
    });
    view.focus();
  },
);

onBeforeUnmount(() => view?.destroy());
</script>

<template>
  <div ref="host" class="h-full min-h-40"></div>
</template>
