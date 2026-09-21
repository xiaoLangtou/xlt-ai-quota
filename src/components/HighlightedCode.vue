<script lang="ts">
const highlighter = Promise.all([
  import("shiki/core"), import("shiki/engine/javascript"), import("shiki/langs/bash.mjs"),
  import("shiki/langs/typescript.mjs"), import("shiki/langs/javascript.mjs"), import("shiki/langs/vue.mjs"),
  import("shiki/langs/rust.mjs"), import("shiki/langs/python.mjs"), import("shiki/langs/json.mjs"),
  import("shiki/langs/sql.mjs"), import("shiki/langs/markdown.mjs"), import("shiki/themes/github-dark-default.mjs"),
  import("shiki/themes/github-light-default.mjs"),
]).then(([core, engine, bash, typescript, javascript, vue, rust, python, json, sql, markdown, githubDark, githubLight]) =>
  core.createHighlighterCore({
    engine: engine.createJavaScriptRegexEngine(),
    langs: [bash.default, typescript.default, javascript.default, vue.default, rust.default, python.default, json.default, sql.default, markdown.default],
    themes: [githubDark.default, githubLight.default],
  }),
);
</script>

<script setup lang="ts">
import { computed, ref, watch } from "vue";

// 超过该长度的内容不再走 Shiki 同步高亮：codeToHtml 会阻塞主线程，
// 且 v-html 注入的 token DOM 体量巨大，复制千行文件时界面会明显卡顿。
// 此时降级为纯文本 <pre>，由 Vue 文本绑定渲染，安全且几乎零成本。
const MAX_HIGHLIGHT_CHARS = 30_000;

const props = withDefaults(defineProps<{ code: string; language: string | null; appearance?: "dark" | "light" }>(), { appearance: "dark" });
const html = ref("");
const plain = computed(() => props.code.length > MAX_HIGHLIGHT_CHARS);

function languageId(value: string | null): string {
  const aliases: Record<string, string> = {
    js: "javascript",
    ts: "typescript",
    shell: "bash",
    sh: "bash",
    yml: "yaml",
    md: "markdown",
  };
  const normalized = value?.trim().toLowerCase() || "text";
  return aliases[normalized] ?? normalized;
}

async function highlight(): Promise<void> {
  if (plain.value) { html.value = ""; return; }
  const instance = await highlighter;
  html.value = instance.codeToHtml(props.code, {
    lang: languageId(props.language),
    theme: props.appearance === "light" ? "github-light-default" : "github-dark-default",
  });
}

watch(() => [props.code, props.language, props.appearance], () => void highlight(), { immediate: true });
</script>

<template>
  <pre v-if="plain" class="highlighted-plain" :class="appearance">{{ code }}</pre>
  <div v-else class="highlighted-code" :class="appearance" v-html="html" />
</template>

<style scoped>
.highlighted-code :deep(pre.shiki) {
  margin: 0;
  overflow: auto;
  background: #24292f !important;
  color: #e6edf3;
  font-family: var(--font-mono);
  font-size: inherit;
  line-height: inherit;
  white-space: pre-wrap;
  word-break: break-word;
}
.highlighted-code :deep(code) { font-family: inherit; }
.highlighted-code.light :deep(pre.shiki) { background:transparent!important; }
.highlighted-plain {
  margin: 0;
  overflow: auto;
  color: inherit;
  font-family: inherit;
  font-size: inherit;
  line-height: inherit;
  white-space: pre-wrap;
  word-break: break-word;
}
</style>
