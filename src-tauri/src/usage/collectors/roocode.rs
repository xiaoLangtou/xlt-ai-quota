use regex::Regex;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use super::{
    file_sig, half_hour, number, read_json, timestamp, vscode_host_roots, Ctx, FileSig,
};

pub const SOURCE: &str = "roocode";
pub const COLLECTOR: &str = "roo-code";

const STORAGE: &str = "User/globalStorage/rooveterinaryinc.roo-cline/tasks";

fn task_files() -> Vec<(std::path::PathBuf, String)> {
    let mut out = Vec::new();
    for root in vscode_host_roots() {
        let tasks = root.join(STORAGE);
        let Ok(entries) = std::fs::read_dir(&tasks) else {
            continue;
        };
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }
            let task_uuid = entry.file_name().to_string_lossy().to_string();
            let file = entry.path().join("ui_messages.json");
            if file.exists() {
                out.push((file, task_uuid));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn read_task_model(ui_messages: &Path) -> Option<String> {
    let history = ui_messages.parent()?.join("api_conversation_history.json");
    let bytes = std::fs::read(&history).ok()?;
    let start = bytes.len().saturating_sub(1_048_576);
    let raw = String::from_utf8_lossy(&bytes[start..]);
    let raw = match raw.find("<environment_details>") {
        Some(index) => &raw[index..],
        None => &raw,
    };
    let regex = Regex::new(r"<model>\s*([^<\s][^<]*?)\s*</model>").ok()?;
    let mut last = None;
    for capture in regex.captures_iter(raw) {
        if let Some(value) = capture.get(1) {
            let value = value.as_str().trim();
            if !value.is_empty() {
                last = Some(value.to_owned());
            }
        }
    }
    last
}

fn normalize_model(explicit: Option<String>, protocol: Option<&Value>) -> String {
    if let Some(model) = explicit {
        if !model.trim().is_empty() {
            return model.trim().to_owned();
        }
    }
    if let Some(protocol) = protocol.and_then(Value::as_str) {
        let slug: String = protocol
            .trim()
            .to_lowercase()
            .chars()
            .filter(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
            })
            .collect();
        if !slug.is_empty() {
            return format!("protocol:{slug}");
        }
    }
    "unknown".to_owned()
}

pub fn collect(ctx: &mut Ctx, previous: Option<Value>) -> Value {
    let previous = previous.unwrap_or_else(|| json!({}));
    let mut files: BTreeMap<String, FileSig> = previous
        .get("fileOffsets")
        .and_then(Value::as_object)
        .map(|object| {
            object
                .iter()
                .filter_map(|(key, value)| {
                    serde_json::from_value::<FileSig>(value.clone())
                        .ok()
                        .map(|sig| (key.clone(), sig))
                })
                .collect()
        })
        .unwrap_or_default();
    let mut seen: Vec<String> = previous
        .get("seenIds")
        .and_then(Value::as_array)
        .map(|array| array.iter().filter_map(Value::as_str).map(str::to_owned).collect())
        .unwrap_or_default();
    let mut seen_set: HashSet<String> = seen.iter().cloned().collect();

    for (file, task_uuid) in task_files() {
        let Some(sig) = file_sig(&file) else {
            continue;
        };
        let key = file.display().to_string();
        if files.get(&key) == Some(&sig) {
            continue;
        }
        let Some(data) = read_json(&file) else {
            continue;
        };
        let Some(messages) = data.as_array() else {
            continue;
        };
        let task_model = read_task_model(&file);

        for message in messages {
            let say = message.get("say").and_then(Value::as_str);
            if say != Some("api_req_started") && say != Some("api_req_deleted") {
                continue;
            }
            let Some(raw_text) = message.get("text").and_then(Value::as_str) else {
                continue;
            };
            if !raw_text.starts_with('{') {
                continue;
            }
            let Ok(payload) = serde_json::from_str::<Value>(raw_text) else {
                continue;
            };
            let Some(stamp) = timestamp(message.get("ts")) else {
                continue;
            };
            if !ctx.in_window(stamp) {
                continue;
            }
            let dedup = format!("{task_uuid}:{}", stamp.timestamp_millis());
            if seen_set.contains(&dedup) {
                continue;
            }
            let input = number(payload.get("tokensIn"));
            let output = number(payload.get("tokensOut"));
            let cache_read = number(payload.get("cacheReads"));
            let cache_write = number(payload.get("cacheWrites"));
            if input == 0 && output == 0 && cache_read == 0 && cache_write == 0 {
                continue;
            }
            let model = normalize_model(task_model.clone(), payload.get("apiProtocol"));
            ctx.add(
                SOURCE,
                COLLECTOR,
                &model,
                "unknown",
                &half_hour(stamp),
                input,
                output,
                cache_read,
                cache_write,
                0,
            );
            seen_set.insert(dedup.clone());
            seen.push(dedup);
        }

        files.insert(key, sig);
        ctx.files_processed += 1;
    }

    if seen.len() > 50_000 {
        seen = seen.split_off(seen.len() - 50_000);
    }
    let files_json: Map<String, Value> = files
        .into_iter()
        .filter_map(|(key, sig)| serde_json::to_value(sig).ok().map(|value| (key, value)))
        .collect();
    json!({ "fileOffsets": files_json, "seenIds": seen })
}
