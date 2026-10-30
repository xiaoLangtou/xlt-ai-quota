use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use super::{env_path, half_hour, home_dir, list_dirs, read_json, text, Ctx};

pub const SOURCE: &str = "jetbrains-acp";
pub const COLLECTOR: &str = "jetbrains-acp";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct FileCursor {
    used: u64,
    mtime_ms: u64,
    #[serde(default)]
    cost: f64,
}

/// JetBrains 各 IDE 的 `aia-task-history` 根目录（WebStorm / IntelliJ / PyCharm…）。
fn history_roots() -> Vec<PathBuf> {
    if let Some(path) = env_path("JETBRAINS_ACP_DIR") {
        return vec![path];
    }
    let mut bases: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "macos") {
        bases.push(home_dir().join("Library").join("Application Support").join("JetBrains"));
    } else if cfg!(target_os = "windows") {
        bases.push(
            std::env::var("APPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|_| home_dir().join("AppData").join("Roaming"))
                .join("JetBrains"),
        );
    } else {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            bases.push(PathBuf::from(xdg).join("JetBrains"));
        }
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            bases.push(PathBuf::from(xdg).join("JetBrains"));
        }
        bases.push(home_dir().join(".local").join("share").join("JetBrains"));
        bases.push(home_dir().join(".config").join("JetBrains"));
    }

    let mut roots = Vec::new();
    for base in bases {
        for product in list_dirs(&base) {
            let history = product.join("aia-task-history");
            if history.is_dir() {
                roots.push(history);
            }
        }
    }
    roots.sort();
    roots.dedup();
    roots
}

fn map_agent(raw: &str) -> &'static str {
    let agent = raw.to_lowercase();
    if agent.contains("codex") {
        "codex-acp"
    } else if agent.contains("claude") {
        "claude-acp"
    } else if agent.contains("opencode") {
        "opencode-acp"
    } else if agent.contains("kiro") {
        "kiro-acp"
    } else if agent.contains("cursor") {
        "cursor-acp"
    } else if agent.contains("qoder") {
        "qoder-acp"
    } else if agent.contains("codebuddy") {
        "codebuddy-acp"
    } else if agent.contains("kimi") {
        "kimi-acp"
    } else if agent.contains("junie") {
        "junie"
    } else if agent.contains("devin") {
        "devin"
    } else if agent.contains("mimo") {
        "mimo-acp"
    } else if agent.contains("copilot") {
        "copilot-acp"
    } else if agent.contains("auggie") {
        "auggie"
    } else {
        SOURCE
    }
}

/// 这些 ACP agent 会同时把会话写进自己的本地 store，已有采集器覆盖，避免重复计数。
fn should_count(agent: &str) -> bool {
    let agent = agent.to_lowercase();
    !(agent.contains("codex")
        || agent.contains("claude")
        || agent.contains("opencode")
        || agent.contains("kiro")
        || agent.contains("qoder"))
}

fn read_agent(stem: &Path) -> String {
    let path = stem.with_extension("agentsession");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return "jetbrains-acp".to_owned();
    };
    let first = raw.lines().next().unwrap_or("").trim();
    if first.is_empty() {
        return "jetbrains-acp".to_owned();
    }
    map_agent(first.split(':').next().unwrap_or(first)).to_owned()
}

fn file_mtime_ms(path: &Path) -> u64 {
    std::fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn usage_files(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("usage"))
        .collect();
    files.sort();
    files
}

/// JetBrains AI / ACP 会话用量。
///
/// `.usage` 只有累计 `used` 与可选 `cost`，没有输入/输出拆分；这里按文件累计值做差分，
/// 把增量记为 `input_tokens`（总量准确，输入/输出拆分无法还原），并把 `cost.amount`
/// 作为 reported cost。源按 ACP agent 区分，避免和本地 CLI 采集器重复。
pub fn collect(ctx: &mut Ctx, previous: Option<Value>) -> Value {
    let previous = previous.unwrap_or_else(|| json!({}));
    let mut files: BTreeMap<String, FileCursor> = previous
        .get("files")
        .and_then(Value::as_object)
        .map(|object| {
            object
                .iter()
                .filter_map(|(key, value)| {
                    serde_json::from_value::<FileCursor>(value.clone())
                        .ok()
                        .map(|cursor| (key.clone(), cursor))
                })
                .collect()
        })
        .unwrap_or_default();

    for root in history_roots() {
        for path in usage_files(&root) {
            let Some(document) = read_json(&path) else {
                continue;
            };
            let used = document.get("used").and_then(Value::as_u64).unwrap_or(0);
            if used == 0 {
                continue;
            }
            let mtime_ms = file_mtime_ms(&path);
            let key = path.display().to_string();
            let previous_used = files.get(&key).map(|cursor| cursor.used).unwrap_or(0);
            let previous_cost = files.get(&key).map(|cursor| cursor.cost).unwrap_or(0.0);
            let delta = if used >= previous_used {
                used - previous_used
            } else {
                used
            };

            // 只有 USD 计价的会话带真实费用；其他币种不猜汇率。
            let cost = document.get("cost");
            let amount = cost
                .and_then(|value| value.get("amount"))
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            let currency = text(cost.and_then(|value| value.get("currency")))
                .unwrap_or_else(|| "USD".to_owned());
            let cost_delta = if amount >= previous_cost {
                amount - previous_cost
            } else {
                amount
            };

            let cursor = FileCursor {
                used,
                mtime_ms,
                cost: amount,
            };
            if delta == 0 {
                files.insert(key, cursor);
                continue;
            }

            let Some(stamp) = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(mtime_ms as i64)
            else {
                files.insert(key, cursor);
                continue;
            };
            // 窗口外的文件不推进游标，之后扩大统计区间时还能补采。
            if !ctx.in_window(stamp) {
                continue;
            }
            let agent = read_agent(&path.with_extension(""));
            files.insert(key, cursor);
            if !should_count(&agent) {
                continue;
            }
            ctx.add(
                &agent,
                COLLECTOR,
                "jetbrains-acp",
                "unknown",
                &half_hour(stamp),
                delta,
                0,
                0,
                0,
                0,
            );

            if cost_delta > 0.0 && currency.eq_ignore_ascii_case("USD") {
                ctx.add_cost(
                    &agent,
                    COLLECTOR,
                    "jetbrains-acp",
                    "unknown",
                    &half_hour(stamp),
                    cost_delta,
                );
            }
        }
    }

    let files_json: Map<String, Value> = files
        .into_iter()
        .filter_map(|(key, cursor)| serde_json::to_value(cursor).ok().map(|value| (key, value)))
        .collect();
    json!({ "files": files_json })
}
