<script setup lang="ts">
import { Button } from "@/components/ui/button";
import { dailyReportService } from "@/services/daily-report-service";

const emit = defineEmits<{ (e: "open"): void }>();
const preferences = dailyReportService.loadPreferences();
</script>

<template>
  <section class="today-work-card" aria-label="今日工作">
    <div class="today-work-head">
      <div>
        <span>DAILY REPORT</span>
        <h2>今日工作</h2>
      </div>
      <i class="done" />
    </div>
    <p class="today-status">从本机 Git 提交生成日报或周报</p>
    <dl>
      <div><dt>关联项目</dt><dd>{{ preferences.projects.length }}</dd></div>
      <div><dt>输出方式</dt><dd>一键复制</dd></div>
    </dl>
    <Button size="sm" variant="outline" @click="emit('open')">打开 Git 日报</Button>
  </section>
</template>

<style scoped>
.today-work-card {
  display: grid;
  grid-template-columns: 150px minmax(180px, 1fr) minmax(190px, 270px) auto;
  align-items: center;
  gap: 22px;
  min-width: 0;
  padding: 16px 18px;
  border: 1px solid var(--glass-border);
  border-radius: 14px;
  background: color-mix(in srgb, var(--surface) 82%, transparent);
  box-shadow: none;
}
.today-work-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 10px; }
.today-work-head span { display: block; color: var(--text-subtle); font-family: var(--font-mono); font-size: 9px; font-weight: 700; letter-spacing: 1.1px; }
.today-work-head h2 { margin: 4px 0 0; color: var(--text); font-size: 16px; letter-spacing: -.25px; }
.today-work-head i { width: 7px; height: 7px; margin-top: 4px; border-radius: 50%; background: var(--u-warn); box-shadow: 0 0 0 3px color-mix(in srgb, var(--u-warn) 14%, transparent); }
.today-work-head i.done { background: var(--u-ok); box-shadow: 0 0 0 3px color-mix(in srgb, var(--u-ok) 14%, transparent); }
.today-status { margin: 0; color: var(--u-ok); font-size: 12.5px; font-weight: 600; }
dl { display: grid; grid-template-columns: 1fr 1fr; gap: 6px; margin: 0; }
dl div { min-width: 0; padding: 6px 9px; border-left: 1px solid var(--border); }
dt { color: var(--text-subtle); font-size: 9.5px; }
dd { margin: 2px 0 0; color: var(--text); font-family: var(--font-mono); font-size: 11.5px; }
@media (max-width: 900px) { .today-work-card { grid-template-columns: 1fr auto; gap: 14px; } .today-status { grid-column: 1 / -1; } dl { grid-column: 1; } }
@media (max-width: 540px) { .today-work-card { grid-template-columns: 1fr; } dl { grid-column: auto; } .today-work-card :deep(.cn-button) { width: 100%; } }
</style>
