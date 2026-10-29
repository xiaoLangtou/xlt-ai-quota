use chrono::{Duration, Utc};

use super::{config, queue};
use super::types::CacheStats;

/// 定期清理：距上次清理超过 24 小时才执行，避免每次同步都扫目录。
pub fn cleanup_if_due() -> Result<Option<Vec<String>>, String> {
    let mut cfg = config::load();
    let now = Utc::now();
    if let Some(last) = cfg.last_cleanup_at.as_deref() {
        if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(last) {
            if now - parsed.with_timezone(&Utc) < Duration::hours(24) {
                return Ok(None);
            }
        }
    }
    let retention = cfg.retention_days.max(7) as i64;
    let cutoff = now - Duration::days(retention);
    let removed = queue::delete_before(cutoff)?;
    cfg.last_cleanup_at = Some(queue::now_iso());
    config::save(&cfg)?;
    Ok(Some(removed))
}

pub fn cleanup(retention_days: u32) -> Result<CacheStats, String> {
    let retention = retention_days.max(7) as i64;
    let cutoff = Utc::now() - Duration::days(retention);
    queue::delete_before(cutoff)?;
    let mut cfg = config::load();
    cfg.retention_days = retention_days;
    cfg.last_cleanup_at = Some(queue::now_iso());
    config::save(&cfg)?;
    Ok(queue::cache_stats())
}

pub fn clear_all() -> Result<CacheStats, String> {
    queue::clear_all()?;
    Ok(queue::cache_stats())
}

pub fn stats() -> CacheStats {
    let mut stats = queue::cache_stats();
    stats.cursor_sources = super::cursors::sources();
    stats
}
