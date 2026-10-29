use std::fs;
use std::path::Path;

use super::paths;
use super::types::{QuotaSnapshot, QuotaStatus, QuotaStoreFile};

pub fn load() -> QuotaStoreFile {
    let path = paths::quotas_path();
    let Ok(content) = fs::read_to_string(&path) else {
        return QuotaStoreFile::default();
    };
    serde_json::from_str::<QuotaStoreFile>(&content).unwrap_or_default()
}

pub fn save(store: &QuotaStoreFile) -> Result<(), String> {
    let path = paths::quotas_path();
    let payload = serde_json::to_string_pretty(store)
        .map_err(|error| format!("序列化额度快照失败: {error}"))?;
    atomic_write(&path, payload.as_bytes())
}

/// 写入一批快照。失败状态保留上一次成功读数并标记 `stale`，避免卡片瞬间退化为空态。
pub fn save_snapshots(snapshots: &[QuotaSnapshot]) -> Result<(), String> {
    let mut store = load();
    let now = super::queue::now_iso();
    for snapshot in snapshots {
        let key = snapshot.store_key();
        let existing = store.snapshots.get(&key).cloned();
        let next = match snapshot.status {
            QuotaStatus::Ready => {
                let mut ready = snapshot.clone();
                if ready.fetched_at.is_none() {
                    ready.fetched_at = Some(now.clone());
                }
                ready.stale = false;
                ready
            }
            _ => match existing {
                Some(previous) if previous.status == QuotaStatus::Ready => QuotaSnapshot {
                    stale: true,
                    message: snapshot
                        .message
                        .clone()
                        .or_else(|| Some("暂时无法获取最新额度，当前展示上次成功结果".to_owned())),
                    ..previous
                },
                _ => snapshot.clone(),
            },
        };
        store.snapshots.insert(key, next);
    }
    save(&store)
}

pub fn list() -> Vec<QuotaSnapshot> {
    load().snapshots.into_values().collect()
}

pub fn clear() -> Result<(), String> {
    save(&QuotaStoreFile::default())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建目录失败: {error}"))?;
    }
    let temp = path.with_extension("tmp");
    fs::write(&temp, bytes).map_err(|error| format!("写入临时文件失败: {error}"))?;
    fs::rename(&temp, path).map_err(|error| format!("替换文件失败: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::types::{QuotaStatus, QuotaWindow};

    #[test]
    fn failure_keeps_last_success_and_marks_stale() {
        let _guard = crate::usage::TEST_ENV_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let root = std::env::temp_dir().join(format!("xlt-quota-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        std::env::set_var("XLT_USAGE_DIR", &root);

        save_snapshots(&[QuotaSnapshot {
            channel: "codex".to_owned(),
            account: Some("plus".to_owned()),
            status: QuotaStatus::Ready,
            plan_label: Some("Plus".to_owned()),
            limits: vec![QuotaWindow {
                id: "five_hour".to_owned(),
                label: "5h".to_owned(),
                used_percent: 30.0,
                resets_at: None,
                description: None,
                remaining: None,
                total: None,
            }],
            fetched_at: Some("2026-10-29T10:00:00Z".to_owned()),
            ..QuotaSnapshot::default()
        }])
        .unwrap();

        save_snapshots(&[QuotaSnapshot {
            channel: "codex".to_owned(),
            account: Some("plus".to_owned()),
            status: QuotaStatus::TemporarilyUnavailable,
            message: Some("网络失败".to_owned()),
            ..QuotaSnapshot::default()
        }])
        .unwrap();

        let stored = list();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].status, QuotaStatus::Ready);
        assert!(stored[0].stale);
        assert_eq!(stored[0].limits.len(), 1);
        assert_eq!(stored[0].message.as_deref(), Some("网络失败"));

        std::env::remove_var("XLT_USAGE_DIR");
        std::fs::remove_dir_all(&root).ok();
    }
}
