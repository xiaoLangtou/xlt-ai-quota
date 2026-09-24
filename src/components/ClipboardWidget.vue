<script setup lang="ts">
import { onMounted, ref } from "vue";
import { Button } from "@/components/ui/button";
import { clipboardService } from "@/services/clipboard-service";

const emit = defineEmits<{ (e: "open"): void }>();
const total = ref<number | null>(null);
const recording = ref(true);

onMounted(async () => {
  try {
    const status = await clipboardService.status();
    total.value = status.total;
    recording.value = status.settings.enabled;
  } catch {
    // 非桌面环境或权限不可用时保持占位，不阻塞概览渲染。
  }
});
</script>

<template>
  <section class="clip-widget" aria-label="剪贴板">
    <header class="cw-head">
      <h2>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"
          stroke-linejoin="round">
          <rect x="5" y="4" width="14" height="17" rx="2" />
          <path d="M9 4a3 3 0 0 1 6 0" />
          <path d="M9 11h6M9 15h4" />
        </svg>
        剪贴板
      </h2>
    </header>

    <div class="cw-value">
      {{ total ?? "—" }}<span class="cw-unit">条 · 本机留存</span>
    </div>

    <div class="cw-state">
      <span class="cw-dot" :class="{ off: !recording }" />
      {{ recording ? "正在记录" : "已暂停记录" }}
    </div>

    <Button size="sm" variant="outline" class="cw-action" @click="emit('open')">查看历史</Button>
  </section>
</template>

<style scoped>
.clip-widget {
  display: flex;
  min-width: 0;
  flex-direction: column;
  padding: 16px 18px;
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  background: var(--surface);
  box-shadow: var(--shadow-card);
}

.cw-head h2 {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 0;
  color: var(--text);
  font-size: 13.5px;
  font-weight: 700;
}

.cw-head h2 svg {
  width: 15px;
  height: 15px;
  color: var(--text-muted);
}

.cw-value {
  display: flex;
  align-items: baseline;
  gap: 7px;
  margin-top: 14px;
  color: var(--text);
  font-family: var(--font-mono);
  font-size: 28px;
  font-weight: 650;
  line-height: 1;
  font-variant-numeric: tabular-nums;
}

.cw-unit {
  color: var(--text-subtle);
  font-family: var(--font-sans);
  font-size: 12.5px;
  font-weight: 500;
}

.cw-state {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 13px;
  padding-top: 13px;
  border-top: 1px solid var(--border);
  color: var(--text-muted);
  font-size: 12px;
}

.cw-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--accent);
}

.cw-dot.off {
  background: var(--text-subtle);
}

.cw-action {
  align-self: flex-start;
  margin-top: 13px;
}
</style>
