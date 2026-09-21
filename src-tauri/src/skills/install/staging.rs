use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use super::records::SourceMetadata;
use super::super::config::SkillsPaths;
use super::super::error::SkillsError;

/// staging 存活时长：30 分钟。
const TTL: Duration = Duration::from_secs(30 * 60);
const META_FILE: &str = "meta.json";
const CONTENT_DIR: &str = "content";

pub type StagingRegistry = Mutex<HashMap<String, StagingRecord>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StagingRecord {
    pub id: String,
    pub path: String,
    pub source: SourceMetadata,
    pub created_at: String,
    pub expires_at: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_subpath: Option<String>,
}

fn staging_root(paths: &SkillsPaths) -> PathBuf {
    paths.staging_dir.clone()
}

/// staging 的内容目录（源内容统一复制到这里）。
pub fn staging_content_dir(paths: &SkillsPaths, id: &str) -> PathBuf {
    staging_root(paths).join(id).join(CONTENT_DIR)
}

fn meta_file(paths: &SkillsPaths, id: &str) -> PathBuf {
    staging_root(paths).join(id).join(META_FILE)
}

fn now() -> SystemTime {
    SystemTime::now()
}

fn to_iso(time: SystemTime) -> String {
    let datetime: chrono::DateTime<chrono::Utc> = time.into();
    datetime.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn is_expired(record: &StagingRecord) -> bool {
    chrono::DateTime::parse_from_rfc3339(&record.expires_at)
        .map(|expires| chrono::Utc::now() > expires.with_timezone(&chrono::Utc))
        .unwrap_or(true)
}

fn persist(paths: &SkillsPaths, record: &StagingRecord) -> Result<(), SkillsError> {
    fs::create_dir_all(staging_root(paths).join(&record.id))?;
    let text = serde_json::to_string_pretty(record)
        .map_err(|error| SkillsError::internal(error.to_string()))?;
    fs::write(meta_file(paths, &record.id), text)?;
    Ok(())
}

/// 新建一个 staging（内容目录已创建，由调用方填充后置为 ready）。
pub fn create_staging(
    paths: &SkillsPaths,
    registry: &StagingRegistry,
    source: SourceMetadata,
) -> Result<StagingRecord, SkillsError> {
    cleanup_expired_staging(paths, registry);
    let id = uuid::Uuid::new_v4().to_string();
    let content_dir = staging_content_dir(paths, &id);
    fs::create_dir_all(&content_dir)?;
    let created = now();
    let record = StagingRecord {
        id: id.clone(),
        path: content_dir.to_string_lossy().to_string(),
        source,
        created_at: to_iso(created),
        expires_at: to_iso(created + TTL),
        status: "preparing".to_owned(),
        search_subpath: None,
    };
    persist(paths, &record)?;
    if let Ok(mut map) = registry.lock() {
        map.insert(id, record.clone());
    }
    Ok(record)
}

fn restore_from_disk(
    paths: &SkillsPaths,
    registry: &StagingRegistry,
    id: &str,
) -> Option<StagingRecord> {
    let raw = fs::read_to_string(meta_file(paths, id)).ok()?;
    let parsed: StagingRecord = serde_json::from_str(&raw).ok()?;
    if parsed.id != id {
        return None;
    }
    if let Ok(mut map) = registry.lock() {
        map.insert(id.to_owned(), parsed.clone());
    }
    Some(parsed)
}

/// 查找 staging；不存在或已过期时报错（过期的顺带清理）。
pub fn get_staging(
    paths: &SkillsPaths,
    registry: &StagingRegistry,
    id: &str,
) -> Result<StagingRecord, SkillsError> {
    let cached = registry.lock().ok().and_then(|map| map.get(id).cloned());
    let record = match cached {
        Some(record) => record,
        None => restore_from_disk(paths, registry, id)
            .ok_or_else(|| SkillsError::not_found("staging 不存在或已过期"))?,
    };
    if is_expired(&record) {
        remove_staging(paths, registry, id);
        return Err(SkillsError::not_found("staging 已过期，请重新准备来源"));
    }
    Ok(record)
}

/// 更新 staging 状态并落盘。
pub fn set_staging_status(
    paths: &SkillsPaths,
    registry: &StagingRegistry,
    id: &str,
    status: &str,
) -> Result<StagingRecord, SkillsError> {
    let mut record = get_staging(paths, registry, id)?;
    record.status = status.to_owned();
    persist(paths, &record)?;
    if let Ok(mut map) = registry.lock() {
        map.insert(id.to_owned(), record.clone());
    }
    Ok(record)
}

/// 更新 staging 的来源元数据 / 搜索子路径并落盘。
pub fn update_staging(
    paths: &SkillsPaths,
    registry: &StagingRegistry,
    id: &str,
    source: Option<SourceMetadata>,
    search_subpath: Option<String>,
) -> Result<StagingRecord, SkillsError> {
    let mut record = get_staging(paths, registry, id)?;
    if let Some(source) = source {
        record.source = source;
    }
    if let Some(subpath) = search_subpath {
        record.search_subpath = Some(subpath);
    }
    persist(paths, &record)?;
    if let Ok(mut map) = registry.lock() {
        map.insert(id.to_owned(), record.clone());
    }
    Ok(record)
}

/// 删除一个 staging（目录 + 登记）。
pub fn remove_staging(paths: &SkillsPaths, registry: &StagingRegistry, id: &str) -> bool {
    let existed = registry
        .lock()
        .map(|mut map| map.remove(id).is_some())
        .unwrap_or(false);
    let dir = staging_root(paths).join(id);
    if dir.exists() {
        let _ = fs::remove_dir_all(&dir);
        return true;
    }
    existed
}

/// 清理全部过期 staging（启动时与每次创建时调用），返回清理数量。
pub fn cleanup_expired_staging(paths: &SkillsPaths, registry: &StagingRegistry) -> usize {
    let root = staging_root(paths);
    let Ok(entries) = fs::read_dir(&root) else {
        return 0;
    };
    let mut cleaned = 0;
    for entry in entries.flatten() {
        let id = entry.file_name().to_string_lossy().to_string();
        let dir = root.join(&id);
        let cached = registry.lock().ok().and_then(|map| map.get(&id).cloned());
        let record = cached.or_else(|| {
            fs::read_to_string(meta_file(paths, &id))
                .ok()
                .and_then(|raw| serde_json::from_str::<StagingRecord>(&raw).ok())
        });
        let expired = match record {
            Some(record) => is_expired(&record),
            None => fs::metadata(&dir)
                .and_then(|meta| meta.modified())
                .map(|modified| {
                    now()
                        .duration_since(modified)
                        .map(|elapsed| elapsed > TTL)
                        .unwrap_or(false)
                })
                .unwrap_or(false),
        };
        if expired {
            if let Ok(mut map) = registry.lock() {
                map.remove(&id);
            }
            let _ = fs::remove_dir_all(&dir);
            cleaned += 1;
        }
    }
    cleaned
}
