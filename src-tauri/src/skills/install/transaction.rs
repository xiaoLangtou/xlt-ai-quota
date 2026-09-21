use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use uuid::Uuid;

use super::inspector::hash_dir;
use super::super::config::SkillRoot;
use super::super::error::SkillsError;
use super::super::paths::{is_inside, is_valid_skill_name, safe_join};
use super::super::trash::copy_dir_all;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallErrorInfo {
    pub code: String,
    pub message: String,
    pub stage: String,
}

/// 单个目标的安装结果（逐目标语义，互不影响）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetInstallResult {
    pub root_id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<InstallErrorInfo>,
}

type RecordCallback<'a> = Box<dyn FnOnce(&Path, Option<&str>) -> Result<(), SkillsError> + 'a>;

pub struct InstallTargetOptions<'a> {
    /// staging 内已就绪的 skill 内容目录。
    pub skill_dir: PathBuf,
    /// staging 快照的内容哈希（复制后校验）。
    pub expected_hash: String,
    pub root: SkillRoot,
    pub final_name: String,
    /// 本地来源的真实源路径（用于源/目标嵌套拒绝）。
    pub source_real_path: Option<String>,
    /// 成功后保留备份目录（更新场景用）。
    pub keep_backup: bool,
    /// 切换成功后的安装记录步骤；报错则整体回滚。
    pub record: Option<RecordCallback<'a>>,
}

fn fail(root_id: &str, stage: &str, code: &str, message: impl Into<String>) -> TargetInstallResult {
    TargetInstallResult {
        root_id: root_id.to_owned(),
        status: "failed".to_owned(),
        final_name: None,
        dir_path: None,
        backup_id: None,
        error: Some(InstallErrorInfo {
            code: code.to_owned(),
            message: message.into(),
            stage: stage.to_owned(),
        }),
    }
}

/// 将一个 staging skill 原子安装到单个目标根目录。
pub fn install_skill_to_target(mut opts: InstallTargetOptions<'_>) -> TargetInstallResult {
    let root = opts.root.clone();
    let root_id = root.id.clone();

    // ---------- preflight ----------
    if !root.writable {
        return fail(&root_id, "preflight", "root_not_writable", format!("该根目录不可写：{}", root.label));
    }
    if !is_valid_skill_name(&opts.final_name) {
        return fail(&root_id, "preflight", "invalid_name", format!("skill 名称非法：{}", opts.final_name));
    }
    let dest = match safe_join(&PathBuf::from(&root.path), &opts.final_name) {
        Ok(path) => path,
        Err(error) => return fail(&root_id, "preflight", "invalid_path", error.message),
    };
    if let Some(source) = &opts.source_real_path {
        let dest_real = if dest.exists() {
            fs::canonicalize(&dest).unwrap_or_else(|_| dest.clone())
        } else {
            dest.clone()
        };
        let source_path = PathBuf::from(source);
        if is_inside(&source_path, &dest_real) || is_inside(&dest_real, &source_path) {
            return fail(
                &root_id,
                "preflight",
                "source_target_nested",
                format!(
                    "源目录与安装目标相同或互相嵌套：{} ↔ {}",
                    source,
                    dest_real.to_string_lossy()
                ),
            );
        }
    }
    if let Err(error) = fs::create_dir_all(&root.path) {
        return fail(&root_id, "preflight", "mkdir_failed", format!("创建目标根目录失败：{error}"));
    }

    // ---------- copy：先写同文件系统的临时目录 ----------
    let tx_id = Uuid::new_v4().to_string();
    let tmp_dir = PathBuf::from(&root.path).join(format!(".install-{tx_id}"));
    if let Err(error) = copy_dir_all(&opts.skill_dir, &tmp_dir) {
        let _ = fs::remove_dir_all(&tmp_dir);
        return fail(&root_id, "copy", "copy_failed", format!("复制失败：{error}"));
    }

    // ---------- validate：复制结果哈希必须与 staging 快照一致 ----------
    let actual = hash_dir(&tmp_dir);
    if actual != opts.expected_hash {
        let _ = fs::remove_dir_all(&tmp_dir);
        return fail(
            &root_id,
            "validate",
            "hash_mismatch",
            format!("内容校验失败（期望 {}，实际 {}），staging 可能已被修改", opts.expected_hash, actual),
        );
    }

    // ---------- switch：备份旧目录 → 原子切换 ----------
    let had_existing = dest.exists();
    let backup_id = if had_existing {
        Some(format!("backup-{tx_id}"))
    } else {
        None
    };
    let backup_dir = backup_id
        .as_ref()
        .map(|id| PathBuf::from(&root.path).join(format!(".{id}")));
    if let (true, Some(backup_dir)) = (had_existing, backup_dir.as_ref()) {
        if let Err(error) = fs::rename(&dest, backup_dir) {
            let _ = fs::remove_dir_all(&tmp_dir);
            return fail(&root_id, "switch", "backup_failed", format!("备份原目录失败：{error}"));
        }
    }
    if let Err(error) = fs::rename(&tmp_dir, &dest) {
        let _ = fs::remove_dir_all(&tmp_dir);
        if let Some(backup_dir) = backup_dir.as_ref() {
            if backup_dir.exists() {
                let _ = fs::rename(backup_dir, &dest);
            }
        }
        return fail(&root_id, "switch", "switch_failed", format!("切换目录失败：{error}"));
    }

    // ---------- record ----------
    if let Some(record) = opts.record.take() {
        if let Err(error) = record(&dest, backup_id.as_deref()) {
            let _ = fs::remove_dir_all(&dest);
            if let Some(backup_dir) = backup_dir.as_ref() {
                if backup_dir.exists() {
                    let _ = fs::rename(backup_dir, &dest);
                }
            }
            return fail(&root_id, "record", "record_failed", format!("写安装记录失败：{error}"));
        }
    }

    // ---------- 清理备份 ----------
    if let Some(backup_dir) = backup_dir.as_ref() {
        if backup_dir.exists() && !opts.keep_backup {
            let _ = fs::remove_dir_all(backup_dir);
        }
    }

    TargetInstallResult {
        root_id,
        status: "installed".to_owned(),
        final_name: Some(opts.final_name),
        dir_path: Some(dest.to_string_lossy().to_string()),
        backup_id,
        error: None,
    }
}

/// 清理某个根目录下残留的事务目录（异常退出后的 .install-* / .backup-*）。
pub fn cleanup_stale_tx_dirs(root_path: &Path, keep_backup_ids: &[String]) {
    let Ok(entries) = fs::read_dir(root_path) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some(backup) = name.strip_prefix('.') {
            if name.starts_with(".backup-") && keep_backup_ids.iter().any(|id| id == backup) {
                continue;
            }
        }
        if name.starts_with(".install-") || name.starts_with(".backup-") {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}
