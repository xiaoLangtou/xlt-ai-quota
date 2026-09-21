<script setup lang="ts">
import { computed } from "vue";
import { useColorMode } from "@vueuse/core";
import { MdEditor } from "md-editor-v3";
import "md-editor-v3/lib/style.css";

const model = defineModel<string>({ required: true });

const colorMode = useColorMode();
const theme = computed<"light" | "dark">(() => (colorMode.value === "dark" ? "dark" : "light"));
</script>

<template>
  <MdEditor
    v-model="model"
    :theme="theme"
    language="zh-CN"
    style="height: 100%"
    :toolbars="[
      'bold',
      'underline',
      'italic',
      'strikeThrough',
      '-',
      'title',
      'sub',
      'sup',
      'quote',
      'unorderedList',
      'orderedList',
      'task',
      '-',
      'codeRow',
      'code',
      'link',
      'image',
      'table',
      '-',
      'revoke',
      'next',
      '=',
      'pageFullscreen',
      'preview',
      'previewOnly',
      'catalog',
    ]"
  />
</template>
