<script setup lang="ts">
import { computed, ref } from "vue";
import { useTranslationHistory } from "@/composables/useTranslationHistory";
import { languageLabel, TRANSLATE_ENGINE_LABELS } from "@/types/translate";
import type { TranslationHistoryEntry } from "@/utils/translate-workspace";
const emit = defineEmits<{ restore: [entry: TranslationHistoryEntry] }>();
const { enabled, retentionDays, entries, remove, clear } = useTranslationHistory();
const search = ref("");
const confirmClear = ref(false);
const statusLabels = { done: "完成", partial: "部分完成", failed: "失败", cancelled: "已取消" };
const filtered = computed(() => {
  const query = search.value.trim().toLowerCase();
  return entries.value.filter(entry => !query || [entry.source, ...entry.segments.map(item => item.translated), languageLabel(entry.from), languageLabel(entry.to), statusLabels[entry.status]].join(" ").toLowerCase().includes(query));
});
</script>
<template>
  <section class="history-panel" aria-label="本地翻译历史">
    <div class="history-controls">
      <label><input v-model="enabled" type="checkbox" /> 保存本地历史</label>
      <label>保留 <select v-model.number="retentionDays"><option :value="7">7 天</option><option :value="30">30 天</option><option :value="90">90 天</option></select></label>
      <input v-model="search" class="history-search" placeholder="搜索原文、译文或语言" aria-label="搜索翻译历史" />
      <button v-if="!confirmClear" type="button" :disabled="!entries.length" @click="confirmClear = true">清空历史</button>
      <template v-else><button type="button" @click="clear(); confirmClear = false">确认清空</button><button type="button" @click="confirmClear = false">取消</button></template>
    </div>
    <p class="history-note">{{ enabled ? '已启用，后续翻译会保存。' : '保存已关闭，后续翻译不会写入历史。' }} 最多 50 条、合计 1MB；单条超过 32KB 不记录。关闭保存不会删除已有记录。</p>
    <div class="history-list">
      <p v-if="!filtered.length" class="history-note">{{ entries.length ? '没有匹配记录' : '暂无翻译历史' }}</p>
      <article v-for="entry in filtered" :key="entry.id" class="history-entry">
        <button type="button" class="history-open" @click="emit('restore', entry)"><span>{{ entry.source.slice(0, 100) }}</span><small>{{ languageLabel(entry.from) }} → {{ languageLabel(entry.to) }} · {{ TRANSLATE_ENGINE_LABELS[entry.engine] }} · {{ statusLabels[entry.status] }} · {{ new Date(entry.createdAt).toLocaleString('zh-CN') }}</small></button>
        <button type="button" aria-label="删除此条历史" @click="remove(entry.id)">删除</button>
      </article>
    </div>
  </section>
</template>
<style scoped>
.history-panel { border: 1px solid var(--border); border-radius: 12px; background: var(--surface); padding: 12px; }
.history-controls { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; font-size: 12px; }
.history-controls label { display: inline-flex; align-items: center; gap: 5px; }
.history-search { flex: 1; min-width: 160px; border: 1px solid var(--border); border-radius: 6px; padding: 6px 8px; background: var(--surface-2); color: var(--text); }
button, select { background: transparent; color: var(--text); font: inherit; cursor: pointer; }
button { border: 0; padding: 4px; } button:disabled { opacity: .4; cursor: default; }
.history-note { margin: 8px 0; color: var(--text-subtle); font-size: 11px; line-height: 1.6; }
.history-list { max-height: 200px; overflow: auto; }
.history-entry { display: flex; align-items: center; gap: 12px; border-top: 1px solid var(--border); font-size: 12px; }
.history-open { display: flex; flex: 1; min-width: 0; flex-direction: column; gap: 4px; text-align: left; padding: 9px 0; }
.history-open span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 100%; }
.history-open small { color: var(--text-subtle); }
</style>
