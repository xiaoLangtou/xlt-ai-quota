import assert from "node:assert/strict";
import { aiModelOptions } from "../src/config/ai-providers";
import { zentaoCopyText, nextPendingTaskId, attentionReasons, matchesWorkView, sortWorkTasks } from "../src/utils/todo-workspace";
import {
  buildCommitInput,
  buildQuickTaskDraft,
  completionProgress,
  externalStatusChip,
  groupTasksByPeriod,
  isTooLongDocument,
  layoutDayEvents,
  monthGridDates,
  parseQuickEvent,
  shiftAnchor,
  threeDayDates,
  viewRange,
  weekDates,
  weekdayLabel,
} from "../src/utils/productivity";
import type { CalendarEvent, DecompositionDraft, TodoTask } from "../src/types/productivity";

function task(overrides: Partial<TodoTask>): TodoTask {
  return {
    id: overrides.id ?? "t",
    title: "任务",
    description: null,
    status: "open",
    priority: "medium",
    category: "work",
    plannedDate: "2026-09-28",
    plannedTime: null,
    estimateMinutes: null,
    sourceType: "manual",
    sourceName: null,
    acceptanceCriteria: [],
    dependencyIds: [],
    createdAt: 0,
    updatedAt: 0,
    completedAt: null,
    ...overrides,
  };
}

// —— 日历范围计算与视图导航 ——
assert.deepEqual(viewRange("day", "2026-09-28"), { start: "2026-09-28", end: "2026-09-28" });
assert.deepEqual(viewRange("week", "2026-09-28"), { start: "2026-09-28", end: "2026-10-04" });
assert.equal(monthGridDates("2026-09-28").length, 42);
assert.equal(monthGridDates("2026-09-28")[0], "2026-08-31");
assert.equal(weekdayLabel(monthGridDates("2026-09-28")[0]), "周一");
assert.deepEqual(weekDates("2026-09-28")[0], "2026-09-28");
assert.deepEqual(threeDayDates("2026-09-28"), ["2026-09-27", "2026-09-28", "2026-09-29"]);
assert.equal(shiftAnchor("day", "2026-09-28", 1), "2026-09-29");
assert.equal(shiftAnchor("week", "2026-09-28", 1), "2026-10-05");
assert.equal(shiftAnchor("month", "2026-01-31", 1), "2026-02-28");
assert.deepEqual(viewRange("month", "2026-09-28"), { start: "2026-08-31", end: "2026-10-11" });

// —— 重叠事件按列排布（不叠加） ——
function calEvent(id: string, startAt: string, endAt: string): CalendarEvent {
  return {
    id,
    taskId: null,
    title: id,
    category: "work",
    startAt,
    endAt,
    allDay: false,
    createdAt: 0,
    updatedAt: 0,
    status: null,
  };
}
const laid = layoutDayEvents(
  [
    calEvent("a", "2026-09-28 09:00", "2026-09-28 10:00"),
    calEvent("b", "2026-09-28 09:30", "2026-09-28 10:30"),
    calEvent("c", "2026-09-28 11:00", "2026-09-28 12:00"),
  ],
  "2026-09-28",
);
const byId = (id: string) => laid.find((item) => item.event.id === id)!;
assert.equal(laid.length, 3);
assert.equal(byId("a").width, 50);
assert.equal(byId("b").width, 50);
assert.equal(byId("a").left, 0);
assert.equal(byId("b").left, 50);
assert.equal(byId("a").top, 9 * 48);
assert.equal(byId("c").width, 100, "不重叠的事件占满整列");

// —— 今日待办分组与完成进度 ——
const grouped = groupTasksByPeriod([
  task({ id: "a", plannedTime: "09:30" }),
  task({ id: "b", plannedTime: "14:00" }),
  task({ id: "c", plannedTime: null }),
  task({ id: "d", plannedTime: "12:00" }),
]);
assert.deepEqual(grouped.morning.map((item) => item.id), ["a"]);
assert.deepEqual(grouped.afternoon.map((item) => item.id), ["b", "d"]);
assert.deepEqual(grouped.unscheduled.map((item) => item.id), ["c"]);
assert.deepEqual(completionProgress([task({ status: "done" }), task({ status: "open" }), task({ status: "done" })]), {
  done: 2,
  total: 3,
  percent: 67,
});
assert.deepEqual(completionProgress([]), { done: 0, total: 0, percent: 0 });
assert.deepEqual(
  completionProgress([task({ status: "done" }), task({ status: "doing" }), task({ status: "open" })]),
  { done: 1, total: 3, percent: 33 },
  "进行中不计入已完成",
);

// —— 右键快速创建的数据映射 ——
const quick = buildQuickTaskDraft({ title: "  评审拆解结果  ", time: "11:00", category: "planning", date: "2026-09-30" });
assert.equal(quick.title, "评审拆解结果");
assert.equal(quick.priority, "medium");
assert.equal(quick.category, "planning");
assert.equal(quick.plannedDate, "2026-09-30");
assert.equal(quick.plannedTime, "11:00");
assert.equal(quick.sourceType, "manual");
assert.equal(buildQuickTaskDraft({ title: "x", time: "", category: "work", date: "2026-09-30" }).plannedTime, null);

// —— 文档字符限制 ——
assert.equal(isTooLongDocument("文".repeat(60_000)), false);
assert.equal(isTooLongDocument("文".repeat(60_001)), true);

// —— 拆解结果选择与确认请求映射 ——
const drafts: DecompositionDraft[] = [
  {
    id: "t1",
    groupId: "g1",
    groupTitle: "日历",
    title: " 定义数据模型 ",
    description: "  覆盖全天事件 ",
    priority: "high",
    category: "work",
    plannedDate: "2026-09-29",
    plannedTime: "09:00",
    estimateMinutes: 60,
    acceptanceCriteria: [" 可观察 ", ""],
    dependencyIds: [],
    selected: true,
  },
  {
    id: "t2",
    groupId: "g1",
    groupTitle: "日历",
    title: "实现月视图",
    description: "",
    priority: "medium",
    category: "planning",
    plannedDate: "",
    plannedTime: "",
    estimateMinutes: 0,
    acceptanceCriteria: [],
    dependencyIds: ["t1", "t3"],
    selected: true,
  },
  {
    id: "t3",
    groupId: "g1",
    groupTitle: "日历",
    title: "未选中的任务",
    description: "",
    priority: "low",
    category: "work",
    plannedDate: "",
    plannedTime: "",
    estimateMinutes: 0,
    acceptanceCriteria: [],
    dependencyIds: [],
    selected: false,
  },
];
const commit = buildCommitInput("粘贴内容", drafts);
assert.equal(commit.tasks.length, 2);
assert.equal(commit.tasks[0].title, "定义数据模型");
assert.equal(commit.tasks[0].description, "覆盖全天事件");
assert.deepEqual(commit.tasks[0].acceptanceCriteria, ["可观察"]);
assert.equal(commit.tasks[1].plannedDate, null);
assert.equal(commit.tasks[1].estimateMinutes, null);
// 依赖只保留本次选中的任务，未选中的 t3 被剔除。
assert.deepEqual(commit.tasks[1].dependencyTempIds, ["t1"]);

// —— Git 报告 AI 配置读取 ——
let rawPreferences = JSON.stringify({
  projects: [],
  authors: [],
  ai: { provider: "deepseek", model: "deepseek-chat" },
  aiApiKeys: { deepseek: "sk-test" },
});
Object.defineProperty(globalThis, "localStorage", {
  configurable: true,
  value: {
    getItem: () => rawPreferences,
    setItem: (_key: string, value: string) => { rawPreferences = value; },
  },
});
const { readAiConfig, isSupportedDocument } = await import("../src/services/document-decomposition-service");
const config = readAiConfig();
assert.deepEqual(config, { provider: "deepseek", model: "deepseek-chat", apiKey: "sk-test" });
// 自定义模型跨保存读取后仍按原始 ID 调用，不混入其他服务商。
const { dailyReportService } = await import("../src/services/daily-report-service");
const preferences = dailyReportService.loadPreferences();
preferences.ai = { provider: "deepseek", model: "custom-deepseek-model" };
preferences.aiCustomModels = { deepseek: ["custom-deepseek-model"], kimi: ["custom-kimi-model"] };
preferences.aiSelectedModels = { deepseek: "custom-deepseek-model", kimi: "custom-kimi-model" };
dailyReportService.savePreferences(preferences);
const restored = dailyReportService.loadPreferences();
assert.deepEqual(restored.aiCustomModels, preferences.aiCustomModels);
assert.deepEqual(restored.aiSelectedModels, preferences.aiSelectedModels);
assert.equal(readAiConfig()?.model, "custom-deepseek-model");
assert.equal(readAiConfig()?.provider, "deepseek");
const customOptions = aiModelOptions("deepseek", [" custom-deepseek-model ", "custom-deepseek-model", "deepseek-chat", ""], "custom-deepseek-model");
assert.equal(customOptions.filter(option => option.value === "custom-deepseek-model").length, 1);
assert.equal(customOptions.filter(option => option.value === "deepseek-chat").length, 1);
assert.ok(customOptions.some(option => option.label === "custom-deepseek-model（自定义）"));
assert.ok(!aiModelOptions("kimi", restored.aiCustomModels?.kimi).some(option => option.value === "custom-deepseek-model"));
assert.ok(aiModelOptions("deepseek", [], "previously-saved-custom").some(option => option.value === "previously-saved-custom"));
rawPreferences = JSON.stringify({ projects: [], authors: [], ai: { provider: "deepseek", model: "deepseek-chat" } });
assert.equal(readAiConfig(), null, "缺少 API Key 时必须返回 null");
rawPreferences = JSON.stringify({ projects: [], authors: [], ai: { provider: "kimi" }, aiApiKeys: { kimi: "sk" } });
assert.equal(readAiConfig()?.model, "moonshot-v1-8k", "模型缺失时回退到服务商默认模型");
assert.equal(isSupportedDocument("需求.DOCX"), true);
assert.equal(isSupportedDocument("会议纪要.pdf"), true);
assert.equal(isSupportedDocument("archive.zip"), false);
Reflect.deleteProperty(globalThis, "localStorage");

// —— 快捷创建的自然语言解析（参考 Nuxt UI calendar 模板） ——
const NOW = new Date(2026, 8, 28, 10, 0, 0); // 2026-09-28 周一
assert.deepEqual(parseQuickEvent("明天下午3点 评审拆解结果", NOW), {
  title: "评审拆解结果",
  date: "2026-09-29",
  time: "15:00",
  allDay: false,
});
assert.deepEqual(parseQuickEvent("Movie at 7pm on Friday", NOW), {
  title: "Movie",
  date: "2026-10-02",
  time: "19:00",
  allDay: false,
});
assert.deepEqual(parseQuickEvent("周五 09:30 站会", NOW), {
  title: "站会",
  date: "2026-10-02",
  time: "09:30",
  allDay: false,
});
assert.deepEqual(parseQuickEvent("周一台风", NOW), {
  title: "台风",
  date: "2026-10-05",
  time: null,
  allDay: true,
});
assert.deepEqual(parseQuickEvent("10月3日 提交周报", NOW), {
  title: "提交周报",
  date: "2026-10-03",
  time: null,
  allDay: true,
});
assert.deepEqual(parseQuickEvent("下午3点", NOW), {
  title: "新建待办",
  date: null,
  time: "15:00",
  allDay: false,
});
assert.equal(parseQuickEvent("   ", NOW), null);

// 禅道原始状态在列表/看板使用统一文案与语义色。
assert.deepEqual(externalStatusChip("resolved"), { label: "已解决", color: "success" });
assert.deepEqual(externalStatusChip("active"), { label: "激活", color: "error" });
assert.equal(externalStatusChip(null), null);

console.log("日历 / 待办 / 文档拆解纯逻辑检查通过");

// 禅道工作视图：转交条目仍可追踪，今日计划只包含还需自己处理的条目。
const workDate = "2026-10-27";
const pendingBug = task({ id: "bug", sourceType: "zentao", externalType: "bug", externalStatus: "active", plannedDate: workDate });
const transferredBug = task({ ...pendingBug, id: "transferred", externalTransferred: true });
const resolvedBug = task({ ...pendingBug, id: "resolved", status: "done", externalStatus: "resolved", completedAt: 200 });
const cancelledTask = task({ id: "cancelled", status: "done", externalStatus: "cancel", plannedDate: workDate });
assert.equal(matchesWorkView(pendingBug, "pending", workDate), true);
assert.equal(matchesWorkView(transferredBug, "pending", workDate), false);
assert.equal(matchesWorkView(transferredBug, "transferred", workDate), true);
assert.equal(matchesWorkView(transferredBug, "all", workDate), true);
assert.equal(matchesWorkView(transferredBug, "today", workDate), false);
assert.equal(matchesWorkView(resolvedBug, "today", workDate), false);
assert.equal(matchesWorkView(pendingBug, "today", "2026-10-28"), false);
assert.equal(matchesWorkView(resolvedBug, "processed", workDate), true);
assert.equal(matchesWorkView(cancelledTask, "pending", workDate), false);
// 有计划的条目优先；同日高优先级优先；已处理按完成时间倒序。
const unscheduled = task({ id: "unscheduled", plannedDate: null, priority: "high" });
const urgent = task({ ...pendingBug, id: "urgent", priority: "high" });
const workItems = [unscheduled, pendingBug, urgent];
assert.deepEqual(sortWorkTasks(workItems, "pending").map(item => item.id), ["urgent", "bug", "unscheduled"]);
assert.deepEqual(workItems.map(item => item.id), ["unscheduled", "bug", "urgent"]);
assert.deepEqual(sortWorkTasks([resolvedBug, task({ ...resolvedBug, id: "newer", completedAt: 300 })], "processed").map(item => item.id), ["newer", "resolved"]);
console.log("[productivity] 禅道工作视图与排序通过");

// 明确日期下的建议排序与截止日口径。
const deadlineBug = task({ ...pendingBug, id: "deadline", priority: "medium", externalDue: "2026-10-25", plannedDate: "2026-10-29" });
const personalLate = task({ ...pendingBug, id: "personal-late", priority: "low", plannedDate: "2026-10-26" });
const todayLow = task({ ...pendingBug, id: "today-low", priority: "low" });
const futureHigh = task({ ...pendingBug, id: "future-high", plannedDate: "2026-10-30", priority: "high" });
assert.deepEqual(attentionReasons(deadlineBug, workDate), ["逾期 2 天"]);
assert.deepEqual(attentionReasons(personalLate, workDate), ["计划已过期 1 天"]);
assert.deepEqual(sortWorkTasks([futureHigh, todayLow, personalLate, deadlineBug], "pending", workDate).map(item => item.id), ["deadline", "personal-late", "today-low", "future-high"]);
assert.deepEqual(attentionReasons({ ...deadlineBug, externalTransferred: true }, workDate), []);
const { summaryImages, summarySources, summaryFingerprint, validSummary } = await import("../src/utils/zentao-summary");
const summaryDetail = { status: "active", assignedTo: "me", externalType: "bug", externalId: 42, description: "复现步骤", files: [], images: ["secret-image"], actions: [] };
const sources = summarySources(pendingBug, summaryDetail);
assert.equal(JSON.stringify(sources).includes("secret-image"), true);
assert.deepEqual(summaryImages(summaryDetail), ["secret-image"]);
assert.notEqual(await summaryFingerprint(sources), await summaryFingerprint(summarySources(pendingBug, { ...summaryDetail, images: ["changed-image"] })));
const summary = { overview: [{ text: "问题待处理", sources: ["description"] }], purpose: [], frontend: [], backend: [] };
assert.equal(validSummary(summary, sources), true);
assert.equal(validSummary({ ...summary, frontend: [{ text: "新增按钮", sources: ["description"] }] }, sources), true);
assert.equal(validSummary({ ...summary, frontend: Array.from({ length: 4 }, () => ({ text: "改动", sources: ["description"] })) }, sources), false);
assert.equal(validSummary({ overview: summary.overview, progress: [], changes: [], questions: [], nextSteps: [] }, sources), false);
assert.equal(validSummary({ ...summary, overview: [{ text: "编造引用", sources: ["unknown"] }] }, sources), false);
assert.equal(validSummary({ ...summary, overview: [] }, sources), false);
assert.notEqual(await summaryFingerprint(sources), await summaryFingerprint(summarySources(pendingBug, { ...summaryDetail, status: "resolved" })));
console.log("[productivity] 处理建议口径、摘要引用和内容版本通过");

assert.equal(zentaoCopyText({ ...pendingBug, title: "【#42】复现问题", externalId: "zentao:bug:42" }), "【42】复现问题：");
assert.equal(nextPendingTaskId(["bug", "resolved", "transferred", "urgent"], "bug", [resolvedBug, transferredBug, urgent]), "urgent");
assert.equal(nextPendingTaskId(["bug", "resolved"], "bug", [resolvedBug]), null);
assert.equal(nextPendingTaskId(["bug"], "missing", [pendingBug]), null);
