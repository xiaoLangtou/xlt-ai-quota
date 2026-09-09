use base64::{
    engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD},
    Engine as _,
};
use chrono::{DateTime, Local, NaiveDate, Utc};
use regex::Regex;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorRequest {
    base_url: String,
    path: String,
    #[serde(default)]
    query: BTreeMap<String, String>,
}

#[tauri::command]
pub async fn connector_get(request: ConnectorRequest) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || connector_get_blocking(request))
        .await
        .map_err(|error| format!("本地连接器后台任务异常: {error}"))?
}

fn connector_get_blocking(request: ConnectorRequest) -> Result<Value, String> {
    match (request.base_url.as_str(), request.path.as_str()) {
        ("/api/ark", "/status") => ark_status(),
        ("/api/ark", "/plan") => ark_plan(),
        ("/api/ark", "/stats") => ark_stats(&request.query),
        ("/api/kiro", "/usage") => kiro_usage(),
        ("/api/qoder", "/usage") => qoder_usage(),
        ("/api/opencode", "/stats") => opencode_stats(),
        ("/api/codex", "/status") => codex_status(),
        ("/api/codex", "/usage") => codex_usage(),
        ("/api/codex", "/stats") => codex_stats(&request.query),
        ("/api/claude", "/stats") => claude_stats(&request.query),
        _ => Err("桌面端不支持该连接器请求".to_owned()),
    }
}

fn ark_status() -> Result<Value, String> {
    run_json(
        "arkcli",
        "ARKCLI_BIN",
        &["auth", "status", "--format", "json"],
    )
}

fn ark_plan() -> Result<Value, String> {
    run_json(
        "arkcli",
        "ARKCLI_BIN",
        &["usage", "plan", "--format", "json"],
    )
}

fn ark_stats(query: &BTreeMap<String, String>) -> Result<Value, String> {
    let start = requested_date(query, "start")?;
    let end = requested_date(query, "end")?;
    if start > end {
        return Err("start 必须早于或等于 end".to_owned());
    }

    let start = start.format("%F").to_string();
    let end = end.format("%F").to_string();
    let attempts: [Vec<&str>; 3] = [
        vec![
            "usage", "stats", "--start", &start, "--end", &end, "--mine", "--format", "json",
        ],
        vec![
            "usage",
            "stats",
            "--start",
            &start,
            "--end",
            &end,
            "--mine",
            "--mine-by",
            "apikey",
            "--format",
            "json",
        ],
        vec![
            "usage", "stats", "--start", &start, "--end", &end, "--format", "json",
        ],
    ];
    let mut last_response: Option<Value> = None;
    let mut last_error = String::new();

    for args in attempts {
        match run_json_with_timeout("arkcli", "ARKCLI_BIN", &args, 15) {
            Ok(data) => {
                if data
                    .get("data_count")
                    .and_then(Value::as_i64)
                    .unwrap_or_default()
                    > 0
                {
                    return Ok(data);
                }
                last_response = Some(data);
            }
            Err(error) => last_error = error,
        }
    }

    last_response.ok_or_else(|| format!("arkcli usage stats 失败: {last_error}"))
}

fn kiro_usage() -> Result<Value, String> {
    let (stdout, stderr) = run_cli(
        "kiro-cli",
        "KIROCLI_BIN",
        &["chat", "/usage", "--no-interactive"],
    )?;
    parse_kiro_usage(&format!("{stdout}\n{stderr}"))
}

/// Qoder CLI 的用量查询只通过官方 Agent SDK 暴露。桌面开发模式从项目内的
/// SDK 脚本启动独立进程，复用已登录的 qodercli 凭证，避免前端绕过 Tauri 访问本机状态。
fn qoder_usage() -> Result<Value, String> {
    let root = qoder_sdk_project_root()?;
    let script = root.join("scripts").join("qoder-usage.mjs");
    if !script.is_file() {
        return Err("未找到 Qoder 用量查询脚本".to_owned());
    }
    let node = executable("node", "NODE_BIN");
    let args = vec![script.to_string_lossy().into_owned()];
    let (stdout, stderr) = run_program_in_dir(&node, &args, 35, &root)?;
    serde_json::from_str(&stdout).map_err(|_| {
        if stderr.trim().is_empty() {
            "Qoder 用量查询没有返回 JSON".to_owned()
        } else {
            format!("Qoder 用量查询失败: {}", snippet(&stderr))
        }
    })
}

fn qoder_sdk_project_root() -> Result<PathBuf, String> {
    let build_project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf);
    let executable_dir = env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf));
    for candidate in [
        env::var_os("QODER_SDK_PROJECT_ROOT").map(PathBuf::from),
        build_project_root,
        env::current_dir().ok(),
        executable_dir,
    ]
    .into_iter()
    .flatten()
    {
        if let Some(root) = find_qoder_sdk_project_root(candidate) {
            return Ok(root);
        }
    }
    Err("未找到 Qoder Agent SDK；请重新安装项目依赖".to_owned())
}

fn find_qoder_sdk_project_root(mut current: PathBuf) -> Option<PathBuf> {
    loop {
        if current
            .join("node_modules")
            .join("@qoder-ai")
            .join("qoder-agent-sdk")
            .join("package.json")
            .is_file()
        {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

fn opencode_stats() -> Result<Value, String> {
    let sql = "SELECT date(time_created/1000,'unixepoch','localtime') AS d, \
        sum(json_extract(data,'$.tokens.input')) AS inp, \
        sum(json_extract(data,'$.tokens.output')) AS outp, \
        sum(COALESCE(json_extract(data,'$.tokens.cache.read'),0)) AS cache \
        FROM message WHERE json_extract(data,'$.role')='assistant' \
        AND json_extract(data,'$.tokens.input') IS NOT NULL GROUP BY d ORDER BY d";
    run_json("opencode", "OPENCODE_BIN", &["db", sql, "--format", "json"])
}

fn codex_stats(query: &BTreeMap<String, String>) -> Result<Value, String> {
    let start = requested_date(query, "start")?;
    let end = requested_date(query, "end")?;
    if start > end {
        return Err("start 必须早于或等于 end".to_owned());
    }

    let mut days = Vec::new();
    let mut day = start;
    while day <= end {
        days.push(day);
        day = day
            .succ_opt()
            .ok_or_else(|| "日期范围超出支持范围".to_owned())?;
    }

    let mut totals: BTreeMap<String, TokenStats> = BTreeMap::new();
    let sessions_root = home_dir().join(".codex").join("sessions");
    for day in days {
        let folder = sessions_root
            .join(day.format("%Y").to_string())
            .join(day.format("%m").to_string())
            .join(day.format("%d").to_string());
        for file in jsonl_files(&folder) {
            for_each_json_line(&file, |event| {
                let payload = event.get("payload");
                let usage = payload
                    .and_then(|value| value.get("info"))
                    .and_then(|value| value.get("last_token_usage"));
                let event_day = event
                    .get("timestamp")
                    .and_then(Value::as_str)
                    .and_then(date_prefix);
                if event.get("type").and_then(Value::as_str) != Some("event_msg")
                    || payload
                        .and_then(|value| value.get("type"))
                        .and_then(Value::as_str)
                        != Some("token_count")
                    || event_day != Some(day.format("%F").to_string())
                    || usage.is_none()
                {
                    return;
                }
                let usage = usage.expect("usage 已检查");
                add_tokens(
                    &mut totals,
                    &day.format("%F").to_string(),
                    value_number(usage.get("input_tokens")),
                    value_number(usage.get("output_tokens")),
                    value_number(usage.get("cached_input_tokens"))
                        + value_number(usage.get("cache_write_input_tokens")),
                );
            });
        }
    }
    Ok(token_stats_json(totals))
}

fn claude_stats(query: &BTreeMap<String, String>) -> Result<Value, String> {
    let start = requested_date(query, "start")?;
    let end = requested_date(query, "end")?;
    if start > end {
        return Err("start 必须早于或等于 end".to_owned());
    }

    let mut totals: BTreeMap<String, TokenStats> = BTreeMap::new();
    for file in jsonl_files(&home_dir().join(".claude").join("projects")) {
        for_each_json_line(&file, |event| {
            let message = event.get("message");
            let usage = message.and_then(|value| value.get("usage"));
            let Some(day) = event
                .get("timestamp")
                .and_then(Value::as_str)
                .and_then(date_prefix)
            else {
                return;
            };
            let Ok(parsed_day) = NaiveDate::parse_from_str(&day, "%F") else {
                return;
            };
            if event.get("type").and_then(Value::as_str) != Some("assistant")
                || message
                    .and_then(|value| value.get("role"))
                    .and_then(Value::as_str)
                    != Some("assistant")
                || usage.is_none()
                || parsed_day < start
                || parsed_day > end
            {
                return;
            }
            let usage = usage.expect("usage 已检查");
            let cached = value_number(usage.get("cache_creation_input_tokens"))
                + value_number(usage.get("cache_read_input_tokens"));
            add_tokens(
                &mut totals,
                &day,
                value_number(usage.get("input_tokens")) + cached,
                value_number(usage.get("output_tokens")),
                cached,
            );
        });
    }
    Ok(token_stats_json(totals))
}

fn codex_status() -> Result<Value, String> {
    let status = run_cli("codex", "CODEX_BIN", &["login", "status"])
        .map(|(stdout, stderr)| format!("{stdout}\n{stderr}"))
        .unwrap_or_default();
    let claims = codex_claims().unwrap_or_else(|_| Value::Null);
    let auth = claims
        .get("https://api.openai.com/auth")
        .unwrap_or(&Value::Null);
    let plan_type = auth.get("chatgpt_plan_type").and_then(Value::as_str);
    Ok(json!({
        "loggedIn": status.to_lowercase().contains("logged in"),
        "email": claims.get("email").and_then(Value::as_str),
        "planType": plan_type,
        "planTag": plan_type.map(capitalize),
        "subscriptionActiveUntil": auth.get("chatgpt_subscription_active_until").and_then(Value::as_str),
    }))
}

fn codex_usage() -> Result<Value, String> {
    let auth = read_codex_auth()?;
    let access_token = auth
        .pointer("/tokens/access_token")
        .and_then(Value::as_str)
        .ok_or_else(|| "auth.json 无 access_token（未登录？）".to_owned())?;
    let claims = auth
        .pointer("/tokens/id_token")
        .and_then(Value::as_str)
        .map(decode_jwt)
        .transpose()?
        .unwrap_or(Value::Null);
    let account = claims
        .get("https://api.openai.com/auth")
        .unwrap_or(&Value::Null);
    let email = claims.get("email").and_then(Value::as_str);
    let plan_type = account.get("chatgpt_plan_type").and_then(Value::as_str);
    let account_id = account.get("chatgpt_account_id").and_then(Value::as_str);

    let mut args = vec![
        "-skS".to_owned(),
        "--connect-timeout".to_owned(),
        "8".to_owned(),
        "--max-time".to_owned(),
        "12".to_owned(),
        "-H".to_owned(),
        format!("Authorization: Bearer {access_token}"),
        "-H".to_owned(),
        "User-Agent: xlt-workbench/0.1".to_owned(),
    ];
    if let Some(account_id) = account_id {
        args.push("-H".to_owned());
        args.push(format!("ChatGPT-Account-Id: {account_id}"));
    }
    add_https_proxy(&mut args);
    args.push("https://chatgpt.com/backend-api/wham/usage".to_owned());
    let (stdout, stderr) = run_program("curl", &args, 35)?;
    let data: Value = serde_json::from_str(&stdout).map_err(|_| {
        if stderr.trim().is_empty() {
            "Codex 额度请求没有返回 JSON".to_owned()
        } else {
            format!("Codex 额度网络请求失败: {}", snippet(&stderr))
        }
    })?;
    let primary = codex_window(data.pointer("/rate_limit/primary_window"));
    let secondary = codex_window(data.pointer("/rate_limit/secondary_window"));
    Ok(json!({
        "email": email,
        "planType": plan_type,
        "planTag": plan_type.map(capitalize),
        "primary": primary,
        "secondary": secondary,
    }))
}

fn parse_kiro_usage(output: &str) -> Result<Value, String> {
    let ansi = Regex::new(r"\x1b\[[0-?]*[ -/]*[@-~]").map_err(|error| error.to_string())?;
    let text = ansi.replace_all(output, "");
    let credits = Regex::new(r"(?i)Credits\s*(?:\(\s*)?([\d,.]+)\s*(?:of|used\s*/)\s*([\d,.]+)\s*(?:covered\s+in\s+(?:the\s+)?plan)?\)?")
        .map_err(|error| error.to_string())?;
    let Some(captures) = credits.captures(&text) else {
        return Err("未匹配到 Kiro Credits 行".to_owned());
    };
    let used = captures
        .get(1)
        .and_then(|value| value.as_str().replace(',', "").parse::<f64>().ok())
        .unwrap_or_default();
    let total = captures
        .get(2)
        .and_then(|value| value.as_str().replace(',', "").parse::<f64>().ok())
        .unwrap_or_default();
    let resets =
        Regex::new(r"(?i)resets on\s+(\d{4}-\d{2}-\d{2})").map_err(|error| error.to_string())?;
    let plan = Regex::new(r"(?i)Estimated Usage[^|]*\|[^|]*\|\s*([^\n]+)")
        .map_err(|error| error.to_string())?;
    Ok(json!({
        "used": used,
        "total": total,
        "resetsAt": resets.captures(&text).and_then(|value| value.get(1)).map(|value| format!("{}T00:00:00+08:00", value.as_str())),
        "planTag": plan.captures(&text).and_then(|value| value.get(1)).map(|value| value.as_str().trim()),
    }))
}

#[cfg(test)]
mod kiro_usage_tests {
    use super::parse_kiro_usage;

    #[test]
    fn parses_ansi_colored_credits_output() {
        let output = "\x1b[1mEstimated Usage\x1b[0m | resets on 2026-10-01 | \x1b[mKIRO PRO+\x1b[0m\n\x1b[1mCredits\x1b[0m (9.09 of 2000 covered in plan)";
        let parsed = parse_kiro_usage(output).expect("Kiro Credits 应可解析");
        assert_eq!(parsed["used"], 9.09);
        assert_eq!(parsed["total"], 2000.0);
        assert_eq!(parsed["planTag"], "KIRO PRO+");
    }
}

fn requested_date(query: &BTreeMap<String, String>, key: &str) -> Result<NaiveDate, String> {
    let default = Local::now().date_naive().format("%F").to_string();
    let raw = query.get(key).map(String::as_str).unwrap_or(&default);
    NaiveDate::parse_from_str(raw, "%F").map_err(|_| format!("{key} 必须是 YYYY-MM-DD"))
}

fn run_json(program: &str, env_key: &str, args: &[&str]) -> Result<Value, String> {
    run_json_with_timeout(program, env_key, args, 30)
}

fn run_json_with_timeout(
    program: &str,
    env_key: &str,
    args: &[&str],
    timeout_seconds: u64,
) -> Result<Value, String> {
    let (stdout, stderr) = run_cli_with_timeout(program, env_key, args, timeout_seconds)?;
    serde_json::from_str(&stdout)
        .or_else(|_| serde_json::from_str(&stderr))
        .map_err(|_| {
            format!(
                "{program} 返回不是 JSON（stdout {} 字节；stderr：{}）",
                stdout.trim().len(),
                snippet(&stderr)
            )
        })
}

fn run_cli(program: &str, env_key: &str, args: &[&str]) -> Result<(String, String), String> {
    run_cli_with_timeout(program, env_key, args, 30)
}

fn run_cli_with_timeout(
    program: &str,
    env_key: &str,
    args: &[&str],
    timeout_seconds: u64,
) -> Result<(String, String), String> {
    let args = args
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    run_program(&executable(program, env_key), &args, timeout_seconds)
}

fn run_program(
    program: &str,
    args: &[String],
    timeout_seconds: u64,
) -> Result<(String, String), String> {
    let mut child = Command::new(program)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|error| format!("无法运行 {program}: {error}"))?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_seconds);

    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait_with_output();
                return Err(format!("{program} 超过 {timeout_seconds} 秒未返回，已终止"));
            }
            Err(error) => return Err(format!("无法等待 {program}: {error}")),
        }
    }

    let output = child
        .wait_with_output()
        .map_err(|error| format!("无法读取 {program} 输出: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if stdout.trim().is_empty() && stderr.trim().is_empty() {
        return Err(format!(
            "{program} 以退出码 {} 结束且未返回数据",
            output.status
        ));
    }
    Ok((stdout, stderr))
}

fn run_program_in_dir(
    program: &str,
    args: &[String],
    timeout_seconds: u64,
    cwd: &Path,
) -> Result<(String, String), String> {
    let mut child = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|error| format!("无法运行 {program}: {error}"))?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_seconds);

    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait_with_output();
                return Err(format!("{program} 超过 {timeout_seconds} 秒未返回，已终止"));
            }
            Err(error) => return Err(format!("无法等待 {program}: {error}")),
        }
    }

    let output = child
        .wait_with_output()
        .map_err(|error| format!("无法读取 {program} 输出: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if stdout.trim().is_empty() && stderr.trim().is_empty() {
        return Err(format!(
            "{program} 以退出码 {} 结束且未返回数据",
            output.status
        ));
    }
    Ok((stdout, stderr))
}

fn executable(program: &str, env_key: &str) -> String {
    if let Ok(configured) = env::var(env_key) {
        if !configured.trim().is_empty() {
            return configured;
        }
    }
    let home = home_dir();
    for candidate in [
        home.join(".local").join("bin").join(program),
        home.join(".volta").join("bin").join(program),
        PathBuf::from("/opt/homebrew/bin").join(program),
        PathBuf::from("/usr/local/bin").join(program),
    ] {
        if candidate.is_file() {
            return candidate.to_string_lossy().into_owned();
        }
    }
    program.to_owned()
}

fn add_https_proxy(args: &mut Vec<String>) {
    if let Some(proxy) = https_proxy() {
        args.push("--proxy".to_owned());
        args.push(proxy);
    }
}

fn https_proxy() -> Option<String> {
    for key in ["HTTPS_PROXY", "https_proxy"] {
        if let Ok(proxy) = env::var(key) {
            if !proxy.trim().is_empty() {
                return Some(proxy);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        return macos_system_https_proxy();
    }

    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

#[cfg(target_os = "macos")]
fn macos_system_https_proxy() -> Option<String> {
    let output = Command::new("/usr/sbin/scutil")
        .arg("--proxy")
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let value = |key: &str| {
        text.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            (name.trim() == key).then(|| value.trim())
        })
    };
    if value("HTTPSEnable") != Some("1") {
        return None;
    }
    let host = value("HTTPSProxy")?;
    let port = value("HTTPSPort")?.parse::<u16>().ok()?;
    Some(format!("http://{host}:{port}"))
}

fn home_dir() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn jsonl_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    visit_jsonl(root, &mut files);
    files
}

fn visit_jsonl(path: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let entry_path = entry.path();
        if entry_path.is_dir() {
            visit_jsonl(&entry_path, files);
        } else if entry_path
            .extension()
            .and_then(|extension| extension.to_str())
            == Some("jsonl")
        {
            files.push(entry_path);
        }
    }
}

fn for_each_json_line(file: &Path, mut visit: impl FnMut(&Value)) {
    let Ok(content) = fs::read_to_string(file) else {
        return;
    };
    for line in content.lines() {
        if let Ok(value) = serde_json::from_str::<Value>(line) {
            visit(&value);
        }
    }
}

#[derive(Default)]
struct TokenStats {
    input: f64,
    output: f64,
    cached: f64,
    requests: u64,
}

fn add_tokens(
    totals: &mut BTreeMap<String, TokenStats>,
    day: &str,
    input: f64,
    output: f64,
    cached: f64,
) {
    if input == 0.0 && output == 0.0 && cached == 0.0 {
        return;
    }
    let current = totals.entry(day.to_owned()).or_default();
    current.input += input;
    current.output += output;
    current.cached += cached;
    current.requests += 1;
}

fn token_stats_json(totals: BTreeMap<String, TokenStats>) -> Value {
    Value::Array(totals.into_iter().map(|(day, stats)| {
        json!({ "d": day, "inp": stats.input, "outp": stats.output, "cache": stats.cached, "requests": stats.requests })
    }).collect())
}

fn value_number(value: Option<&Value>) -> f64 {
    value
        .and_then(Value::as_f64)
        .or_else(|| {
            value
                .and_then(Value::as_str)
                .and_then(|value| value.parse::<f64>().ok())
        })
        .unwrap_or_default()
}

fn date_prefix(value: &str) -> Option<String> {
    let prefix = value.get(..10)?;
    NaiveDate::parse_from_str(prefix, "%F")
        .ok()
        .map(|_| prefix.to_owned())
}

fn read_codex_auth() -> Result<Value, String> {
    let path = home_dir().join(".codex").join("auth.json");
    let content = fs::read_to_string(&path)
        .map_err(|error| format!("无法读取 {}: {error}", path.display()))?;
    serde_json::from_str(&content).map_err(|error| format!("Codex auth.json 格式错误: {error}"))
}

fn codex_claims() -> Result<Value, String> {
    let auth = read_codex_auth()?;
    let id_token = auth
        .pointer("/tokens/id_token")
        .and_then(Value::as_str)
        .ok_or_else(|| "auth.json 无 id_token".to_owned())?;
    decode_jwt(id_token)
}

fn decode_jwt(token: &str) -> Result<Value, String> {
    let payload = token
        .split('.')
        .nth(1)
        .ok_or_else(|| "JWT 缺少 payload".to_owned())?;
    let bytes = URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| URL_SAFE.decode(payload))
        .map_err(|_| "JWT payload 无法解码".to_owned())?;
    serde_json::from_slice(&bytes).map_err(|_| "JWT payload 不是 JSON".to_owned())
}

fn codex_window(window: Option<&Value>) -> Option<Value> {
    let window = window?;
    let used_percent = window.get("used_percent")?.as_f64()?;
    let reset_at = window.get("reset_at")?.as_i64()?;
    let resets_at = DateTime::<Utc>::from_timestamp(reset_at, 0)?.to_rfc3339();
    Some(json!({
        "usedPercent": used_percent.round(),
        "windowSeconds": window.get("limit_window_seconds").and_then(Value::as_i64).unwrap_or_default(),
        "resetsAt": resets_at,
    }))
}

fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn snippet(value: &str) -> String {
    value.trim().chars().take(300).collect()
}
