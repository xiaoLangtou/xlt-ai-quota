use chrono::{DateTime, Datelike, Utc};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use super::paths;
use super::types::{CacheStats, Manifest, MonthMeta, MonthStats, QueueBucket};

pub const MANIFEST_VERSION: u32 = 1;

pub fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn month_from_hour(hour: &str) -> Option<String> {
    DateTime::parse_from_rfc3339(hour)
        .ok()
        .map(|timestamp| timestamp.with_timezone(&Utc).format("%Y-%m").to_string())
}

fn parse_hour(hour: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(hour)
        .ok()
        .map(|timestamp| timestamp.with_timezone(&Utc))
}

pub fn list_months() -> Vec<String> {
    let dir = paths::queue_dir();
    let mut months = Vec::new();
    let Ok(entries) = fs::read_dir(&dir) else {
        return months;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|value| value.to_str()) {
            if is_month_key(stem) {
                months.push(stem.to_owned());
            }
        }
    }
    months.sort();
    months
}

fn is_month_key(value: &str) -> bool {
    value.len() == 7
        && value.as_bytes().get(4) == Some(&b'-')
        && value[..4].chars().all(|c| c.is_ascii_digit())
        && value[5..].chars().all(|c| c.is_ascii_digit())
}

fn month_path(month: &str) -> PathBuf {
    paths::queue_dir().join(format!("{month}.jsonl"))
}

fn read_month(path: &Path) -> Vec<QueueBucket> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<QueueBucket>(line).ok())
        .collect()
}

/// 读取用量桶，按 `bucket.key()` 去重（后写入的覆盖先写入的）。
pub fn load_buckets(months: Option<&[String]>) -> Vec<QueueBucket> {
    let selected = months
        .map(|values| values.to_vec())
        .unwrap_or_else(list_months);
    let mut rows: BTreeMap<String, QueueBucket> = BTreeMap::new();
    for month in selected {
        for bucket in read_month(&month_path(&month)) {
            rows.insert(bucket.key(), bucket);
        }
    }
    rows.into_values().collect()
}

pub fn load_buckets_in_range(start: &str, end: &str) -> Vec<QueueBucket> {
    let Some(start_at) = parse_hour(start) else {
        return Vec::new();
    };
    let Some(end_at) = parse_hour(end) else {
        return Vec::new();
    };
    let months = months_between(start_at, end_at);
    let mut rows: BTreeMap<String, QueueBucket> = BTreeMap::new();
    for bucket in load_buckets(Some(&months)) {
        let Some(at) = parse_hour(&bucket.hour_start) else {
            continue;
        };
        if at >= start_at && at <= end_at {
            rows.insert(bucket.key(), bucket);
        }
    }
    rows.into_values().collect()
}

fn months_between(start: DateTime<Utc>, end: DateTime<Utc>) -> Vec<String> {
    let mut months = BTreeSet::new();
    let mut year = start.year();
    let mut month = start.month();
    let end_year = end.year();
    let end_month = end.month();
    loop {
        months.insert(format!("{year:04}-{month:02}"));
        if year == end_year && month == end_month {
            break;
        }
        if month == 12 {
            year += 1;
            month = 1;
        } else {
            month += 1;
        }
    }
    months.into_iter().collect()
}

pub fn month_meta(path: &Path) -> MonthMeta {
    let bytes = fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
    let rows = read_month(path);
    let mut first: Option<String> = None;
    let mut last: Option<String> = None;
    for row in &rows {
        if first.as_ref().map(|value| &row.hour_start < value).unwrap_or(true) {
            first = Some(row.hour_start.clone());
        }
        if last.as_ref().map(|value| &row.hour_start > value).unwrap_or(true) {
            last = Some(row.hour_start.clone());
        }
    }
    MonthMeta {
        rows: rows.len() as u64,
        bytes,
        first_hour: first,
        last_hour: last,
    }
}

pub fn load_manifest() -> Manifest {
    let path = paths::manifest_path();
    let Ok(content) = fs::read_to_string(&path) else {
        return Manifest::default();
    };
    serde_json::from_str::<Manifest>(&content).unwrap_or_default()
}

pub fn save_manifest(manifest: &Manifest) -> Result<(), String> {
    let path = paths::manifest_path();
    let payload = serde_json::to_string_pretty(manifest)
        .map_err(|error| format!("序列化 manifest 失败: {error}"))?;
    atomic_write(&path, payload.as_bytes())
}

pub fn rebuild_manifest() -> Result<Manifest, String> {
    let mut manifest = Manifest {
        version: MANIFEST_VERSION,
        months: BTreeMap::new(),
        updated_at: Some(now_iso()),
    };
    for month in list_months() {
        manifest
            .months
            .insert(month.clone(), month_meta(&month_path(&month)));
    }
    save_manifest(&manifest)?;
    Ok(manifest)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建目录失败: {error}"))?;
    }
    let temp = path.with_extension("tmp");
    fs::write(&temp, bytes).map_err(|error| format!("写入临时文件失败: {error}"))?;
    fs::rename(&temp, path).map_err(|error| format!("替换文件失败: {error}"))
}

fn write_month(month: &str, rows: &[QueueBucket]) -> Result<(), String> {
    let path = month_path(month);
    if rows.is_empty() {
        let _ = fs::remove_file(&path);
        return Ok(());
    }
    let mut payload = String::new();
    for row in rows {
        let line =
            serde_json::to_string(row).map_err(|error| format!("序列化用量桶失败: {error}"))?;
        payload.push_str(&line);
        payload.push('\n');
    }
    atomic_write(&path, payload.as_bytes())
}

#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppendResult {
    pub appended: usize,
    pub skipped: usize,
}

/// 追加用量桶。与磁盘现有内容相同的桶会跳过，避免重复同步把文件撑大。
pub fn append_buckets(buckets: &[QueueBucket]) -> Result<AppendResult, String> {
    let mut grouped: BTreeMap<String, Vec<QueueBucket>> = BTreeMap::new();
    for bucket in buckets {
        let Some(month) = month_from_hour(&bucket.hour_start) else {
            continue;
        };
        grouped.entry(month).or_default().push(bucket.clone());
    }

    let mut result = AppendResult::default();
    for (month, incoming) in grouped {
        let path = month_path(&month);
        let mut existing: HashMap<String, QueueBucket> = read_month(&path)
            .into_iter()
            .map(|row| (row.key(), row))
            .collect();

        let mut lines = String::new();
        for mut bucket in incoming {
            bucket.recompute_total();
            let key = bucket.key();
            if existing.get(&key) == Some(&bucket) {
                result.skipped += 1;
                continue;
            }
            existing.insert(key, bucket.clone());
            let line =
                serde_json::to_string(&bucket).map_err(|error| format!("序列化用量桶失败: {error}"))?;
            lines.push_str(&line);
            lines.push('\n');
            result.appended += 1;
        }

        if !lines.is_empty() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|error| format!("创建队列目录失败: {error}"))?;
            }
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .map_err(|error| format!("打开队列文件失败: {error}"))?;
            file.write_all(lines.as_bytes())
                .map_err(|error| format!("写入队列文件失败: {error}"))?;
            file.flush()
                .map_err(|error| format!("刷新队列文件失败: {error}"))?;
        }
    }

    if result.appended > 0 {
        let mut manifest = load_manifest();
        manifest.version = MANIFEST_VERSION;
        manifest.updated_at = Some(now_iso());
        for month in list_months() {
            manifest
                .months
                .insert(month.clone(), month_meta(&month_path(&month)));
        }
        save_manifest(&manifest)?;
    }

    Ok(result)
}

#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceResult {
    pub replaced: usize,
    pub removed: usize,
}

/// 用一次采集结果整体替换某来源在 [start, end] 内的桶（本地版的 replace_window）。
pub fn replace_source_range(
    source: &str,
    start: &str,
    end: &str,
    buckets: &[QueueBucket],
) -> Result<ReplaceResult, String> {
    let Some(start_at) = parse_hour(start) else {
        return Err("start 不是合法的 RFC3339 时间".to_owned());
    };
    let Some(end_at) = parse_hour(end) else {
        return Err("end 不是合法的 RFC3339 时间".to_owned());
    };
    let months = months_between(start_at, end_at);
    let mut incoming: HashMap<String, Vec<QueueBucket>> = HashMap::new();
    for mut bucket in buckets.iter().cloned() {
        bucket.recompute_total();
        let Some(month) = month_from_hour(&bucket.hour_start) else {
            continue;
        };
        incoming.entry(month).or_default().push(bucket);
    }

    let mut result = ReplaceResult::default();
    for month in months {
        let path = month_path(&month);
        let existing = read_month(&path);
        let before = existing.len();
        let mut rows: BTreeMap<String, QueueBucket> = existing
            .into_iter()
            .filter(|row| {
                if row.source != source {
                    return true;
                }
                match parse_hour(&row.hour_start) {
                    Some(at) => at < start_at || at > end_at,
                    None => true,
                }
            })
            .map(|row| (row.key(), row))
            .collect();
        result.removed += before.saturating_sub(rows.len());

        for bucket in incoming.remove(&month).unwrap_or_default() {
            if let Some(at) = parse_hour(&bucket.hour_start) {
                if at >= start_at && at <= end_at {
                    rows.insert(bucket.key(), bucket);
                }
            }
        }

        let rows: Vec<QueueBucket> = rows.into_values().collect();
        result.replaced += rows.len();
        write_month(&month, &rows)?;
    }

    let mut manifest = load_manifest();
    manifest.version = MANIFEST_VERSION;
    manifest.updated_at = Some(now_iso());
    manifest.months.clear();
    for month in list_months() {
        manifest
            .months
            .insert(month.clone(), month_meta(&month_path(&month)));
    }
    save_manifest(&manifest)?;

    Ok(result)
}

/// 删除 `cutoff` 之前的整月数据（清理过期缓存）。
pub fn delete_before(cutoff: DateTime<Utc>) -> Result<Vec<String>, String> {
    let cutoff_month = cutoff.format("%Y-%m").to_string();
    let mut removed = Vec::new();
    for month in list_months() {
        if month < cutoff_month {
            let _ = fs::remove_file(month_path(&month));
            removed.push(month);
        }
    }
    if !removed.is_empty() {
        rebuild_manifest()?;
    }
    Ok(removed)
}

pub fn clear_all() -> Result<(), String> {
    for month in list_months() {
        let _ = fs::remove_file(month_path(&month));
    }
    rebuild_manifest()?;
    Ok(())
}

pub fn cache_stats() -> CacheStats {
    let mut stats = CacheStats {
        data_dir: paths::data_dir().display().to_string(),
        ..CacheStats::default()
    };
    for month in list_months() {
        let meta = month_meta(&month_path(&month));
        stats.total_rows += meta.rows;
        stats.total_bytes += meta.bytes;
        if let Some(first) = &meta.first_hour {
            if stats
                .oldest_hour
                .as_ref()
                .map(|value| first < value)
                .unwrap_or(true)
            {
                stats.oldest_hour = Some(first.clone());
            }
        }
        if let Some(last) = &meta.last_hour {
            if stats
                .newest_hour
                .as_ref()
                .map(|value| last > value)
                .unwrap_or(true)
            {
                stats.newest_hour = Some(last.clone());
            }
        }
        stats.months.push(MonthStats {
            month,
            rows: meta.rows,
            bytes: meta.bytes,
            first_hour: meta.first_hour,
            last_hour: meta.last_hour,
        });
    }
    stats
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_temp_dir(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("xlt-queue-{name}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        std::env::set_var("XLT_USAGE_DIR", &root);
        root
    }

    fn bucket(hour: &str, source: &str, input: u64) -> QueueBucket {
        QueueBucket {
            hour_start: hour.to_owned(),
            source: source.to_owned(),
            collector: None,
            project: "p".to_owned(),
            model: "m".to_owned(),
            input_tokens: input,
            output_tokens: 1,
            cached_input_tokens: 0,
            cache_creation_input_tokens: 0,
            reasoning_output_tokens: 0,
            total_tokens: 0,
            conversation_count: 0,
            reported_cost_usd: None,
            local_metrics: None,
        }
    }

    #[test]
    fn append_dedupes_and_updates_manifest() {
        let _guard = crate::usage::TEST_ENV_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let root = with_temp_dir("append");
        let row = bucket("2026-10-29T10:00:00.000Z", "qwen", 10);

        let first = append_buckets(&[row.clone()]).unwrap();
        assert_eq!(first.appended, 1);
        let second = append_buckets(&[row.clone()]).unwrap();
        assert_eq!(second.appended, 0);
        assert_eq!(second.skipped, 1);

        let manifest = load_manifest();
        assert_eq!(manifest.months.get("2026-10").unwrap().rows, 1);
        assert_eq!(load_buckets(None).len(), 1);

        std::env::remove_var("XLT_USAGE_DIR");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn replace_source_range_removes_old_rows() {
        let _guard = crate::usage::TEST_ENV_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let root = with_temp_dir("replace");
        append_buckets(&[
            bucket("2026-10-29T10:00:00.000Z", "qwen", 10),
            bucket("2026-10-29T11:00:00.000Z", "amp", 20),
        ])
        .unwrap();

        replace_source_range(
            "qwen",
            "2026-10-01T00:00:00.000Z",
            "2026-10-31T23:59:59.000Z",
            &[bucket("2026-10-29T10:30:00.000Z", "qwen", 99)],
        )
        .unwrap();

        let rows = load_buckets(None);
        assert_eq!(rows.len(), 2);
        let qwen = rows.iter().find(|row| row.source == "qwen").unwrap();
        assert_eq!(qwen.input_tokens, 99);
        assert!(rows.iter().any(|row| row.source == "amp"));

        std::env::remove_var("XLT_USAGE_DIR");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn delete_before_removes_old_months() {
        let _guard = crate::usage::TEST_ENV_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let root = with_temp_dir("cleanup");
        append_buckets(&[
            bucket("2026-08-15T10:00:00.000Z", "qwen", 10),
            bucket("2026-10-15T10:00:00.000Z", "qwen", 20),
        ])
        .unwrap();

        let cutoff = DateTime::parse_from_rfc3339("2026-09-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let removed = delete_before(cutoff).unwrap();
        assert_eq!(removed, vec!["2026-08".to_owned()]);
        assert_eq!(list_months(), vec!["2026-10".to_owned()]);

        std::env::remove_var("XLT_USAGE_DIR");
        fs::remove_dir_all(&root).ok();
    }
}
