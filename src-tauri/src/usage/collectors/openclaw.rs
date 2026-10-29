use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::Path;

use super::{
    env_path, file_sig, half_hour, home_dir, number, read_json_lines_from, text, timestamp, Ctx,
};

pub const SOURCE: &str = "openclaw";
pub const COLLECTOR: &str = "openclaw";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct OffsetCursor {
    inode: u64,
    offset: u64,
}

fn roots() -> Vec<std::path::PathBuf> {
    if let Some(path) = env_path("OPENCLAW_STATE_DIR") {
        return vec![path];
    }
    let home = home_dir();
    let mut roots = vec![
        home.join(".clawdbot"),
        home.join(".moltbot"),
        home.join(".moldbot"),
    ];
    if let Ok(entries) = std::fs::read_dir(&home) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !entry.path().is_dir() {
                continue;
            }
            if name == ".openclaw" || (name.starts_with(".openclaw-") && !name.starts_with(".openclaw-autoclaw")) {
                roots.push(entry.path());
            }
        }
    }
    roots.sort();
    roots.dedup();
    roots
}

fn session_files() -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    for root in roots() {
        let agents = root.join("agents");
        let Ok(entries) = std::fs::read_dir(&agents) else {
            continue;
        };
        for agent in entries.flatten() {
            let sessions = agent.path().join("sessions");
            let Ok(entries) = std::fs::read_dir(&sessions) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|value| value.to_str()) == Some("jsonl") {
                    files.push(path);
                }
            }
        }
    }
    files.sort();
    files
}

fn project_from_path(path: &Path) -> String {
    let parts: Vec<String> = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect();
    if let Some(index) = parts.iter().rposition(|part| part == "agents") {
        if let Some(agent) = parts.get(index + 1) {
            return agent.clone();
        }
    }
    "unknown".to_owned()
}

fn first_number(usage: &Value, keys: &[&str]) -> u64 {
    for key in keys {
        let value = number(usage.get(*key));
        if value > 0 {
            return value;
        }
    }
    0
}

pub fn collect(ctx: &mut Ctx, previous: Option<Value>) -> Value {
    let previous = previous.unwrap_or_else(|| json!({}));
    let mut files: BTreeMap<String, OffsetCursor> = previous
        .get("files")
        .and_then(Value::as_object)
        .map(|object| {
            object
                .iter()
                .filter_map(|(key, value)| {
                    serde_json::from_value::<OffsetCursor>(value.clone())
                        .ok()
                        .map(|cursor| (key.clone(), cursor))
                })
                .collect()
        })
        .unwrap_or_default();

    for file in session_files() {
        let Some(sig) = file_sig(&file) else {
            continue;
        };
        let key = file.display().to_string();
        let previous_cursor = files.get(&key).cloned();
        let same_inode = previous_cursor
            .as_ref()
            .map(|cursor| cursor.inode == sig.inode)
            .unwrap_or(false);
        let truncated = same_inode
            && previous_cursor
                .as_ref()
                .map(|cursor| cursor.offset > sig.size)
                .unwrap_or(false);
        let start = if same_inode && !truncated {
            previous_cursor.as_ref().map(|cursor| cursor.offset).unwrap_or(0)
        } else {
            0
        };
        if same_inode && !truncated && start >= sig.size {
            continue;
        }

        let project = project_from_path(&file);
        let (lines, next_offset) = read_json_lines_from(&file, start);
        for event in lines {
            if event.get("type").and_then(Value::as_str) != Some("message") {
                continue;
            }
            let Some(message) = event.get("message") else {
                continue;
            };
            if message.get("role").and_then(Value::as_str) != Some("assistant") {
                continue;
            }
            let Some(usage) = message.get("usage") else {
                continue;
            };
            let Some(stamp) = timestamp(event.get("timestamp").or_else(|| message.get("timestamp")))
            else {
                continue;
            };
            if !ctx.in_window(stamp) {
                continue;
            }
            let raw_input = first_number(
                usage,
                &[
                    "input",
                    "inputTokens",
                    "input_tokens",
                    "promptTokens",
                    "prompt_tokens",
                ],
            );
            let cache_read = first_number(
                usage,
                &[
                    "cacheRead",
                    "cache_read",
                    "cache_read_input_tokens",
                    "cachedInputTokens",
                    "cached_input_tokens",
                ],
            );
            let cache_write = first_number(
                usage,
                &[
                    "cacheWrite",
                    "cache_write",
                    "cache_creation_input_tokens",
                    "cacheCreationInputTokens",
                ],
            );
            let output = first_number(
                usage,
                &[
                    "output",
                    "outputTokens",
                    "output_tokens",
                    "completionTokens",
                    "completion_tokens",
                ],
            );
            let input = raw_input.saturating_sub(cache_read);
            if input == 0 && output == 0 && cache_read == 0 && cache_write == 0 {
                continue;
            }
            let model = text(message.get("model"))
                .or_else(|| text(event.get("model")))
                .unwrap_or_else(|| "unknown".to_owned());
            ctx.add(
                SOURCE,
                COLLECTOR,
                &model,
                &project,
                &half_hour(stamp),
                input,
                output,
                cache_read,
                cache_write,
                0,
            );
        }

        files.insert(
            key,
            OffsetCursor {
                inode: sig.inode,
                offset: next_offset,
            },
        );
        ctx.files_processed += 1;
    }

    let files_json: Map<String, Value> = files
        .into_iter()
        .filter_map(|(key, cursor)| serde_json::to_value(cursor).ok().map(|value| (key, value)))
        .collect();
    json!({ "files": files_json })
}
