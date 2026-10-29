use chrono::{NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};

pub const MAX_TITLE: usize = 160;
pub const MAX_DESCRIPTION: usize = 4000;
pub const MAX_ACCEPTANCE: usize = 6;
pub const MAX_ACCEPTANCE_LEN: usize = 300;
pub const MAX_DECOMPOSITION_TASKS: usize = 50;
pub const MAX_DOCUMENT_CHARS: usize = 60_000;
pub const MAX_DOCUMENT_BYTES: u64 = 20 * 1024 * 1024;
pub const MIN_ESTIMATE_MINUTES: i64 = 30;
pub const MAX_SUGGEST_DAYS_AHEAD: i64 = 180;

pub fn is_status(value: &str) -> bool {
    matches!(value, "open" | "doing" | "done")
}

pub fn is_priority(value: &str) -> bool {
    matches!(value, "high" | "medium" | "low")
}

pub fn is_category(value: &str) -> bool {
    matches!(value, "work" | "planning" | "personal")
}

pub fn is_source(value: &str) -> bool {
    matches!(value, "manual" | "document" | "zentao")
}

pub fn is_date(value: &str) -> bool {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok()
}

pub fn is_time(value: &str) -> bool {
    NaiveTime::parse_from_str(value, "%H:%M").is_ok()
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptanceCriterion {
    pub id: String,
    pub content: String,
    pub sort_order: i64,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoTask {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: String,
    pub category: String,
    pub planned_date: Option<String>,
    pub planned_time: Option<String>,
    pub estimate_minutes: Option<i64>,
    pub source_type: String,
    pub source_name: Option<String>,
    /// 外部来源幂等键（如 zentao:task:123），手动/文档来源为空。
    pub external_id: Option<String>,
    pub external_type: Option<String>,
    pub external_url: Option<String>,
    pub synced_at: Option<i64>,
    pub external_status: Option<String>,
    pub external_due: Option<String>,
    pub external_updated_date: Option<String>,
    pub external_assigned_to: Option<String>,
    pub external_transferred: bool,
    pub acceptance_criteria: Vec<AcceptanceCriterion>,
    pub dependency_ids: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub completed_at: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoDraft {
    #[serde(default)]
    pub id: Option<String>,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    pub priority: String,
    pub category: String,
    #[serde(default)]
    pub planned_date: Option<String>,
    #[serde(default)]
    pub planned_time: Option<String>,
    #[serde(default)]
    pub estimate_minutes: Option<i64>,
    #[serde(default)]
    pub source_type: Option<String>,
    #[serde(default)]
    pub source_name: Option<String>,
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
    #[serde(default)]
    pub dependency_ids: Vec<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TodoFilter {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub source_type: Option<String>,
    /// 计划日期下界（含），YYYY-MM-DD。
    #[serde(default)]
    pub start: Option<String>,
    /// 计划日期上界（含），YYYY-MM-DD。
    #[serde(default)]
    pub end: Option<String>,
    /// 标题模糊搜索。
    #[serde(default)]
    pub query: Option<String>,
    /// 只返回有计划日期的待办（用于日历范围读取）。
    #[serde(default)]
    pub scheduled_only: bool,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEvent {
    pub id: String,
    pub task_id: Option<String>,
    pub title: String,
    pub category: String,
    pub start_at: String,
    pub end_at: Option<String>,
    pub all_day: bool,
    pub created_at: i64,
    pub updated_at: i64,
    /// 关联待办状态；普通事件为 None。
    pub status: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEventDraft {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub task_id: Option<String>,
    pub title: String,
    pub category: String,
    pub start_at: String,
    #[serde(default)]
    pub end_at: Option<String>,
    #[serde(default)]
    pub all_day: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveEventInput {
    pub id: String,
    #[serde(default)]
    pub start_at: Option<String>,
    #[serde(default)]
    pub end_at: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentContent {
    pub name: String,
    pub content: String,
    pub char_count: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecompositionInput {
    pub source_name: String,
    pub content: String,
    pub provider: String,
    pub model: String,
    pub api_key: String,
    pub current_date: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecompositionTask {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub priority: String,
    #[serde(default)]
    pub estimated_minutes: Option<i64>,
    #[serde(default)]
    pub suggested_date: Option<String>,
    #[serde(default)]
    pub suggested_time: Option<String>,
    #[serde(default)]
    pub dependency_ids: Vec<String>,
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecompositionGroup {
    pub id: String,
    pub title: String,
    pub tasks: Vec<DecompositionTask>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecompositionResult {
    pub summary: String,
    #[serde(default)]
    pub assumptions: Vec<String>,
    pub groups: Vec<DecompositionGroup>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitTask {
    pub temp_id: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub priority: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub planned_date: Option<String>,
    #[serde(default)]
    pub planned_time: Option<String>,
    #[serde(default)]
    pub estimate_minutes: Option<i64>,
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
    #[serde(default)]
    pub dependency_temp_ids: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitInput {
    pub source_name: String,
    pub tasks: Vec<CommitTask>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CommitResult {
    pub created_count: usize,
    pub task_ids: Vec<String>,
}
