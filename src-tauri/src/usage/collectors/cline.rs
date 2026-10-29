use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use super::{
    env_path, file_sig, half_hour, home_dir, number, read_json, text, timestamp, vscode_host_roots,
    Ctx, FileSig,
};

pub const SOURCE: &str = "cline";
pub const COLLECTOR: &str = "cline";

const EXTENSION_STORAGE: &str = "User/globalStorage/saoudrizwan.claude-dev";

fn project_from_cwd(value: Option<&Value>) -> String {
    let Some(cwd) = text(value) else {
        return "unknown".to_owned();
    };
    Path::new(&cwd)
        .file_name()
        .and_then(|value| value.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| "unknown".to_owned())
}

fn session_dir() -> PathBuf {
    if let Some(path) = env_path("CLINE_SESSION_DATA_DIR") {
        return path;
    }
    if let Some(path) = env_path("CLINE_DATA_DIR") {
        return path.join("sessions");
    }
    if let Some(path) = env_path("CLINE_DIR") {
        return path.join("data").join("sessions");
    }
    home_dir().join(".cline").join("data").join("sessions")
}

struct TaskTarget {
    task_id: String,
    project: String,
    fallback_model: String,
    messages: PathBuf,
}

fn task_targets() -> Vec<TaskTarget> {
    let mut out = Vec::new();
    for root in vscode_host_roots() {
        let ext = root.join(EXTENSION_STORAGE);
        let Some(history) = read_json(&ext.join("state").join("taskHistory.json")) else {
            continue;
        };
        let Some(items) = history.as_array() else {
            continue;
        };
        for item in items {
            let Some(task_id) = item.get("id").and_then(Value::as_str) else {
                continue;
            };
            let project = project_from_cwd(
                item.get("cwdOnTaskInitialization")
                    .or_else(|| item.get("shadowGitConfigWorkTree")),
            );
            let fallback_model =
                text(item.get("modelId")).unwrap_or_else(|| "unknown".to_owned());
            let messages = ext.join("tasks").join(task_id).join("ui_messages.json");
            if messages.exists() {
                out.push(TaskTarget {
                    task_id: task_id.to_owned(),
                    project,
                    fallback_model,
                    messages,
                });
            }
        }
    }
    out.sort_by(|a, b| a.messages.cmp(&b.messages));
    out
}

struct SdkTarget {
    session_id: String,
    project: String,
    fallback_model: String,
    messages: PathBuf,
}

fn sdk_targets() -> Vec<SdkTarget> {
    let dir = session_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let session_id = entry.file_name().to_string_lossy().to_string();
        let session_dir = entry.path();
        let manifest = read_json(&session_dir.join(format!("{session_id}.json")));
        if let Some(value) = manifest
            .as_ref()
            .and_then(|value| value.get("session_id"))
            .and_then(Value::as_str)
        {
            if value != session_id {
                continue;
            }
        }
        let configured = manifest
            .as_ref()
            .and_then(|value| text(value.get("messages_path")));
        let messages = configured
            .map(|value| {
                let path = PathBuf::from(&value);
                if path.is_absolute() {
                    path
                } else {
                    session_dir.join(path)
                }
            })
            .unwrap_or_else(|| session_dir.join(format!("{session_id}.messages.json")));
        if !messages.exists() {
            continue;
        }
        let project = project_from_cwd(
            manifest
                .as_ref()
                .and_then(|value| value.get("cwd").or_else(|| value.get("workspace_root"))),
        );
        let fallback_model = manifest
            .as_ref()
            .and_then(|value| text(value.get("model")))
            .unwrap_or_else(|| "unknown".to_owned());
        out.push(SdkTarget {
            session_id,
            project,
            fallback_model,
            messages,
        });
    }
    out.sort_by(|a, b| a.messages.cmp(&b.messages));
    out
}

fn tokens(payload: &Value, in_key: &str, out_key: &str, read_key: &str, write_key: &str) -> Option<(u64, u64, u64, u64)> {
    let input = number(payload.get(in_key));
    let output = number(payload.get(out_key));
    let cache_read = number(payload.get(read_key));
    let cache_write = number(payload.get(write_key));
    if input == 0 && output == 0 && cache_read == 0 && cache_write == 0 {
        None
    } else {
        Some((input, output, cache_read, cache_write))
    }
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

    for target in task_targets() {
        let Some(sig) = file_sig(&target.messages) else {
            continue;
        };
        let key = target.messages.display().to_string();
        if files.get(&key) == Some(&sig) {
            continue;
        }
        let Some(data) = read_json(&target.messages) else {
            continue;
        };
        let Some(messages) = data.as_array() else {
            continue;
        };
        for message in messages {
            if message.get("type").and_then(Value::as_str) != Some("say")
                || message.get("say").and_then(Value::as_str) != Some("api_req_started")
            {
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
            let dedup = format!("{}:{}", target.task_id, stamp.timestamp_millis());
            if seen_set.contains(&dedup) {
                continue;
            }
            let Some((input, output, cache_read, cache_write)) =
                tokens(&payload, "tokensIn", "tokensOut", "cacheReads", "cacheWrites")
            else {
                continue;
            };
            let model = text(payload.get("model"))
                .unwrap_or_else(|| target.fallback_model.clone());
            ctx.add(
                SOURCE,
                COLLECTOR,
                &model,
                &target.project,
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

    for target in sdk_targets() {
        let Some(sig) = file_sig(&target.messages) else {
            continue;
        };
        let key = target.messages.display().to_string();
        if files.get(&key) == Some(&sig) {
            continue;
        }
        let Some(data) = read_json(&target.messages) else {
            continue;
        };
        if data.get("version").and_then(Value::as_u64) != Some(1) {
            continue;
        }
        let Some(messages) = data.get("messages").and_then(Value::as_array) else {
            continue;
        };
        for (index, message) in messages.iter().enumerate() {
            if message.get("role").and_then(Value::as_str) != Some("assistant") {
                continue;
            }
            let Some(metrics) = message.get("metrics") else {
                continue;
            };
            let Some(stamp) = timestamp(message.get("ts")) else {
                continue;
            };
            if !ctx.in_window(stamp) {
                continue;
            }
            let message_id = text(message.get("id"))
                .unwrap_or_else(|| format!("{}:{index}", stamp.timestamp_millis()));
            let dedup = format!("sdk:{}:{message_id}", target.session_id);
            if seen_set.contains(&dedup) {
                continue;
            }
            let Some((input, output, cache_read, cache_write)) = tokens(
                metrics,
                "inputTokens",
                "outputTokens",
                "cacheReadTokens",
                "cacheWriteTokens",
            ) else {
                continue;
            };
            let model = text(message.get("modelInfo").and_then(|value| value.get("id")))
                .unwrap_or_else(|| target.fallback_model.clone());
            ctx.add(
                SOURCE,
                COLLECTOR,
                &model,
                &target.project,
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
