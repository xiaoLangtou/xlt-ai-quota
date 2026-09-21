import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  AddSourceRequest,
  ApplyUpdateRequest,
  CatalogSkill,
  CommitPlanRequest,
  CommitPlanResponse,
  CreateInstallPlanRequest,
  CreateSkillRequest,
  DirScanResult,
  InstallPlan,
  InstallRecord,
  InstallUpdateInfo,
  PrepareSourceResponse,
  ProjectAgentDir,
  RemoteSkillDetail,
  RemoteSource,
  SaveSkillRequest,
  ScanFailure,
  SkillDetail,
  SkillProject,
  SkillRoot,
  SkillSource,
  SkillSummary,
  SkillsShSearchItem,
  TargetInstallResult,
  TrashItem,
  UpdateDiffPreview,
} from "@/types/skills";

/** 通用「已移除」响应。 */
export interface Removed {
  removed: boolean;
}

/** 把 invoke 抛出的错误（字符串或 SkillsError 对象）归一为可读消息。 */
export function skillsErrorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  if (error && typeof error === "object" && "message" in error) {
    return String((error as { message: unknown }).message);
  }
  return String(error);
}

/** 调用 Skills 命令并把错误统一转为 Error（便于上层 `(e as Error).message` 沿用）。 */
async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw new Error(skillsErrorMessage(error));
  }
}

/**
 * Skills 命令客户端。桌面端通过 Tauri invoke；浏览器模式无此能力（入口隐藏）。
 */
export const skillsApi = {
  // ---------- roots ----------
  listRoots: () => call<SkillRoot[]>("skills_list_roots"),
  addRoot: (path: string) => call<SkillRoot>("skills_add_root", { path }),
  removeRoot: (id: string) => call<Removed>("skills_remove_root", { id }),
  updateRootLabel: (id: string, label: string) =>
    call<SkillRoot>("skills_update_root_label", { id, label }),
  initRoot: (id: string) => call<SkillRoot>("skills_init_root", { id }),

  // ---------- projects ----------
  listProjects: () => call<SkillProject[]>("skills_list_projects"),
  addProject: (path: string, label?: string) =>
    call<SkillProject>("skills_add_project", { path, label: label ?? null }),
  removeProject: (id: string) => call<Removed>("skills_remove_project", { id }),
  listProjectAgentDirs: (id: string) =>
    call<ProjectAgentDir[]>("skills_list_project_agent_dirs", { id }),

  // ---------- 目录扫描 ----------
  scanDir: (path: string) => call<DirScanResult>("skills_scan_dir", { path }),

  // ---------- 技能扫描（只读） ----------
  listSkills: () => call<SkillSummary[]>("skills_scan"),
  listIssues: () => call<ScanFailure[]>("skills_list_issues"),
  getSkill: (rootId: string, name: string) =>
    call<SkillDetail>("skills_get_detail", { rootId, name }),
  getSkillFile: (rootId: string, name: string, path: string) =>
    call<{ path: string; content: string }>("skills_read_file", { rootId, name, path }),

  // ---------- 编辑 ----------
  saveSkill: (rootId: string, name: string, request: SaveSkillRequest) =>
    call<SkillDetail>("skills_save", { rootId, name, request }),
  createSkill: (request: CreateSkillRequest) => call<SkillDetail>("skills_create", { request }),
  renameSkill: (rootId: string, name: string, newName: string) =>
    call<SkillDetail>("skills_rename", { rootId, name, newName }),
  deleteSkill: (rootId: string, name: string) =>
    call<{ deleted: boolean; wasSymlink: boolean }>("skills_delete", { rootId, name }),

  // ---------- 回收站 ----------
  listTrash: () => call<TrashItem[]>("skills_list_trash"),
  restoreTrash: (id: string) => call<SkillDetail>("skills_restore_trash", { id }),
  purgeTrash: (id: string) => call<{ purged: boolean }>("skills_purge_trash", { id }),

  // ---------- 安装流水线 ----------
  prepareLocalSource: (sourcePath: string) =>
    call<PrepareSourceResponse>("skills_prepare_local_source", { sourcePath }),
  prepareUploadSource: (archivePath: string, filename: string) =>
    call<PrepareSourceResponse>("skills_prepare_upload_source", { archivePath, filename }),
  prepareRemoteSource: (source: RemoteSource, reference: string) =>
    call<PrepareSourceResponse>("skills_prepare_remote_source", { source, reference }),
  removeStaging: (id: string) => call<Removed>("skills_remove_staging", { id }),
  createInstallPlan: (request: CreateInstallPlanRequest) =>
    call<InstallPlan>("skills_create_install_plan", { request }),
  getInstallPlan: (id: string) => call<InstallPlan>("skills_get_install_plan", { id }),
  cancelInstallPlan: (id: string) =>
    call<{ cancelled: boolean }>("skills_cancel_install_plan", { id }),
  commitInstallPlan: (id: string, request: CommitPlanRequest) =>
    call<CommitPlanResponse>("skills_commit_install_plan", { id, request }),

  // ---------- 目录 / 数据源 ----------
  getCatalog: () => call<CatalogSkill[]>("skills_get_catalog"),
  listSources: () => call<SkillSource[]>("skills_list_sources"),
  addSource: (request: AddSourceRequest) => call<SkillSource>("skills_add_source", { request }),
  removeSource: (id: string) => call<Removed>("skills_remove_source", { id }),
  syncSource: (id: string) => call<SkillSource>("skills_sync_source", { id }),

  // ---------- skills.sh 搜索 / 远程详情 ----------
  searchSkillsSh: (query: string) =>
    call<SkillsShSearchItem[]>("skills_search_skillssh", { query }),
  getRemoteDetail: (source: RemoteSource, reference: string) =>
    call<RemoteSkillDetail>("skills_remote_detail", { source, reference }),
  getRemoteFile: (source: RemoteSource, reference: string, path: string) =>
    call<{ path: string; content: string }>("skills_remote_file", { source, reference, path }),

  // ---------- 安装记录 / 更新（P4） ----------
  listInstalls: () => call<InstallRecord[]>("skills_list_installs"),
  checkInstallUpdate: (recordId: string) =>
    call<InstallUpdateInfo>("skills_check_install_update", { id: recordId }),
  previewInstallUpdate: (recordId: string) =>
    call<UpdateDiffPreview>("skills_preview_update", { id: recordId }),
  applyInstallUpdate: (recordId: string, request: ApplyUpdateRequest) =>
    call<TargetInstallResult>("skills_apply_update", { id: recordId, request }),
  rollbackInstall: (recordId: string, backupId: string) =>
    call<InstallRecord>("skills_rollback_install", { id: recordId, backupId }),
};

/** 原生目录选择（返回绝对路径；用户取消为 null）。 */
export async function pickDirectory(prompt?: string, defaultPath?: string): Promise<string | null> {
  const selected = await open({
    directory: true,
    title: prompt,
    defaultPath: defaultPath || undefined,
  });
  return typeof selected === "string" ? selected : null;
}

/** 原生文件选择（返回绝对路径；用户取消为 null）。 */
export async function pickFile(prompt?: string): Promise<string | null> {
  const selected = await open({
    multiple: false,
    title: prompt,
    filters: [{ name: "Skill 归档", extensions: ["zip", "gz", "tgz"] }],
  });
  return typeof selected === "string" ? selected : null;
}
