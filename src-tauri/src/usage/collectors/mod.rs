use chrono::{DateTime, SecondsFormat, Timelike, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use super::types::QueueBucket;

pub mod amp;
pub mod cline;
pub mod codebuddy;
pub mod kilocode;
pub mod openclaw;
pub mod qwen;
pub mod roocode;
pub mod workbuddy;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileSig {
    pub size: u64,
    pub mtime_ms: u64,
    #[serde(default)]
    pub inode: u64,
}

pub struct Ctx {
    pub since: DateTime<Utc>,
    pub until: DateTime<Utc>,
    buckets: BTreeMap<String, QueueBucket>,
    pub events_parsed: usize,
    pub files_processed: usize,
}

impl Ctx {
    pub fn new(since: &str, until: &str) -> Result<Self, String> {
        let since = DateTime::parse_from_rfc3339(since)
            .map_err(|error| format!("since 不是合法 RFC3339: {error}"))?
            .with_timezone(&Utc);
        let until = DateTime::parse_from_rfc3339(until)
            .map_err(|error| format!("until 不是合法 RFC3339: {error}"))?
            .with_timezone(&Utc);
        Ok(Self {
            since,
            until,
            buckets: BTreeMap::new(),
            events_parsed: 0,
            files_processed: 0,
        })
    }

    pub fn in_window(&self, ts: DateTime<Utc>) -> bool {
        ts >= self.since && ts <= self.until
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add(
        &mut self,
        source: &str,
        collector: &str,
        model: &str,
        project: &str,
        hour_start: &str,
        input: u64,
        output: u64,
        cache_read: u64,
        cache_write: u64,
        reasoning: u64,
    ) {
        if input == 0 && output == 0 && cache_read == 0 && cache_write == 0 && reasoning == 0 {
            return;
        }
        let key = format!(
            "{}|{}|{}|{}|{}",
            source,
            collector,
            model.trim(),
            project,
            hour_start
        );
        let entry = self.buckets.entry(key).or_insert_with(|| QueueBucket {
            hour_start: hour_start.to_owned(),
            source: source.to_owned(),
            collector: Some(collector.to_owned()),
            project: project.to_owned(),
            model: model.trim().to_owned(),
            input_tokens: 0,
            output_tokens: 0,
            cached_input_tokens: 0,
            cache_creation_input_tokens: 0,
            reasoning_output_tokens: 0,
            total_tokens: 0,
            conversation_count: 0,
            reported_cost_usd: None,
            local_metrics: None,
        });
        entry.input_tokens += input;
        entry.output_tokens += output;
        entry.cached_input_tokens += cache_read;
        entry.cache_creation_input_tokens += cache_write;
        entry.reasoning_output_tokens += reasoning;
        entry.conversation_count += 1;
        entry.recompute_total();
        self.events_parsed += 1;
    }

    pub fn into_buckets(self) -> Vec<QueueBucket> {
        self.buckets.into_values().collect()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectResult {
    pub source: String,
    pub buckets: Vec<QueueBucket>,
    pub events_parsed: usize,
    pub files_processed: usize,
    pub skipped: bool,
    pub error: Option<String>,
}

pub fn half_hour(ts: DateTime<Utc>) -> String {
    let minute = if ts.minute() < 30 { 0 } else { 30 };
    ts.with_minute(minute)
        .and_then(|value| value.with_second(0))
        .and_then(|value| value.with_nanosecond(0))
        .unwrap_or(ts)
        .to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub fn expand_home(path: &str) -> PathBuf {
    let trimmed = path.trim();
    if trimmed == "~" {
        return home_dir();
    }
    if let Some(rest) = trimmed.strip_prefix("~/") {
        return home_dir().join(rest);
    }
    PathBuf::from(trimmed)
}

pub fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

pub fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .map(|value| expand_home(&value))
}

pub fn file_sig(path: &Path) -> Option<FileSig> {
    let meta = fs::metadata(path).ok()?;
    let mtime_ms = meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
    #[cfg(unix)]
    let inode = {
        use std::os::unix::fs::MetadataExt;
        meta.ino()
    };
    #[cfg(not(unix))]
    let inode = 0;
    Some(FileSig {
        size: meta.len(),
        mtime_ms,
        inode,
    })
}

pub fn read_json_lines(path: &Path, mut visit: impl FnMut(&Value)) {
    let Ok(content) = fs::read_to_string(path) else {
        return;
    };
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
            visit(&value);
        }
    }
}

pub fn read_json(path: &Path) -> Option<Value> {
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str::<Value>(&content).ok()
}

pub fn number(value: Option<&Value>) -> u64 {
    let Some(value) = value else {
        return 0;
    };
    let parsed = match value {
        Value::Number(number) => number.as_f64().unwrap_or(0.0),
        Value::String(text) => text.trim().parse::<f64>().unwrap_or(0.0),
        _ => 0.0,
    };
    if !parsed.is_finite() || parsed < 0.0 {
        0
    } else {
        parsed.floor() as u64
    }
}

pub fn text(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

pub fn timestamp(value: Option<&Value>) -> Option<DateTime<Utc>> {
    let value = value?;
    if let Some(text) = value.as_str() {
        let trimmed = text.trim();
        if let Ok(parsed) = DateTime::parse_from_rfc3339(trimmed) {
            return Some(parsed.with_timezone(&Utc));
        }
        if let Ok(number) = trimmed.parse::<f64>() {
            return millis_to_time(number);
        }
        return None;
    }
    if let Some(number) = value.as_f64() {
        return millis_to_time(number);
    }
    None
}

fn millis_to_time(value: f64) -> Option<DateTime<Utc>> {
    if !value.is_finite() || value <= 0.0 {
        return None;
    }
    let millis = if value < 1e12 { value * 1000.0 } else { value };
    DateTime::<Utc>::from_timestamp_millis(millis as i64)
}

pub fn list_files_recursive(root: &Path, predicate: &dyn Fn(&Path) -> bool) -> Vec<PathBuf> {
    let mut files = Vec::new();
    visit_files(root, predicate, &mut files);
    files.sort();
    files
}

fn visit_files(root: &Path, predicate: &dyn Fn(&Path) -> bool, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            visit_files(&path, predicate, files);
        } else if predicate(&path) {
            files.push(path);
        }
    }
}

pub fn list_dirs(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    dirs
}

/// VS Code 系宿主根目录（Application Support / APPDATA / XDG）。
pub fn vscode_host_roots() -> Vec<PathBuf> {
    if let Some(override_roots) = std::env::var("AI_USAGE_VSCODE_ROOTS")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    {
        return override_roots
            .split([':', ';', ','])
            .map(expand_home)
            .filter(|path| !path.as_os_str().is_empty())
            .collect();
    }
    let base = if cfg!(target_os = "macos") {
        home_dir().join("Library").join("Application Support")
    } else if cfg!(target_os = "windows") {
        std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home_dir().join("AppData").join("Roaming"))
    } else {
        std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home_dir().join(".config"))
    };
    [
        "Code",
        "Code - Insiders",
        "Cursor",
        "VSCodium",
        "Windsurf",
        "Trae",
        "Trae CN",
        "CodeBuddy",
    ]
    .into_iter()
    .map(|name| base.join(name))
    .collect()
}

pub fn collect(source: &str, since: &str, until: &str) -> Result<CollectResult, String> {
    let mut ctx = Ctx::new(since, until)?;
    let mut cursors = super::cursors::load();
    let previous = cursors.sources.get(source).cloned();
    let next = match source {
        "qwen" => qwen::collect(&mut ctx, previous),
        "amp" => amp::collect(&mut ctx, previous),
        "roocode" => roocode::collect(&mut ctx, previous),
        "kilocode" => kilocode::collect(&mut ctx, previous),
        "openclaw" => openclaw::collect(&mut ctx, previous),
        "cline" => cline::collect(&mut ctx, previous),
        "codebuddy" => codebuddy::collect(&mut ctx, previous),
        "workbuddy" => workbuddy::collect(&mut ctx, previous),
        other => {
            return Err(format!("尚未移植的采集源: {other}"));
        }
    };
    let events = ctx.events_parsed;
    let files = ctx.files_processed;
    let buckets = ctx.into_buckets();
    // 先落盘再推进游标：写盘失败时下次仍会重放这批增量。
    super::queue::append_buckets(&buckets)?;
    cursors.sources.insert(source.to_owned(), next);
    super::cursors::save(&cursors)?;
    Ok(CollectResult {
        source: source.to_owned(),
        buckets,
        events_parsed: events,
        files_processed: files,
        skipped: false,
        error: None,
    })
}

pub fn known_sources() -> Vec<&'static str> {
    vec![
        "qwen",
        "amp",
        "roocode",
        "kilocode",
        "openclaw",
        "cline",
        "codebuddy",
        "workbuddy",
    ]
}

/// 从指定字节偏移读取 JSONL，只消费完整行，返回新偏移。
pub fn read_json_lines_from(path: &Path, offset: u64) -> (Vec<Value>, u64) {
    let Ok(bytes) = fs::read(path) else {
        return (Vec::new(), offset);
    };
    if offset as usize > bytes.len() {
        return (Vec::new(), bytes.len() as u64);
    }
    let slice = &bytes[offset as usize..];
    let Some(last_newline) = slice.iter().rposition(|byte| *byte == b'\n') else {
        return (Vec::new(), offset);
    };
    let consumed = &slice[..=last_newline];
    let mut values = Vec::new();
    for line in consumed.split(|byte| *byte == b'\n') {
        if line.iter().all(|byte| byte.is_ascii_whitespace()) {
            continue;
        }
        if let Ok(text) = std::str::from_utf8(line) {
            if let Ok(value) = serde_json::from_str::<Value>(text) {
                values.push(value);
            }
        }
    }
    (values, offset + consumed.len() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("xlt-{name}-{}", uuid::Uuid::new_v4()))
    }

    fn write(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, format!("{body}\n")).unwrap();
    }

    #[test]
    fn qwen_collects_and_skips_unchanged_files() {
        let _guard = crate::usage::TEST_ENV_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let root = temp_dir("qwen");
        let file = root.join("proj").join("chats").join("s.jsonl");
        write(
            &file,
            r#"{"type":"assistant","uuid":"u1","timestamp":"2026-10-29T10:15:00Z","model":"qwen3","usageMetadata":{"promptTokenCount":100,"candidatesTokenCount":40,"cachedContentTokenCount":20,"thoughtsTokenCount":5}}"#,
        );
        std::env::set_var("QWEN_TMP_DIR", &root);

        let mut ctx = Ctx::new("2026-10-01T00:00:00Z", "2026-11-01T00:00:00Z").unwrap();
        let cursor = qwen::collect(&mut ctx, None);
        let buckets = ctx.into_buckets();
        assert_eq!(buckets.len(), 1);
        let bucket = &buckets[0];
        assert_eq!(bucket.source, "qwen");
        assert_eq!(bucket.input_tokens, 80);
        assert_eq!(bucket.cached_input_tokens, 20);
        assert_eq!(bucket.output_tokens, 35);
        assert_eq!(bucket.reasoning_output_tokens, 5);
        assert_eq!(bucket.hour_start, "2026-10-29T10:00:00.000Z");

        let mut second = Ctx::new("2026-10-01T00:00:00Z", "2026-11-01T00:00:00Z").unwrap();
        qwen::collect(&mut second, Some(cursor));
        assert!(second.into_buckets().is_empty());

        std::env::remove_var("QWEN_TMP_DIR");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn amp_reads_usage_ledger() {
        let _guard = crate::usage::TEST_ENV_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let root = temp_dir("amp");
        let file = root.join("nested").join("T-1.json");
        write(
            &file,
            r#"{"id":"thread-1","created":"2026-10-29T09:00:00Z","usageLedger":{"events":[{"timestamp":"2026-10-29T09:10:00Z","tokens":{"input":120,"output":30},"model":"amp-model","toMessageId":0}]},"messages":[{"usage":{"cacheReadInputTokens":40}}]}"#,
        );
        std::env::set_var("AMP_DATA_DIR", &root);

        let mut ctx = Ctx::new("2026-10-01T00:00:00Z", "2026-11-01T00:00:00Z").unwrap();
        amp::collect(&mut ctx, None);
        let buckets = ctx.into_buckets();
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0].input_tokens, 120);
        assert_eq!(buckets[0].output_tokens, 30);
        assert_eq!(buckets[0].cached_input_tokens, 40);
        assert_eq!(buckets[0].model, "amp-model");

        std::env::remove_var("AMP_DATA_DIR");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn roocode_and_kilocode_read_ui_messages() {
        let _guard = crate::usage::TEST_ENV_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let root = temp_dir("vscode");
        let roo = root
            .join("User/globalStorage/rooveterinaryinc.roo-cline/tasks/t1/ui_messages.json");
        write(
            &roo,
            r#"[{"say":"api_req_started","ts":1793183400000,"text":"{\"tokensIn\":100,\"tokensOut\":20,\"cacheReads\":10,\"cacheWrites\":5,\"apiProtocol\":\"openai\"}"}]"#,
        );
        write(
            &root.join("User/globalStorage/rooveterinaryinc.roo-cline/tasks/t1/api_conversation_history.json"),
            "<model>roo-model</model>",
        );
        let kilo = root
            .join("User/globalStorage/kilocode.kilo-code/tasks/t2/ui_messages.json");
        write(
            &kilo,
            r#"[{"say":"api_req_started","ts":1793183400000,"text":"{\"tokensIn\":50,\"tokensOut\":10,\"cacheReads\":0,\"cacheWrites\":0,\"inferenceProvider\":\"Anthropic\"}"}]"#,
        );
        std::env::set_var("AI_USAGE_VSCODE_ROOTS", &root);

        let mut ctx = Ctx::new("2026-10-01T00:00:00Z", "2026-11-01T00:00:00Z").unwrap();
        roocode::collect(&mut ctx, None);
        let buckets = ctx.into_buckets();
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0].model, "roo-model");
        assert_eq!(buckets[0].input_tokens, 100);
        assert_eq!(buckets[0].cache_creation_input_tokens, 5);

        let mut ctx = Ctx::new("2026-10-01T00:00:00Z", "2026-11-01T00:00:00Z").unwrap();
        kilocode::collect(&mut ctx, None);
        let buckets = ctx.into_buckets();
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0].model, "provider:anthropic");
        assert_eq!(buckets[0].input_tokens, 50);

        std::env::remove_var("AI_USAGE_VSCODE_ROOTS");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn codebuddy_and_workbuddy_read_raw_usage() {
        let _guard = crate::usage::TEST_ENV_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let codebuddy_root = temp_dir("codebuddy");
        write(
            &codebuddy_root.join("projects/p/s.jsonl"),
            r#"{"type":"message","role":"assistant","uuid":"m1","timestamp":"2026-10-29T10:00:00Z","providerData":{"model":"cb-model","rawUsage":{"prompt_tokens":100,"completion_tokens":20,"prompt_tokens_details":{"cached_tokens":10,"reasoning_tokens":5}}}}"#,
        );
        std::env::set_var("CODEBUDDY_HOME", &codebuddy_root);

        let mut ctx = Ctx::new("2026-10-01T00:00:00Z", "2026-11-01T00:00:00Z").unwrap();
        codebuddy::collect(&mut ctx, None);
        let buckets = ctx.into_buckets();
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0].model, "cb-model");
        assert_eq!(buckets[0].input_tokens, 90);
        assert_eq!(buckets[0].cached_input_tokens, 10);
        assert_eq!(buckets[0].output_tokens, 20);
        assert_eq!(buckets[0].reasoning_output_tokens, 5);

        let workbuddy_root = temp_dir("workbuddy");
        write(
            &workbuddy_root.join("projects/p/s.jsonl"),
            r#"{"id":"m1","timestamp":"2026-10-29T10:00:00Z","cwd":"/x/proj","providerData":{"model":"wb-model","rawUsage":{"prompt_tokens":100,"completion_tokens":20,"prompt_tokens_details":{"cached_tokens":10},"completion_tokens_details":{"reasoning_tokens":5}}}}"#,
        );
        std::env::set_var("WORKBUDDY_HOME", &workbuddy_root);

        let mut ctx = Ctx::new("2026-10-01T00:00:00Z", "2026-11-01T00:00:00Z").unwrap();
        workbuddy::collect(&mut ctx, None);
        let buckets = ctx.into_buckets();
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0].model, "wb-model");
        assert_eq!(buckets[0].project, "proj");
        assert_eq!(buckets[0].input_tokens, 90);
        assert_eq!(buckets[0].output_tokens, 15);
        assert_eq!(buckets[0].reasoning_output_tokens, 5);

        std::env::remove_var("CODEBUDDY_HOME");
        std::env::remove_var("WORKBUDDY_HOME");
        fs::remove_dir_all(&codebuddy_root).ok();
        fs::remove_dir_all(&workbuddy_root).ok();
    }

    #[test]
    fn cline_reads_task_history() {
        let _guard = crate::usage::TEST_ENV_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let root = temp_dir("cline");
        write(
            &root.join("User/globalStorage/saoudrizwan.claude-dev/state/taskHistory.json"),
            r#"[{"id":"t1","cwdOnTaskInitialization":"/x/cline-proj","modelId":"cline-model"}]"#,
        );
        write(
            &root.join("User/globalStorage/saoudrizwan.claude-dev/tasks/t1/ui_messages.json"),
            r#"[{"type":"say","say":"api_req_started","ts":1793183400000,"text":"{\"tokensIn\":100,\"tokensOut\":20,\"cacheReads\":10,\"cacheWrites\":5}"}]"#,
        );
        std::env::set_var("AI_USAGE_VSCODE_ROOTS", &root);

        let mut ctx = Ctx::new("2026-10-01T00:00:00Z", "2026-11-01T00:00:00Z").unwrap();
        cline::collect(&mut ctx, None);
        let buckets = ctx.into_buckets();
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0].model, "cline-model");
        assert_eq!(buckets[0].project, "cline-proj");
        assert_eq!(buckets[0].input_tokens, 100);
        assert_eq!(buckets[0].cache_creation_input_tokens, 5);

        std::env::remove_var("AI_USAGE_VSCODE_ROOTS");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn openclaw_reads_incremental_offsets() {
        let _guard = crate::usage::TEST_ENV_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let root = temp_dir("openclaw");
        let file = root.join("agents/a1/sessions/s.jsonl");
        write(
            &file,
            "{\"type\":\"message\",\"timestamp\":\"2026-10-29T10:00:00Z\",\"message\":{\"role\":\"assistant\",\"model\":\"oc-model\",\"usage\":{\"input\":100,\"cacheRead\":20,\"output\":30}}}\n",
        );
        std::env::set_var("OPENCLAW_STATE_DIR", &root);

        let mut ctx = Ctx::new("2026-10-01T00:00:00Z", "2026-11-01T00:00:00Z").unwrap();
        let cursor = openclaw::collect(&mut ctx, None);
        let buckets = ctx.into_buckets();
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0].input_tokens, 80);
        assert_eq!(buckets[0].cached_input_tokens, 20);
        assert_eq!(buckets[0].output_tokens, 30);
        assert_eq!(buckets[0].project, "a1");

        let mut second = Ctx::new("2026-10-01T00:00:00Z", "2026-11-01T00:00:00Z").unwrap();
        openclaw::collect(&mut second, Some(cursor));
        assert!(second.into_buckets().is_empty());

        std::env::remove_var("OPENCLAW_STATE_DIR");
        fs::remove_dir_all(&root).ok();
    }
}
