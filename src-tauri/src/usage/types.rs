use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

fn default_project() -> String {
    "unknown".to_owned()
}

fn default_manifest_version() -> u32 {
    1
}

/// 一条半小时用量桶。`hour_start` 统一为 UTC RFC3339，展示时再按用户时区落日期。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QueueBucket {
    pub hour_start: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collector: Option<String>,
    #[serde(default = "default_project")]
    pub project: String,
    pub model: String,
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cached_input_tokens: u64,
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
    #[serde(default)]
    pub reasoning_output_tokens: u64,
    #[serde(default)]
    pub total_tokens: u64,
    #[serde(default)]
    pub conversation_count: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reported_cost_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_metrics: Option<LocalMetrics>,
}

impl QueueBucket {
    pub fn key(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}",
            self.source,
            self.collector.as_deref().unwrap_or(""),
            self.model,
            self.project,
            self.hour_start
        )
    }

    pub fn recompute_total(&mut self) {
        self.total_tokens = self.input_tokens
            + self.output_tokens
            + self.cached_input_tokens
            + self.cache_creation_input_tokens
            + self.reasoning_output_tokens;
    }
}

/// 仅本地保存的请求/缓存证据，不参与云端同步。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LocalMetrics {
    #[serde(default)]
    pub request_count: u64,
    #[serde(default)]
    pub cache_hit_tokens: u64,
    #[serde(default)]
    pub cache_miss_tokens: u64,
    #[serde(default)]
    pub request_count_complete: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonthMeta {
    #[serde(default)]
    pub rows: u64,
    #[serde(default)]
    pub bytes: u64,
    #[serde(default)]
    pub first_hour: Option<String>,
    #[serde(default)]
    pub last_hour: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    #[serde(default = "default_manifest_version")]
    pub version: u32,
    #[serde(default)]
    pub months: BTreeMap<String, MonthMeta>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            version: 1,
            months: BTreeMap::new(),
            updated_at: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cursors {
    #[serde(default = "default_manifest_version")]
    pub version: u32,
    #[serde(default)]
    pub sources: BTreeMap<String, serde_json::Value>,
}

/// 额度读取状态机。前端据此决定展示额度、空态还是操作提示。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum QuotaStatus {
    Ready,
    NotInstalled,
    NotSignedIn,
    #[default]
    Unavailable,
    Expired,
    AccessDenied,
    CustomProvider,
    UnsupportedAccount,
    UnsupportedPlan,
    UnsupportedVersion,
    TemporarilyUnavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QuotaWindow {
    pub id: String,
    pub label: String,
    pub used_percent: f64,
    #[serde(default)]
    pub resets_at: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub remaining: Option<f64>,
    #[serde(default)]
    pub total: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QuotaBalance {
    pub total: f64,
    pub currency: String,
    #[serde(default)]
    pub granted: f64,
    #[serde(default)]
    pub topped_up: f64,
    #[serde(default)]
    pub available: bool,
}

/// 三类快照（命名窗口 / 模型池 / 额度列表）共用一个结构，按字段存在性区分。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QuotaSnapshot {
    pub channel: String,
    #[serde(default)]
    pub account: Option<String>,
    #[serde(default)]
    pub status: QuotaStatus,
    #[serde(default)]
    pub plan_label: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub limits: Vec<QuotaWindow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub five_hour: Option<QuotaWindow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weekly: Option<QuotaWindow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seven_day: Option<QuotaWindow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_models: Option<QuotaWindow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub other_models: Option<QuotaWindow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<QuotaWindow>,
    #[serde(default)]
    pub fetched_at: Option<String>,
    #[serde(default)]
    pub stale: bool,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balance: Option<QuotaBalance>,
}

impl QuotaSnapshot {
    pub fn store_key(&self) -> String {
        format!("{}|{}", self.channel, self.account.as_deref().unwrap_or(""))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaStoreFile {
    #[serde(default = "default_manifest_version")]
    pub version: u32,
    #[serde(default)]
    pub snapshots: BTreeMap<String, QuotaSnapshot>,
}

impl Default for QuotaStoreFile {
    fn default() -> Self {
        Self {
            version: 1,
            snapshots: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PricingRates {
    #[serde(default)]
    pub input: f64,
    #[serde(default)]
    pub output: f64,
    #[serde(default)]
    pub cache_read: f64,
    #[serde(default)]
    pub cache_write: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FuzzyRule {
    pub r#match: String,
    pub r#ref: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PricingData {
    #[serde(default)]
    pub exact: BTreeMap<String, PricingRates>,
    #[serde(default)]
    pub alias: BTreeMap<String, String>,
    #[serde(default)]
    pub fuzzy: Vec<FuzzyRule>,
    #[serde(default)]
    pub source_alias: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(default)]
    pub default: Option<PricingRates>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PricingQuery {
    pub model: String,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PricingHit {
    pub model: String,
    pub rates: PricingRates,
    /// 是否命中内置/覆盖表；false 表示走 default 兜底。
    pub matched: bool,
    /// 命中的表名（exact / alias / fuzzy / sourceAlias / default）。
    pub via: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PricingStatus {
    pub has_overlay: bool,
    pub url: Option<String>,
    pub updated_at: Option<String>,
    pub exact_count: usize,
    pub alias_count: usize,
    pub fuzzy_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageConfig {
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub stats_since: Option<String>,
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
    #[serde(default)]
    pub pricing_url: Option<String>,
    #[serde(default)]
    pub last_cleanup_at: Option<String>,
}

fn default_retention_days() -> u32 {
    180
}

impl Default for UsageConfig {
    fn default() -> Self {
        Self {
            timezone: None,
            stats_since: None,
            retention_days: default_retention_days(),
            pricing_url: Some(
                "https://api.juejin.cn/aiusage_api/functions/tud-pricing".to_owned(),
            ),
            last_cleanup_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonthStats {
    pub month: String,
    pub rows: u64,
    pub bytes: u64,
    pub first_hour: Option<String>,
    pub last_hour: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheStats {
    pub data_dir: String,
    pub total_rows: u64,
    pub total_bytes: u64,
    pub oldest_hour: Option<String>,
    pub newest_hour: Option<String>,
    pub months: Vec<MonthStats>,
    pub cursor_sources: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyTokenRow {
    pub platform: String,
    pub date: String,
    /// 旧数据没有小时粒度；前端可传入按本地中午换算好的 UTC 时刻。
    #[serde(default)]
    pub hour_start: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cached_tokens: u64,
    #[serde(default)]
    pub cache_write_tokens: u64,
    #[serde(default)]
    pub reasoning_tokens: u64,
    #[serde(default)]
    pub request_count: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyQuotaRow {
    pub platform: String,
    #[serde(default)]
    pub account_name: Option<String>,
    #[serde(default)]
    pub metric: Option<String>,
    #[serde(default)]
    pub used: f64,
    #[serde(default)]
    pub limit: f64,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub resets_at: Option<String>,
    #[serde(default)]
    pub collected_at: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyImport {
    #[serde(default)]
    pub tokens: Vec<LegacyTokenRow>,
    #[serde(default)]
    pub quotas: Vec<LegacyQuotaRow>,
    #[serde(default)]
    pub bills: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub tokens: usize,
    pub quotas: usize,
    pub bills: bool,
    pub skipped: bool,
}
