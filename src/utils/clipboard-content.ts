export type ClipboardSpecialKind = "url" | "email" | "color";

export interface ClipboardSpecialContent {
  kind: ClipboardSpecialKind;
  value: string;
}

const URL_PATTERN = /https?:\/\/[^\s<>{}\[\]"'，。；！？、]+/gi;
const EMAIL_PATTERN = /\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b/gi;
const COLOR_PATTERN = /(?:^|(?<=\s))(?:#[\da-f]{3,4}|#[\da-f]{6}|#[\da-f]{8})(?=$|\s|[,;.)])/gi;

// 检测只扫描内容前缀：对整段超长文本跑正则/JSON.parse 会拖慢选中预览，
// 而链接、邮箱、色值与语言特征基本都出现在开头，扫描前缀已足够。
const MAX_SCAN_LENGTH = 20_000;

function scanPrefix(content: string): string {
  return content.length > MAX_SCAN_LENGTH ? content.slice(0, MAX_SCAN_LENGTH) : content;
}

function stripTrailingPunctuation(value: string): string {
  return value.replace(/[.,;:!?)]*$/, "");
}

export function detectClipboardSpecialContent(content: string): ClipboardSpecialContent[] {
  const scan = scanPrefix(content);
  const detected: ClipboardSpecialContent[] = [];
  const seen = new Set<string>();
  const collect = (kind: ClipboardSpecialKind, values: string[]) => {
    for (const raw of values) {
      const value = kind === "url" ? stripTrailingPunctuation(raw) : raw;
      const key = `${kind}:${value.toLowerCase()}`;
      if (!value || seen.has(key)) continue;
      seen.add(key);
      detected.push({ kind, value });
      if (detected.length === 8) return;
    }
  };
  collect("url", scan.match(URL_PATTERN) ?? []);
  if (detected.length < 8) collect("email", scan.match(EMAIL_PATTERN) ?? []);
  if (detected.length < 8) collect("color", scan.match(COLOR_PATTERN) ?? []);
  return detected;
}

export function detectClipboardCodeLanguage(content: string): string | null {
  const value = scanPrefix(content).trim();
  if (!value) return null;
  if ((value.startsWith("{") || value.startsWith("[")) && (() => {
    try { JSON.parse(value); return true; }
    catch { return false; }
  })()) return "json";
  if (/<template[\s>]/.test(value) || /<script\s+setup/.test(value)) return "vue";
  if (/^(?:use\s+[\w:]+|pub\s+(?:fn|struct|enum)|fn\s+\w+\s*\()/m.test(value)) return "rust";
  if (/^(?:from\s+[\w.]+\s+import|import\s+[\w.]+|def\s+\w+\s*\(|class\s+\w+\s*[:(])/m.test(value)) return "python";
  if (/^(?:SELECT|INSERT|UPDATE|DELETE|CREATE\s+TABLE|ALTER\s+TABLE)\b/im.test(value)) return "sql";
  if (/^(?:#!\/.*\b(?:bash|sh)|(?:pnpm|npm|yarn|git|cargo|docker)\s+\S+)/m.test(value)) return "bash";
  if (/^(?:interface|type)\s+\w+|\b(?:const|let)\s+\w+\s*(?::[^=]+)?=|=>/m.test(value)) return /:\s*[A-Z_a-z][\w<>, |\[\]]*\s*[=;)]/.test(value) ? "typescript" : "javascript";
  if (/^#{1,6}\s+\S+/m.test(value) || /^```\w*/m.test(value)) return "markdown";
  return null;
}
