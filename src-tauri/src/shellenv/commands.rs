//! Tauri 命令边界：全部 Shell 文件能力经 Snapshot + 计划管线暴露。
//!
//! 旧版直写命令（表格直存、源码直写、恢复直拷等 13 个）已按 R0 迁移要求移除，
//! 所有写操作统一走 preview / commit / cancel，不再保留绕过预览的通道。

use crate::shellenv::{pathmodel, plan, source};
use crate::shellenv::files;
use crate::shellenv::types::*;
use tauri::State;
use std::sync::Mutex;

/// 共享互斥：同一时刻只允许一个变更会话（预览 / 提交），避免并发快照错乱。
#[derive(Default)]
pub struct ShellEnvLock(pub Mutex<()>);

#[tauri::command]
pub fn shell_snapshot(lock: State<'_, ShellEnvLock>) -> Result<ShellSnapshot, String> {
    let _guard = lock.0.lock().expect("shell env lock poisoned");
    Ok(ShellSnapshot {
        alias: plan::build_snapshot(KIND_ALIAS)?,
        zshrc: plan::build_snapshot(KIND_ZSHRC)?,
    })
}

#[tauri::command]
pub fn shell_change_preview(
    lock: State<'_, ShellEnvLock>,
    request: ShellChangeRequest,
) -> Result<ShellChangePlan, String> {
    let _guard = lock.0.lock().expect("shell env lock poisoned");
    plan::preview(request)
}

#[tauri::command]
pub fn shell_change_commit(
    lock: State<'_, ShellEnvLock>,
    plan_id: String,
    expected_version: String,
) -> Result<ShellCommitResult, String> {
    let _guard = lock.0.lock().expect("shell env lock poisoned");
    plan::commit(&plan_id, &expected_version)
}

#[tauri::command]
pub fn shell_change_cancel(lock: State<'_, ShellEnvLock>, plan_id: String) -> Result<(), String> {
    let _guard = lock.0.lock().expect("shell env lock poisoned");
    plan::cancel(&plan_id)
}

#[tauri::command]
pub fn shell_config_backup_list() -> Result<Vec<ShellBackup>, String> {
    files::list_backups()
}

/// R1 只读来源地图：来源文件登记 + 字面量 source 引用跟踪 + 环境定义归因。
/// `.zshenv` / `.zprofile` / 系统路径清单仅在 includeExtended 时读取。
#[tauri::command]
pub fn shell_source_map(include_extended: bool) -> Result<source::SourceMap, String> {
    let set = source::build_scanned_set(&source::SourceOptions { include_extended })?;
    Ok(source::SourceMap {
        files: set.files.iter().map(|scanned| scanned.meta.clone()).collect(),
        envs: source::build_env_origins(&set),
        diagnostics: set.diagnostics,
    })
}

/// R1 只读 PATH 报告：条目分类、静态合并结果与诊断（不执行任何程序）。
#[tauri::command]
pub fn shell_path_report(include_extended: bool) -> Result<pathmodel::PathReport, String> {
    let set = source::build_scanned_set(&source::SourceOptions { include_extended })?;
    let home = files::home_dir()?;
    Ok(pathmodel::build_path_report(&set, &home))
}

/// 两个配置文件的当前语法状态（只读，不产生计划）。
#[tauri::command]
pub fn shell_config_validate() -> Result<ValidateResult, String> {
    let mut errors = Vec::new();
    for kind in [KIND_ALIAS, KIND_ZSHRC] {
        let path = files::kind_path(kind)?;
        if !path.exists() {
            continue;
        }
        if let Err(error) = files::validate_with_zsh(&path) {
            errors.push(format!("{}: {error}", path.display()));
        }
    }
    Ok(ValidateResult { ok: errors.is_empty(), errors })
}

/// 用系统默认文本编辑器打开配置文件（仅允许两个受管配置文件）。
#[tauri::command]
pub fn shell_file_open(path: String) -> Result<(), String> {
    let requested = files::allowed_config_path(&path)?;
    #[cfg(target_os = "macos")]
    let status = std::process::Command::new("open").arg("-t").arg(&requested).status();
    #[cfg(all(unix, not(target_os = "macos")))]
    let status = std::process::Command::new("xdg-open").arg(&requested).status();
    #[cfg(windows)]
    let status = std::process::Command::new("notepad").arg(&requested).status();
    match status {
        Ok(_) => Ok(()),
        Err(error) => Err(format!("打开文件失败: {error}")),
    }
}
