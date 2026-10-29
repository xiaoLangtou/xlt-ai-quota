use crate::ai_provider::{self, ChatChunk, ChatRequest};
use crate::productivity::types::{
    is_date, is_priority, is_time, DecompositionInput, DecompositionResult, DecompositionTask,
    MAX_ACCEPTANCE, MAX_ACCEPTANCE_LEN, MAX_DECOMPOSITION_TASKS, MAX_DESCRIPTION,
    MAX_DOCUMENT_CHARS, MAX_SUGGEST_DAYS_AHEAD, MAX_TITLE, MIN_ESTIMATE_MINUTES,
};
use chrono::{Duration, NaiveDate};
use serde_json::Value;
use std::collections::HashSet;

const SYSTEM_PROMPT: &str =
    "你是资深技术项目经理，负责把需求文档拆解成可执行的开发待办。只输出符合要求的 JSON，不要输出 Markdown 代码块或额外解释。";

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

fn build_prompt(input: &DecompositionInput, max_date: &str) -> String {
    format!(
        "请把下面的文档拆解成可独立确认和完成的开发待办。\n\
         只输出一个 JSON 对象，不要输出任何解释文字或 Markdown 代码块。JSON 结构如下：\n\
         {{\n\
           \"summary\": \"一句话概述\",\n\
           \"assumptions\": [\"识别出的前提假设\"],\n\
           \"groups\": [\n\
             {{ \"id\": \"g1\", \"title\": \"功能模块名\", \"tasks\": [\n\
               {{ \"id\": \"t1\", \"title\": \"动词开头的任务标题\", \"description\": \"实现要点\", \"priority\": \"high|medium|low\",\n\
                  \"estimatedMinutes\": 60, \"suggestedDate\": \"YYYY-MM-DD 或 null\", \"suggestedTime\": \"HH:mm 或 null\",\n\
                  \"dependencyIds\": [\"t0\"], \"acceptanceCriteria\": [\"可观察的验收标准\"] }}\n\
             ]}}\n\
           ]\n\
         }}\n\
         规则：\n\
         - 每条任务必须可以独立确认和完成，标题用中文动词开头并明确产出。\n\
         - 任务按功能模块分组。\n\
         - 最多 {max_tasks} 条任务。\n\
         - estimatedMinutes 为 {step} 的整数倍；无法估算时为 null。\n\
         - suggestedDate 不早于 {current} 且不超过 {max_date}；无法安排时为 null。\n\
         - dependencyIds 只能引用本结果内已经存在的任务 id，不能引用自身。\n\
         - acceptanceCriteria 最多 {max_acceptance} 条，每条不超过 {max_acceptance_len} 字，必须可观察、可验证。\n\
         - 控制输出长度：description 不超过 60 字，acceptanceCriteria 建议 1–3 条、每条 40 字以内。\n\
         - 必须输出完整且可被解析的 JSON；内容过多时减少任务数量，禁止截断 JSON。\n\
         - 所有文本使用简体中文。\n\
         文档标题：{source_name}\n\
         文档正文：\n---\n{content}\n---",
        max_tasks = MAX_DECOMPOSITION_TASKS,
        step = MIN_ESTIMATE_MINUTES,
        current = input.current_date,
        max_date = max_date,
        max_acceptance = MAX_ACCEPTANCE,
        max_acceptance_len = MAX_ACCEPTANCE_LEN,
        source_name = input.source_name,
        content = input.content,
    )
}

/// 关闭未闭合的字符串与括号，用于修复被截断的模型输出。
fn close_json(candidate: &str) -> String {
    let mut out = String::with_capacity(candidate.len() + 16);
    let mut stack: Vec<char> = Vec::new();
    let mut in_string = false;
    let mut escape = false;
    for ch in candidate.chars() {
        out.push(ch);
        if in_string {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => stack.push('}'),
            '[' => stack.push(']'),
            '}' | ']' => {
                stack.pop();
            }
            _ => {}
        }
    }
    if in_string {
        out.push('"');
    }
    while let Some(close) = stack.pop() {
        out.push(close);
    }
    out
}

fn truncated_error() -> String {
    "AI 返回的 JSON 不完整（输出可能被长度截断）。请缩短文档正文、减少拆解任务数量，或在设置中改用输出上限更高的模型后重试。".to_owned()
}

/// 提取模型输出中的 JSON 对象（容忍代码块包裹和前后解释文字），
/// 并在输出被截断时尽量关闭未闭合的结构。返回 (json, 是否发生截断修复)。
fn extract_json(raw: &str) -> Result<(String, bool), String> {
    let start = raw.find('{').ok_or_else(|| "AI 未返回 JSON 结构".to_owned())?;
    let body = &raw[start..];
    let close_positions: Vec<usize> = body.match_indices('}').map(|(index, _)| index).collect();
    if close_positions.is_empty() {
        let repaired = close_json(body);
        if serde_json::from_str::<Value>(&repaired).is_ok() {
            return Ok((repaired, true));
        }
        return Err(truncated_error());
    }
    let exact = &body[..=*close_positions.last().unwrap()];
    if serde_json::from_str::<Value>(exact).is_ok() {
        return Ok((exact.to_owned(), false));
    }
    // 输出被截断：从最后一个完整对象往回找，关闭未闭合的数组/对象。
    for index in close_positions.iter().rev() {
        let repaired = close_json(&body[..=*index]);
        if serde_json::from_str::<Value>(&repaired).is_ok() {
            return Ok((repaired, true));
        }
    }
    Err(truncated_error())
}

fn normalize_ids(result: &mut DecompositionResult) {
    let mut seen: HashSet<String> = HashSet::new();
    let mut counter = 0usize;
    for (group_index, group) in result.groups.iter_mut().enumerate() {
        if group.id.trim().is_empty() {
            group.id = format!("g{}", group_index + 1);
        }
        let mut kept = Vec::with_capacity(group.tasks.len());
        for task in group.tasks.drain(..) {
            let mut task = task;
            let id = task.id.trim().to_owned();
            if id.is_empty() || seen.contains(&id) {
                counter += 1;
                task.id = format!("t{}", counter);
            }
            if seen.insert(task.id.clone()) {
                kept.push(task);
            }
        }
        group.tasks = kept;
    }
}

fn sanitize_task(
    mut task: DecompositionTask,
    valid_ids: &HashSet<String>,
    current: NaiveDate,
    max_date: NaiveDate,
) -> Option<DecompositionTask> {
    task.title = truncate_chars(task.title.trim(), MAX_TITLE);
    if task.title.is_empty() {
        return None;
    }
    task.description = truncate_chars(task.description.trim(), MAX_DESCRIPTION);
    if !is_priority(&task.priority) {
        task.priority = "medium".to_owned();
    }
    task.estimated_minutes = task.estimated_minutes.and_then(round_estimate);
    let suggested_date = task.suggested_date.filter(|value| {
        is_date(value)
            && NaiveDate::parse_from_str(value, "%Y-%m-%d")
                .map(|date| date >= current && date <= max_date)
                .unwrap_or(false)
    });
    task.suggested_time = suggested_date
        .as_ref()
        .and(task.suggested_time.as_deref())
        .filter(|value| is_time(value))
        .map(ToOwned::to_owned);
    task.suggested_date = suggested_date;
    task.acceptance_criteria = task
        .acceptance_criteria
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .take(MAX_ACCEPTANCE)
        .map(|value| truncate_chars(value, MAX_ACCEPTANCE_LEN))
        .collect();
    let mut seen = HashSet::new();
    task.dependency_ids = task
        .dependency_ids
        .iter()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty() && valid_ids.contains(value) && value != &task.id)
        .filter(|value| seen.insert(value.clone()))
        .collect();
    Some(task)
}

fn validate(mut result: DecompositionResult, current_date: &str) -> Result<DecompositionResult, String> {
    let current = NaiveDate::parse_from_str(current_date, "%Y-%m-%d")
        .map_err(|_| "当前日期格式应为 YYYY-MM-DD".to_owned())?;
    let max_date = current + Duration::days(MAX_SUGGEST_DAYS_AHEAD);
    normalize_ids(&mut result);
    // 限制任务总数，超出部分按顺序丢弃。
    let mut remaining = MAX_DECOMPOSITION_TASKS;
    for group in result.groups.iter_mut() {
        if remaining == 0 {
            group.tasks.clear();
            continue;
        }
        if group.tasks.len() > remaining {
            group.tasks.truncate(remaining);
        }
        remaining -= group.tasks.len();
    }
    let valid_ids = result
        .groups
        .iter()
        .flat_map(|group| group.tasks.iter().map(|task| task.id.clone()))
        .collect::<HashSet<_>>();
    let mut tasks = 0usize;
    result.groups = result
        .groups
        .drain(..)
        .filter_map(|mut group| {
            let sanitized = group
                .tasks
                .drain(..)
                .filter_map(|task| sanitize_task(task, &valid_ids, current, max_date))
                .collect::<Vec<_>>();
            if sanitized.is_empty() {
                return None;
            }
            tasks += sanitized.len();
            group.tasks = sanitized;
            group.title = truncate_chars(group.title.trim(), MAX_TITLE);
            if group.title.is_empty() {
                group.title = "未命名模块".to_owned();
            }
            Some(group)
        })
        .collect();
    if tasks == 0 {
        return Err("AI 未返回可用的待办，请调整内容后重试".to_owned());
    }
    result.summary = truncate_chars(result.summary.trim(), MAX_DESCRIPTION);
    result.assumptions = result
        .assumptions
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .take(10)
        .map(|value| truncate_chars(value, MAX_ACCEPTANCE_LEN))
        .collect();
    Ok(result)
}

pub fn generate(
    input: DecompositionInput,
    mut on_progress: impl FnMut(&str, String),
) -> Result<DecompositionResult, String> {
    ai_provider::validate_provider(&input.provider)?;
    let content = input.content.trim();
    if content.is_empty() {
        return Err("请提供需要拆解的文档内容".to_owned());
    }
    let char_count = content.chars().count();
    if char_count > MAX_DOCUMENT_CHARS {
        return Err(format!(
            "正文超过 {MAX_DOCUMENT_CHARS} 个字符，请缩短后重试"
        ));
    }
    if !is_date(&input.current_date) {
        return Err("当前日期格式应为 YYYY-MM-DD".to_owned());
    }
    on_progress("config", "已读取 AI 配置".to_owned());
    on_progress("request", format!("已构建拆解请求，正文 {char_count} 字"));
    let current = NaiveDate::parse_from_str(&input.current_date, "%Y-%m-%d")
        .map_err(|_| "当前日期格式应为 YYYY-MM-DD".to_owned())?;
    let max_date = (current + Duration::days(MAX_SUGGEST_DAYS_AHEAD))
        .format("%Y-%m-%d")
        .to_string();
    let prompt = build_prompt(&input, &max_date);
    // DeepSeek reasoner 不支持 response_format，其余 deepseek/kimi 可开启 json_object。
    let json_mode =
        input.provider == "kimi" || (input.provider == "deepseek" && input.model != "deepseek-reasoner");
    on_progress("sent", "已发送请求，等待 AI 响应…".to_owned());
    let mut reasoning_buffer = String::new();
    let mut last_reasoning = std::time::Instant::now();
    let mut content_buffer = String::new();
    let mut last_content = std::time::Instant::now();
    let raw = ai_provider::chat_stream(
        ChatRequest {
            provider: &input.provider,
            model: &input.model,
            api_key: &input.api_key,
            system: SYSTEM_PROMPT,
            user: &prompt,
            temperature: 0.2,
            max_tokens: 8000,
            json_mode,
        },
        |kind, text| match kind {
            ChatChunk::Reasoning => {
                reasoning_buffer.push_str(text);
                if last_reasoning.elapsed() >= std::time::Duration::from_millis(120)
                    || reasoning_buffer.chars().count() >= 400
                {
                    on_progress("reasoning", std::mem::take(&mut reasoning_buffer));
                    last_reasoning = std::time::Instant::now();
                }
            }
            ChatChunk::Content => {
                content_buffer.push_str(text);
                if last_content.elapsed() >= std::time::Duration::from_millis(120)
                    || content_buffer.chars().count() >= 400
                {
                    on_progress("content", std::mem::take(&mut content_buffer));
                    last_content = std::time::Instant::now();
                }
            }
        },
    )?;
    if !reasoning_buffer.is_empty() {
        on_progress("reasoning", std::mem::take(&mut reasoning_buffer));
    }
    if !content_buffer.is_empty() {
        on_progress("content", std::mem::take(&mut content_buffer));
    }
    on_progress("received", format!("已收到响应，正文 {} 字", raw.chars().count()));
    let (json, repaired) = extract_json(&raw)?;
    on_progress(
        "parsed",
        if repaired {
            "JSON 解析成功（已修复截断，尾部可能缺失）".to_owned()
        } else {
            "JSON 解析成功".to_owned()
        },
    );
    let parsed: DecompositionResult = serde_json::from_str(&json)
        .map_err(|error| format!("AI 返回的结构不符合约定：{error}"))?;
    let mut result = validate(parsed, &input.current_date)?;
    let task_count: usize = result.groups.iter().map(|group| group.tasks.len()).sum();
    on_progress("validated", format!("结构校验通过，共 {task_count} 条任务"));
    if repaired {
        result.assumptions.push(
            "注意：AI 输出可能被截断，结果或许不完整；建议缩短正文或减少任务数量后重试。".to_owned(),
        );
        result.assumptions.truncate(10);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{extract_json, validate};
    use crate::productivity::types::{DecompositionGroup, DecompositionResult, DecompositionTask};

    fn task(id: &str, date: Option<&str>) -> DecompositionTask {
        DecompositionTask {
            id: id.to_owned(),
            title: format!("实现 {id}"),
            description: String::new(),
            priority: "high".to_owned(),
            estimated_minutes: Some(45),
            suggested_date: date.map(ToOwned::to_owned),
            suggested_time: Some("09:00".to_owned()),
            dependency_ids: vec!["t1".to_owned(), id.to_owned(), "missing".to_owned()],
            acceptance_criteria: vec![" 可观察 ".to_owned(), "".to_owned()],
        }
    }

    #[test]
    fn extracts_json_from_fenced_output() {
        let raw = "好的，结果如下：\n```json\n{\"summary\":\"x\"}\n```\n";
        let (json, repaired) = extract_json(raw).unwrap();
        assert_eq!(json, "{\"summary\":\"x\"}");
        assert!(!repaired);
    }

    #[test]
    fn repairs_truncated_json_by_closing_open_structures() {
        // 模拟模型输出在第二个任务中途被截断。
        let raw = r#"{"summary":"x","assumptions":[],"groups":[{"id":"g1","title":"模块","tasks":[{"id":"t1","title":"甲","description":"","priority":"high","estimatedMinutes":60,"suggestedDate":null,"suggestedTime":null,"dependencyIds":[],"acceptanceCriteria":[]},{"id":"t2","title":"乙","desc"#;
        let (json, repaired) = extract_json(raw).expect("修复被截断的 JSON");
        assert!(repaired);
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        let tasks = value["groups"][0]["tasks"].as_array().unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0]["title"], "甲");
    }

    #[test]
    fn rejects_output_without_any_json_object() {
        assert!(extract_json("抱歉，我无法处理这个请求。").is_err());
    }

    #[test]
    fn normalizes_ids_dates_estimates_and_dependencies() {
        let result = DecompositionResult {
            summary: "  概述  ".to_owned(),
            assumptions: vec!["  假设 ".to_owned()],
            groups: vec![DecompositionGroup {
                id: "g1".to_owned(),
                title: "模块".to_owned(),
                tasks: vec![task("t1", Some("2026-09-29")), task("t2", Some("2030-01-01"))],
            }],
        };
        let validated = validate(result, "2026-09-28").expect("校验结果");
        let tasks = &validated.groups[0].tasks;
        assert_eq!(tasks[0].estimated_minutes, Some(60));
        assert!(tasks[0].dependency_ids.is_empty());
        assert_eq!(tasks[1].dependency_ids, vec!["t1".to_owned()] as Vec<String>);
        assert_eq!(tasks[1].suggested_date, None);
        assert_eq!(tasks[1].suggested_time, None);
        assert_eq!(validated.summary, "概述");
    }

    #[test]
    fn rejects_empty_tasks() {
        let result = DecompositionResult {
            summary: String::new(),
            assumptions: vec![],
            groups: vec![],
        };
        assert!(validate(result, "2026-09-28").is_err());
    }
}
