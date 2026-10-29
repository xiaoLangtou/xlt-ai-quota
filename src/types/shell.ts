/** Shell 环境管理领域类型（Snapshot + 变更计划管线）。 */

/** 值语义：literal 可表格编辑；expression 含展开语法，仅源码视图编辑 */
export type ValueKind = "literal" | "expression";

/** `~/.alias` 中的一条别名 */
export interface AliasEntry {
  /** 稳定条目身份：`alias:<name>` */
  id: string;
  name: string;
  value: string;
  valueKind: ValueKind;
  /** 分组标题（上方最近的分组注释；被注释掉的 alias 不算分组） */
  group: string | null;
  /** 描述（alias 行尾 `#` 注释） */
  description: string | null;
  line: number;
  raw: string;
}

/** `~/.zshrc` 中的一条环境变量 */
export interface EnvEntry {
  /** 托管条目为 `env:<key>`；非托管为 `env:<key>@<line>` */
  id: string;
  key: string;
  value: string;
  valueKind: ValueKind;
  /** 是否经 `export` 导出（区别于普通赋值） */
  exported: boolean;
  /** 位于托管区块内，表格可编辑（且仅 literal 可编辑） */
  managed: boolean;
  line: number;
  raw: string;
}

export interface BlockInfo {
  beginLine: number;
  endLine: number;
  entryCount: number;
}

/** 单个配置文件的快照（UI 数据源） */
export interface FileSnapshot {
  kind: "alias" | "zshrc";
  path: string;
  exists: boolean;
  /** 内容 sha256，作为并发写入的版本基线 */
  version: string;
  /** 原始内容（源码视图数据源，不擅自规范化） */
  content: string;
  aliases: AliasEntry[];
  envs: EnvEntry[];
  managedBlock: BlockInfo | null;
  /** 区块结构问题；非空时托管写操作被后端拒绝 */
  blockIssues: string[];
  lineEnding: "lf" | "crlf";
}

export interface ShellSnapshot {
  alias: FileSnapshot;
  zshrc: FileSnapshot;
}

export interface PlanOp {
  /** 动作名（中文，用于差异确认） */
  action: string;
  detail: string;
}

export interface ShellChangePlan {
  id: string;
  /** 目标文件："alias" | "zshrc" */
  kind: "alias" | "zshrc";
  /** 计划基线版本；commit 时与磁盘当前版本比对 */
  baseVersion: string;
  ops: PlanOp[];
  /** 本次变更对应的会话命令（用户手动复制；不再默认自动复制） */
  sessionCommand: string;
  /** 预计算的整文件后像 */
  postimage: string;
  createdAt: string;
}

export interface ShellCommitResult {
  kind: "alias" | "zshrc";
  version: string;
  content: string;
  sessionCommand: string;
  ops: PlanOp[];
  /** 提交后重新解析出的文件快照，直接替换对应文件状态 */
  snapshot: FileSnapshot;
}

export interface ShellBackup {
  id: string;
  file: "alias" | "zshrc";
  createdAt: string;
  size: number;
}

export interface ShellValidateResult {
  ok: boolean;
  errors: string[];
}

/** 变更请求（所有写操作统一入口，含源码整写与备份恢复） */
export type ShellChangeRequest =
  | {
      type: "alias_add";
      name: string;
      value: string;
      group?: string | null;
      description?: string | null;
    }
  | { type: "alias_edit"; entryId: string; value: string; description?: string | null }
  | { type: "alias_rename"; entryId: string; newName: string }
  | { type: "alias_delete"; entryId: string }
  | { type: "env_set"; key: string; value: string; confirmBlockCreate?: boolean }
  | { type: "env_delete"; entryId: string }
  | { type: "source_write"; fileKind: "alias" | "zshrc"; content: string }
  | { type: "restore_backup"; backupId: string }
  | { type: "block_normalize" };

// ---------------------------------------------------------------------------
// R1 只读来源模型（shell_source_map / shell_path_report）
// ---------------------------------------------------------------------------

/** 纳入扫描的来源文件；登记 ≠ 读取（included 才读了内容） */
export interface SourceFileMeta {
  id: string;
  path: string;
  /** alias / zshrc / zshenv / zprofile / system-paths / system-paths-d / sourced */
  role: string;
  exists: boolean;
  size: number;
  included: boolean;
  /** registered / opt-in / sourced-ref */
  via: string;
  /** 经 source 引入时的引入位置（`file:line`） */
  sourcedFrom: string | null;
}

/** 跨文件环境定义归因 */
export interface EnvOrigin {
  fileId: string;
  path: string;
  line: number;
  key: string;
  value: string;
  valueKind: ValueKind;
  exported: boolean;
  /** 仅 ~/.zshrc 托管区块内为 true */
  managed: boolean;
  /** static（顶层）/ dynamic（条件块等） */
  context: string;
  /** 工具归属，未知为 unknown */
  owner: string;
  /** signature / marker / unknown */
  ownerEvidence: string;
  raw: string;
}

export interface SourceDiagnostic {
  code: string;
  message: string;
}

export interface ShellSourceMap {
  files: SourceFileMeta[];
  envs: EnvOrigin[];
  diagnostics: SourceDiagnostic[];
}

export interface PathSegment {
  text: string;
  /** literal / home-relative / variable-expression / dynamic / relative / empty / reference / system */
  kind: string;
  /** 可确定的解析路径（literal / home-relative） */
  resolved: string | null;
}

export interface PathContribution {
  fileId: string;
  path: string;
  line: number;
  owner: string;
  /** static / dynamic / system */
  context: string;
  /** prepend / append / replace */
  mode: string;
  raw: string;
  segments: PathSegment[];
}

export interface PathDiagnostic {
  code: string;
  /** info / warn */
  severity: string;
  message: string;
  sources: string[];
}

export interface MergedPathEntry {
  text: string;
  kind: string;
  resolved: string | null;
  /** static / system */
  certainty: string;
  /** 添加位置（`file:line`） */
  via: string;
}

export interface PathSummary {
  contributionCount: number;
  dynamicCount: number;
  mergedCount: number;
  duplicateGroups: number;
  missingCount: number;
  relativeCount: number;
  emptyCount: number;
}

export interface ShellPathReport {
  contributions: PathContribution[];
  merged: MergedPathEntry[];
  diagnostics: PathDiagnostic[];
  summary: PathSummary;
}

/** 手动复制整份刷新命令（顶栏按钮；保存后的本次变更命令单独展示） */
export const SHELL_RELOAD_COMMAND = "source ~/.alias && source ~/.zshrc";
