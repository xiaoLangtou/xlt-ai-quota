<script setup lang="ts">
import { useToast } from "@/composables/useToast";

const { items, dismiss } = useToast();

const ICON: Record<string, string> = {
  success: "✓",
  error: "✕",
  warn: "!",
  info: "i",
};
</script>

<template>
  <Teleport to="body">
    <div class="toast-host" aria-live="polite" aria-atomic="false">
      <TransitionGroup name="toast">
        <div
          v-for="t in items"
          :key="t.id"
          class="toast"
          :class="t.tone"
          role="status"
          @click="dismiss(t.id)"
        >
          <span class="toast-icon">{{ ICON[t.tone] }}</span>
          <span class="toast-msg">{{ t.message }}</span>
        </div>
      </TransitionGroup>
    </div>
  </Teleport>
</template>

<style scoped>
.toast-host {
  position: fixed;
  right: 20px;
  bottom: 20px;
  z-index: 2000;
  display: flex;
  flex-direction: column;
  gap: 10px;
  max-width: min(380px, calc(100vw - 40px));
  pointer-events: none;
}
.toast {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 15px;
  border: 1px solid var(--glass-border, var(--border));
  border-radius: 14px;
  background: var(--glass-fill, var(--surface));
  box-shadow: var(--glass-shadow, var(--shadow-pop));
  backdrop-filter: blur(18px) saturate(180%);
  -webkit-backdrop-filter: blur(18px) saturate(180%);
  color: var(--text);
  font-size: 13px;
  cursor: pointer;
  pointer-events: auto;
}
.toast-icon {
  display: grid;
  place-items: center;
  width: 20px;
  height: 20px;
  flex: 0 0 20px;
  border-radius: 999px;
  color: #fff;
  font-size: 12px;
  font-weight: 700;
}
.toast.success .toast-icon {
  background: var(--u-ok, #1e9a76);
}
.toast.error .toast-icon {
  background: var(--u-crit, #e9556b);
}
.toast.warn .toast-icon {
  background: var(--u-warn, #f2924a);
}
.toast.info .toast-icon {
  background: var(--accent, #5457e6);
}
.toast-msg {
  min-width: 0;
  line-height: 1.4;
  word-break: break-word;
}
.toast-enter-active,
.toast-leave-active {
  transition: opacity 0.22s ease, transform 0.22s ease;
}
.toast-enter-from,
.toast-leave-to {
  opacity: 0;
  transform: translateY(8px);
}
</style>
