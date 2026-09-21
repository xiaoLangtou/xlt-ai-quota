/**
 * Skills 领域类型与命令 DTO。迁移自 xlt-local-skills 的 `@xlt-skills/shared`，
 * 前后端以此保持一致契约（Rust 侧对应 `src-tauri/src/skills`）。
 */

/** skill 归属范围：用户级（全局）或项目级 */
export type SkillScope = "global" | "project";

/** 一个 skill 根目录（如 ~/.qoder/skills） */
export interface SkillRoot {
  /** 稳定 id，基于路径生成 */
  id: string;
  /** 展示名 */
  label: string;
  /** 绝对路径 */
  path: string;
  /** 是否可写（可安装 / 编辑 / 删除） */
  writable: boolean;
  /** 是否为内置默认根目录 */
  isDefault: boolean;
  /** 目录是否存在 */
  exists: boolean;
  /** 识别出的所属 Agent（如 Qoder / Claude） */
  agent?: string;
  /** 目录下的 skill 数量 */
  skillCount?: number;
  /** 归属范围：全局（用户级）或项目级 */
  scope: SkillScope;
  /** 项目级 root 所属的项目 id */
  projectId?: string;
  /** 项目级 root 所属项目的展示名 */
  projectLabel?: string;
}

/** 一个已登记的项目（项目级 skill 的来源目录） */
export interface SkillProject {
  /** 稳定 id，基于项目绝对路径生成 */
  id: string;
  /** 展示名（默认取目录名） */
  label: string;
  /** 项目根目录绝对路径 */
  path: string;
  /** 目录是否存在 */
  exists: boolean;
  /** 项目内探测到的 skill 根目录数量 */
  rootCount: number;
  /** 项目内 skill 总数 */
  skillCount: number;
}

/** skill 内的文件树节点 */
export interface SkillFileNode {
  /** 相对 skill 目录的 POSIX 路径 */
  path: string;
  name: string;
  type: "file" | "dir";
  sizeBytes?: number;
  /** 是否为可在线编辑的文本文件 */
  editable?: boolean;
  /** 是否为软链接 */
  isSymlink?: boolean;
  children?: SkillFileNode[];
}

/** 校验问题等级 */
export type ValidationLevel = "error" | "warning" | "info";

/** 结构化校验问题 */
export interface ValidationIssue {
  level: ValidationLevel;
  /** 机器可读的问题编码 */
  code: string;
  message: string;
  /** 关联文件（相对 skill 目录） */
  file?: string;
  /** 修复建议 */
  fix?: string;
}

/** 扫描 / 解析失败项 */
export interface ScanFailure {
  rootId: string;
  rootLabel: string;
  dirName: string;
  reason: string;
}

/** 回收站条目 */
export interface TrashItem {
  id: string;
  name: string;
  rootId: string;
  rootLabel: string;
  originalPath: string;
  deletedAt: string;
  sizeBytes: number;
  wasSymlink: boolean;
  /** 软链接的目标（仅软链接有值） */
  symlinkTarget?: string;
}

/** 重命名请求体 */
export interface RenameSkillRequest {
  newName: string;
}

/** skill 概览（列表用） */
export interface SkillSummary {
  /** rootId::name 复合 id */
  id: string;
  name: string;
  version?: string;
  description?: string;
  rootId: string;
  rootLabel: string;
  /** 所属 Agent（如 Qoder / Claude，继承自根目录） */
  agent?: string;
  dirPath: string;
  /** SKILL.md 内容指纹（用于同名去重：同名 + 同 hash 视为同一技能） */
  contentHash: string;
  /** 归属范围：全局（用户级）或项目级 */
  scope: SkillScope;
  /** 项目级 skill 所属项目的展示名 */
  projectLabel?: string;
  isSymlink: boolean;
  symlinkTarget?: string;
  sizeBytes: number;
  mtime: string;
  /** frontmatter/结构校验结果 */
  valid: boolean;
  warnings: string[];
  /** 结构化校验问题 */
  issues: ValidationIssue[];
}

/** skill 详情 */
export interface SkillDetail extends SkillSummary {
  /** SKILL.md 原始 frontmatter 对象 */
  frontmatter: Record<string, unknown>;
  /** frontmatter 以外的原始 YAML 文本（用于保留未知字段的编辑） */
  frontmatterYaml: string;
  /** SKILL.md 正文（frontmatter 之后的 markdown） */
  body: string;
  /** 子文件树 */
  files: SkillFileNode[];
}

export type ConflictStrategy = "overwrite" | "skip" | "keep-both";

/** 保存 skill 的请求体 */
export interface SaveSkillRequest {
  /** frontmatter 结构化字段 */
  name: string;
  version?: string;
  description?: string;
  /** 其余字段的原始 YAML（不含 name/version/description） */
  extraYaml?: string;
  /** SKILL.md 正文 */
  body: string;
  /** 需要一并写回的子文件（相对路径 -> 文本内容） */
  files?: Record<string, string>;
}

/** 新建 skill 请求体 */
export interface CreateSkillRequest {
  name: string;
  description: string;
  version?: string;
  rootId: string;
}

/** 本地安装请求体（非上传场景，走本地路径） */
export interface LocalInstallRequest {
  /** 本机 skill 目录的绝对路径 */
  sourcePath: string;
  targetRootId: string;
  onConflict: ConflictStrategy;
}

/** 项目下已知 Agent 的候选 skill 目录（含尚未创建的，供安装目标选择） */
export interface ProjectAgentDir {
  /** 基于绝对路径生成的稳定 root id（与已登记根目录一致） */
  rootId: string;
  /** Agent 名；普通 skills 目录时为 undefined */
  agent?: string;
  path: string;
  exists: boolean;
  writable: boolean;
  skillCount: number;
}

/** 服务端目录扫描：递归发现含 agent skill 目录的项目 */
export interface DirScanAgent {
  agent: string;
  skills: string[];
}

export interface DirScanProject {
  /** 项目绝对路径 */
  path: string;
  label: string;
  /** 是否已登记为项目 */
  registered: boolean;
  agents: DirScanAgent[];
  total: number;
}

export interface DirScanResult {
  /** 扫描的根目录 */
  root: string;
  /** 发现的 skill 目录总数（含未归属到 agent 的） */
  skillDirCount: number;
  projects: DirScanProject[];
}

export type RemoteSource = "github" | "skills-sh" | "market" | "http";

/** 远程安装请求体 */
export interface RemoteInstallRequest {
  source: RemoteSource;
  /** github: owner/repo[/subpath] 或完整 URL；skills-sh: owner/repo/skill；market/http: URL */
  ref: string;
  targetRootId: string;
  onConflict: ConflictStrategy;
}

/** 安装前预览 */
export interface InstallPreview {
  name: string;
  version?: string;
  description?: string;
  files: SkillFileNode[];
  /** 已存在同名 skill */
  conflict: boolean;
  warnings: string[];
  /** 临时暂存 id，确认后据此落盘 */
  stagingId: string;
  /** skill 总大小（字节） */
  sizeBytes?: number;
}

/** 安装确认（落盘）请求 */
export interface InstallCommitRequest {
  stagingId: string;
  targetRootId: string;
  /** 多目标安装：提供时优先生效，一次落盘到多个根目录 */
  targetRootIds?: string[];
  onConflict: ConflictStrategy;
}

/** 安装结果 */
export interface InstallResult {
  installed: boolean;
  finalName: string;
  rootId: string;
  dirPath: string;
}

export interface ApiError {
  error: string;
  message: string;
  details?: unknown;
}

/** 目录条目的来源类型：内置精选 / 数据源裸目录扫描 */
export type CatalogSourceType = "curated" | "raw-scan";

/** 精选目录中的一个热门开源 skill */
export interface CatalogSkill {
  id: string;
  name: string;
  description: string;
  /** GitHub 仓库 owner/repo */
  repo: string;
  installRef: string;
  homepage: string;
  tags: string[];
  stars?: number;
  sourceType?: CatalogSourceType;
  sourceId?: string;
  pathInRepo?: string;
  hasScripts?: boolean;
  version?: string;
  updatedAt?: string;
}

/** 已接入的 skill 数据源 */
export interface SkillSource {
  id: string;
  repo: string;
  branch?: string;
  label: string;
  builtin: boolean;
  kind?: "repo" | "search";
  description?: string;
  skillCount: number;
  lastSyncAt?: string;
  lastSyncError?: string;
}

/** 新增数据源请求 */
export interface AddSourceRequest {
  repo: string;
  branch?: string;
  label?: string;
}

/** skills.sh 发现式搜索的一条结果 */
export interface SkillsShSearchItem {
  id: string;
  name: string;
  source: string;
  installs?: number;
  detailUrl: string;
  repoUrl: string;
}

/** 远程技能详情（安装前查看，不落 staging） */
export interface RemoteSkillDetail {
  name: string;
  description?: string;
  version?: string;
  license?: string;
  body: string;
  files?: { path: string; sizeBytes?: number }[];
}

// ============ 安装流水线（Source → Staging → Inspect → Plan → Commit） ============

export type SourceType = "local" | "archive" | "github" | "skills-sh" | "http" | "market";

export interface SourceMetadata {
  type: SourceType;
  ref: string;
  repository?: string;
  revision?: string;
  branch?: string;
  subpath?: string;
}

export type StagingStatus = "preparing" | "ready" | "committing" | "completed" | "failed";

export interface StagingRecord {
  id: string;
  path: string;
  source: SourceMetadata;
  createdAt: string;
  expiresAt: string;
  status: StagingStatus;
  searchSubpath?: string;
}

export interface InspectedSkill {
  id: string;
  relativePath: string;
  name: string;
  version?: string;
  description?: string;
  contentHash: string;
  sizeBytes: number;
  fileCount: number;
  hasScripts: boolean;
  hasBinaryFiles: boolean;
  issues: ValidationIssue[];
  installable: boolean;
}

export interface PrepareSourceResponse {
  stagingId: string;
  expiresAt: string;
  source: SourceMetadata;
  skills: InspectedSkill[];
}

export interface CreateInstallPlanRequest {
  stagingId: string;
  skillIds: string[];
  targetRootIds: string[];
  onConflict: ConflictStrategy;
}

export interface PlannedSkill {
  skillId: string;
  name: string;
  version?: string;
  contentHash: string;
  sizeBytes: number;
  targets: PlannedSkillTarget[];
}

export interface PlannedSkillTarget {
  rootId: string;
  finalName: string;
  conflict: boolean;
  existingVersion?: string;
  existingHash?: string;
  action: "install" | "overwrite" | "skip" | "keep-both";
}

export interface PlannedTarget {
  rootId: string;
  label: string;
  path: string;
  writable: boolean;
  needsCreate: boolean;
}

export interface InstallPlan {
  id: string;
  stagingId: string;
  createdAt: string;
  expiresAt: string;
  source: SourceMetadata;
  skills: PlannedSkill[];
  targets: PlannedTarget[];
  issues: ValidationIssue[];
  confirmationHash: string;
}

export interface CommitPlanRequest {
  confirmationHash: string;
  acknowledgedIssueCodes: string[];
}

export type InstallErrorStage = "preflight" | "copy" | "validate" | "switch" | "record";

export interface TargetInstallResult {
  rootId: string;
  status: "installed" | "skipped" | "failed";
  finalName?: string;
  dirPath?: string;
  backupId?: string;
  error?: {
    code: string;
    message: string;
    stage: InstallErrorStage;
  };
}

export interface SkillInstallOutcome {
  skillId: string;
  name: string;
  targets: TargetInstallResult[];
}

export interface CommitPlanResponse {
  planId: string;
  outcomes: SkillInstallOutcome[];
}

export interface InstallRecord {
  id: string;
  rootId: string;
  name: string;
  dirPath: string;
  source: SourceMetadata;
  installedHash: string;
  currentHash: string;
  installedAt: string;
  updatedAt?: string;
  backupIds: string[];
}

export type UpdateStatus =
  | "up-to-date"
  | "remote-changed"
  | "local-changed"
  | "both-changed"
  | "unknown";

export interface InstallUpdateInfo {
  recordId: string;
  status: UpdateStatus;
  remoteRevision?: string;
  message?: string;
}

export interface FileDiffEntry {
  path: string;
  change: "added" | "modified" | "removed";
}

export interface UpdateDiffPreview {
  recordId: string;
  stagingId: string;
  expiresAt: string;
  skillId: string;
  newHash: string;
  entries: FileDiffEntry[];
}

export interface ApplyUpdateRequest {
  stagingId: string;
  skillId: string;
}
