use std::collections::{BTreeMap, BTreeSet};
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

    // 成功批次里出现的 key 是当前有效套餐；其余同渠道旧 key（旧套餐名/旧失败卡）会被清掉。
    let mut ready_keys: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for snapshot in snapshots.iter().filter(|item| item.status == QuotaStatus::Ready) {
        ready_keys
            .entry(snapshot.channel.clone())
            .or_default()
            .insert(snapshot.store_key());
    }

    for snapshot in snapshots {
        if snapshot.status != QuotaStatus::Ready {
            // 失败：把该渠道（或该套餐）已有的成功读数标记为陈旧，不再新增一张失败卡。
            let account = snapshot.account.as_deref();
            let affected: Vec<String> = store
                .snapshots
                .iter()
                .filter(|(_, value)| {
                    value.channel == snapshot.channel
                        && value.status == QuotaStatus::Ready
                        && account.map(|name| value.account.as_deref() == Some(name)).unwrap_or(true)
                })
                .map(|(key, _)| key.clone())
                .collect();
            if affected.is_empty() {
                store.snapshots.insert(snapshot.store_key(), snapshot.clone());
            } else {
                for key in affected {
                    if let Some(previous) = store.snapshots.get(&key).cloned() {
                        store.snapshots.insert(
                            key,
                            QuotaSnapshot {
                                stale: true,
                                message: snapshot.message.clone().or_else(|| {
                                    Some("暂时无法获取最新额度，当前展示上次成功结果".to_owned())
                                }),
                                ..previous
                            },
                        );
                    }
                }
            }
            continue;
        }

        let mut ready = snapshot.clone();
        if ready.fetched_at.is_none() {
            ready.fetched_at = Some(now.clone());
        }
        ready.stale = false;
        store.snapshots.insert(ready.store_key(), ready);
    }

    for (channel, keys) in ready_keys {
        store
            .snapshots
            .retain(|_, value| value.channel != channel || keys.contains(&value.store_key()));
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
    fn normalize_keeps_one_card_per_channel_and_ark_plan() {
        let _guard = crate::usage::TEST_ENV_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let root = std::env::temp_dir().join(format!("xlt-quota-normalize-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        std::env::set_var("XLT_USAGE_DIR", &root);

        save_snapshots(&[
            QuotaSnapshot {
                channel: "codex".to_owned(),
                account: Some("Free".to_owned()),
                status: QuotaStatus::Ready,
                fetched_at: Some("2026-10-29T09:00:00Z".to_owned()),
                ..QuotaSnapshot::default()
            },
            QuotaSnapshot {
                channel: "codex".to_owned(),
                account: Some("Plus".to_owned()),
                status: QuotaStatus::Ready,
                fetched_at: Some("2026-10-29T10:00:00Z".to_owned()),
                ..QuotaSnapshot::default()
            },
            QuotaSnapshot {
                channel: "ark".to_owned(),
                account: Some("agent-plan".to_owned()),
                status: QuotaStatus::Ready,
                fetched_at: Some("2026-10-29T10:00:00Z".to_owned()),
                ..QuotaSnapshot::default()
            },
            QuotaSnapshot {
                channel: "ark".to_owned(),
                account: None,
                status: QuotaStatus::TemporarilyUnavailable,
                fetched_at: Some("2026-10-29T10:00:00Z".to_owned()),
                ..QuotaSnapshot::default()
            },
        ])
        .unwrap();

        normalize().unwrap();
        let stored = list();
        assert_eq!(stored.len(), 2);
        assert!(stored.iter().any(|item| item.channel == "codex" && item.account.as_deref() == Some("Plus")));
        assert!(stored.iter().any(|item| item.channel == "ark" && item.account.as_deref() == Some("agent-plan")));

        std::env::remove_var("XLT_USAGE_DIR");
        std::fs::remove_dir_all(&root).ok();
    }

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

/// 一次性收敛历史重复卡：
/// - 非方舟：每渠道只保留最新一张成功快照（没有成功则保留最新失败卡）；
/// - 方舟：每个套餐只保留最新一张；存在带套餐名的卡时，丢弃无套餐名的历史失败卡。
pub fn normalize() -> Result<(), String> {
    let store = load();
    let original_len = store.snapshots.len();
    let mut channels: BTreeMap<String, Vec<(String, QuotaSnapshot)>> = BTreeMap::new();
    for (key, value) in &store.snapshots {
        channels
            .entry(value.channel.clone())
            .or_default()
            .push((key.clone(), value.clone()));
    }

    let mut keep: BTreeSet<String> = BTreeSet::new();
    for (channel, items) in channels {
        if channel == "ark" {
            let has_account = items.iter().any(|(_, snapshot)| snapshot.account.is_some());
            let mut by_account: BTreeMap<String, Vec<(String, QuotaSnapshot)>> = BTreeMap::new();
            for item in items {
                let account = item.1.account.clone().unwrap_or_default();
                by_account.entry(account).or_default().push(item);
            }
            for (account, group) in by_account {
                if has_account && account.is_empty() {
                    continue;
                }
                let best = group
                    .iter()
                    .filter(|(_, snapshot)| snapshot.status == QuotaStatus::Ready)
                    .max_by_key(|(_, snapshot)| snapshot.fetched_at.clone())
                    .or_else(|| {
                        group
                            .iter()
                            .max_by_key(|(_, snapshot)| snapshot.fetched_at.clone())
                    });
                if let Some((key, _)) = best {
                    keep.insert(key.clone());
                }
            }
        } else {
            let best = items
                .iter()
                .filter(|(_, snapshot)| snapshot.status == QuotaStatus::Ready)
                .max_by_key(|(_, snapshot)| snapshot.fetched_at.clone())
                .or_else(|| {
                    items
                        .iter()
                        .max_by_key(|(_, snapshot)| snapshot.fetched_at.clone())
                });
            if let Some((key, _)) = best {
                keep.insert(key.clone());
            }
        }
    }

    let snapshots: BTreeMap<String, QuotaSnapshot> = store
        .snapshots
        .into_iter()
        .filter(|(key, _)| keep.contains(key))
        .collect();
    if snapshots.len() != original_len {
        save(&QuotaStoreFile {
            version: 1,
            snapshots,
        })?;
    }
    Ok(())
}
