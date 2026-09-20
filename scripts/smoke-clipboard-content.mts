import assert from "node:assert/strict";
import { detectClipboardCodeLanguage, detectClipboardSpecialContent } from "../src/utils/clipboard-content";

const detected = detectClipboardSpecialContent(
  "文档 https://example.com/path?q=1，联系 dev@example.com，颜色 #16A085 和 #fff",
);

assert.deepEqual(detected, [
  { kind: "url", value: "https://example.com/path?q=1" },
  { kind: "email", value: "dev@example.com" },
  { kind: "color", value: "#16A085" },
  { kind: "color", value: "#fff" },
]);

assert.deepEqual(
  detectClipboardSpecialContent("https://example.com. https://example.com"),
  [{ kind: "url", value: "https://example.com" }],
);

assert.equal(detectClipboardCodeLanguage('{"name":"xlt-workbench","version":"0.1.3"}'), "json");
assert.equal(detectClipboardCodeLanguage("const total: number = rows.length"), "typescript");
assert.equal(detectClipboardCodeLanguage("普通的剪贴板文本"), null);

console.log("clipboard content detection smoke passed");
