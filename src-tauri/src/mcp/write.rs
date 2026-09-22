use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use super::adapters::adapter_by_id;
use super::error::McpError;
use super::metadata;
use super::paths::McpPaths;
use super::plan::{file_mtime, ops_for, resolve_target, PlanRegistry};
use super::types::{ApplyPlanRequest, ApplyResult, WrittenFile};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupManifest {
    agent: String,
    created_at: String,
    name: String,
    original: String,
}

struct Written {
    path: PathBuf,
    backup_dir: Option<PathBuf>,
    existed: bool,
}

fn timestamp() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S-%3f").to_string()
}

#[cfg(unix)]
fn restrict_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(metadata) = fs::metadata(path) {
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o700);
        let _ = fs::set_permissions(path, permissions);
    }
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &Path) {}

/// 备份原文件到 `~/.xlt/backups/mcp/<agent>/<时间戳>/`，写入 manifest 供恢复。
fn backup_file(paths: &McpPaths, agent: &str, path: &Path) -> Result<PathBuf, McpError> {
    let dir = paths
        .backups_root
        .join(agent)
        .join(format!("{}-{}", timestamp(), &uuid::Uuid::new_v4().to_string()[..6]));
    fs::create_dir_all(&dir)?;
    restrict_permissions(&dir);
    let name = path
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "config".to_owned());
    fs::copy(path, dir.join(&name))?;
    let manifest = BackupManifest {
        agent: agent.to_owned(),
        created_at: timestamp(),
        name,
        original: path.display().to_string(),
    };
    let text = serde_json::to_string_pretty(&manifest).unwrap_or_default();
    fs::write(dir.join("manifest.json"), text)?;
    prune_backups(&paths.backups_root.join(agent), paths.backup_keep());
    Ok(dir)
}

/// 每个 Agent 仅保留最近 N 份备份（目录名按时间戳可排序）。
fn prune_backups(agent_dir: &Path, keep: usize) {
    let Ok(entries) = fs::read_dir(agent_dir) else {
        return;
    };
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    if dirs.len() <= keep {
        return;
    }
    for old in &dirs[..dirs.len() - keep] {
        let _ = fs::remove_dir_all(old);
    }
}

/// 原子写入：同目录临时文件 → rename。
fn atomic_write(path: &Path, content: &str) -> Result<(), McpError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file_name = path
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "config".to_owned());
    let tmp = path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()));
    fs::write(&tmp, content)?;
    if let Err(error) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(McpError::from(error));
    }
    Ok(())
}

fn rollback(written: &[Written]) -> bool {
    let mut ok = true;
    for entry in written.iter().rev() {
        if let Some(dir) = &entry.backup_dir {
            if let Ok(entries) = fs::read_dir(dir) {
                for file in entries.flatten() {
                    let candidate = file.path();
                    if candidate.file_name().map(|name| name == "manifest.json").unwrap_or(false) {
                        continue;
                    }
                    if fs::copy(&candidate, &entry.path).is_err() {
                        ok = false;
                    }
                }
            }
        } else if !entry.existed {
            let _ = fs::remove_file(&entry.path);
        }
    }
    ok
}

/// 提交计划：写前 mtime 校验 → 备份 → 原子写入 → 校验 → 失败回滚 → 暂存区动作。
pub fn apply(
    app: &AppHandle,
    paths: &McpPaths,
    registry: &PlanRegistry,
    plan_id: &str,
    request: &ApplyPlanRequest,
) -> Result<ApplyResult, McpError> {
    let stored = registry
        .lock()
        .map_err(|_| McpError::internal("计划表锁定失败"))?
        .remove(plan_id)
        .ok_or_else(|| McpError::not_found("计划不存在或已过期"))?;

    // 冲突检测：生成计划后文件被其他程序改动即中止。
    for (path_str, expected) in stored.mtimes.iter().chain(request.expected_mtimes.iter()) {
        let current = file_mtime(Path::new(path_str));
        if current != *expected {
            return Err(McpError::conflict(format!(
                "配置文件已被其他程序修改（{path_str}），请重新扫描后再试"
            )));
        }
    }

    let mut written: Vec<Written> = Vec::new();
    let mut failure: Option<String> = None;

    for target in &stored.request.targets {
        let adapter = adapter_by_id(&target.agent)
            .ok_or_else(|| McpError::bad_request(format!("未知 Agent：{}", target.agent)))?;
        let path = resolve_target(adapter.as_ref(), paths, target)?;
        let existed = path.is_file();
        let raw = if existed {
            fs::read_to_string(&path).unwrap_or_default()
        } else {
            String::new()
        };
        let (ops, _) = ops_for(adapter.as_ref(), &stored.request, target)?;
        let after = if ops.is_empty() {
            raw.clone()
        } else {
            adapter.apply(&raw, &ops)?
        };

        let backup_dir = if existed {
            Some(backup_file(paths, &target.agent, &path)?)
        } else {
            None
        };

        if let Err(error) = atomic_write(&path, &after) {
            failure = Some(error.message);
            break;
        }

        written.push(Written {
            path: path.clone(),
            backup_dir,
            existed,
        });

        let verified = fs::read_to_string(&path)
            .map_err(McpError::from)
            .and_then(|text| adapter.parse(&text));
        if let Err(error) = verified {
            failure = Some(format!("写入后校验失败：{}（{error}）", path.display()));
            break;
        }
    }

    if let Some(message) = failure {
        let rolled_back = rollback(&written);
        return Ok(ApplyResult {
            written: Vec::new(),
            rolled_back,
            error: Some(if rolled_back {
                format!("{message}；已自动回滚")
            } else {
                format!("{message}；自动回滚失败，请从备份恢复")
            }),
        });
    }

    for action in &stored.stage_actions {
        match action.action.as_str() {
            "stage" => metadata::stage(
                app,
                &action.agent,
                &action.scope,
                &action.project_path,
                &action.server,
            )?,
            "unstage" => {
                metadata::unstage(
                    app,
                    &action.agent,
                    &action.scope,
                    &action.project_path,
                    &action.server.name,
                )?;
            }
            _ => {}
        }
    }

    let written_files = written
        .iter()
        .map(|entry| WrittenFile {
            config_file: entry.path.display().to_string(),
            backup_dir: entry
                .backup_dir
                .as_ref()
                .map(|dir| dir.display().to_string())
                .unwrap_or_default(),
        })
        .collect();

    Ok(ApplyResult {
        written: written_files,
        rolled_back: false,
        error: None,
    })
}

/// 从备份目录恢复原文件（读取 manifest 中的原始路径）。
pub fn restore_backup(paths: &McpPaths, backup_dir: &str) -> Result<(), McpError> {
    let dir = PathBuf::from(backup_dir);
    // 仅允许恢复备份根目录内的内容，避免任意路径写入。
    if !dir.starts_with(&paths.backups_root) {
        return Err(McpError::forbidden("备份目录不在允许范围内"));
    }
    let manifest_text = fs::read_to_string(dir.join("manifest.json"))
        .map_err(|_| McpError::not_found("备份 manifest 缺失"))?;
    let manifest: BackupManifest =
        serde_json::from_str(&manifest_text).map_err(McpError::from)?;
    let source = dir.join(&manifest.name);
    if !source.is_file() {
        return Err(McpError::not_found("备份文件缺失"));
    }
    let target = PathBuf::from(&manifest.original);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(&source, &target)?;
    Ok(())
}
