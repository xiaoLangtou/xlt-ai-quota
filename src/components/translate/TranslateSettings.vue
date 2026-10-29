<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { Languages, Sparkles } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import ShortcutRecorder from "@/components/ui/shortcut-recorder/ShortcutRecorder.vue";
import { pushToast } from "@/composables/useToast";
import { clipboardService } from "@/services/clipboard-service";
import { translateService } from "@/services/translate-service";
import {
  DEFAULT_TRANSLATE_CONFIG,
  TRANSLATE_DOMAINS,
  TRANSLATE_ENGINE_LABELS,
  TRANSLATE_ENGINES,
  type TranslateConfig,
  type TranslateShortcutStatus,
} from "@/types/translate";

/**
 * 配置是纯 JSON 结构，但 ref 里存的是响应式代理，
 * 直接 structuredClone 会抛 DataCloneError（The object can not be cloned）。
 */
function cloneConfig(source: TranslateConfig): TranslateConfig {
  return JSON.parse(JSON.stringify(source)) as TranslateConfig;
}

const config = ref<TranslateConfig>(cloneConfig(DEFAULT_TRANSLATE_CONFIG));
const draft = ref<TranslateConfig>(cloneConfig(DEFAULT_TRANSLATE_CONFIG));
const loading = ref(true);
const saving = ref(false);
const testing = ref(false);
const error = ref("");
const conflicts = ref<{ label: string; value: string }[]>([]);
const shortcutStatus = ref<TranslateShortcutStatus | null>(null);

const domainOptions = TRANSLATE_DOMAINS.map((item) => ({ label: item.label, value: item.value }));
const thinkingOptions = [
  { label: "已禁用", value: "disabled" },
  { label: "已启用", value: "enabled" },
];
const engineLabel = computed(() => TRANSLATE_ENGINE_LABELS[config.value.engine] ?? config.value.engine);
const dirty = computed(() => JSON.stringify(config.value) !== JSON.stringify(draft.value));

onMounted(async () => {
  try {
    const loaded = await translateService.getConfig();
    config.value = loaded;
    draft.value = cloneConfig(loaded);
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : String(reason);
  } finally {
    loading.value = false;
  }
  try {
    const status = await clipboardService.status();
    conflicts.value = [{ label: "剪贴板历史", value: status.settings.shortcut }];
  } catch {
    // 剪贴板面板不可用时不做冲突提示，保存时仍由 Rust 侧检查占用
  }
  await refreshShortcutStatus();
});

async function refreshShortcutStatus(): Promise<void> {
  try {
    shortcutStatus.value = await translateService.shortcutStatus();
  } catch {
    shortcutStatus.value = null;
  }
}

/** 录制即生效：只持久化快捷键，不牵动表单里其他未保存的改动。 */
async function applyShortcut(value: string): Promise<void> {
  error.value = "";
  try {
    const status = await translateService.setShortcut(value);
    shortcutStatus.value = status;
    config.value = { ...config.value, shortcut: value };
    draft.value = { ...draft.value, shortcut: value };
    pushToast(`快捷键已更新：${value}`, "success");
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : String(reason);
    await refreshShortcutStatus();
  }
}

async function previewPanel(): Promise<void> {
  try {
    await translateService.showPanel();
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : String(reason);
  }
}

async function save(): Promise<void> {
  if (saving.value) return;
  saving.value = true;
  error.value = "";
  try {
    const saved = await translateService.saveConfig(cloneConfig(draft.value));
    config.value = saved;
    draft.value = cloneConfig(saved);
    await refreshShortcutStatus();
    pushToast("翻译设置已保存", "success");
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : String(reason);
    await refreshShortcutStatus();
  } finally {
    saving.value = false;
  }
}

async function test(): Promise<void> {
  if (testing.value) return;
  testing.value = true;
  error.value = "";
  try {
    const translated = await translateService.test(cloneConfig(draft.value));
    pushToast(`连接成功：Hello world → ${translated}`, "success");
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : String(reason);
  } finally {
    testing.value = false;
  }
}
</script>

<template>
  <section class="ts-sec">
    <div class="ts-head">
      <span class="ts-icon"><Languages :size="18" /></span>
      <span class="ts-copy">
        <strong>翻译服务</strong>
        <small>内置服务、超时与快捷面板快捷键（凭据只保存在本机）</small>
      </span>
    </div>
    <div class="ts-body">
      <p v-if="loading" class="ts-note">正在读取翻译配置…</p>
      <template v-else>
        <div class="ts-row">
          <span class="ts-label">
            内置服务
            <small>当前使用 {{ engineLabel }}；切换后需保存才生效</small>
          </span>
          <Select v-model="draft.engine" :options="TRANSLATE_ENGINES" />
        </div>
        <div class="ts-row">
          <span class="ts-label">
            超时时间
            <small>单次请求超时，单位毫秒</small>
          </span>
          <span class="ts-inline">
            <Input v-model.number="draft.timeoutMs" type="number" min="3000" max="120000" step="1000" />
            <span class="ts-unit">ms</span>
          </span>
        </div>
        <div class="ts-row">
          <span class="ts-label">
            唤起快捷键
            <small>全局生效，任何应用下都能唤起快捷面板；录制完立即注册</small>
          </span>
          <div class="ts-shortcut">
            <ShortcutRecorder
              :model-value="draft.shortcut"
              :conflicts="conflicts"
              @update:model-value="applyShortcut"
            />
            <p
              v-if="shortcutStatus"
              class="ts-shortcut-state"
              :class="{
                warn: draft.shortcut !== shortcutStatus.shortcut,
                bad: draft.shortcut === shortcutStatus.shortcut && !shortcutStatus.registered,
              }"
            >
              <template v-if="draft.shortcut !== shortcutStatus.shortcut">
                未保存：保存后才会注册为 {{ draft.shortcut }}（当前生效 {{ shortcutStatus.shortcut }}）
              </template>
              <template v-else-if="shortcutStatus.registered">
                已注册到系统：{{ shortcutStatus.shortcut }}
              </template>
              <template v-else>
                未生效：{{ shortcutStatus.error || "快捷键被系统或其他应用占用，换一个组合后再保存" }}
              </template>
            </p>
            <Button type="button" variant="ghost" size="sm" @click="previewPanel">测试唤起面板</Button>
          </div>
        </div>
      </template>
    </div>
  </section>

  <section v-if="!loading" class="ts-sec">
    <div class="ts-head">
      <span class="ts-icon"><Sparkles :size="18" /></span>
      <span class="ts-copy">
        <strong>{{ engineLabel }}</strong>
        <small v-if="draft.engine === 'youdao'">有道智云文本翻译 API v3，按段落请求</small>
        <small v-else>OpenAI 兼容 chat completions，提示词可自定义</small>
      </span>
    </div>
    <div class="ts-body">
      <template v-if="draft.engine === 'youdao'">
        <div class="ts-row">
          <span class="ts-label">
            基础 URL
            <small>使用有道 OpenAPI 地址，不要以斜杠结尾</small>
          </span>
          <Input v-model.trim="draft.youdao.baseUrl" placeholder="https://openapi.youdao.com/api" />
        </div>
        <div class="ts-row">
          <span class="ts-label">
            应用 ID
            <small>有道智云控制台的应用 appKey</small>
          </span>
          <Input v-model.trim="draft.youdao.appKey" placeholder="你的有道翻译应用 ID" />
        </div>
        <div class="ts-row">
          <span class="ts-label">
            应用密钥
            <small>只参与签名，不会随请求上传</small>
          </span>
          <Input v-model.trim="draft.youdao.appSecret" type="password" placeholder="你的有道翻译应用密钥" />
        </div>
        <div class="ts-row">
          <span class="ts-label">
            领域
            <small>仅中英互译生效，需在控制台开通</small>
          </span>
          <Select v-model="draft.youdao.domain" :options="domainOptions" />
        </div>
        <div class="ts-row">
          <span class="ts-label">
            术语表 ID
            <small>可选，控制台术语表的 out_id</small>
          </span>
          <Input v-model.trim="draft.youdao.vocabId" placeholder="可选的术语表 ID" />
        </div>
        <div class="ts-row">
          <span class="ts-label">
            严格翻译
            <small>开启后严格按所选方向翻译，不自动中英互译</small>
          </span>
          <span class="ts-check">
            <Checkbox
              :model-value="draft.youdao.strict"
              @update:model-value="draft.youdao.strict = Boolean($event)"
            />
          </span>
        </div>
        <div class="ts-row">
          <span class="ts-label">
            拒绝回退
            <small>领域翻译失败时直接报错，不退回通用翻译</small>
          </span>
          <span class="ts-check">
            <Checkbox
              :model-value="draft.youdao.rejectFallback"
              @update:model-value="draft.youdao.rejectFallback = Boolean($event)"
            />
          </span>
        </div>
      </template>

      <template v-else>
        <div class="ts-row">
          <span class="ts-label">
            基础 URL
            <small>OpenAI 兼容地址，自动补 /chat/completions</small>
          </span>
          <Input v-model.trim="draft.deepseek.baseUrl" placeholder="https://api.deepseek.com" />
        </div>
        <div class="ts-row">
          <span class="ts-label">
            API 密钥
            <small>在 DeepSeek 控制台的 API Keys 页面获取</small>
          </span>
          <Input v-model.trim="draft.deepseek.apiKey" type="password" placeholder="你的 DeepSeek API key" />
        </div>
        <div class="ts-row">
          <span class="ts-label">
            模型
            <small>填写服务商提供的完整 model name</small>
          </span>
          <Input v-model.trim="draft.deepseek.model" placeholder="deepseek-v4-flash" />
        </div>
        <div class="ts-row">
          <span class="ts-label">
            温度
            <small>控制生成随机性，0 更稳定</small>
          </span>
          <Input v-model.number="draft.deepseek.temperature" type="number" min="0" max="2" step="0.1" />
        </div>
        <div class="ts-row">
          <span class="ts-label">
            思考模式
            <small>关闭时不传 thinking 字段，翻译场景建议关闭</small>
          </span>
          <Select v-model="draft.deepseek.thinking" :options="thinkingOptions" />
        </div>
        <div class="ts-row ts-row-top">
          <span class="ts-label">
            System Prompt
            <small>可用变量：{source_language}、{target_language}、{detected_source_language}、{text}</small>
          </span>
          <Textarea v-model="draft.deepseek.systemPrompt" class="ts-prompt" rows="6" spellcheck="false" />
        </div>
        <div class="ts-row ts-row-top">
          <span class="ts-label">
            User Prompt
            <small>待翻译文本通过 {text} 注入</small>
          </span>
          <Textarea v-model="draft.deepseek.userPrompt" class="ts-prompt" rows="4" spellcheck="false" />
        </div>
      </template>

      <p v-if="error" class="ts-error" role="alert">{{ error }}</p>

      <div class="ts-actions">
        <Button type="button" variant="outline" :disabled="testing || saving" @click="test">
          {{ testing ? "测试中…" : "测试连接" }}
        </Button>
        <span class="ts-spacer" />
        <span v-if="dirty" class="ts-dirty">有未保存的修改</span>
        <Button type="button" :disabled="saving || !dirty" @click="save">
          {{ saving ? "保存中…" : "保存翻译设置" }}
        </Button>
      </div>
    </div>
  </section>
</template>

<style scoped>
.ts-sec {
  flex: 0 0 auto;
  overflow: hidden;
  border: 1px solid var(--border);
  border-radius: var(--r-lg);
  background: var(--surface);
}
.ts-head {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 15px 18px;
}
.ts-icon {
  display: grid;
  width: 36px;
  height: 36px;
  flex: 0 0 36px;
  place-items: center;
  border-radius: 10px;
  background: color-mix(in srgb, var(--accent) 14%, transparent);
  color: var(--accent);
}
.ts-copy {
  min-width: 0;
  flex: 1;
}
.ts-copy strong {
  display: block;
  font-size: 13.5px;
  font-weight: 650;
  color: var(--text);
}
.ts-copy small {
  display: block;
  margin-top: 2px;
  font-size: 11.5px;
  line-height: 1.5;
  color: var(--text-subtle);
}
.ts-body {
  display: flex;
  flex-direction: column;
  padding: 0 18px 16px;
}
.ts-row {
  display: grid;
  grid-template-columns: 240px minmax(0, 1fr);
  align-items: center;
  gap: 16px;
  padding: 11px 0;
  border-top: 1px solid var(--border);
}
.ts-row-top {
  align-items: start;
}
.ts-label {
  display: flex;
  flex-direction: column;
  gap: 2px;
  font-size: 12.5px;
  color: var(--text);
}
.ts-label small {
  font-size: 11px;
  line-height: 1.5;
  color: var(--text-subtle);
}
.ts-inline {
  display: flex;
  align-items: center;
  gap: 8px;
}
.ts-inline :deep(.cn-input) {
  width: 140px;
}
.ts-unit {
  font-size: 12px;
  color: var(--text-subtle);
}
.ts-check {
  display: flex;
  align-items: center;
}
.ts-shortcut {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 6px;
}
.ts-shortcut-state {
  margin: 0;
  font-size: 11px;
  line-height: 1.5;
  color: var(--text-subtle);
}
.ts-shortcut-state.bad {
  color: var(--u-crit);
}
.ts-shortcut-state.warn {
  color: var(--u-warn);
}
.ts-prompt {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 12px;
  line-height: 1.65;
}
.ts-note {
  margin: 0;
  padding: 12px 0 4px;
  font-size: 12px;
  color: var(--text-subtle);
}
.ts-error {
  margin: 10px 0 0;
  border-radius: var(--r-md);
  background: color-mix(in srgb, var(--u-crit) 10%, transparent);
  padding: 8px 12px;
  font-size: 12px;
  line-height: 1.6;
  color: var(--u-crit);
}
.ts-actions {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 14px;
}
.ts-spacer {
  flex: 1;
}
.ts-dirty {
  font-size: 11.5px;
  color: var(--u-warn);
}
@media (max-width: 900px) {
  .ts-row {
    grid-template-columns: minmax(0, 1fr);
    gap: 8px;
  }
}
</style>
