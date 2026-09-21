<script setup lang="ts">
import { Button } from "@/components/ui/button";
import { dailyReportService } from "@/services/daily-report-service";

const emit = defineEmits<{ (e: "open"): void }>();
const preferences = dailyReportService.loadPreferences();
</script>

<template>
  <section class="today-work-card" aria-label="今日工作">
    <header class="tw-head">
      <h2>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"
          stroke-linejoin="round">
          <path d="M9 11l3 3L22 4" />
          <path d="M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11" />
        </svg>
        今日工作
      </h2>
      <p class="tw-sub">从本机 Git 提交生成日报或周报</p>
    </header>

    <dl class="tw-stats">
      <div class="tw-stat">
        <dt>关联项目</dt>
        <dd>{{ preferences.projects.length }}</dd>
      </div>
      <div class="tw-stat">
        <dt>输出方式</dt>
        <dd>一键复制</dd>
      </div>
      <div class="tw-stat">
        <dt>作者筛选</dt>
        <dd>{{ preferences.authors.length ? `${preferences.authors.length} 人` : "全部" }}</dd>
      </div>
      <div class="tw-stat">
        <dt>报告类型</dt>
        <dd>日报 / 周报</dd>
      </div>
    </dl>

    <Button size="sm" variant="outline" class="tw-action" @click="emit('open')">打开 Git 报告 →</Button>
  </section>
</template>

<style scoped>
.today-work-card {
  display: flex;
  min-width: 0;
  flex-direction: column;
  padding: 16px 18px;
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  background: var(--surface);
  box-shadow: var(--shadow-card);
}

.tw-head h2 {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 0;
  color: var(--text);
  font-size: 13.5px;
  font-weight: 700;
}

.tw-head h2 svg {
  width: 15px;
  height: 15px;
  color: var(--accent);
}

.tw-sub {
  margin: 2px 0 0;
  color: var(--text-subtle);
  font-size: 11.5px;
}

.tw-stats {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 7px 14px;
  margin: 13px 0 0;
}

.tw-stat {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 8px;
  padding: 6px 0;
  border-bottom: 1px dashed var(--border);
  font-size: 12.5px;
}

.tw-stat dt {
  color: var(--text-muted);
}

.tw-stat dd {
  margin: 0;
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 11.5px;
  font-weight: 650;
}

.tw-action {
  align-self: flex-start;
  margin-top: 13px;
}
</style>
