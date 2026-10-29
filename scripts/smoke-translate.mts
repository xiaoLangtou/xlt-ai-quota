/** 翻译交接和 IPC 契约检查；不启动服务，不发真实翻译请求。 */
import assert from "node:assert/strict";
import { useTranslateRequest } from "../src/composables/useTranslateRequest";
import { translateService } from "../src/services/translate-service";

const panel = useTranslateRequest();
const page = useTranslateRequest();
panel.request({ text: "同样的文字", from: "ja", to: "zh-CHS" });
assert.deepEqual(page.take(), { text: "同样的文字", from: "ja", to: "zh-CHS" });
assert.equal(page.take(), null);
panel.request({ text: "同样的文字", from: "zh-CHS", to: "en" });
assert.equal(page.pendingRequest.value?.to, "en");
assert.equal(page.take()?.from, "zh-CHS");

const calls: { command: string; args: unknown }[] = [];
const previousWindow = globalThis.window;
Object.assign(globalThis, { window: {
  __TAURI_INTERNALS__: {
    invoke: async (command: string, args: unknown) => {
      calls.push({ command, args });
      return { segments: [], lang: "ja2en", ms: 1, requests: 0 };
    },
  },
} });
try {
  await translateService.run("original", "ja", "en", "page-request");
  await translateService.run("panel text", "auto", "zh-CHS");
  assert.deepEqual(calls[0], { command: "translate_run", args: { text: "original", from: "ja", to: "en", requestId: "page-request" } });
  await translateService.cancel("page-request");
  await translateService.retry("failed paragraph", 2, "ja", "en", "retry-request", "version");
  assert.deepEqual(calls[2], { command: "translate_cancel", args: { requestId: "page-request" } });
  assert.deepEqual(calls[3], { command: "translate_retry", args: { text: "failed paragraph", index: 2, from: "ja", to: "en", requestId: "retry-request", expectedConfig: "version" } });
  const panelRequest = calls[1].args as { requestId: string };
  assert.match(panelRequest.requestId, /^[a-f\d-]{36}$/);
  assert.notEqual(panelRequest.requestId, "page-request");
} finally { Object.assign(globalThis, { window: previousWindow }); }
console.log("[translate] 重复文本交接、语言方向和请求隔离标识通过");

const { completedTranslation, mergeTranslationSegment, historyEntry, pruneTranslationHistory, HISTORY_MAX_BYTES } = await import("../src/utils/translate-workspace");
const success = { index: 0, original: "first", translated: "第一段", status: "done" as const, error: null };
const failed = { index: 2, original: "third", translated: "", status: "failed" as const, error: "upstream failed" };
const queued = { index: 1, original: "second", translated: "", status: "queued" as const, error: null };
let rows = mergeTranslationSegment([failed, success], queued);
assert.deepEqual(rows.map(row => row.index), [0, 1, 2]);
assert.equal(completedTranslation(rows), "第一段");
rows = mergeTranslationSegment(rows, { ...failed, translated: "第三段", status: "done", error: null });
assert.equal(rows[0].translated, "第一段");
assert.equal(completedTranslation(rows, true), "first\n第一段\n\nthird\n第三段");
const now = Date.now();
const input = { id: "history", createdAt: now, source: "original", from: "en", to: "zh-CHS", engine: "deepseek", segments: rows, apiKey: "synthetic-secret" };
const entry = historyEntry(input, false);
assert.equal(entry.status, "partial");
assert.equal(JSON.stringify(entry).includes("synthetic-secret"), false);
assert.equal(historyEntry({ ...input, segments: [failed] }, false).status, "failed");
assert.equal(historyEntry({ ...input, segments: [success] }, false).status, "done");
assert.equal(historyEntry(input, true).status, "cancelled");
assert.equal(pruneTranslationHistory([{ ...entry, createdAt: now - 31 * 86400000 }, entry], 30, now).length, 1);
assert.equal(pruneTranslationHistory([entry, entry], 30, now).length, 1);
assert.equal(pruneTranslationHistory(Array.from({ length: 70 }, (_, id) => ({ ...entry, id: String(id) })), 30, now).length, 50);
assert.equal(pruneTranslationHistory([{ ...entry, source: "长".repeat(20000) }], 30, now).length, 0);
const large = pruneTranslationHistory(Array.from({ length: 50 }, (_, id) => ({ ...entry, id: String(id), source: "a".repeat(30000) })), 30, now);
assert.ok(new TextEncoder().encode(JSON.stringify(large)).length <= HISTORY_MAX_BYTES);
assert.ok(large.length < 50);
console.log("[translate] 段落顺序、独立重试合并、历史隐私及容量/期限限制通过");
