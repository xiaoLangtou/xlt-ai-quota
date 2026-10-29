use crate::productivity::types::{
    is_category, is_date, is_priority, is_source, is_status, is_time, AcceptanceCriterion,
    CalendarEvent, CalendarEventDraft, CommitInput, CommitResult, MoveEventInput, TodoDraft,
    TodoFilter, TodoTask, MAX_ACCEPTANCE, MAX_ACCEPTANCE_LEN, MAX_DECOMPOSITION_TASKS,
    MAX_DESCRIPTION, MAX_TITLE, MIN_ESTIMATE_MINUTES,
};
use chrono::{Duration, NaiveDateTime, Utc};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Transaction};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

pub fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

struct CleanDraft {
    id: Option<String>,
    title: String,
    description: Option<String>,
    status: String,
    priority: String,
    category: String,
    planned_date: Option<String>,
    planned_time: Option<String>,
    estimate_minutes: Option<i64>,
    source_type: String,
    source_name: Option<String>,
    acceptance: Vec<String>,
    dependency_ids: Vec<String>,
}

fn optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

fn truncate_chars(value: &str, max: usize) -> String {
    value.chars().take(max).collect()
}

fn round_estimate(value: i64) -> Option<i64> {
    if value <= 0 {
        return None;
    }
    let steps = ((value as f64) / MIN_ESTIMATE_MINUTES as f64).round() as i64;
    Some(steps.max(1) * MIN_ESTIMATE_MINUTES)
}

/// 校验并归一化待办草稿。标题、优先级、分类必填；时间只允许与日期同时出现。
fn clean_draft(draft: TodoDraft) -> Result<CleanDraft, String> {
    let title = draft.title.trim().to_owned();
    if title.is_empty() {
        return Err("请输入待办标题".to_owned());
    }
    if title.chars().count() > MAX_TITLE {
        return Err(format!("待办标题不能超过 {MAX_TITLE} 个字符"));
    }
    let description = optional_text(draft.description);
    if let Some(text) = &description {
        if text.chars().count() > MAX_DESCRIPTION {
            return Err(format!("待办描述不能超过 {MAX_DESCRIPTION} 个字符"));
        }
    }
    let status = draft.status.unwrap_or_else(|| "open".to_owned());
    if !is_status(&status) {
        return Err("待办状态无效".to_owned());
    }
    if !is_priority(&draft.priority) {
        return Err("待办优先级无效".to_owned());
    }
    if !is_category(&draft.category) {
        return Err("待办分类无效".to_owned());
    }
    let planned_date = optional_text(draft.planned_date);
    if let Some(date) = &planned_date {
        if !is_date(date) {
            return Err("计划日期格式应为 YYYY-MM-DD".to_owned());
        }
    }
    let mut planned_time = optional_text(draft.planned_time);
    if planned_time.is_some() && planned_date.is_none() {
        planned_time = None;
    }
    if let Some(time) = &planned_time {
        if !is_time(time) {
            return Err("计划时间格式应为 HH:mm".to_owned());
        }
    }
    let estimate_minutes = draft
        .estimate_minutes
        .and_then(round_estimate)
        .filter(|_| planned_date.is_some());
    let source_type = draft.source_type.unwrap_or_else(|| "manual".to_owned());
    if !is_source(&source_type) {
        return Err("待办来源无效".to_owned());
    }
    let acceptance = draft
        .acceptance_criteria
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .take(MAX_ACCEPTANCE)
        .map(|value| truncate_chars(value, MAX_ACCEPTANCE_LEN))
        .collect::<Vec<_>>();
    let mut seen = HashSet::new();
    let dependency_ids = draft
        .dependency_ids
        .into_iter()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .filter(|value| seen.insert(value.clone()))
        .collect::<Vec<_>>();
    Ok(CleanDraft {
        id: draft.id.filter(|id| !id.trim().is_empty()),
        title,
        description,
        status,
        priority: draft.priority,
        category: draft.category,
        planned_date,
        planned_time,
        estimate_minutes,
        source_type,
        source_name: optional_text(draft.source_name),
        acceptance,
        dependency_ids,
    })
}

fn row_to_task(row: &rusqlite::Row<'_>) -> rusqlite::Result<TodoTask> {
    Ok(TodoTask {
        id: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        status: row.get(3)?,
        priority: row.get(4)?,
        category: row.get(5)?,
        planned_date: row.get(6)?,
        planned_time: row.get(7)?,
        estimate_minutes: row.get(8)?,
        source_type: row.get(9)?,
        source_name: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
        completed_at: row.get(13)?,
        external_id: row.get(14)?,
        external_type: row.get(15)?,
        external_url: row.get(16)?,
        synced_at: row.get(17)?,
        external_status: row.get(18)?,
        external_assigned_to: row.get(19)?,
        external_transferred: row.get(20)?,
        external_due: row.get(21)?,
        external_updated_date: row.get(22)?,
        acceptance_criteria: Vec::new(),
        dependency_ids: Vec::new(),
    })
}

const TASK_COLUMNS: &str = "id, title, description, status, priority, category, planned_date, planned_time, estimate_minutes, source_type, source_name, created_at, updated_at, completed_at, external_id, external_type, external_url, synced_at, external_status, external_assigned_to, external_transferred, external_due, external_updated_date";

fn load_acceptance(
    connection: &Connection,
) -> Result<HashMap<String, Vec<AcceptanceCriterion>>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, task_id, content, sort_order
             FROM todo_acceptance_criterion
             ORDER BY task_id, sort_order",
        )
        .map_err(|error| format!("无法读取验收标准: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,
                AcceptanceCriterion {
                    id: row.get(0)?,
                    content: row.get(2)?,
                    sort_order: row.get(3)?,
                },
            ))
        })
        .map_err(|error| format!("无法读取验收标准: {error}"))?;
    let mut map: HashMap<String, Vec<AcceptanceCriterion>> = HashMap::new();
    for row in rows {
        let (task_id, criterion) = row.map_err(|error| format!("无法读取验收标准: {error}"))?;
        map.entry(task_id).or_default().push(criterion);
    }
    Ok(map)
}

fn load_dependencies(connection: &Connection) -> Result<HashMap<String, Vec<String>>, String> {
    let mut statement = connection
        .prepare("SELECT task_id, depends_on_task_id FROM todo_dependency")
        .map_err(|error| format!("无法读取依赖关系: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| format!("无法读取依赖关系: {error}"))?;
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    for row in rows {
        let (task_id, dependency_id) = row.map_err(|error| format!("无法读取依赖关系: {error}"))?;
        map.entry(task_id).or_default().push(dependency_id);
    }
    Ok(map)
}

fn attach_relations(connection: &Connection, tasks: &mut [TodoTask]) -> Result<(), String> {
    if tasks.is_empty() {
        return Ok(());
    }
    let mut acceptance = load_acceptance(connection)?;
    let mut dependencies = load_dependencies(connection)?;
    for task in tasks.iter_mut() {
        task.acceptance_criteria = acceptance.remove(&task.id).unwrap_or_default();
        task.dependency_ids = dependencies.remove(&task.id).unwrap_or_default();
    }
    Ok(())
}

pub fn list_tasks(connection: &Connection, filter: TodoFilter) -> Result<Vec<TodoTask>, String> {
    let mut sql = format!("SELECT {TASK_COLUMNS} FROM todo_task WHERE 1 = 1");
    let mut values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(status) = filter.status.filter(|value| is_status(value)) {
        sql.push_str(" AND status = ?");
        values.push(Box::new(status));
    }
    if let Some(priority) = filter.priority.filter(|value| is_priority(value)) {
        sql.push_str(" AND priority = ?");
        values.push(Box::new(priority));
    }
    if let Some(category) = filter.category.filter(|value| is_category(value)) {
        sql.push_str(" AND category = ?");
        values.push(Box::new(category));
    }
    if let Some(source) = filter.source_type.filter(|value| is_source(value)) {
        sql.push_str(" AND source_type = ?");
        values.push(Box::new(source));
    }
    if let Some(start) = filter.start.filter(|value| is_date(value)) {
        sql.push_str(" AND planned_date IS NOT NULL AND planned_date >= ?");
        values.push(Box::new(start));
    }
    if let Some(end) = filter.end.filter(|value| is_date(value)) {
        sql.push_str(" AND planned_date IS NOT NULL AND planned_date <= ?");
        values.push(Box::new(end));
    }
    if filter.scheduled_only {
        sql.push_str(" AND planned_date IS NOT NULL");
    }
    if let Some(query) = optional_text(filter.query) {
        sql.push_str(" AND title LIKE ?");
        values.push(Box::new(format!("%{query}%")));
    }
    sql.push_str(
        " ORDER BY CASE WHEN planned_date IS NULL THEN 1 ELSE 0 END,
          planned_date ASC,
          CASE WHEN planned_time IS NULL THEN 1 ELSE 0 END,
          planned_time ASC,
          created_at ASC",
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("无法读取待办: {error}"))?;
    let rows = statement
        .query_map(params_from_iter(values.iter().map(|value| value.as_ref())), row_to_task)
        .map_err(|error| format!("无法读取待办: {error}"))?;
    let mut tasks = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("无法读取待办: {error}"))?;
    attach_relations(connection, &mut tasks)?;
    Ok(tasks)
}

pub fn read_task(connection: &Connection, id: &str) -> Result<TodoTask, String> {
    let mut task = connection
        .query_row(
            &format!("SELECT {TASK_COLUMNS} FROM todo_task WHERE id = ?1"),
            params![id],
            row_to_task,
        )
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => "待办不存在或已删除".to_owned(),
            _ => format!("无法读取待办: {error}"),
        })?;
    task.acceptance_criteria = connection
        .prepare(
            "SELECT id, content, sort_order FROM todo_acceptance_criterion
             WHERE task_id = ?1 ORDER BY sort_order",
        )
        .map_err(|error| format!("无法读取验收标准: {error}"))?
        .query_map(params![id], |row| {
            Ok(AcceptanceCriterion {
                id: row.get(0)?,
                content: row.get(1)?,
                sort_order: row.get(2)?,
            })
        })
        .map_err(|error| format!("无法读取验收标准: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("无法读取验收标准: {error}"))?;
    task.dependency_ids = connection
        .prepare("SELECT depends_on_task_id FROM todo_dependency WHERE task_id = ?1")
        .map_err(|error| format!("无法读取依赖关系: {error}"))?
        .query_map(params![id], |row| row.get::<_, String>(0))
        .map_err(|error| format!("无法读取依赖关系: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("无法读取依赖关系: {error}"))?;
    Ok(task)
}

fn task_exists(transaction: &Transaction<'_>, id: &str) -> Result<bool, String> {
    transaction
        .query_row(
            "SELECT 1 FROM todo_task WHERE id = ?1",
            params![id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map(|value| value.is_some())
        .map_err(|error| format!("无法读取待办: {error}"))
}

fn replace_acceptance(
    transaction: &Transaction<'_>,
    task_id: &str,
    acceptance: &[String],
) -> Result<(), String> {
    transaction
        .execute(
            "DELETE FROM todo_acceptance_criterion WHERE task_id = ?1",
            params![task_id],
        )
        .map_err(|error| format!("无法更新验收标准: {error}"))?;
    for (index, content) in acceptance.iter().enumerate() {
        transaction
            .execute(
                "INSERT INTO todo_acceptance_criterion (id, task_id, content, sort_order)
                 VALUES (?1, ?2, ?3, ?4)",
                params![Uuid::new_v4().to_string(), task_id, content, index as i64],
            )
            .map_err(|error| format!("无法更新验收标准: {error}"))?;
    }
    Ok(())
}

fn replace_dependencies(
    transaction: &Transaction<'_>,
    task_id: &str,
    dependency_ids: &[String],
) -> Result<(), String> {
    transaction
        .execute(
            "DELETE FROM todo_dependency WHERE task_id = ?1",
            params![task_id],
        )
        .map_err(|error| format!("无法更新依赖关系: {error}"))?;
    for dependency_id in dependency_ids {
        if dependency_id == task_id || !task_exists(transaction, dependency_id)? {
            continue;
        }
        transaction
            .execute(
                "INSERT OR IGNORE INTO todo_dependency (task_id, depends_on_task_id)
                 VALUES (?1, ?2)",
                params![task_id, dependency_id],
            )
            .map_err(|error| format!("无法更新依赖关系: {error}"))?;
    }
    Ok(())
}

fn end_at_for(date: &str, time: &str, estimate_minutes: Option<i64>) -> Option<String> {
    let start = NaiveDateTime::parse_from_str(&format!("{date} {time}"), "%Y-%m-%d %H:%M").ok()?;
    let minutes = estimate_minutes.unwrap_or(MIN_ESTIMATE_MINUTES).max(MIN_ESTIMATE_MINUTES);
    Some(
        (start + Duration::minutes(minutes))
            .format("%Y-%m-%d %H:%M")
            .to_string(),
    )
}

/// 按待办的计划日期 / 时间 / 预估工作量，创建、更新或删除其关联日历事件。
pub(crate) fn sync_task_event(transaction: &Transaction<'_>, task_id: &str) -> Result<(), String> {
    let row = transaction
        .query_row(
            "SELECT title, category, planned_date, planned_time, estimate_minutes
             FROM todo_task WHERE id = ?1",
            params![task_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                ))
            },
        )
        .map_err(|error| format!("无法同步日历事件: {error}"))?;
    let (title, category, date, time, estimate) = row;
    let Some(date) = date else {
        transaction
            .execute(
                "DELETE FROM calendar_event WHERE task_id = ?1",
                params![task_id],
            )
            .map_err(|error| format!("无法同步日历事件: {error}"))?;
        return Ok(());
    };
    let all_day = time.is_none();
    let start_at = match &time {
        Some(time) => format!("{date} {time}"),
        None => date.clone(),
    };
    let end_at = time
        .as_deref()
        .and_then(|time| end_at_for(&date, time, estimate));
    let now = now_ms();
    transaction
        .execute(
            "INSERT INTO calendar_event (id, task_id, title, category, start_at, end_at, all_day, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
             ON CONFLICT(task_id) DO UPDATE SET
               title = excluded.title,
               category = excluded.category,
               start_at = excluded.start_at,
               end_at = excluded.end_at,
               all_day = excluded.all_day,
               updated_at = excluded.updated_at",
            params![
                Uuid::new_v4().to_string(),
                task_id,
                title,
                category,
                start_at,
                end_at,
                all_day as i64,
                now
            ],
        )
        .map_err(|error| format!("无法同步日历事件: {error}"))?;
    Ok(())
}

pub fn save_task(connection: &mut Connection, draft: TodoDraft) -> Result<TodoTask, String> {
    let clean = clean_draft(draft)?;
    if let Some(id) = clean.id.as_deref() {
        let previous = read_task(connection, id)?;
        if previous.source_type == "zentao" &&
            (previous.status != clean.status || clean.source_type != "zentao") {
            return Err("禅道条目请在详情中执行指派、解决、开始或完成操作".to_owned());
        }
    }
    let transaction = connection
        .transaction()
        .map_err(|error| format!("无法保存待办: {error}"))?;
    let id = clean
        .id
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let previous = transaction
        .query_row(
            "SELECT status, completed_at FROM todo_task WHERE id = ?1",
            params![&id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<i64>>(1)?)),
        )
        .optional()
        .map_err(|error| format!("无法保存待办: {error}"))?;
    let now = now_ms();
    let completed_at = match clean.status.as_str() {
        "done" => match &previous {
            Some((_, Some(value))) => Some(*value),
            _ => Some(now),
        },
        _ => None,
    };
    match previous {
        Some(_) => {
            transaction
                .execute(
                    "UPDATE todo_task SET title = ?2, description = ?3, status = ?4, priority = ?5,
                        category = ?6, planned_date = ?7, planned_time = ?8, estimate_minutes = ?9,
                        source_type = ?10, source_name = ?11, updated_at = ?12, completed_at = ?13
                     WHERE id = ?1",
                    params![
                        &id,
                        &clean.title,
                        &clean.description,
                        &clean.status,
                        &clean.priority,
                        &clean.category,
                        &clean.planned_date,
                        &clean.planned_time,
                        &clean.estimate_minutes,
                        &clean.source_type,
                        &clean.source_name,
                        now,
                        &completed_at
                    ],
                )
                .map_err(|error| format!("无法保存待办: {error}"))?;
        }
        None => {
            transaction
                .execute(
                    "INSERT INTO todo_task (id, title, description, status, priority, category,
                        planned_date, planned_time, estimate_minutes, source_type, source_name,
                        created_at, updated_at, completed_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12, ?13)",
                    params![
                        &id,
                        &clean.title,
                        &clean.description,
                        &clean.status,
                        &clean.priority,
                        &clean.category,
                        &clean.planned_date,
                        &clean.planned_time,
                        &clean.estimate_minutes,
                        &clean.source_type,
                        &clean.source_name,
                        now,
                        &completed_at
                    ],
                )
                .map_err(|error| format!("无法保存待办: {error}"))?;
        }
    }
    replace_acceptance(&transaction, &id, &clean.acceptance)?;
    replace_dependencies(&transaction, &id, &clean.dependency_ids)?;
    sync_task_event(&transaction, &id)?;
    transaction
        .commit()
        .map_err(|error| format!("无法保存待办: {error}"))?;
    read_task(connection, &id)
}

pub fn set_task_status(
    connection: &mut Connection,
    id: &str,
    status: &str,
) -> Result<TodoTask, String> {
    if !is_status(status) {
        return Err("待办状态无效".to_owned());
    }
    if read_task(connection, id)?.source_type == "zentao" {
        return Err("禅道条目请在详情中执行指派、解决、开始或完成操作".to_owned());
    }
    let now = now_ms();
    let completed_at = (status == "done").then_some(now);
    let affected = connection
        .execute(
            "UPDATE todo_task SET status = ?2, completed_at = ?3, updated_at = ?4 WHERE id = ?1",
            params![id, status, &completed_at, now],
        )
        .map_err(|error| format!("无法更新待办状态: {error}"))?;
    if affected == 0 {
        return Err("待办不存在或已删除".to_owned());
    }
    read_task(connection, id)
}

pub fn delete_task(connection: &Connection, id: &str) -> Result<(), String> {
    let affected = connection
        .execute("DELETE FROM todo_task WHERE id = ?1", params![id])
        .map_err(|error| format!("无法删除待办: {error}"))?;
    if affected == 0 {
        return Err("待办不存在或已删除".to_owned());
    }
    Ok(())
}

fn row_to_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<CalendarEvent> {
    Ok(CalendarEvent {
        id: row.get(0)?,
        task_id: row.get(1)?,
        title: row.get(2)?,
        category: row.get(3)?,
        start_at: row.get(4)?,
        end_at: row.get(5)?,
        all_day: row.get::<_, i64>(6)? != 0,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        status: row.get(9)?,
    })
}

const EVENT_COLUMNS: &str = "e.id, e.task_id, e.title, e.category, e.start_at, e.end_at, e.all_day, e.created_at, e.updated_at, t.status";

pub fn list_events(
    connection: &Connection,
    start: &str,
    end: &str,
) -> Result<Vec<CalendarEvent>, String> {
    if !is_date(start) || !is_date(end) {
        return Err("日期范围格式应为 YYYY-MM-DD".to_owned());
    }
    let mut statement = connection
        .prepare(&format!(
            "SELECT {EVENT_COLUMNS}
             FROM calendar_event e
             LEFT JOIN todo_task t ON t.id = e.task_id
             WHERE substr(e.start_at, 1, 10) <= ?2
               AND COALESCE(substr(e.end_at, 1, 10), substr(e.start_at, 1, 10)) >= ?1
             ORDER BY e.start_at ASC, e.all_day DESC"
        ))
        .map_err(|error| format!("无法读取日历事件: {error}"))?;
    let events = statement
        .query_map(params![start, end], row_to_event)
        .map_err(|error| format!("无法读取日历事件: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("无法读取日历事件: {error}"));
    events
}

pub fn read_event(connection: &Connection, id: &str) -> Result<CalendarEvent, String> {
    connection
        .query_row(
            &format!(
                "SELECT {EVENT_COLUMNS}
                 FROM calendar_event e
                 LEFT JOIN todo_task t ON t.id = e.task_id
                 WHERE e.id = ?1"
            ),
            params![id],
            row_to_event,
        )
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => "日历事件不存在或已删除".to_owned(),
            _ => format!("无法读取日历事件: {error}"),
        })
}

fn split_start_at(value: &str) -> Result<(String, Option<String>), String> {
    let value = value.trim();
    if value.len() == 10 {
        if !is_date(value) {
            return Err("事件日期格式应为 YYYY-MM-DD".to_owned());
        }
        Ok((value.to_owned(), None))
    } else {
        let parsed = NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M")
            .map_err(|_| "事件时间格式应为 YYYY-MM-DD HH:mm".to_owned())?;
        Ok((
            parsed.date().format("%Y-%m-%d").to_string(),
            Some(parsed.time().format("%H:%M").to_string()),
        ))
    }
}

fn duration_minutes(start_at: &str, end_at: Option<&str>) -> Option<i64> {
    let (_, time) = split_start_at(start_at).ok()?;
    let time = time?;
    let end = end_at?;
    let start = NaiveDateTime::parse_from_str(&format!("{} {}", &start_at[..10], time), "%Y-%m-%d %H:%M").ok()?;
    let end = NaiveDateTime::parse_from_str(end, "%Y-%m-%d %H:%M").ok()?;
    let minutes = (end - start).num_minutes();
    (minutes > 0).then_some(minutes)
}

/// 把事件的日期 / 时间和时长同步回关联待办，保持日历与待办一致。
fn apply_schedule_to_task(
    transaction: &Transaction<'_>,
    task_id: &str,
    start_at: &str,
    end_at: Option<&str>,
) -> Result<(), String> {
    let (date, time) = split_start_at(start_at)?;
    let estimate = duration_minutes(start_at, end_at).and_then(round_estimate);
    transaction
        .execute(
            "UPDATE todo_task SET planned_date = ?2, planned_time = ?3,
                estimate_minutes = COALESCE(?4, estimate_minutes), updated_at = ?5
             WHERE id = ?1",
            params![task_id, date, time, estimate, now_ms()],
        )
        .map_err(|error| format!("无法同步待办计划: {error}"))?;
    Ok(())
}

pub fn save_event(
    connection: &mut Connection,
    draft: CalendarEventDraft,
) -> Result<CalendarEvent, String> {
    let title = draft.title.trim().to_owned();
    if title.is_empty() {
        return Err("请输入事件标题".to_owned());
    }
    if title.chars().count() > MAX_TITLE {
        return Err(format!("事件标题不能超过 {MAX_TITLE} 个字符"));
    }
    if !is_category(&draft.category) {
        return Err("事件分类无效".to_owned());
    }
    let (_, time) = split_start_at(&draft.start_at)?;
    let all_day = draft.all_day || time.is_none();
    let start_at = if all_day {
        draft.start_at.trim().chars().take(10).collect::<String>()
    } else {
        draft.start_at.trim().to_owned()
    };
    let end_at = if all_day {
        None
    } else {
        draft
            .end_at
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    };
    let transaction = connection
        .transaction()
        .map_err(|error| format!("无法保存日历事件: {error}"))?;
    if let Some(task_id) = draft.task_id.filter(|value| !value.trim().is_empty()) {
        if !task_exists(&transaction, &task_id)? {
            return Err("关联待办不存在或已删除".to_owned());
        }
        apply_schedule_to_task(&transaction, &task_id, &start_at, end_at.as_deref())?;
        sync_task_event(&transaction, &task_id)?;
        transaction
            .commit()
            .map_err(|error| format!("无法保存日历事件: {error}"))?;
        return read_event(connection, &task_event_id(connection, &task_id)?);
    }
    let now = now_ms();
    let id = draft
        .id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    transaction
        .execute(
            "INSERT INTO calendar_event (id, task_id, title, category, start_at, end_at, all_day, created_at, updated_at)
             VALUES (?1, NULL, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
             ON CONFLICT(id) DO UPDATE SET
               title = excluded.title,
               category = excluded.category,
               start_at = excluded.start_at,
               end_at = excluded.end_at,
               all_day = excluded.all_day,
               updated_at = excluded.updated_at",
            params![&id, title, &draft.category, start_at, end_at, all_day as i64, now],
        )
        .map_err(|error| format!("无法保存日历事件: {error}"))?;
    transaction
        .commit()
        .map_err(|error| format!("无法保存日历事件: {error}"))?;
    read_event(connection, &id)
}

fn task_event_id(connection: &Connection, task_id: &str) -> Result<String, String> {
    connection
        .query_row(
            "SELECT id FROM calendar_event WHERE task_id = ?1",
            params![task_id],
            |row| row.get::<_, String>(0),
        )
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => "关联日历事件不存在".to_owned(),
            _ => format!("无法读取日历事件: {error}"),
        })
}

pub fn move_event(
    connection: &mut Connection,
    input: MoveEventInput,
) -> Result<CalendarEvent, String> {
    let event = read_event(connection, &input.id)?;
    let start_at = input
        .start_at
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(event.start_at.as_str())
        .to_owned();
    let (_, time) = split_start_at(&start_at)?;
    let all_day = time.is_none();
    let end_at = if all_day {
        None
    } else {
        input
            .end_at
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| event.end_at.clone())
    };
    let transaction = connection
        .transaction()
        .map_err(|error| format!("无法移动日历事件: {error}"))?;
    if let Some(task_id) = event.task_id {
        apply_schedule_to_task(&transaction, &task_id, &start_at, end_at.as_deref())?;
        sync_task_event(&transaction, &task_id)?;
    } else {
        transaction
            .execute(
                "UPDATE calendar_event SET start_at = ?2, end_at = ?3, all_day = ?4, updated_at = ?5
                 WHERE id = ?1",
                params![&input.id, &start_at, &end_at, all_day as i64, now_ms()],
            )
            .map_err(|error| format!("无法移动日历事件: {error}"))?;
    }
    transaction
        .commit()
        .map_err(|error| format!("无法移动日历事件: {error}"))?;
    read_event(connection, &input.id)
}

/// 删除普通日历事件；关联待办的事件必须通过删除待办来处理，避免留下孤立待办。
pub fn delete_event(connection: &Connection, id: &str) -> Result<(), String> {
    let affected = connection
        .execute(
            "DELETE FROM calendar_event WHERE id = ?1 AND task_id IS NULL",
            params![id],
        )
        .map_err(|error| format!("无法删除日历事件: {error}"))?;
    if affected > 0 {
        return Ok(());
    }
    let linked = connection
        .query_row(
            "SELECT task_id FROM calendar_event WHERE id = ?1",
            params![id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(|error| format!("无法删除日历事件: {error}"))?;
    match linked {
        Some(Some(_)) => Err("该事件关联待办，请删除对应待办".to_owned()),
        _ => Err("日历事件不存在或已删除".to_owned()),
    }
}

pub fn commit_decomposition(
    connection: &mut Connection,
    input: CommitInput,
) -> Result<CommitResult, String> {
    let tasks = input.tasks;
    if tasks.is_empty() {
        return Err("请至少选择一条待办".to_owned());
    }
    if tasks.len() > MAX_DECOMPOSITION_TASKS {
        return Err(format!("一次最多创建 {MAX_DECOMPOSITION_TASKS} 条待办"));
    }
    let source_name = optional_text(Some(input.source_name));
    let transaction = connection
        .transaction()
        .map_err(|error| format!("无法创建待办: {error}"))?;
    let mut id_map: HashMap<String, String> = HashMap::new();
    let mut prepared: Vec<(String, CleanDraft, Vec<String>)> = Vec::new();
    for (index, task) in tasks.into_iter().enumerate() {
        let temp_id = if task.temp_id.trim().is_empty() {
            format!("task-{index}")
        } else {
            task.temp_id.clone()
        };
        if id_map.contains_key(&temp_id) {
            return Err("拆解结果包含重复的任务标识".to_owned());
        }
        let dependency_temp_ids = task.dependency_temp_ids.clone();
        let clean = clean_draft(TodoDraft {
            id: None,
            title: task.title,
            description: task.description,
            status: Some("open".to_owned()),
            priority: task.priority,
            category: task.category.unwrap_or_else(|| "work".to_owned()),
            planned_date: task.planned_date,
            planned_time: task.planned_time,
            estimate_minutes: task.estimate_minutes,
            source_type: Some("document".to_owned()),
            source_name: source_name.clone(),
            acceptance_criteria: task.acceptance_criteria,
            dependency_ids: Vec::new(),
        })?;
        let id = Uuid::new_v4().to_string();
        id_map.insert(temp_id, id.clone());
        prepared.push((id, clean, dependency_temp_ids));
    }
    let now = now_ms();
    for (id, clean, dependency_temp_ids) in prepared {
        transaction
            .execute(
                "INSERT INTO todo_task (id, title, description, status, priority, category,
                    planned_date, planned_time, estimate_minutes, source_type, source_name,
                    created_at, updated_at, completed_at)
                 VALUES (?1, ?2, ?3, 'open', ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11, NULL)",
                params![
                    &id,
                    &clean.title,
                    &clean.description,
                    &clean.priority,
                    &clean.category,
                    &clean.planned_date,
                    &clean.planned_time,
                    &clean.estimate_minutes,
                    &clean.source_type,
                    &clean.source_name,
                    now
                ],
            )
            .map_err(|error| format!("无法创建待办: {error}"))?;
        replace_acceptance(&transaction, &id, &clean.acceptance)?;
        let dependency_ids = dependency_temp_ids
            .iter()
            .filter_map(|temp_id| id_map.get(temp_id))
            .filter(|dependency_id| **dependency_id != id)
            .cloned()
            .collect::<Vec<_>>();
        replace_dependencies(&transaction, &id, &dependency_ids)?;
        sync_task_event(&transaction, &id)?;
    }
    transaction
        .commit()
        .map_err(|error| format!("无法创建待办: {error}"))?;
    let mut task_ids = id_map.into_values().collect::<Vec<_>>();
    task_ids.sort();
    Ok(CommitResult {
        created_count: task_ids.len(),
        task_ids,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        clean_draft, commit_decomposition, delete_event, delete_task, list_events, list_tasks,
        move_event, round_estimate, save_event, save_task, set_task_status,
    };
    use crate::database;
    use crate::productivity::types::{
        CalendarEventDraft, CommitInput, CommitTask, MoveEventInput, TodoDraft, TodoFilter,
    };
    use rusqlite::Connection;

    fn connection() -> Connection {
        let connection = Connection::open_in_memory().expect("创建内存数据库");
        database::initialize(&connection).expect("初始化数据库");
        connection
    }

    fn draft(title: &str, date: Option<&str>, time: Option<&str>, estimate: Option<i64>) -> TodoDraft {
        TodoDraft {
            id: None,
            title: title.to_owned(),
            description: Some("  说明  ".to_owned()),
            status: None,
            priority: "medium".to_owned(),
            category: "work".to_owned(),
            planned_date: date.map(ToOwned::to_owned),
            planned_time: time.map(ToOwned::to_owned),
            estimate_minutes: estimate,
            source_type: None,
            source_name: None,
            acceptance_criteria: vec!["  可见  ".to_owned(), "  ".to_owned()],
            dependency_ids: vec!["a".to_owned(), "a".to_owned(), "".to_owned()],
        }
    }

    #[test]
    fn normalizes_title_description_and_relations() {
        let clean = clean_draft(draft("  实现日历  ", Some("2026-09-29"), Some("09:00"), Some(45)))
            .expect("清理草稿");
        assert_eq!(clean.title, "实现日历");
        assert_eq!(clean.description.as_deref(), Some("说明"));
        assert_eq!(clean.status, "open");
        assert_eq!(clean.source_type, "manual");
        assert_eq!(clean.acceptance, vec!["可见"]);
        assert_eq!(clean.dependency_ids, vec!["a"]);
        assert_eq!(clean.estimate_minutes, Some(60));
    }

    #[test]
    fn drops_time_when_date_is_missing() {
        let clean = clean_draft(draft("仅时间", None, Some("09:00"), Some(60))).expect("清理草稿");
        assert_eq!(clean.planned_time, None);
        assert_eq!(clean.estimate_minutes, None);
    }

    #[test]
    fn rejects_blank_title_and_bad_priority() {
        assert!(clean_draft(draft("   ", None, None, None)).is_err());
        let mut value = draft("优先级", None, None, None);
        value.priority = "urgent".to_owned();
        assert!(clean_draft(value).is_err());
    }

    #[test]
    fn rounds_estimate_to_half_hour_steps() {
        assert_eq!(round_estimate(0), None);
        assert_eq!(round_estimate(20), Some(30));
        assert_eq!(round_estimate(45), Some(60));
        assert_eq!(round_estimate(90), Some(90));
    }

    #[test]
    fn saving_task_syncs_calendar_event_and_status() {
        let mut connection = connection();
        let task = save_task(
            &mut connection,
            draft("实现日历", Some("2026-09-29"), Some("09:00"), Some(60)),
        )
        .expect("保存待办");
        assert_eq!(task.acceptance_criteria.len(), 1);

        let events = list_events(&connection, "2026-09-29", "2026-09-29").expect("读取事件");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].start_at, "2026-09-29 09:00");
        assert_eq!(events[0].end_at.as_deref(), Some("2026-09-29 10:00"));
        assert_eq!(events[0].task_id.as_deref(), Some(task.id.as_str()));
        assert_eq!(events[0].status.as_deref(), Some("open"));

        set_task_status(&mut connection, &task.id, "done").expect("更新状态");
        let events = list_events(&connection, "2026-09-29", "2026-09-29").expect("读取事件");
        assert_eq!(events[0].status.as_deref(), Some("done"));

        move_event(
            &mut connection,
            MoveEventInput {
                id: events[0].id.clone(),
                start_at: Some("2026-09-30 14:00".to_owned()),
                end_at: Some("2026-09-30 15:30".to_owned()),
            },
        )
        .expect("移动事件");
        let task = list_tasks(&connection, TodoFilter::default()).expect("读取待办");
        assert_eq!(task[0].planned_date.as_deref(), Some("2026-09-30"));
        assert_eq!(task[0].planned_time.as_deref(), Some("14:00"));
        assert_eq!(task[0].estimate_minutes, Some(90));

        delete_task(&connection, &task[0].id).expect("删除待办");
        assert!(list_events(&connection, "2026-09-25", "2026-10-05")
            .expect("读取事件")
            .is_empty());
    }

    #[test]
    fn deleting_task_removes_dependency_rows() {
        let mut connection = connection();
        let first = save_task(&mut connection, draft("前置", Some("2026-09-29"), None, None))
            .expect("保存待办");
        let mut dependent = draft("后置", Some("2026-09-30"), None, None);
        dependent.dependency_ids = vec![first.id.clone()];
        let second = save_task(&mut connection, dependent).expect("保存待办");
        assert_eq!(second.dependency_ids, vec![first.id.clone()]);

        delete_task(&connection, &first.id).expect("删除前置任务");
        let remaining = list_tasks(&connection, TodoFilter::default()).expect("读取待办");
        assert_eq!(remaining.len(), 1);
        assert!(remaining[0].dependency_ids.is_empty());
    }

    #[test]
    fn commits_decomposition_atomically_with_dependencies() {
        let mut connection = connection();
        let result = commit_decomposition(
            &mut connection,
            CommitInput {
                source_name: "需求.md".to_owned(),
                tasks: vec![
                    CommitTask {
                        temp_id: "t1".to_owned(),
                        title: "定义数据模型".to_owned(),
                        description: Some("覆盖全天事件".to_owned()),
                        priority: "high".to_owned(),
                        category: Some("work".to_owned()),
                        planned_date: Some("2026-09-29".to_owned()),
                        planned_time: None,
                        estimate_minutes: Some(60),
                        acceptance_criteria: vec!["可验证".to_owned()],
                        dependency_temp_ids: vec![],
                    },
                    CommitTask {
                        temp_id: "t2".to_owned(),
                        title: "实现月视图".to_owned(),
                        description: None,
                        priority: "medium".to_owned(),
                        category: None,
                        planned_date: Some("2026-09-30".to_owned()),
                        planned_time: Some("10:00".to_owned()),
                        estimate_minutes: None,
                        acceptance_criteria: vec![],
                        dependency_temp_ids: vec!["t1".to_owned(), "missing".to_owned()],
                    },
                ],
            },
        )
        .expect("提交拆解结果");
        assert_eq!(result.created_count, 2);

        let tasks = list_tasks(&connection, TodoFilter::default()).expect("读取待办");
        assert_eq!(tasks.len(), 2);
        assert!(tasks.iter().all(|task| task.source_type == "document"));
        let dependent = tasks.iter().find(|task| task.title == "实现月视图").unwrap();
        assert_eq!(dependent.dependency_ids.len(), 1);
        assert_eq!(
            dependent.dependency_ids[0],
            tasks.iter().find(|task| task.title == "定义数据模型").unwrap().id,
        );
        let events = list_events(&connection, "2026-09-29", "2026-09-30").expect("读取事件");
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn deletes_plain_event_but_blocks_linked_task_event() {
        let mut connection = connection();
        let plain = save_event(
            &mut connection,
            CalendarEventDraft {
                id: None,
                task_id: None,
                title: "团队同步".to_owned(),
                category: "work".to_owned(),
                start_at: "2026-09-29 13:00".to_owned(),
                end_at: Some("2026-09-29 14:00".to_owned()),
                all_day: false,
            },
        )
        .expect("保存普通事件");
        delete_event(&connection, &plain.id).expect("删除普通事件");
        assert!(list_events(&connection, "2026-09-25", "2026-10-05")
            .expect("读取事件")
            .is_empty());

        let task = save_task(
            &mut connection,
            draft("实现日历", Some("2026-09-29"), Some("09:00"), Some(60)),
        )
        .expect("保存待办");
        let linked = list_events(&connection, "2026-09-29", "2026-09-29").expect("读取事件");
        assert!(delete_event(&connection, &linked[0].id).is_err());
        assert_eq!(linked[0].task_id.as_deref(), Some(task.id.as_str()));
    }

    #[test]
    fn rejects_duplicate_task_ids_without_writing_anything() {
        let mut connection = connection();
        let task = |temp_id: &str| CommitTask {
            temp_id: temp_id.to_owned(),
            title: "重复".to_owned(),
            description: None,
            priority: "medium".to_owned(),
            category: None,
            planned_date: None,
            planned_time: None,
            estimate_minutes: None,
            acceptance_criteria: vec![],
            dependency_temp_ids: vec![],
        };
        let error = commit_decomposition(
            &mut connection,
            CommitInput {
                source_name: "粘贴内容".to_owned(),
                tasks: vec![task("t1"), task("t1")],
            },
        )
        .unwrap_err();
        assert!(error.contains("重复"));
        assert!(list_tasks(&connection, TodoFilter::default())
            .expect("读取待办")
            .is_empty());
    }
}

