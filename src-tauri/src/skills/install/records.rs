use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::super::config::SkillsPaths;
use super::super::error::SkillsError;

/// 标准化的来源元数据（安装记录与更新检查复用）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceMetadata {
    #[serde(rename = "type")]
    pub source_type: String,
    #[serde(rename = "ref")]
    pub reference: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subpath: Option<String>,
}

/// 一条安装记录（<configDir>/installs.json）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRecord {
    pub id: String,
    pub root_id: String,
    pub name: String,
    pub dir_path: String,
    pub source: SourceMetadata,
    pub installed_hash: String,
    pub current_hash: String,
    pub installed_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub backup_ids: Vec<String>,
}

pub struct UpsertRecordInput {
    pub root_id: String,
    pub name: String,
    pub dir_path: String,
    pub source: SourceMetadata,
    pub installed_hash: String,
}

fn records_file(paths: &SkillsPaths) -> PathBuf {
    paths.config_dir.join("installs.json")
}

fn read_all(paths: &SkillsPaths) -> Vec<InstallRecord> {
    fs::read_to_string(records_file(paths))
        .ok()
        .and_then(|raw| serde_json::from_str::<Vec<InstallRecord>>(&raw).ok())
        .unwrap_or_default()
}

fn write_all(paths: &SkillsPaths, records: &[InstallRecord]) -> Result<(), SkillsError> {
    fs::create_dir_all(&paths.config_dir)?;
    let text = serde_json::to_string_pretty(records)
        .map_err(|error| SkillsError::internal(error.to_string()))?;
    fs::write(records_file(paths), text)?;
    Ok(())
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// 全部安装记录。
pub fn list_install_records(paths: &SkillsPaths) -> Vec<InstallRecord> {
    read_all(paths)
}

/// 按 id 查找。
pub fn get_install_record(paths: &SkillsPaths, id: &str) -> Option<InstallRecord> {
    read_all(paths).into_iter().find(|record| record.id == id)
}

/// 按目标目录查找。
pub fn find_record_by_dir(paths: &SkillsPaths, root_id: &str, name: &str) -> Option<InstallRecord> {
    read_all(paths)
        .into_iter()
        .find(|record| record.root_id == root_id && record.name == name)
}

/// 写入/更新一条安装记录（同一 rootId+name 视为同一安装位）。
pub fn upsert_install_record(
    paths: &SkillsPaths,
    input: UpsertRecordInput,
) -> Result<InstallRecord, SkillsError> {
    let mut records = read_all(paths);
    let timestamp = now_iso();
    if let Some(existing) = records
        .iter_mut()
        .find(|record| record.root_id == input.root_id && record.name == input.name)
    {
        existing.dir_path = input.dir_path;
        existing.source = input.source;
        existing.installed_hash = input.installed_hash.clone();
        existing.current_hash = input.installed_hash;
        existing.updated_at = Some(timestamp);
        let updated = existing.clone();
        write_all(paths, &records)?;
        return Ok(updated);
    }
    let record = InstallRecord {
        id: uuid::Uuid::new_v4().to_string(),
        root_id: input.root_id,
        name: input.name,
        dir_path: input.dir_path,
        source: input.source,
        installed_hash: input.installed_hash.clone(),
        current_hash: input.installed_hash,
        installed_at: timestamp,
        updated_at: None,
        backup_ids: Vec::new(),
    };
    records.push(record.clone());
    write_all(paths, &records)?;
    Ok(record)
}

/// 刷新某条记录的本地内容哈希。
pub fn update_record_current_hash(paths: &SkillsPaths, id: &str, hash: &str) {
    let mut records = read_all(paths);
    let Some(record) = records.iter_mut().find(|record| record.id == id) else {
        return;
    };
    if record.current_hash == hash {
        return;
    }
    record.current_hash = hash.to_owned();
    let _ = write_all(paths, &records);
}

/// 登记一个保留的备份。
pub fn append_record_backup(paths: &SkillsPaths, id: &str, backup_id: &str) {
    let mut records = read_all(paths);
    let Some(record) = records.iter_mut().find(|record| record.id == id) else {
        return;
    };
    if record.backup_ids.iter().any(|item| item == backup_id) {
        return;
    }
    record.backup_ids.push(backup_id.to_owned());
    let _ = write_all(paths, &records);
}

/// 回滚/清理后同步记录：移除备份 id，可选重置哈希基线。
pub fn settle_record_backup(
    paths: &SkillsPaths,
    id: &str,
    backup_id: &str,
    new_hash: Option<&str>,
) -> Option<InstallRecord> {
    let mut records = read_all(paths);
    let record = records.iter_mut().find(|record| record.id == id)?;
    record.backup_ids.retain(|item| item != backup_id);
    if let Some(hash) = new_hash {
        record.installed_hash = hash.to_owned();
        record.current_hash = hash.to_owned();
        record.updated_at = Some(now_iso());
    }
    let updated = record.clone();
    let _ = write_all(paths, &records);
    Some(updated)
}

/// 删除一条记录。
pub fn remove_install_record(paths: &SkillsPaths, id: &str) -> bool {
    let records = read_all(paths);
    let next: Vec<InstallRecord> = records.iter().filter(|record| record.id != id).cloned().collect();
    if next.len() == records.len() {
        return false;
    }
    write_all(paths, &next).is_ok()
}

/// 按目录删除记录（删除 skill 时调用）。
pub fn remove_record_by_dir(paths: &SkillsPaths, root_id: &str, name: &str) {
    let records = read_all(paths);
    let next: Vec<InstallRecord> = records
        .iter()
        .filter(|record| !(record.root_id == root_id && record.name == name))
        .cloned()
        .collect();
    if next.len() != records.len() {
        let _ = write_all(paths, &next);
    }
}

/// 重命名 skill 时同步迁移记录（保留来源追踪）。
pub fn rename_record_dir(
    paths: &SkillsPaths,
    root_id: &str,
    old_name: &str,
    new_name: &str,
    new_dir_path: &str,
) {
    let mut records = read_all(paths);
    let Some(record) = records
        .iter_mut()
        .find(|record| record.root_id == root_id && record.name == old_name)
    else {
        return;
    };
    record.name = new_name.to_owned();
    record.dir_path = new_dir_path.to_owned();
    record.updated_at = Some(now_iso());
    let _ = write_all(paths, &records);
}
