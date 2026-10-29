use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashSet};

use super::{
    env_path, file_sig, half_hour, home_dir, list_files_recursive, number, read_json, text,
    timestamp, Ctx, FileSig,
};

pub const SOURCE: &str = "amp";
pub const COLLECTOR: &str = "amp";

fn root_dir() -> std::path::PathBuf {
    if let Some(path) = env_path("AMP_DATA_DIR") {
        return path;
    }
    if let Some(xdg) = env_path("XDG_DATA_HOME") {
        return xdg.join("amp").join("threads");
    }
    home_dir().join(".local").join("share").join("amp").join("threads")
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

    for file in list_files_recursive(&root_dir(), &|path| {
        path.file_name()
            .and_then(|value| value.to_str())
            .map(|name| name.starts_with("T-") && name.ends_with(".json"))
            .unwrap_or(false)
    }) {
        let Some(sig) = file_sig(&file) else {
            continue;
        };
        let key = file.display().to_string();
        if files.get(&key) == Some(&sig) {
            continue;
        }
        let Some(thread) = read_json(&file) else {
            continue;
        };
        let thread_id = text(thread.get("id")).unwrap_or_else(|| key.clone());
        let messages = thread
            .get("messages")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let ledger = thread
            .get("usageLedger")
            .and_then(|value| value.get("events"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        if !ledger.is_empty() {
            for (index, event) in ledger.iter().enumerate() {
                let dedup = format!("{thread_id}|ledger|{index}");
                if seen_set.contains(&dedup) {
                    continue;
                }
                let Some(stamp) = timestamp(event.get("timestamp")) else {
                    continue;
                };
                if !ctx.in_window(stamp) {
                    continue;
                }
                let input = number(event.get("tokens").and_then(|value| value.get("input")));
                let output = number(event.get("tokens").and_then(|value| value.get("output")));
                if input == 0 && output == 0 {
                    continue;
                }
                let cache_read = event
                    .get("toMessageId")
                    .and_then(Value::as_u64)
                    .and_then(|index| messages.get(index as usize))
                    .and_then(|message| message.get("usage"))
                    .and_then(|usage| usage.get("cacheReadInputTokens"))
                    .map(|value| number(Some(value)))
                    .unwrap_or(0);
                let model = text(event.get("model")).unwrap_or_else(|| "unknown".to_owned());
                ctx.add(
                    SOURCE,
                    COLLECTOR,
                    &model,
                    "unknown",
                    &half_hour(stamp),
                    input,
                    output,
                    cache_read,
                    0,
                    0,
                );
                seen_set.insert(dedup.clone());
                seen.push(dedup);
            }
        } else {
            let created = thread.get("created").cloned();
            for (index, message) in messages.iter().enumerate() {
                let dedup = format!("{thread_id}|msg|{index}");
                if seen_set.contains(&dedup) {
                    continue;
                }
                let Some(usage) = message.get("usage") else {
                    continue;
                };
                let Some(stamp) = timestamp(message.get("timestamp").or(created.as_ref())) else {
                    continue;
                };
                if !ctx.in_window(stamp) {
                    continue;
                }
                let input = number(usage.get("inputTokens"));
                let output = number(usage.get("outputTokens"));
                let cache_read = number(usage.get("cacheReadInputTokens"));
                if input == 0 && output == 0 && cache_read == 0 {
                    continue;
                }
                let model = text(usage.get("model")).unwrap_or_else(|| "unknown".to_owned());
                ctx.add(
                    SOURCE,
                    COLLECTOR,
                    &model,
                    "unknown",
                    &half_hour(stamp),
                    input,
                    output,
                    cache_read,
                    0,
                    0,
                );
                seen_set.insert(dedup.clone());
                seen.push(dedup);
            }
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
