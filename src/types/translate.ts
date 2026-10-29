/** 内置翻译服务；新增引擎时同步扩展 Rust 侧的 `Engine` 与这里的选项。 */
export type TranslateEngine = "youdao" | "deepseek";

/** 有道智云文本翻译配置。 */
export interface YoudaoConfig {
  /** 接口地址，默认 https://openapi.youdao.com/api */
  baseUrl: string;
  /** 应用 ID（控制台 appKey） */
  appKey: string;
  /** 应用密钥，只参与签名，不上传 */
  appSecret: string;
  /** 领域化翻译：general / computers / medicine / finance / game */
  domain: string;
  /** 控制台术语表 ID */
  vocabId: string;
  /** 严格按 from/to 翻译；关闭时有道会自行中英互译 */
  strict: boolean;
  /** 领域翻译失败时不降级到通用翻译 */
  rejectFallback: boolean;
}

/** DeepSeek（OpenAI 兼容）翻译配置。 */
export interface DeepSeekConfig {
  baseUrl: string;
  apiKey: string;
  model: string;
  temperature: number;
  /** disabled 时不传 thinking 字段；enabled 时开启思考模式 */
  thinking: "disabled" | "enabled";
  /** 支持 {source_language} / {target_language} / {detected_source_language} / {text} */
  systemPrompt: string;
  userPrompt: string;
}

export interface TranslateConfig {
  engine: TranslateEngine;
  timeoutMs: number;
  /** 快捷面板全局快捷键 */
  shortcut: string;
  youdao: YoudaoConfig;
  deepseek: DeepSeekConfig;
}

/** 一段原文与它的译文；按空行分段，一段一次请求。 */
export type TranslateSegmentStatus = "queued" | "running" | "done" | "failed" | "cancelled";

export interface TranslateSegment {
  index: number;
  status: TranslateSegmentStatus;
  error: string | null;
  original: string;
  translated: string;
}

export interface TranslateOutcome {
  engine: string;
  cancelled: boolean;
  configVersion: string;
  segments: TranslateSegment[];
  /** 实际语言方向，如 EN2zh-CHS；引擎不返回时按请求方向拼装 */
  lang: string;
  /** 总耗时（毫秒） */
  ms: number;
  /** 本次已开始请求的段落数，不含限流重试次数。 */
  requests: number;
}

const DEFAULT_SYSTEM_PROMPT = [
  "You are a professional translator. Translate the input text to {target_language}.",
  "Rules:",
  "- Output ONLY the translation, no explanation, no quotes.",
  "- Preserve original formatting, line breaks, and punctuation.",
  "- For technical terms, keep the English original in parentheses.",
].join("\n");

export const DEFAULT_TRANSLATE_CONFIG: TranslateConfig = {
  engine: "youdao",
  timeoutMs: 30000,
  shortcut: "Alt+Space",
  youdao: {
    baseUrl: "https://openapi.youdao.com/api",
    appKey: "",
    appSecret: "",
    domain: "general",
    vocabId: "",
    strict: false,
    rejectFallback: false,
  },
  deepseek: {
    baseUrl: "https://api.deepseek.com",
    apiKey: "",
    model: "deepseek-v4-flash",
    temperature: 0,
    thinking: "disabled",
    systemPrompt: DEFAULT_SYSTEM_PROMPT,
    userPrompt: "Translate into {target_language}:\n\n{text}",
  },
};

/** 设置页「内置服务」下拉选项。 */
export const TRANSLATE_ENGINES: { label: string; value: TranslateEngine }[] = [
  { label: "有道翻译", value: "youdao" },
  { label: "DeepSeek", value: "deepseek" },
];

export const TRANSLATE_ENGINE_LABELS: Record<string, string> = {
  youdao: "有道翻译",
  deepseek: "DeepSeek",
};

/** 领域选项：有道仅在中英互译时生效。 */
export const TRANSLATE_DOMAINS = [
  { label: "通用文本翻译", value: "general" },
  { label: "计算机", value: "computers" },
  { label: "医学", value: "medicine" },
  { label: "金融经济", value: "finance" },
  { label: "游戏", value: "game" },
] as const;

/** 语言方向：有道语言码，auto 表示自动检测。 */
export const TRANSLATE_PAIRS = [
  { label: "自动检测 → 简体中文", from: "auto", to: "zh-CHS" },
  { label: "English → 简体中文", from: "en", to: "zh-CHS" },
  { label: "简体中文 → English", from: "zh-CHS", to: "en" },
  { label: "自动检测 → English", from: "auto", to: "en" },
] as const;

/** 快捷面板的源语言选项。 */
export const TRANSLATE_SOURCE_LANGUAGES: { label: string; value: string }[] = [
  { label: "自动检测", value: "auto" },
  { label: "English", value: "en" },
  { label: "简体中文", value: "zh-CHS" },
  { label: "繁體中文", value: "zh-CHT" },
  { label: "日本語", value: "ja" },
  { label: "한국어", value: "ko" },
];

/** 快捷面板的目标语言选项（不含自动检测）。 */
export const TRANSLATE_TARGET_LANGUAGES: { label: string; value: string }[] = [
  { label: "简体中文", value: "zh-CHS" },
  { label: "繁體中文", value: "zh-CHT" },
  { label: "English", value: "en" },
  { label: "日本語", value: "ja" },
  { label: "한국어", value: "ko" },
];

export function languageLabel(code: string): string {
  return (
    TRANSLATE_SOURCE_LANGUAGES.find((item) => item.value === code)?.label ?? code
  );
}

/** 面板快捷键在系统里的真实注册状态。 */
export interface TranslateShortcutStatus {
  shortcut: string;
  registered: boolean;
  /** 注册失败原因（被占用等），成功时为空串 */
  error: string;
}

/** 当前引擎是否已具备可用凭据。 */
export function translateConfigured(config: TranslateConfig): boolean {
  if (config.engine === "deepseek") return Boolean(config.deepseek.apiKey);
  return Boolean(config.youdao.appKey && config.youdao.appSecret);
}

/** 每个事件只归属一次翻译请求，段落按 index 排序。 */
export interface TranslateProgress {
  requestId: string;
  done: number;
  total: number;
  index: number;
  original: string;
  translated: string;
  status: TranslateSegmentStatus;
  configVersion: string;
  error: string | null;
}
