use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::Path;

use super::{
    env_path, file_sig, half_hour, home_dir, list_dirs, number, read_json_lines, text, timestamp,
    Ctx, FileSig,
};

pub const SOURCE: &str = "qwen";
pub const COLLECTOR: &str = "qwen-code";

fn root_dir() -> std::path::PathBuf {
    env_path("QWEN_TMP_DIR").unwrap_or_else(|| home_dir().join(".qwen").join("tmp"))
}

fn project_name(cwd: Option<&Value>, file: &Path, base: &Path) -> String {
    if let Some(cwd) = text(cwd) {
        if let Some(name) = Path::new(&cwd).file_name().and_then(|value| value.to_str()) {
            return name.to_owned();
        }
    }
    if let Ok(relative) = file.strip_prefix(base) {
        if let Some(first) = relative.components().next() {
            if let Some(name) = first.as_os_str().to_str() {
                return name.to_owned();
            }
        }
    }
    "unknown".to_owned()
}

pub fn collect(ctx: &mut Ctx, previous: Option<Value>) -> Value {
    let previous = previous.unwrap_or_else(|| json!({}));
    let mut files: BTreeMap<String, FileSig> = previous
        .get("files")
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
        .map(|array| {
            array
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let mut seen_set: std::collections::HashSet<String> = seen.iter().cloned().collect();

    let base = root_dir();
    for project_dir in list_dirs(&base) {
        let chats = project_dir.join("chats");
        let Ok(entries) = std::fs::read_dir(&chats) else {
            continue;
        };
        let mut session_files: Vec<std::path::PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("jsonl"))
            .collect();
        session_files.sort();

        for file in session_files {
            let Some(sig) = file_sig(&file) else {
                continue;
            };
            let key = file.display().to_string();
            if files.get(&key) == Some(&sig) {
                continue;
            }
            let base_for_project = base.clone();
            read_json_lines(&file, |event| {
                if event.get("type").and_then(Value::as_str) != Some("assistant") {
                    return;
                }
                let Some(usage) = event.get("usageMetadata") else {
                    return;
                };
                if usage.get("promptTokenCount").is_none()
                    && usage.get("candidatesTokenCount").is_none()
                {
                    return;
                }
                let uuid = text(event.get("uuid"));
                if let Some(uuid) = &uuid {
                    if seen_set.contains(uuid) {
                        return;
                    }
                }
                let Some(stamp) = timestamp(event.get("timestamp")) else {
                    return;
                };
                if !ctx.in_window(stamp) {
                    return;
                }
                let cached = number(usage.get("cachedContentTokenCount"));
                let thoughts = number(usage.get("thoughtsTokenCount"));
                let prompt = number(
                    usage
                        .get("promptTokenCount")
                        .or_else(|| usage.get("input_tokens")),
                );
                let candidates = number(
                    usage
                        .get("candidatesTokenCount")
                        .or_else(|| usage.get("output_tokens")),
                );
                let input = prompt.saturating_sub(cached);
                let output = candidates.saturating_sub(thoughts);
                if input == 0 && output == 0 && cached == 0 && thoughts == 0 {
                    return;
                }
                let model = text(event.get("model")).unwrap_or_else(|| "unknown".to_owned());
                let project = project_name(event.get("cwd"), &file, &base_for_project);
                ctx.add(
                    SOURCE,
                    COLLECTOR,
                    &model,
                    &project,
                    &half_hour(stamp),
                    input,
                    output,
                    cached,
                    0,
                    thoughts,
                );
                if let Some(uuid) = uuid {
                    seen_set.insert(uuid.clone());
                    seen.push(uuid);
                }
            });
            files.insert(key, sig);
            ctx.files_processed += 1;
        }
    }

    if seen.len() > 50_000 {
        seen = seen.split_off(seen.len() - 50_000);
    }
    let files_json: Map<String, Value> = files
        .into_iter()
        .filter_map(|(key, sig)| serde_json::to_value(sig).ok().map(|value| (key, value)))
        .collect();
    json!({ "files": files_json, "seenIds": seen })
}
