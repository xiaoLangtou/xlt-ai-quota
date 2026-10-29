import type {
  CalendarEvent,
  CalendarViewMode,
  CommitInput,
  DecompositionDraft,
  TodoCategory,
  TodoDraft,
  TodoPriority,
  TodoTask,
} from "@/types/productivity";

/** 日历与待办的纯计算工具：不依赖 Vue / Tauri，便于单元测试。 */

const WEEKDAYS = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"];

function pad(value: number): string {
  return String(value).padStart(2, "0");
}

/** 本地日期 → YYYY-MM-DD（不经过 UTC，避免时区偏移）。 */
export function toDateString(date: Date): string {
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

/** 今天（本地）的 YYYY-MM-DD。 */
export function today(now: Date = new Date()): string {
  return toDateString(now);
}

/** YYYY-MM-DD → 本地 Date（取正午，规避夏令时边界）。 */
export function parseDate(value: string): Date {
  const [year, month, day] = value.split("-").map(Number);
  return new Date(year, (month ?? 1) - 1, day ?? 1, 12, 0, 0, 0);
}

export function addDays(value: string, amount: number): string {
  const date = parseDate(value);
  date.setDate(date.getDate() + amount);
  return toDateString(date);
}

export function addMonths(value: string, amount: number): string {
  const date = parseDate(value);
  const day = date.getDate();
  date.setDate(1);
  date.setMonth(date.getMonth() + amount);
  const lastDay = new Date(date.getFullYear(), date.getMonth() + 1, 0).getDate();
  date.setDate(Math.min(day, lastDay));
  return toDateString(date);
}

/** 所在周的周一。 */
export function startOfWeek(value: string): string {
  const date = parseDate(value);
  const weekday = (date.getDay() + 6) % 7;
  date.setDate(date.getDate() - weekday);
  return toDateString(date);
}

export function startOfMonth(value: string): string {
  const date = parseDate(value);
  date.setDate(1);
  return toDateString(date);
}

/** 月视图固定六周（42 天）网格，从所在周周一开始。 */
export function monthGridDates(anchor: string): string[] {
  const start = startOfWeek(startOfMonth(anchor));
  return Array.from({ length: 42 }, (_, index) => addDays(start, index));
}

export function weekDates(anchor: string): string[] {
  const start = startOfWeek(anchor);
  return Array.from({ length: 7 }, (_, index) => addDays(start, index));
}

/** 三日窗口：以锚点为中心，前天 / 今天 / 后天。 */
export function threeDayDates(anchor: string): string[] {
  return [-1, 0, 1].map((offset) => addDays(anchor, offset));
}

/** 指定视图需要读取的事件日期范围（含端点）。 */
export function viewRange(view: CalendarViewMode, anchor: string): { start: string; end: string } {
  if (view === "day") return { start: anchor, end: anchor };
  if (view === "week") {
    const dates = weekDates(anchor);
    return { start: dates[0], end: dates[dates.length - 1] };
  }
  const dates = monthGridDates(anchor);
  return { start: dates[0], end: dates[dates.length - 1] };
}

/** 按视图前后移动：日 ±1 天，周 ±7 天，月 ±1 月。 */
export function shiftAnchor(view: CalendarViewMode, anchor: string, direction: number): string {
  if (view === "day") return addDays(anchor, direction);
  if (view === "week") return addDays(anchor, direction * 7);
  return addMonths(anchor, direction);
}

export function monthTitle(anchor: string): string {
  const date = parseDate(anchor);
  return `${date.getFullYear()} 年 ${date.getMonth() + 1} 月`;
}

export function weekTitle(anchor: string): string {
  const dates = weekDates(anchor);
  const start = parseDate(dates[0]);
  const end = parseDate(dates[6]);
  if (start.getMonth() === end.getMonth()) {
    return `${start.getFullYear()} 年 ${start.getMonth() + 1} 月 ${start.getDate()} – ${end.getDate()} 日`;
  }
  return `${start.getMonth() + 1} 月 ${start.getDate()} 日 – ${end.getMonth() + 1} 月 ${end.getDate()} 日`;
}

export function dayTitle(anchor: string): string {
  return formatDateLabel(anchor);
}

export function formatDateLabel(value: string): string {
  const date = parseDate(value);
  return `${date.getMonth() + 1} 月 ${date.getDate()} 日 · ${WEEKDAYS[date.getDay()]}`;
}

export function weekdayLabel(value: string): string {
  return WEEKDAYS[parseDate(value).getDay()];
}

export function dayOfMonth(value: string): number {
  return parseDate(value).getDate();
}

export function isWeekend(value: string): boolean {
  const day = parseDate(value).getDay();
  return day === 0 || day === 6;
}

/** 事件开始日期（startAt 兼容 YYYY-MM-DD 与 YYYY-MM-DD HH:mm）。 */
export function eventStartDate(event: { startAt: string }): string {
  return event.startAt.slice(0, 10);
}

export function eventStartTime(event: { startAt: string; allDay: boolean }): string | null {
  if (event.allDay) return null;
  const time = event.startAt.slice(11, 16);
  return time.length === 5 ? time : null;
}

export function timeToMinutes(time: string): number {
  const [hour, minute] = time.split(":").map(Number);
  return (hour || 0) * 60 + (minute || 0);
}

export function minutesToTime(minutes: number): string {
  const clamped = Math.max(0, Math.min(24 * 60 - 1, Math.round(minutes)));
  return `${pad(Math.floor(clamped / 60))}:${pad(clamped % 60)}`;
}

export function combineDateTime(date: string, time: string): string {
  return `${date} ${time}`;
}

export function snapMinutes(minutes: number, step = 15): number {
  return Math.round(minutes / step) * step;
}

export function eventDurationMinutes(
  event: { startAt: string; endAt: string | null; allDay: boolean },
  fallback = 30,
): number {
  if (event.allDay || !event.endAt) return fallback;
  const start = timeToMinutes(event.startAt.slice(11, 16));
  const end = timeToMinutes(event.endAt.slice(11, 16));
  const sameDay = event.endAt.slice(0, 10) === event.startAt.slice(0, 10);
  if (!sameDay) return fallback;
  const diff = end - start;
  return diff > 0 ? diff : fallback;
}

/** 24 小时内的时间轴小时刻度。 */
export const TIMELINE_HOURS = Array.from({ length: 24 }, (_, hour) => hour);

export const HOUR_HEIGHT = 48;

export type DayPeriod = "morning" | "afternoon" | "unscheduled";

export const DAY_PERIOD_LABELS: Record<DayPeriod, string> = {
  morning: "上午",
  afternoon: "下午",
  unscheduled: "未定时间",
};

/** 今日待办分组：有时间的按上午 / 下午，无时间的归入未定时间。 */
export function groupTasksByPeriod(tasks: TodoTask[]): Record<DayPeriod, TodoTask[]> {
  const groups: Record<DayPeriod, TodoTask[]> = { morning: [], afternoon: [], unscheduled: [] };
  for (const task of tasks) {
    if (!task.plannedTime) {
      groups.unscheduled.push(task);
      continue;
    }
    (timeToMinutes(task.plannedTime) < 12 * 60 ? groups.morning : groups.afternoon).push(task);
  }
  return groups;
}

export interface CompletionProgress {
  done: number;
  total: number;
  percent: number;
}

export function completionProgress(tasks: Pick<TodoTask, "status">[]): CompletionProgress {
  const total = tasks.length;
  const done = tasks.filter((task) => task.status === "done").length;
  return { done, total, percent: total === 0 ? 0 : Math.round((done / total) * 100) };
}

export function formatEstimate(minutes: number | null): string {
  if (!minutes) return "未估";
  if (minutes >= 480 && minutes % 480 === 0) return `${minutes / 480}d`;
  if (minutes >= 60) {
    const hours = minutes / 60;
    return `${Number.isInteger(hours) ? hours : hours.toFixed(1)}h`;
  }
  return `${minutes}m`;
}

/** 优先级分组顺序：高 → 中 → 低。 */
export const PRIORITY_ORDER: Record<TodoPriority, number> = { high: 0, medium: 1, low: 2 };

export type ExternalStatusTone = "neutral" | "info" | "success" | "warning" | "error" | "primary";

/** 禅道原始状态 → 展示文案与语义色。列表和看板共用，避免两处各写一份映射。 */
export const EXTERNAL_STATUS_CHIPS: Record<string, { label: string; color: ExternalStatusTone }> = {
  active: { label: "激活", color: "error" },
  resolved: { label: "已解决", color: "success" },
  closed: { label: "已关闭", color: "neutral" },
  wait: { label: "未开始", color: "neutral" },
  doing: { label: "进行中", color: "info" },
  pause: { label: "已暂停", color: "warning" },
  done: { label: "已完成", color: "success" },
  cancel: { label: "已取消", color: "neutral" },
};

export function externalStatusChip(status?: string | null): { label: string; color: ExternalStatusTone } | null {
  return status ? EXTERNAL_STATUS_CHIPS[status] ?? { label: status, color: "neutral" } : null;
}

/** 事件颜色使用分类标识，前端不存样式值。 */
export function categoryClass(value: string): string {
  if (value === "personal") return "cat-personal";
  if (value === "planning") return "cat-planning";
  return "cat-work";
}

export const MAX_DOCUMENT_CHARS = 60_000;

export function isTooLongDocument(text: string): boolean {
  return text.length > MAX_DOCUMENT_CHARS;
}

/** 右键快速创建 / 双击时间轴创建的数据映射。 */
export function buildQuickTaskDraft(input: {
  title: string;
  time: string;
  category: TodoCategory;
  date: string;
}): TodoDraft {
  return {
    title: input.title.trim(),
    priority: "medium",
    category: input.category,
    plannedDate: input.date,
    plannedTime: input.time || null,
    sourceType: "manual",
  };
}

/** 拆解结果中选中的草稿。 */
export function selectedDrafts(drafts: DecompositionDraft[]): DecompositionDraft[] {
  return drafts.filter((draft) => draft.selected);
}

/**
 * 把可编辑草稿映射为批量写入请求：依赖只保留本次选中的任务，避免引用未创建的任务。
 */
export function buildCommitInput(sourceName: string, drafts: DecompositionDraft[]): CommitInput {
  const chosen = selectedDrafts(drafts);
  const ids = new Set(chosen.map((draft) => draft.id));
  return {
    sourceName,
    tasks: chosen.map((draft) => ({
      tempId: draft.id,
      title: draft.title.trim(),
      description: draft.description.trim() || null,
      priority: draft.priority,
      category: draft.category,
      plannedDate: draft.plannedDate || null,
      plannedTime: draft.plannedTime || null,
      estimateMinutes: draft.estimateMinutes || null,
      acceptanceCriteria: draft.acceptanceCriteria.map((value) => value.trim()).filter(Boolean),
      dependencyTempIds: draft.dependencyIds.filter((id) => ids.has(id)),
    })),
  };
}

// —— 参考 Nuxt UI calendar 模板的日历交互补充 ——

export interface QuickEventParse {
  title: string;
  /** 未识别到日期时为 null，调用方回退到锚点日期。 */
  date: string | null;
  /** 未识别到时间时为 null，调用方回退到默认时间。 */
  time: string | null;
  allDay: boolean;
}

const WEEKDAY_CN: Record<string, number> = { 一: 1, 二: 2, 三: 3, 四: 4, 五: 5, 六: 6, 日: 7, 天: 7 };
const WEEKDAY_EN: Record<string, number> = {
  monday: 1, tuesday: 2, wednesday: 3, thursday: 4, friday: 5, saturday: 6, sunday: 7,
  mon: 1, tue: 2, tues: 2, wed: 3, thu: 4, thur: 4, thurs: 4, fri: 5, sat: 6, sun: 7,
};

function nextWeekday(from: string, target: number, forceNextWeek: boolean): string {
  if (forceNextWeek) {
    const nextMonday = addDays(startOfWeek(from), 7);
    return addDays(nextMonday, target - 1);
  }
  const weekday = ((parseDate(from).getDay() + 6) % 7) + 1;
  const delta = ((target - weekday + 7) % 7) || 7;
  return addDays(from, delta);
}

function resolveMonthDay(month: number, day: number, base: string): string | null {
  if (month < 1 || month > 12 || day < 1 || day > 31) return null;
  const baseDate = parseDate(base);
  let candidate = `${baseDate.getFullYear()}-${pad(month)}-${pad(day)}`;
  const parsed = parseDate(candidate);
  if (parsed.getMonth() + 1 !== month || parsed.getDate() !== day) return null;
  if (candidate < base) candidate = `${baseDate.getFullYear() + 1}-${pad(month)}-${pad(day)}`;
  return candidate;
}

function extractTime(text: string): { time: string | null; matched: boolean } {
  const afternoon = /(晚上|夜里|今晚|明晚|下午|傍晚)/.test(text);
  const noon = /(中午|正午)/.test(text);
  let match = text.match(/(\d{1,2})[:：](\d{2})/);
  if (match) {
    return { time: `${pad(Number(match[1]) % 24)}:${match[2]}`, matched: true };
  }
  match = text.match(/(\d{1,2})\s*[点時时](?:\s*(半|\d{1,2})\s*分?)?/);
  if (match) {
    let hour = Number(match[1]);
    const minutes = match[2] === "半" ? 30 : match[2] ? Number(match[2]) : 0;
    if (afternoon && hour < 12) hour += 12;
    else if (noon && hour < 12) hour += 12;
    return { time: `${pad(hour % 24)}:${pad(Math.min(minutes, 59))}`, matched: true };
  }
  match = text.match(/(\d{1,2})\s*(am|pm|a\.m\.|p\.m\.)/i);
  if (match) {
    let hour = Number(match[1]) % 12;
    if (/^p/i.test(match[2])) hour += 12;
    return { time: `${pad(hour)}:00`, matched: true };
  }
  if (noon) return { time: "12:00", matched: true };
  return { time: null, matched: false };
}

function extractDate(text: string, base: string): { date: string | null; matched: boolean } {
  let match = text.match(/大后天/);
  if (match) return { date: addDays(base, 3), matched: true };
  match = text.match(/后天/);
  if (match) return { date: addDays(base, 2), matched: true };
  match = text.match(/(明天|tomorrow)/i);
  if (match) return { date: addDays(base, 1), matched: true };
  match = text.match(/(今天|今日|today)/i);
  if (match) return { date: base, matched: true };
  match = text.match(/前天/);
  if (match) return { date: addDays(base, -2), matched: true };
  match = text.match(/(昨天|yesterday)/i);
  if (match) return { date: addDays(base, -1), matched: true };
  match = text.match(/(下+)(?:个)?(?:周|星期|礼拜)([一二三四五六日天])/);
  if (match) return { date: nextWeekday(base, WEEKDAY_CN[match[2]], true), matched: true };
  match = text.match(/(?:周|星期|礼拜)([一二三四五六日天])/);
  if (match) return { date: nextWeekday(base, WEEKDAY_CN[match[1]], false), matched: true };
  match = text.match(/\b(monday|tuesday|wednesday|thursday|friday|saturday|sunday|mon|tues|tue|wed|thurs|thur|thu|fri|sat|sun)\b/i);
  if (match) return { date: nextWeekday(base, WEEKDAY_EN[match[1].toLowerCase()], false), matched: true };
  match = text.match(/(\d{1,2})\s*月\s*(\d{1,2})\s*[日号]?/);
  if (match) {
    const resolved = resolveMonthDay(Number(match[1]), Number(match[2]), base);
    if (resolved) return { date: resolved, matched: true };
  }
  match = text.match(/(\d{1,2})[/-](\d{1,2})(?!\d)/);
  if (match) {
    const resolved = resolveMonthDay(Number(match[1]), Number(match[2]), base);
    if (resolved) return { date: resolved, matched: true };
  }
  return { date: null, matched: false };
}

const QUICK_CONNECTOR = "at|on|in|from|for|until|till|to|by|@|the|this|next|and";

function stripConnectors(text: string, aggressive: boolean): string {
  let result = text.replace(/[，,、；;]/g, " ").replace(/\s+/g, " ").trim();
  if (aggressive) {
    result = result.replace(new RegExp(`^(?:${QUICK_CONNECTOR})\\s+`, "i"), "");
  }
  let previous = "";
  while (result !== previous) {
    previous = result;
    result = result.replace(new RegExp(`\\s+(?:${QUICK_CONNECTOR})$`, "i"), "").trim();
  }
  return result.replace(/^[\s\-–—]+|[\s\-–—]+$/g, "").trim();
}

/**
 * 解析“明天下午3点评审”“Movie at 7pm on Friday”这类短语。
 * 纯本地正则实现，不下载模型、不联网，结果可直接单元测试。
 */
export function parseQuickEvent(input: string, now: Date = new Date()): QuickEventParse | null {
  const phrase = input.trim();
  if (!phrase) return null;
  const base = toDateString(now);
  const { time, matched: hasTime } = extractTime(phrase);
  const { date, matched: hasDate } = extractDate(phrase, base);
  let title = phrase
    .replace(/(\d{1,2})[:：](\d{2})/g, " ")
    .replace(/(\d{1,2})\s*[点時时](?:\s*(半|\d{1,2})\s*分?)?/g, " ")
    .replace(/(\d{1,2})\s*(am|pm|a\.m\.|p\.m\.)/gi, " ")
    .replace(/大后天|后天|明天|今天|今日|前天|昨天|tomorrow|today|yesterday/gi, " ")
    .replace(/(下+)(?:个)?(?:周|星期|礼拜)([一二三四五六日天])/g, " ")
    .replace(/(?:周|星期|礼拜)([一二三四五六日天])/g, " ")
    .replace(/\b(monday|tuesday|wednesday|thursday|friday|saturday|sunday|mon|tues|tue|wed|thurs|thur|thu|fri|sat|sun)\b/gi, " ")
    .replace(/(\d{1,2})\s*月\s*(\d{1,2})\s*[日号]?/g, " ")
    .replace(/(\d{1,2})[/-](\d{1,2})(?!\d)/g, " ")
    .replace(/上午|下午|晚上|夜里|今晚|明晚|傍晚|凌晨|早上|早晨|中午|正午/g, " ");
  title = stripConnectors(title, hasTime || hasDate);
  return { title: title || "新建待办", date, time, allDay: hasDate && !hasTime };
}

export function splitStartAt(value: string): { date: string; time: string | null } {
  const date = value.slice(0, 10);
  const time = value.slice(11, 16);
  return { date, time: time.length === 5 ? time : null };
}

export function addMinutesToTime(time: string, minutes: number): string {
  return minutesToTime(timeToMinutes(time) + minutes);
}

export function durationMinutesBetween(startAt: string, endAt: string | null): number | null {
  if (!endAt) return null;
  const start = timeToMinutes(splitStartAt(startAt).time ?? "00:00");
  const end = timeToMinutes(splitStartAt(endAt).time ?? "00:00");
  const diff = end - start;
  return diff > 0 ? diff : null;
}

export const CATEGORY_DOT_CLASS: Record<TodoCategory, string> = {
  work: "dot-work",
  planning: "dot-planning",
  personal: "dot-personal",
};

/** Tailwind 语义色：分类圆点与文字。 */
export const CATEGORY_DOT_BG: Record<TodoCategory, string> = {
  work: "bg-success",
  planning: "bg-warning",
  personal: "bg-violet-500",
};

export const CATEGORY_TEXT: Record<TodoCategory, string> = {
  work: "text-success",
  planning: "text-warning",
  personal: "text-violet-500",
};

export interface PositionedDayEvent {
  event: CalendarEvent;
  /** 相对当天时间轴顶部的像素偏移。 */
  top: number;
  height: number;
  /** 重叠列左侧百分比（0–100，相对当天列宽）。 */
  left: number;
  /** 重叠列宽度百分比（相对当天列宽）。 */
  width: number;
}

/**
 * 把同一天相互重叠的定时事件按“簇 + 最少列数”排布，重叠事件并排显示而不是叠在一起。
 * 参考 Nuxt UI calendar 模板的 layoutDay。
 */
export function layoutDayEvents(events: CalendarEvent[], date: string): PositionedDayEvent[] {
  const items = events
    .filter((event) => !event.allDay && eventStartDate(event) === date)
    .map((event) => {
      const startMin = timeToMinutes(eventStartTime(event) ?? "00:00");
      const duration = eventDurationMinutes(event, 30);
      return { event, startMin, endMin: startMin + duration };
    })
    .filter((item) => item.endMin > item.startMin)
    // 过短的事件至少画 30 分钟，避免与相邻事件在视觉上叠住。
    .map((item) => ({ ...item, endMin: Math.max(item.endMin, item.startMin + 30) }))
    .sort((a, b) => a.startMin - b.startMin || b.endMin - a.endMin);

  const positioned: PositionedDayEvent[] = [];
  let cluster: typeof items = [];
  let clusterEnd = -Infinity;

  const flush = () => {
    const columns: number[] = [];
    const assigned = cluster.map((item) => {
      let index = columns.findIndex((end) => end <= item.startMin);
      if (index === -1) index = columns.length;
      columns[index] = item.endMin;
      return index;
    });
    const count = columns.length || 1;
    cluster.forEach((item, index) => {
      positioned.push({
        event: item.event,
        top: (item.startMin / 60) * HOUR_HEIGHT,
        height: ((item.endMin - item.startMin) / 60) * HOUR_HEIGHT,
        left: (assigned[index] / count) * 100,
        width: 100 / count,
      });
    });
  };

  for (const item of items) {
    if (cluster.length && item.startMin >= clusterEnd) {
      flush();
      cluster = [];
      clusterEnd = -Infinity;
    }
    cluster.push(item);
    clusterEnd = Math.max(clusterEnd, item.endMin);
  }
  if (cluster.length) flush();
  return positioned;
}

