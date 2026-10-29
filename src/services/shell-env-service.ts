import { invoke } from "@tauri-apps/api/core";
import type {
  ShellBackup,
  ShellChangePlan,
  ShellChangeRequest,
  ShellCommitResult,
  ShellPathReport,
  ShellSnapshot,
  ShellSourceMap,
  ShellValidateResult,
} from "@/types/shell";

export const shellEnvService = {
  snapshot(): Promise<ShellSnapshot> {
    return invoke<ShellSnapshot>("shell_snapshot");
  },

  previewChange(request: ShellChangeRequest): Promise<ShellChangePlan> {
    return invoke<ShellChangePlan>("shell_change_preview", { request });
  },

  commitChange(planId: string, expectedVersion: string): Promise<ShellCommitResult> {
    return invoke<ShellCommitResult>("shell_change_commit", { planId, expectedVersion });
  },

  cancelChange(planId: string): Promise<void> {
    return invoke<void>("shell_change_cancel", { planId });
  },

  listBackups(): Promise<ShellBackup[]> {
    return invoke<ShellBackup[]>("shell_config_backup_list");
  },

  validate(): Promise<ShellValidateResult> {
    return invoke<ShellValidateResult>("shell_config_validate");
  },

  openFile(path: string): Promise<void> {
    return invoke<void>("shell_file_open", { path });
  },

  /** R1 只读来源地图（扩展文件需用户勾选 includeExtended） */
  sourceMap(includeExtended: boolean): Promise<ShellSourceMap> {
    return invoke<ShellSourceMap>("shell_source_map", { includeExtended });
  },

  /** R1 只读 PATH 报告：分类 + 静态合并 + 诊断 */
  pathReport(includeExtended: boolean): Promise<ShellPathReport> {
    return invoke<ShellPathReport>("shell_path_report", { includeExtended });
  },
};
