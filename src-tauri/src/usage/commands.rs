use std::collections::BTreeMap;
use std::sync::Mutex;

use serde::Serialize;
use serde_json::Value;

use super::{cleanup, collectors, config, cursors, paths, pricing, queue, quota};
use super::types::{
    CacheStats, ImportResult, LegacyImport, LegacyQuotaRow, PricingHit, PricingQuery,
    PricingStatus, QuotaSnapshot, QuotaStatus, QuotaWindow, QueueBucket, UsageConfig,
};

static USAGE_LOCK: Mutex<()> = Mutex::new(());

fn lock() -> std::sync::MutexGuard<'static, ()> {
    USAGE_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageBootstrap {
    pub config: UsageConfig,
    pub buckets: Vec<QueueBucket>,
    pub quotas: Vec<QuotaSnapshot>,
    pub cache_stats: CacheStats,
    pub pricing_status: PricingStatus,
    pub data_dir: String,
}

#[tauri::command]
pub fn usage_init() -> Result<UsageBootstrap, String> {
    let _guard = lock();
    paths::ensure_dirs()?;
    let _ = cleanup::cleanup_if_due();
    let _ = quota::normalize();
    let cfg = config::load();
    Ok(UsageBootstrap {
        config: cfg,
        buckets: queue::load_buckets(None),
        quotas: quota::list(),
        cache_stats: cleanup::stats(),
        pricing_status: pricing::status(),
        data_dir: paths::data_dir().display().to_string(),
    })
}

#[tauri::command]
pub fn usage_collect(
    source: String,
    since: String,
    until: String,
) -> Result<collectors::CollectResult, String> {
    let _guard = lock();
    let _ = cleanup::cleanup_if_due();
    collectors::collect(&source, &since, &until)
}

#[tauri::command]
pub fn usage_collector_sources() -> Vec<&'static str> {
    collectors::known_sources()
}

#[tauri::command]
pub fn usage_load_buckets(since: Option<String>) -> Result<Vec<QueueBucket>, String> {
    let _guard = lock();
    match since.as_deref() {
        Some(start) if !start.trim().is_empty() => {
            let end = queue::now_iso();
            Ok(queue::load_buckets_in_range(start, &end))
        }
        _ => Ok(queue::load_buckets(None)),
    }
}

#[tauri::command]
pub fn usage_append_buckets(buckets: Vec<QueueBucket>) -> Result<queue::AppendResult, String> {
    let _guard = lock();
    queue::append_buckets(&buckets)
}

#[tauri::command]
pub fn usage_replace_source_range(
    source: String,
    start: String,
    end: String,
    buckets: Vec<QueueBucket>,
) -> Result<queue::ReplaceResult, String> {
    let _guard = lock();
    queue::replace_source_range(&source, &start, &end, &buckets)
}

#[tauri::command]
pub fn usage_save_quotas(snapshots: Vec<QuotaSnapshot>) -> Result<(), String> {
    let _guard = lock();
    quota::save_snapshots(&snapshots)
}

#[tauri::command]
pub fn usage_list_quotas() -> Result<Vec<QuotaSnapshot>, String> {
    let _guard = lock();
    Ok(quota::list())
}

#[tauri::command]
pub fn usage_clear_quotas() -> Result<(), String> {
    let _guard = lock();
    quota::clear()
}

#[tauri::command]
pub fn usage_config() -> Result<UsageConfig, String> {
    let _guard = lock();
    Ok(config::load())
}

#[tauri::command]
pub fn usage_set_config(patch: Value) -> Result<UsageConfig, String> {
    let _guard = lock();
    let mut current = serde_json::to_value(config::load())
        .map_err(|error| format!("读取用量配置失败: {error}"))?;
    merge_json(&mut current, &patch);
    let next: UsageConfig = serde_json::from_value(current)
        .map_err(|error| format!("用量配置格式非法: {error}"))?;
    config::save(&next)?;
    Ok(next)
}

fn merge_json(target: &mut Value, patch: &Value) {
    match (target, patch) {
        (Value::Object(target), Value::Object(patch)) => {
            for (key, value) in patch {
                merge_json(target.entry(key.clone()).or_insert(Value::Null), value);
            }
        }
        (target, patch) => *target = patch.clone(),
    }
}

#[tauri::command]
pub fn usage_cache_stats() -> Result<CacheStats, String> {
    let _guard = lock();
    Ok(cleanup::stats())
}

#[tauri::command]
pub fn usage_cleanup(retention_days: Option<u32>) -> Result<CacheStats, String> {
    let _guard = lock();
    let days = retention_days.unwrap_or_else(|| config::load().retention_days);
    cleanup::cleanup(days)
}

#[tauri::command]
pub fn usage_clear_cache() -> Result<CacheStats, String> {
    let _guard = lock();
    let _ = cursors::clear();
    cleanup::clear_all()
}

#[tauri::command]
pub fn usage_cursors_get(source: String) -> Result<Option<Value>, String> {
    let _guard = lock();
    Ok(cursors::get_source(&source))
}

#[tauri::command]
pub fn usage_cursors_set(source: String, value: Value) -> Result<(), String> {
    let _guard = lock();
    cursors::set_source(&source, value)
}

#[tauri::command]
pub fn usage_cursors_clear(source: Option<String>) -> Result<(), String> {
    let _guard = lock();
    match source {
        Some(source) => cursors::remove_source(&source),
        None => cursors::clear(),
    }
}

#[tauri::command]
pub fn usage_pricing_lookup(queries: Vec<PricingQuery>) -> Result<Vec<PricingHit>, String> {
    Ok(pricing::lookup_many(&queries))
}

#[tauri::command]
pub fn usage_pricing_table() -> Result<Value, String> {
    let _guard = lock();
    serde_json::to_value(pricing::merged())
        .map_err(|error| format!("序列化定价表失败: {error}"))
}

#[tauri::command]
pub fn usage_pricing_status() -> Result<PricingStatus, String> {
    let _guard = lock();
    Ok(pricing::status())
}

#[tauri::command]
pub fn usage_refresh_pricing(url: Option<String>) -> Result<PricingStatus, String> {
    let _guard = lock();
    let target = url
        .filter(|value| !value.trim().is_empty())
        .or_else(|| config::load().pricing_url)
        .ok_or_else(|| "未配置定价覆盖层地址".to_owned())?;
    pricing::refresh_overlay(&target)
}

/// 一次性迁移旧 localStorage 数据。成功后前端才删除旧键。
#[tauri::command]
pub fn usage_import_legacy(payload: LegacyImport) -> Result<ImportResult, String> {
    let _guard = lock();
    paths::ensure_dirs()?;

    let buckets: Vec<QueueBucket> = payload
        .tokens
        .iter()
        .map(|row| {
            let hour_start = row
                .hour_start
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| format!("{}T12:00:00.000Z", row.date));
            let mut bucket = QueueBucket {
                hour_start,
                source: row.platform.clone(),
                collector: None,
                project: "unknown".to_owned(),
                model: row
                    .model
                    .clone()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| "unknown".to_owned()),
                input_tokens: row.input_tokens,
                output_tokens: row.output_tokens,
                cached_input_tokens: row.cached_tokens,
                cache_creation_input_tokens: row.cache_write_tokens,
                reasoning_output_tokens: row.reasoning_tokens,
                total_tokens: 0,
                conversation_count: 0,
                reported_cost_usd: None,
                local_metrics: (row.request_count > 0).then(|| super::types::LocalMetrics {
                    request_count: row.request_count,
                    cache_hit_tokens: row.cached_tokens,
                    cache_miss_tokens: 0,
                    request_count_complete: true,
                }),
            };
            bucket.recompute_total();
            bucket
        })
        .collect();

    let appended = queue::append_buckets(&buckets)?;

    let mut grouped: BTreeMap<String, Vec<LegacyQuotaRow>> = BTreeMap::new();
    for row in payload.quotas {
        let key = format!("{}|{}", row.platform, row.account_name.clone().unwrap_or_default());
        grouped.entry(key).or_default().push(row);
    }
    let mut snapshots = Vec::new();
    for (_, rows) in grouped {
        let first = rows.first().cloned().unwrap_or_default();
        let limits: Vec<QuotaWindow> = rows
            .iter()
            .map(|row| {
                let used_percent = if row.limit > 0.0 {
                    (row.used / row.limit * 100.0).clamp(0.0, 100.0)
                } else {
                    row.used.clamp(0.0, 100.0)
                };
                QuotaWindow {
                    id: row.metric.clone().unwrap_or_else(|| "limit".to_owned()),
                    label: metric_label(row.metric.as_deref()),
                    used_percent,
                    resets_at: row.resets_at.clone(),
                    description: None,
                    remaining: (row.limit > 0.0).then_some((row.limit - row.used).max(0.0)),
                    total: (row.limit > 0.0).then_some(row.limit),
                }
            })
            .collect();
        snapshots.push(QuotaSnapshot {
            channel: first.platform.clone(),
            account: first.account_name.clone(),
            status: QuotaStatus::Ready,
            plan_label: first.account_name.clone(),
            region: None,
            limits,
            five_hour: None,
            weekly: None,
            seven_day: None,
            primary_models: None,
            other_models: None,
            plan: None,
            fetched_at: first.collected_at.clone(),
            stale: false,
            message: None,
            balance: None,
        });
    }
    quota::save_snapshots(&snapshots)?;

    let bills = payload.bills.is_some();
    if let Some(bills) = payload.bills {
        let path = paths::data_dir().join("bills.json");
        let body = serde_json::to_string_pretty(&bills)
            .map_err(|error| format!("序列化账单失败: {error}"))?;
        std::fs::write(path, body).map_err(|error| format!("写入账单失败: {error}"))?;
    }

    Ok(ImportResult {
        tokens: appended.appended,
        quotas: snapshots.len(),
        bills,
        skipped: false,
    })
}

fn metric_label(metric: Option<&str>) -> String {
    match metric.unwrap_or("limit") {
        "five_hour" => "5 小时额度".to_owned(),
        "weekly" => "周额度".to_owned(),
        "monthly" => "月额度".to_owned(),
        "monthly_code" => "月 Code 额度".to_owned(),
        "session" => "Session 额度".to_owned(),
        "credits" => "本周期已消耗".to_owned(),
        "addon_credits" => "加购已消耗".to_owned(),
        "account" => "账户".to_owned(),
        "balance" => "账户余额".to_owned(),
        other => other.to_owned(),
    }
}
