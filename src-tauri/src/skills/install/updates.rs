use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use super::github::resolve_github_revision;
use super::inspector::{hash_dir, inspect_staging_content, list_files, InspectedSkill};
use super::records::{
    append_record_backup, get_install_record, settle_record_backup, update_record_current_hash,
    upsert_install_record, InstallRecord, UpsertRecordInput,
};
use super::sources::{prepare_local_source, prepare_remote_source, PrepareSourceResponse};
use super::staging::{get_staging, remove_staging, set_staging_status, staging_content_dir, StagingRegistry};
use super::transaction::{install_skill_to_target, InstallTargetOptions, TargetInstallResult};
use super::super::config::{find_target_root, SkillsPaths};
use super::super::error::SkillsError;
use super::super::paths::safe_join;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallUpdateInfo {
    pub record_id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_revision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDiffEntry {
    pub path: String,
    pub change: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDiffPreview {
    pub record_id: String,
    pub staging_id: String,
    pub expires_at: String,
    pub skill_id: String,
    pub new_hash: String,
    pub entries: Vec<FileDiffEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyUpdateRequest {
    pub staging_id: String,
    pub skill_id: String,
}

fn must_get_record(paths: &SkillsPaths, record_id: &str) -> Result<InstallRecord, SkillsError> {
    get_install_record(paths, record_id).ok_or_else(|| SkillsError::not_found("安装记录不存在"))
}

/// 检查一条安装记录的本地/上游变化。
pub async fn check_install_update(
    paths: &SkillsPaths,
    record_id: &str,
) -> Result<InstallUpdateInfo, SkillsError> {
    let record = must_get_record(paths, record_id)?;
    if !Path::new(&record.dir_path).exists() {
        return Ok(InstallUpdateInfo {
            record_id: record_id.to_owned(),
            status: "unknown".to_owned(),
            remote_revision: None,
            message: Some("本地目录已不存在，无法检查".to_owned()),
        });
    }
    let current_hash = hash_dir(Path::new(&record.dir_path));
    update_record_current_hash(paths, record.id.as_str(), &current_hash);
    let local_changed = current_hash != record.installed_hash;

    let mut remote_changed: Option<bool> = None;
    let mut remote_revision: Option<String> = None;
    let mut message: Option<String> = None;
    match record.source.source_type.as_str() {
        "github" => {
            if let Some(repository) = &record.source.repository {
                let mut parts = repository.split('/');
                let owner = parts.next().unwrap_or_default().to_owned();
                let repo = parts.next().unwrap_or_default().to_owned();
                remote_revision = resolve_github_revision(&super::github::GitHubLocation {
                    owner,
                    repo,
                    revision: record.source.branch.clone(),
                    subpath: None,
                })
                .await;
                match &remote_revision {
                    None => message = Some("无法解析上游最新版本（GitHub API 不可达或引用已失效）".to_owned()),
                    Some(_) => match &record.source.revision {
                        Some(installed) => remote_changed = Some(remote_revision.as_deref() != Some(installed)),
                        None => message = Some("安装时未记录不可变版本，无法比较上游变化".to_owned()),
                    },
                }
            }
        }
        "local" => {
            if Path::new(&record.source.reference).exists() {
                remote_changed = Some(hash_dir(Path::new(&record.source.reference)) != record.installed_hash);
            } else {
                message = Some("来源目录已不存在，无法比较".to_owned());
            }
        }
        _ => message = Some("该来源类型不支持上游变化检查".to_owned()),
    }

    let status = match remote_changed {
        None => {
            if local_changed {
                "local-changed"
            } else if message.is_none() {
                "up-to-date"
            } else {
                "unknown"
            }
        }
        Some(true) if local_changed => "both-changed",
        Some(true) => "remote-changed",
        Some(false) if local_changed => "local-changed",
        Some(false) => "up-to-date",
    };

    Ok(InstallUpdateInfo {
        record_id: record_id.to_owned(),
        status: status.to_owned(),
        remote_revision,
        message,
    })
}

/// 按来源类型重新准备一份最新内容（archive 无法自动重取）。
async fn reprepare_source(
    paths: &SkillsPaths,
    registry: &StagingRegistry,
    record: &InstallRecord,
) -> Result<PrepareSourceResponse, SkillsError> {
    match record.source.source_type.as_str() {
        "local" => prepare_local_source(paths, registry, &record.source.reference),
        "archive" => Err(SkillsError::new(422, "上传归档来源无法自动重新获取，请重新上传安装")),
        other => prepare_remote_source(paths, registry, other, &record.source.reference).await,
    }
}

/// 在候选中定位与安装记录对应的 Skill：名称精确匹配优先，唯一候选兜底。
fn match_candidate(record: &InstallRecord, skills: &[InspectedSkill]) -> Result<InspectedSkill, SkillsError> {
    let by_name: Vec<&InspectedSkill> = skills.iter().filter(|skill| skill.name == record.name).collect();
    if by_name.len() == 1 {
        return Ok(by_name[0].clone());
    }
    if skills.len() == 1 {
        return Ok(skills[0].clone());
    }
    let stripped = record
        .name
        .trim_end_matches(|c: char| c.is_ascii_digit())
        .trim_end_matches('-')
        .to_owned();
    let by_stripped: Vec<&InspectedSkill> = skills.iter().filter(|skill| skill.name == stripped).collect();
    if by_stripped.len() == 1 {
        return Ok(by_stripped[0].clone());
    }
    Err(SkillsError::new(
        422,
        format!("无法在来源中唯一定位「{}」对应的 Skill（候选 {} 个）", record.name, skills.len()),
    ))
}

fn file_fingerprint(path: &Path, symlink: bool) -> String {
    let mut hasher = Sha1::new();
    if symlink {
        if let Ok(target) = fs::read_link(path) {
            hasher.update(format!("link:{}", target.to_string_lossy()).as_bytes());
        }
    } else if let Ok(bytes) = fs::read(path) {
        hasher.update(&bytes);
    }
    format!("{:x}", hasher.finalize())
}

/// 两个目录的逐文件差异（相对 POSIX 路径）。
pub fn diff_dirs(old_dir: &Path, new_dir: &Path) -> Vec<FileDiffEntry> {
    let collect = |dir: &Path| -> HashMap<String, String> {
        let mut map = HashMap::new();
        if !dir.exists() {
            return map;
        }
        for file in list_files(dir) {
            map.insert(file.rel, file_fingerprint(&file.abs, file.symlink));
        }
        map
    };
    let old_files = collect(old_dir);
    let new_files = collect(new_dir);
    let mut entries = Vec::new();
    for (rel, hash) in &new_files {
        match old_files.get(rel) {
            None => entries.push(FileDiffEntry {
                path: rel.clone(),
                change: "added".to_owned(),
            }),
            Some(old) if old != hash => entries.push(FileDiffEntry {
                path: rel.clone(),
                change: "modified".to_owned(),
            }),
            Some(_) => {}
        }
    }
    for rel in old_files.keys() {
        if !new_files.contains_key(rel) {
            entries.push(FileDiffEntry {
                path: rel.clone(),
                change: "removed".to_owned(),
            });
        }
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    entries
}

/// 重新获取来源并产出文件级差异预览（staging 保留供 apply 复用）。
pub async fn preview_update_diff(
    paths: &SkillsPaths,
    registry: &StagingRegistry,
    record_id: &str,
) -> Result<UpdateDiffPreview, SkillsError> {
    let record = must_get_record(paths, record_id)?;
    let prep = reprepare_source(paths, registry, &record).await?;
    let result = (|| -> Result<UpdateDiffPreview, SkillsError> {
        let candidate = match_candidate(&record, &prep.skills)?;
        if !candidate.installable {
            let messages: Vec<String> = candidate
                .issues
                .iter()
                .filter(|issue| issue.level == super::super::validation::ValidationLevel::Error)
                .map(|issue| issue.message.clone())
                .collect();
            return Err(SkillsError::new(
                422,
                format!("上游版本存在 error 级问题，无法用于更新：{}", messages.join("；")),
            ));
        }
        let content_dir = staging_content_dir(paths, &prep.staging_id);
        let skill_dir = if candidate.relative_path.is_empty() {
            content_dir
        } else {
            safe_join(&content_dir, &candidate.relative_path)?
        };
        let entries = diff_dirs(Path::new(&record.dir_path), &skill_dir);
        Ok(UpdateDiffPreview {
            record_id: record_id.to_owned(),
            staging_id: prep.staging_id.clone(),
            expires_at: prep.expires_at.clone(),
            skill_id: candidate.id.clone(),
            new_hash: candidate.content_hash.clone(),
            entries,
        })
    })();
    if result.is_err() {
        remove_staging(paths, registry, &prep.staging_id);
    }
    result
}

/// 应用更新（复用事务安装器，保留备份支持回滚）。
pub fn apply_install_update(
    paths: &SkillsPaths,
    registry: &StagingRegistry,
    record_id: &str,
    request: &ApplyUpdateRequest,
) -> Result<TargetInstallResult, SkillsError> {
    let record = must_get_record(paths, record_id)?;
    let staging = get_staging(paths, registry, &request.staging_id)?;
    if staging.status != "ready" {
        return Err(SkillsError::conflict(format!(
            "staging 当前状态不可用于更新：{}",
            staging.status
        )));
    }
    let root = find_target_root(paths, &record.root_id)
        .ok_or_else(|| SkillsError::not_found(format!("安装记录对应的根目录不存在：{}", record.root_id)))?;

    let content_dir = staging_content_dir(paths, &staging.id);
    let search_root = match staging.search_subpath.as_ref() {
        Some(subpath) => Some(safe_join(&content_dir, subpath)?),
        None => None,
    };
    let candidates = inspect_staging_content(&content_dir, search_root.as_deref())?;
    let candidate = candidates
        .into_iter()
        .find(|skill| skill.id == request.skill_id)
        .ok_or_else(|| SkillsError::not_found(format!("候选 Skill 不存在：{}", request.skill_id)))?;

    set_staging_status(paths, registry, &staging.id, "committing")?;
    let skill_dir = if candidate.relative_path.is_empty() {
        content_dir.clone()
    } else {
        safe_join(&content_dir, &candidate.relative_path)?
    };
    let expected_hash = candidate.content_hash.clone();
    let staging_source = staging.source.clone();
    let source_real_path = if staging.source.source_type == "local" {
        Some(staging.source.reference.clone())
    } else {
        None
    };
    let root_id = root.id.clone();
    let final_name = record.name.clone();
    let paths_ref: &SkillsPaths = paths;
    let result = install_skill_to_target(InstallTargetOptions {
        skill_dir,
        expected_hash: expected_hash.clone(),
        root,
        final_name: final_name.clone(),
        source_real_path,
        keep_backup: true,
        record: Some(Box::new(move |dir_path, backup_id| {
            let updated = upsert_install_record(
                paths_ref,
                UpsertRecordInput {
                    root_id: root_id.clone(),
                    name: final_name.clone(),
                    dir_path: dir_path.to_string_lossy().to_string(),
                    source: staging_source.clone(),
                    installed_hash: expected_hash.clone(),
                },
            )?;
            if let Some(backup) = backup_id {
                append_record_backup(paths_ref, &updated.id, backup);
            }
            Ok(())
        })),
    });
    if result.status == "installed" {
        remove_staging(paths, registry, &staging.id);
    } else {
        let _ = set_staging_status(paths, registry, &staging.id, "ready");
    }
    Ok(result)
}

/// 回滚到指定备份（原子交换，失败恢复现场）。
pub fn rollback_install(
    paths: &SkillsPaths,
    record_id: &str,
    backup_id: &str,
) -> Result<InstallRecord, SkillsError> {
    let record = must_get_record(paths, record_id)?;
    if !record.backup_ids.iter().any(|item| item == backup_id) {
        return Err(SkillsError::not_found(format!("该安装记录没有此备份：{backup_id}")));
    }
    let dir_path = PathBuf::from(&record.dir_path);
    let root_dir = dir_path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| SkillsError::internal("安装目录无父目录"))?;
    let backup_dir = root_dir.join(format!(".{backup_id}"));
    if !backup_dir.exists() || !backup_dir.is_dir() {
        settle_record_backup(paths, &record.id, backup_id, None);
        return Err(SkillsError::new(410, "备份目录已丢失，无法回滚"));
    }
    let aside = root_dir.join(format!(".install-rollback-{}", uuid::Uuid::new_v4()));
    let had_current = dir_path.exists();
    if had_current {
        fs::rename(&dir_path, &aside)?;
    }
    if let Err(error) = fs::rename(&backup_dir, &dir_path) {
        if had_current {
            let _ = fs::rename(&aside, &dir_path);
        }
        return Err(SkillsError::internal(format!("回滚失败：{error}")));
    }
    if had_current {
        let _ = fs::remove_dir_all(&aside);
    }
    let new_hash = hash_dir(&dir_path);
    Ok(settle_record_backup(paths, &record.id, backup_id, Some(&new_hash))
        .unwrap_or(record))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("xlt-updates-{}-{}", tag, uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn diffs_added_modified_removed_files() {
        let old = temp_dir("old");
        let new = temp_dir("new");
        fs::write(old.join("keep.md"), "same").unwrap();
        fs::write(old.join("edit.md"), "before").unwrap();
        fs::write(old.join("gone.md"), "bye").unwrap();
        fs::write(new.join("keep.md"), "same").unwrap();
        fs::write(new.join("edit.md"), "after").unwrap();
        fs::write(new.join("added.md"), "new").unwrap();

        let entries = diff_dirs(&old, &new);
        let pairs: Vec<(&str, &str)> = entries
            .iter()
            .map(|entry| (entry.path.as_str(), entry.change.as_str()))
            .collect();
        assert_eq!(
            pairs,
            vec![
                ("added.md", "added"),
                ("edit.md", "modified"),
                ("gone.md", "removed"),
            ]
        );
    }

    #[test]
    fn matches_candidate_by_name_then_stripped_suffix() {
        let record = InstallRecord {
            id: "r1".to_owned(),
            root_id: "root".to_owned(),
            name: "foo-2".to_owned(),
            dir_path: "/tmp/foo-2".to_owned(),
            source: crate::skills::install::records::SourceMetadata {
                source_type: "github".to_owned(),
                reference: "a/b".to_owned(),
                repository: None,
                revision: None,
                branch: None,
                subpath: None,
            },
            installed_hash: "h".to_owned(),
            current_hash: "h".to_owned(),
            installed_at: "2026-01-01T00:00:00.000Z".to_owned(),
            updated_at: None,
            backup_ids: Vec::new(),
        };
        let candidate = InspectedSkill {
            id: "s1".to_owned(),
            relative_path: "foo".to_owned(),
            name: "foo".to_owned(),
            version: None,
            description: None,
            content_hash: "h".to_owned(),
            size_bytes: 0,
            file_count: 0,
            has_scripts: false,
            has_binary_files: false,
            issues: Vec::new(),
            installable: true,
        };
        let matched = match_candidate(&record, &[candidate.clone()]).unwrap();
        assert_eq!(matched.id, "s1");
    }
}
