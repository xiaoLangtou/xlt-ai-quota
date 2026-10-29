/** 日历、待办与文档拆解领域类型，对应 Rust 侧 productivity::types。 */

export type TodoStatus = "open" | "doing" | "done";
export type TodoPriority = "high" | "medium" | "low";
export type TodoCategory = "work" | "planning" | "personal";
export type TodoSource = "manual" | "document" | "zentao";
export type CalendarViewMode = "day" | "week" | "month";

export const TODO_STATUSES: TodoStatus[] = ["open", "doing", "done"];
export const TODO_PRIORITIES: TodoPriority[] = ["high", "medium", "low"];
export const TODO_CATEGORIES: TodoCategory[] = ["work", "planning", "personal"];
export const TODO_SOURCES: TodoSource[] = ["manual", "document"];

export const CATEGORY_LABELS: Record<TodoCategory, string> = {
  work: "产品迭代",
  planning: "需求拆解",
  personal: "个人",
};

export const PRIORITY_LABELS: Record<TodoPriority, string> = {
  high: "高",
  medium: "中",
  low: "低",
};

export const STATUS_LABELS: Record<TodoStatus, string> = {
  open: "未开始",
  doing: "进行中",
  done: "已完成",
};

export const SOURCE_LABELS: Record<TodoSource, string> = {
  manual: "手动创建",
  document: "文档拆解",
  zentao: "禅道同步",
};

export const EXTERNAL_TYPE_LABELS: Record<string, string> = {
  task: "任务",
  bug: "Bug",
  story: "需求",
};

export const VIEW_LABELS: Record<CalendarViewMode, string> = {
  day: "日",
  week: "周",
  month: "月",
};

export interface AcceptanceCriterion {
  id: string;
  content: string;
  sortOrder: number;
}

export interface TodoTask {
  id: string;
  title: string;
  description: string | null;
  status: TodoStatus;
  priority: TodoPriority;
  category: TodoCategory;
  plannedDate: string | null;
  plannedTime: string | null;
  estimateMinutes: number | null;
  sourceType: TodoSource;
  sourceName: string | null;
  /** 外部来源幂等键（如 zentao:task:123），手动/文档来源为空。 */
  externalId: string | null;
  externalType: string | null;
  externalUrl: string | null;
  syncedAt: number | null;
  externalStatus?: string | null;
  externalDue?: string | null;
  externalUpdatedDate?: string | null;
  externalAssignedTo?: string | null;
  externalTransferred?: boolean;
  acceptanceCriteria: AcceptanceCriterion[];
  dependencyIds: string[];
  createdAt: number;
  updatedAt: number;
  completedAt: number | null;
}

export interface TodoDraft {
  id?: string | null;
  title: string;
  description?: string | null;
  status?: TodoStatus;
  priority: TodoPriority;
  category: TodoCategory;
  plannedDate?: string | null;
  plannedTime?: string | null;
  estimateMinutes?: number | null;
  sourceType?: TodoSource;
  sourceName?: string | null;
  acceptanceCriteria?: string[];
  dependencyIds?: string[];
}

export interface TodoFilter {
  status?: TodoStatus;
  priority?: TodoPriority;
  category?: TodoCategory;
  sourceType?: TodoSource;
  start?: string;
  end?: string;
  query?: string;
  scheduledOnly?: boolean;
}

export interface CalendarEvent {
  id: string;
  taskId: string | null;
  title: string;
  category: TodoCategory;
  startAt: string;
  endAt: string | null;
  allDay: boolean;
  createdAt: number;
  updatedAt: number;
  status: TodoStatus | null;
}

export interface CalendarEventDraft {
  id?: string | null;
  taskId?: string | null;
  title: string;
  category: TodoCategory;
  startAt: string;
  endAt?: string | null;
  allDay?: boolean;
}

export interface MoveEventInput {
  id: string;
  startAt?: string;
  endAt?: string;
}

export interface DocumentContent {
  name: string;
  content: string;
  charCount: number;
}

export interface DecompositionRequest {
  sourceName: string;
  content: string;
  provider: "minimax" | "kimi" | "deepseek";
  model: string;
  apiKey: string;
  currentDate: string;
}

export interface DecompositionTask {
  id: string;
  title: string;
  description: string;
  priority: TodoPriority;
  estimatedMinutes: number | null;
  suggestedDate: string | null;
  suggestedTime: string | null;
  dependencyIds: string[];
  acceptanceCriteria: string[];
}

export interface DecompositionGroup {
  id: string;
  title: string;
  tasks: DecompositionTask[];
}

export interface DecompositionResult {
  summary: string;
  assumptions: string[];
  groups: DecompositionGroup[];
}

export interface CommitTask {
  tempId: string;
  title: string;
  description?: string | null;
  priority: TodoPriority;
  category?: TodoCategory;
  plannedDate?: string | null;
  plannedTime?: string | null;
  estimateMinutes?: number | null;
  acceptanceCriteria?: string[];
  dependencyTempIds?: string[];
}

export interface CommitInput {
  sourceName: string;
  tasks: CommitTask[];
}

export interface CommitResult {
  createdCount: number;
  taskIds: string[];
}

/** 拆解结果的可编辑草稿（前端确认阶段使用）。 */export interface DecompositionDraft {
  id: string;
  groupId: string;
  groupTitle: string;
  title: string;
  description: string;
  priority: TodoPriority;
  category: TodoCategory;
  /** 空字符串表示未安排。 */
  plannedDate: string;
  plannedTime: string;
  /** 0 表示未估。 */
  estimateMinutes: number;
  acceptanceCriteria: string[];
  dependencyIds: string[];
  selected: boolean;
}

/** 禅道同步：本机登录态（~/.zentao-token.json）状态。 */
export interface ZentaoStatus {
  configured: boolean;
  url: string;
  account: string;
}

/** 同步明细：单条条目的写入结果。action ∈ created | updated | unchanged | closed。 */
export interface ZentaoSyncItem {
  zentaoId: number;
  externalType: string;
  title: string;
  priority: string;
  status: string;
  action: string;
  externalUrl: string;
  due: string | null;
  estimateMinutes: number | null;
}

/** 禅道同步摘要：两源条数与本地写入统计。 */
export interface ZentaoSyncResult {
  taskCount: number;
  bugCount: number;
  created: number;
  updated: number;
  closed: number;
  skipped: string;
  elapsedMs: number;
  requestCount?: number;
  historyDeferred?: number;
  syncedAt: number;
  items: ZentaoSyncItem[];
}

/** 禅道附件元数据。origin：task = 任务本体，story = 关联需求（关联材料）。 */
export interface ZentaoFile {
  id: number;
  title: string;
  extension: string;
  size: number;
  origin: string;
}

/** 历史记录里的一条字段变更。rich=true 表示富文本字段（如 steps）：
 *  用 newText/newImages 折叠展示变更后的内容，不直接抛原文 HTML。 */
export interface ZentaoActionChange {
  field: string;
  old: string;
  new: string;
  rich: boolean;
  /** 富文本变更后的可读文本（含【截图n】占位符）。 */
  newText: string;
  /** newText 中占位符对应的截图（data URL，按序）。 */
  newImages: string[];
}

/** 一条处理历史。desc 为禅道预格式化的中文描述行；images 为该次操作引用的截图。 */
export interface ZentaoAction {
  date: string;
  actor: string;
  desc: string;
  comment: string;
  changes: ZentaoActionChange[];
  images: string[];
}

/** 附件预览内容：kind ∈ text | image | binary。 */
export interface ZentaoFileContent {
  kind: string;
  /** text = UTF-8 文本；image = data URL。 */
  content: string;
}

/** zip 附件内的文件条目。path 为包内相对路径（'/' 分隔）；index 为 zip 内条目序号，按索引读取。 */
export interface ZentaoArchiveEntry {
  index: number;
  path: string;
  name: string;
  size: number;
  extension: string;
}

/** 禅道条目详情：纯文本描述 + 附件列表 + 内嵌截图 + 处理历史。 */
export interface ZentaoDetail {
  status: string;
  assignedTo: string;
  externalType: string;
  externalId: number;
  description: string;
  files: ZentaoFile[];
  /** 描述中的内嵌截图（data URL），按出现顺序。 */
  images: string[];
  /** 处理历史，旧到新。 */
  actions: ZentaoAction[];
}

export type ZentaoOperation = "assign" | "resolve" | "start" | "finish";
export interface ZentaoActionInput {
  taskId: string;
  operation: ZentaoOperation;
  assignedTo: string;
  resolution: string;
  duplicateBug: number | null;
  resolvedBuild: string;
  actionDate: string;
  realStarted: string;
  consumed: number;
  left: number;
  comment: string;
}
export interface ZentaoActionContext {
  status: string;
  openedBy: string;
  assignedTo: string;
  account: string;
  realStarted: string;
  consumed: number;
  left: number;
  users: { value: string; label: string }[];
  builds: { value: string; label: string }[];
}
export interface ZentaoActionResult {
  task: TodoTask | null;
  warning: string;
}

/** 批量解决 Bug：逐条提交到禅道并回读核实，单条失败不影响其他条目。 */
export interface ZentaoBatchResolveInput {
  taskIds: string[];
  resolution: string;
  resolvedBuild: string;
  actionDate: string;
  comment: string;
}

export interface ZentaoBatchItemResult {
  taskId: string;
  title: string;
  ok: boolean;
  error: string;
  task: TodoTask | null;
}

export interface ZentaoBatchResolveResult {
  items: ZentaoBatchItemResult[];
  succeeded: number;
  failed: number;
}
