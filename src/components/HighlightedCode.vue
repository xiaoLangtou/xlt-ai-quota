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
import { ref, watch } from "vue";

const props = withDefaults(defineProps<{ code: string; language: string | null; appearance?: "dark" | "light" }>(), { appearance: "dark" });
const html = ref("");

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
  const instance = await highlighter;
  html.value = instance.codeToHtml(props.code, {
    lang: languageId(props.language),
    theme: props.appearance === "light" ? "github-light-default" : "github-dark-default",
  });
}

watch(() => [props.code, props.language, props.appearance], () => void highlight(), { immediate: true });
</script>

<template><div class="highlighted-code" :class="appearance" v-html="html" /></template>

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
</style>
