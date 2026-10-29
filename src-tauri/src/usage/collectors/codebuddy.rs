use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashSet};

use super::{
    env_path, file_sig, half_hour, home_dir, list_files_recursive, number, read_json_lines_from,
    text, timestamp, Ctx,
};

pub const SOURCE: &str = "codebuddy";
pub const COLLECTOR: &str = "codebuddy";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct OffsetSig {
    size: u64,
    mtime_ms: u64,
    inode: u64,
    offset: u64,
}

fn root_dir() -> std::path::PathBuf {
    env_path("CODEBUDDY_HOME").unwrap_or_else(|| home_dir().join(".codebuddy"))
}

fn default_model() -> String {
    let path = root_dir().join("settings.json");
    super::read_json(&path)
        .and_then(|value| text(value.get("model")))
        .unwrap_or_else(|| "codebuddy-unknown".to_owned())
}

fn normalize(raw: &Value) -> Option<(u64, u64, u64, u64, u64)> {
    let prompt = number(raw.get("prompt_tokens"));
    let completion = number(raw.get("completion_tokens"));
    let details = raw.get("prompt_tokens_details");
    let cached_tokens = number(details.and_then(|value| value.get("cached_tokens")));
    let cache_read_alt = number(raw.get("cache_read_input_tokens"));
    let cache_creation = number(raw.get("cache_creation_input_tokens"));
    let reasoning = number(details.and_then(|value| value.get("reasoning_tokens")));
    let cache_read = cached_tokens.max(cache_read_alt);
    let input = prompt.saturating_sub(cache_read);
    if input == 0 && completion == 0 && cache_read == 0 && cache_creation == 0 {
        return None;
    }
    Some((input, completion, cache_read, cache_creation, reasoning))
}

pub fn collect(ctx: &mut Ctx, previous: Option<Value>) -> Value {
    let previous = previous.unwrap_or_else(|| json!({}));
    let mut files: BTreeMap<String, OffsetSig> = previous
        .get("fileOffsets")
        .and_then(Value::as_object)
        .map(|object| {
            object
                .iter()
                .filter_map(|(key, value)| {
                    serde_json::from_value::<OffsetSig>(value.clone())
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
    let fallback_model = default_model();

    for file in list_files_recursive(&root_dir().join("projects"), &|path| {
        path.extension().and_then(|value| value.to_str()) == Some("jsonl")
    }) {
        let Some(sig) = file_sig(&file) else {
            continue;
        };
        let key = file.display().to_string();
        let previous_sig = files.get(&key).cloned();
        let inode_changed = previous_sig
            .as_ref()
            .map(|value| value.inode != sig.inode)
            .unwrap_or(false);
        let start = match &previous_sig {
            Some(value) if !inode_changed && sig.size >= value.size => value.offset,
            _ => 0,
        };
        if sig.size <= start {
            continue;
        }
        let (lines, next_offset) = read_json_lines_from(&file, start);
        let session_id = file
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("unknown")
            .to_owned();

        for entry in lines {
            if entry.get("type").and_then(Value::as_str) != Some("message")
                || entry.get("role").and_then(Value::as_str) != Some("assistant")
            {
                continue;
            }
            let Some(raw_usage) = entry
                .get("providerData")
                .and_then(|value| value.get("rawUsage"))
            else {
                continue;
            };
            let Some(stamp) = timestamp(entry.get("timestamp")) else {
                continue;
            };
            if !ctx.in_window(stamp) {
                continue;
            }
            let Some((input, output, cache_read, cache_write, reasoning)) = normalize(raw_usage)
            else {
                continue;
            };
            let message_id = text(entry.get("uuid"))
                .or_else(|| text(entry.get("id")))
                .unwrap_or_else(|| format!("{session_id}:{}", stamp.timestamp_millis()));
            if seen_set.contains(&message_id) {
                continue;
            }
            let model = text(entry.get("providerData").and_then(|value| value.get("model")))
                .or_else(|| text(entry.get("model")))
                .unwrap_or_else(|| fallback_model.clone());
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
                reasoning,
            );
            seen_set.insert(message_id.clone());
            seen.push(message_id);
        }

        files.insert(
            key,
            OffsetSig {
                size: sig.size,
                mtime_ms: sig.mtime_ms,
                inode: sig.inode,
                offset: next_offset,
            },
        );
        ctx.files_processed += 1;
    }

    if seen.len() > 10_000 {
        seen = seen.split_off(seen.len() - 10_000);
    }
    let files_json: Map<String, Value> = files
        .into_iter()
        .filter_map(|(key, sig)| serde_json::to_value(sig).ok().map(|value| (key, value)))
        .collect();
    json!({ "fileOffsets": files_json, "seenIds": seen })
}
