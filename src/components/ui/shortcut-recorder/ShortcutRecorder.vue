<script setup lang="ts">
import { computed, ref } from "vue";
import { Keyboard } from "lucide-vue-next";

/**
 * 全局快捷键录制器：点击后直接按下组合键。
 * 输出 Tauri 的快捷键格式（CommandOrControl+Shift+K），
 * 校验「F1–F12 可单独使用、其他必须带修饰键」并检测与已有快捷键的冲突。
 */
const props = withDefaults(
  defineProps<{
    modelValue: string;
    /** 需要避让的其他快捷键 */
    conflicts?: { label: string; value: string }[];
    /** 保存失败等服务端返回的错误 */
    error?: string;
  }>(),
  { conflicts: () => [], error: "" },
);
const emit = defineEmits<{ (event: "update:modelValue", value: string): void }>();

const recorder = ref<HTMLButtonElement | null>(null);
const recording = ref(false);
const localError = ref("");

const message = computed(() => localError.value || props.error);

const isMac = computed(() => /Mac|iPhone|iPad/.test(navigator.platform));

function label(value: string): string {
  return value
    .split("+")
    .map(
      (part) =>
        ({
          CommandOrControl: isMac.value ? "⌘" : "Ctrl",
          Command: "⌘",
          Control: "Ctrl",
          Alt: isMac.value ? "⌥" : "Alt",
          Shift: "⇧",
          Space: "Space",
        })[part] ?? part,
    )
    .join("  ");
}

function begin(): void {
  localError.value = "";
  recording.value = true;
  requestAnimationFrame(() => recorder.value?.focus());
}

function keyOf(event: KeyboardEvent): string | null {
  if (/^Key[A-Z]$/.test(event.code)) return event.code.slice(3);
  if (/^Digit\d$/.test(event.code)) return event.code.slice(5);
  if (/^F([1-9]|1[0-2])$/.test(event.code)) return event.code;
  const keys: Record<string, string> = {
    ArrowUp: "ArrowUp",
    ArrowDown: "ArrowDown",
    ArrowLeft: "ArrowLeft",
    ArrowRight: "ArrowRight",
    Space: "Space",
    Enter: "Enter",
    Tab: "Tab",
    Backspace: "Backspace",
    Delete: "Delete",
    Home: "Home",
    End: "End",
    PageUp: "PageUp",
    PageDown: "PageDown",
    Minus: "-",
    Equal: "=",
    BracketLeft: "[",
    BracketRight: "]",
    Backslash: "\\",
    Semicolon: ";",
    Quote: "'",
    Comma: ",",
    Period: ".",
    Slash: "/",
    Backquote: "`",
  };
  return keys[event.code] ?? null;
}

function record(event: KeyboardEvent): void {
  event.preventDefault();
  event.stopPropagation();
  if (event.key === "Escape") {
    recording.value = false;
    localError.value = "";
    return;
  }
  const key = keyOf(event);
  if (!key) return;
  const standaloneFunctionKey = /^F([1-9]|1[0-2])$/.test(key);
  if (!(event.metaKey || event.ctrlKey || event.altKey || event.shiftKey) && !standaloneFunctionKey) {
    localError.value = "F1–F12 可单独使用，其他按键需要组合键。";
    return;
  }
  const modifiers: string[] = [];
  if (event.metaKey || event.ctrlKey) modifiers.push("CommandOrControl");
  if (event.altKey) modifiers.push("Alt");
  if (event.shiftKey) modifiers.push("Shift");
  const combo = [...modifiers, key].join("+");
  const conflict = props.conflicts.find((item) => item.value && item.value === combo);
  if (conflict) {
    localError.value = `与「${conflict.label}」的快捷键冲突，请换一个组合。`;
    return;
  }
  localError.value = "";
  recording.value = false;
  emit("update:modelValue", combo);
}
</script>

<template>
  <div class="sr">
    <button
      ref="recorder"
      class="sr-button"
      :class="{ recording }"
      type="button"
      :aria-pressed="recording"
      @click="begin"
      @keydown="record"
      @blur="recording = false"
    >
      <Keyboard :size="16" />
      <span v-if="recording">请按下快捷键…</span>
      <kbd v-else>{{ label(modelValue) }}</kbd>
      <em>{{ recording ? "Esc 取消" : "点击录制" }}</em>
    </button>
    <small :class="{ 'sr-error': message }">
      {{ message || "F1–F12 可单独使用，其他按键需要组合；保存时还会检查系统占用" }}
    </small>
  </div>
</template>

<style scoped>
.sr {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.sr-button {
  display: flex;
  width: 100%;
  min-height: 40px;
  align-items: center;
  gap: 10px;
  padding: 0 12px;
  border: 1px solid var(--border);
  border-radius: var(--r-sm);
  outline: 0;
  background: var(--surface-2);
  color: var(--text-muted);
  font: inherit;
  cursor: pointer;
  transition: border-color 0.15s, box-shadow 0.15s, background 0.15s;
}
.sr-button:hover {
  border-color: var(--border-strong);
  background: var(--surface);
}
.sr-button:focus-visible,
.sr-button.recording {
  border-color: var(--accent);
  background: var(--surface);
  box-shadow: 0 0 0 3px var(--accent-weak);
}
.sr-button.recording > svg {
  color: var(--accent);
}
.sr-button > span {
  color: var(--accent);
  font-weight: 650;
}
.sr-button kbd {
  padding: 3px 8px;
  border: 1px solid var(--border-strong);
  border-radius: 5px;
  background: var(--surface);
  box-shadow: 0 1px 0 var(--border-strong);
  color: var(--text);
  font-family: inherit;
  font-size: 12px;
  font-weight: 700;
}
.sr-button em {
  margin-left: auto;
  color: var(--text-subtle);
  font-size: 10.5px;
  font-style: normal;
  font-weight: 500;
}
.sr small {
  color: var(--text-subtle);
  font-size: 11px;
  line-height: 1.5;
}
.sr-error {
  color: var(--u-crit);
}
</style>
