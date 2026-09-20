use base64::{
    engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD},
    Engine as _,
};
use chrono::{DateTime, Datelike, Local, NaiveDate, Utc};
use regex::Regex;
use rusqlite::{Connection, OpenFlags};
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
        ("/api/kiro", "/stats") => kiro_stats(&request.query),
        ("/api/qoder", "/usage") => qoder_usage(),
        ("/api/qoder", "/stats") => qoder_stats(&request.query),
        ("/api/gemini", "/stats") => gemini_stats(&request.query),
        ("/api/copilot", "/stats") => copilot_stats(&request.query),
        ("/api/opencode", "/stats") => opencode_stats(),
        ("/api/codex", "/status") => codex_status(),
        ("/api/codex", "/usage") => codex_usage(),
        ("/api/codex", "/stats") => codex_stats(&request.query),
        ("/api/kimi", "/usage") => kimi_usage(),
        ("/api/kimi", "/stats") => kimi_stats(&request.query),
        ("/api/claude", "/stats") => claude_stats(&request.query),
        ("/api/oil", "/price") => current_oil_prices(&request.query),
        ("/api/oil", "/forecast") => oil_forecast(&request.query),
        ("/api/oil", "/adjustment") => official_oil_adjustment(),
        _ => Err("桌面端不支持该连接器请求".to_owned()),
    }
}

fn oil_province(query: &BTreeMap<String, String>) -> Result<&str, String> {
    query
        .get("province")
        .map(|value| value.trim())
        .filter(|value| !value.is_empty() && value.chars().count() <= 12)
        .ok_or_else(|| "需要有效的省份名称".to_owned())
}

fn oil_province_slug(province: &str) -> Option<&'static str> {
    match province {
        "北京" => Some("beijing"),
        "天津" => Some("tianjin"),
        "河北" => Some("hebei"),
        "山西" => Some("shanxi"),
        "内蒙古" => Some("neimenggu"),
        "辽宁" => Some("liaoning"),
        "吉林" => Some("jilin"),
        "黑龙江" => Some("heilongjiang"),
        "上海" => Some("shanghai"),
        "江苏" => Some("jiangsu"),
        "浙江" => Some("zhejiang"),
        "安徽" => Some("anhui"),
        "福建" => Some("fujian"),
        "江西" => Some("jiangxi"),
        "山东" => Some("shandong"),
        "河南" => Some("henan"),
        "湖北" => Some("hubei"),
        "湖南" => Some("hunan"),
        "广东" => Some("guangdong"),
        "广西" => Some("guangxi"),
        "海南" => Some("hainan"),
        "重庆" => Some("chongqing"),
        "四川" => Some("sichuan"),
        "贵州" => Some("guizhou"),
        "云南" => Some("yunnan"),
        "西藏" => Some("xizang"),
        "陕西" => Some("shannxi"),
        "甘肃" => Some("gansu"),
        "青海" => Some("qinghai"),
        "宁夏" => Some("ningxia"),
        "新疆" => Some("xinjiang"),
        _ => None,
    }
}

fn current_oil_prices(query: &BTreeMap<String, String>) -> Result<Value, String> {
    let province = oil_province(query)?;
    let slug = oil_province_slug(province).ok_or_else(|| "暂不支持该省份的今日油价".to_owned())?;
    let source_url = format!("https://www.chajiage.com/youjia/{slug}.html");
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("Mozilla/5.0 xlt-workbench/0.1 (+local oil monitor)")
        .build()
        .map_err(|error| format!("创建油价请求失败: {error}"))?;
    let response = client
        .get(&source_url)
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|error| format!("今日油价数据源请求失败: {error}"))?;
    let html = response
        .text()
        .map_err(|error| format!("读取今日油价页面失败: {error}"))?;
    parse_current_oil_prices_page(province, &source_url, &html)
}

fn parse_current_oil_prices_page(
    province: &str,
    source_url: &str,
    html: &str,
) -> Result<Value, String> {
    let blocks_re = Regex::new(r"(?is)<script.*?</script>|<style.*?</style>")
        .map_err(|error| error.to_string())?;
    let tags_re = Regex::new(r"<[^>]+>").map_err(|error| error.to_string())?;
    let spaces_re = Regex::new(r"\s+").map_err(|error| error.to_string())?;
    let without_blocks = blocks_re.replace_all(&html, " ");
    let without_tags = tags_re.replace_all(&without_blocks, " ");
    let plain = spaces_re
        .replace_all(
            &without_tags
                .replace("&nbsp;", " ")
                .replace("&#160;", " ")
                .replace("&yen;", " "),
            " ",
        )
        .into_owned();
    let prices_re = Regex::new(
        r"(\d{4})年(\d{2})月(\d{2})日，?[^。]{0,40}?汽油、柴油每升最新价格为：?\s*92号汽油\s*为\s*(\d+(?:\.\d+)?)元，?\s*95号汽油\s*为\s*(\d+(?:\.\d+)?)元，?\s*98号汽油\s*为\s*(\d+(?:\.\d+)?)元，?\s*0号柴油\s*为\s*(\d+(?:\.\d+)?)元",
    )
    .map_err(|error| error.to_string())?;
    let prices = prices_re
        .captures(&plain)
        .ok_or_else(|| "今日油价页面缺少 92/95/98 号汽油或 0 号柴油价格".to_owned())?;
    let value = |index: usize| -> Result<f64, String> {
        prices
            .get(index)
            .ok_or_else(|| "今日油价字段不完整".to_owned())?
            .as_str()
            .parse::<f64>()
            .map_err(|error| format!("今日油价不是有效数字: {error}"))
    };
    let price_date = format!(
        "{}-{}-{}",
        prices
            .get(1)
            .map(|value| value.as_str())
            .unwrap_or_default(),
        prices
            .get(2)
            .map(|value| value.as_str())
            .unwrap_or_default(),
        prices
            .get(3)
            .map(|value| value.as_str())
            .unwrap_or_default(),
    );
    Ok(json!({
        "code": 0,
        "msg": "成功",
        "data": {
            "province": province,
            "price_date": price_date,
            "source_url": source_url,
            "prices": {
                "92号汽油": value(4)?,
                "95号汽油": value(5)?,
                "98号汽油": value(6)?,
                "0号柴油": value(7)?,
            }
        }
    }))
}

fn oil_forecast(query: &BTreeMap<String, String>) -> Result<Value, String> {
    let province = oil_province(query)?;
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("xlt-workbench/0.1 (+local oil monitor)")
        .build()
        .map_err(|error| format!("创建油价请求失败: {error}"))?;
    let year = Local::now().year().to_string();
    let mut request = client
        .get("https://v1.apizero.cn/api/oil-price-forecast")
        .query(&[
            ("action", "forecast"),
            ("province", province),
            ("year", year.as_str()),
        ]);
    if let Some(api_key) = query
        .get("apiKey")
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        request = request.bearer_auth(api_key);
    }
    let response = request
        .send()
        .map_err(|error| format!("油价数据源请求失败: {error}"))?;
    let status = response.status();
    let body = response
        .text()
        .map_err(|error| format!("读取油价响应失败: {error}"))?;
    if !status.is_success() {
        return Err(format!("油价数据源 HTTP {status}: {}", snippet(&body)));
    }
    serde_json::from_str(&body).map_err(|error| format!("油价数据源返回无效 JSON: {error}"))
}

fn official_oil_adjustment() -> Result<Value, String> {
    const LIST_URL: &str = "https://www.ndrc.gov.cn/xwdt/xwfb/wap_index.html";
    const ARTICLE_BASE: &str = "https://www.ndrc.gov.cn/xwdt/xwfb/";
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("xlt-workbench/0.1 (+local oil monitor)")
        .build()
        .map_err(|error| format!("创建发改委公告请求失败: {error}"))?;
    let list_html = client
        .get(LIST_URL)
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|error| format!("国家发改委新闻列表请求失败: {error}"))?
        .text()
        .map_err(|error| format!("读取国家发改委新闻列表失败: {error}"))?;
    let entry_re =
        Regex::new(r#"<li><a href="([^"]+)"[^>]*>([^<]*成品油价格[^<]*)</a><span>([^<]+)</span>"#)
            .map_err(|error| error.to_string())?;
    let entry = entry_re
        .captures(&list_html)
        .ok_or_else(|| "国家发改委新闻列表中未找到成品油调价公告".to_owned())?;
    let href = entry.get(1).map(|value| value.as_str()).unwrap_or_default();
    let title = entry
        .get(2)
        .map(|value| value.as_str().trim())
        .unwrap_or_default();
    let published_at = entry
        .get(3)
        .map(|value| value.as_str().trim().replace('/', "-"))
        .unwrap_or_default();
    let source_url = format!("{ARTICLE_BASE}{}", href.trim_start_matches("./"));
    let article_html = client
        .get(&source_url)
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|error| format!("国家发改委调价公告请求失败: {error}"))?
        .text()
        .map_err(|error| format!("读取国家发改委调价公告失败: {error}"))?;
    let blocks_re = Regex::new(r"(?is)<script.*?</script>|<style.*?</style>")
        .map_err(|error| error.to_string())?;
    let tags_re = Regex::new(r"<[^>]+>").map_err(|error| error.to_string())?;
    let spaces_re = Regex::new(r"\s+").map_err(|error| error.to_string())?;
    let without_blocks = blocks_re.replace_all(&article_html, " ");
    let without_tags = tags_re.replace_all(&without_blocks, " ");
    let plain = spaces_re
        .replace_all(
            &without_tags.replace("&nbsp;", " ").replace("&#160;", " "),
            " ",
        )
        .into_owned();
    let actual_re = Regex::new(r"调控后实际(上调|下调)\s*(\d+)元[、，]\s*(\d+)元")
        .map_err(|error| error.to_string())?;
    let normal_re =
        Regex::new(r"汽、柴油[^。]{0,80}?价格每吨分别(?:应)?(上调|下调)\s*(\d+)元[、，]\s*(\d+)元")
            .map_err(|error| error.to_string())?;
    let amounts = actual_re
        .captures(&plain)
        .or_else(|| normal_re.captures(&plain))
        .map(|captures| {
            let action = captures
                .get(1)
                .map(|value| value.as_str())
                .unwrap_or_default();
            let gasoline = captures
                .get(2)
                .and_then(|value| value.as_str().parse::<i64>().ok());
            let diesel = captures
                .get(3)
                .and_then(|value| value.as_str().parse::<i64>().ok());
            (action.to_owned(), gasoline, diesel)
        });
    let direction = if title.contains("不作调整") || plain.contains("不作调整") {
        "unchanged"
    } else if amounts.as_ref().map(|value| value.0.as_str()) == Some("下调") {
        "down"
    } else if amounts.as_ref().map(|value| value.0.as_str()) == Some("上调") {
        "up"
    } else {
        return Err("无法识别国家发改委公告中的调价方向".to_owned());
    };
    let sign = if direction == "down" { -1 } else { 1 };
    let effective_date = NaiveDate::parse_from_str(&published_at, "%Y-%m-%d")
        .map_err(|error| format!("公告日期无效: {error}"))?
        .succ_opt()
        .ok_or_else(|| "公告生效日期超出范围".to_owned())?;
    Ok(json!({
        "title": title,
        "publishedAt": published_at,
        "effectiveAt": format!("{}T00:00:00+08:00", effective_date.format("%F")),
        "direction": direction,
        "gasolineChangePerTon": amounts.as_ref().and_then(|value| value.1).map(|value| value * sign),
        "dieselChangePerTon": amounts.as_ref().and_then(|value| value.2).map(|value| value * sign),
        "sourceUrl": source_url,
    }))
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

fn stats_date_range(query: &BTreeMap<String, String>) -> Result<(NaiveDate, NaiveDate), String> {
    let start = requested_date(query, "start")?;
    let end = requested_date(query, "end")?;
    if start > end {
        return Err("start 必须早于或等于 end".to_owned());
    }
    Ok((start, end))
}

fn value_timestamp_millis(value: &Value) -> Option<i64> {
    if let Some(number) = value.as_i64() {
        return Some(if number.abs() < 1_000_000_000_000 {
            number.saturating_mul(1_000)
        } else {
            number
        });
    }
    if let Some(number) = value.as_f64() {
        let millis = if number.abs() < 1_000_000_000_000.0 {
            number * 1_000.0
        } else {
            number
        };
        return millis.is_finite().then_some(millis as i64);
    }
    let raw = value.as_str()?.trim();
    if let Ok(number) = raw.parse::<i64>() {
        return Some(if number.abs() < 1_000_000_000_000 {
            number.saturating_mul(1_000)
        } else {
            number
        });
    }
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|timestamp| timestamp.timestamp_millis())
}

fn local_date_from_value(value: &Value) -> Option<NaiveDate> {
    DateTime::<Utc>::from_timestamp_millis(value_timestamp_millis(value)?)
        .map(|timestamp| timestamp.with_timezone(&Local).date_naive())
}

fn in_date_range(day: NaiveDate, start: NaiveDate, end: NaiveDate) -> bool {
    day >= start && day <= end
}

fn model_name(value: Option<&Value>, default: &str) -> String {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default)
        .to_owned()
}

fn estimate_text_tokens(text: &str) -> f64 {
    if text.is_empty() {
        return 0.0;
    }
    let mut cjk = 0_u64;
    let mut other = 0_u64;
    for character in text.chars() {
        let code = character as u32;
        if matches!(
            code,
            0x3000..=0x303f
                | 0x3040..=0x30ff
                | 0x3400..=0x4dbf
                | 0x4e00..=0x9fff
                | 0xac00..=0xd7af
                | 0xf900..=0xfaff
                | 0xff00..=0xffef
                | 0x20000..=0x2fa1f
        ) {
            cjk += 1;
        } else {
            other += 1;
        }
    }
    ((cjk as f64) / 1.7).ceil() + ((other as f64) / 4.0).ceil()
}

fn estimate_json_tokens(value: &Value) -> f64 {
    match value {
        Value::String(text) => estimate_text_tokens(text),
        Value::Array(values) => values.iter().map(estimate_json_tokens).sum(),
        Value::Object(values) => values
            .iter()
            .filter(|(key, _)| {
                !matches!(
                    key.as_str(),
                    "signature"
                        | "redactedContent"
                        | "toolUseId"
                        | "modelId"
                        | "message_id"
                        | "format"
                        | "id"
                )
            })
            .map(|(_, value)| estimate_json_tokens(value))
            .sum(),
        _ => 0.0,
    }
}

fn add_tokens_with_requests(
    totals: &mut BTreeMap<(String, String), TokenStats>,
    day: NaiveDate,
    model: &str,
    input: f64,
    output: f64,
    cached: f64,
    requests: u64,
) {
    if input == 0.0 && output == 0.0 && cached == 0.0 && requests == 0 {
        return;
    }
    let current = totals
        .entry((day.format("%F").to_string(), model.trim().to_owned()))
        .or_default();
    current.input += input;
    current.output += output;
    current.cached += cached;
    current.requests += requests;
}

fn canonicalize_kiro_model(value: Option<&Value>) -> String {
    let Some(raw) = value.and_then(Value::as_str).map(str::trim) else {
        return "kiro-cli-agent".to_owned();
    };
    if raw.is_empty() || raw.eq_ignore_ascii_case("auto") {
        return "kiro-cli-agent".to_owned();
    }
    let mut model = raw.to_lowercase();
    if let Some(index) = model.rfind("foundation-model/") {
        model = model[index + "foundation-model/".len()..].to_owned();
    } else {
        for prefix in ["anthropic.", "openai.", "aws."] {
            if let Some(stripped) = model.strip_prefix(prefix) {
                model = stripped.to_owned();
                break;
            }
        }
    }
    let suffix = Regex::new(r"(?i)(:\d+|-\d{8}-v\d+|-v\d+|-\d{8}|\.v\d+)$")
        .expect("固定的 Kiro 模型后缀正则应有效");
    while suffix.is_match(&model) {
        model = suffix.replace(&model, "").into_owned();
    }
    if model.is_empty() {
        "kiro-cli-agent".to_owned()
    } else {
        model
    }
}

fn kiro_session_model(cli_dir: &Path, session_id: &str) -> String {
    let path = cli_dir.join(format!("{session_id}.json"));
    let Ok(content) = fs::read_to_string(path) else {
        return "kiro-cli-agent".to_owned();
    };
    let Ok(session) = serde_json::from_str::<Value>(&content) else {
        return "kiro-cli-agent".to_owned();
    };
    canonicalize_kiro_model(session.pointer("/session_state/rts_model_state/model_info/model_id"))
}

fn collect_kiro_cli_stats(
    cli_dir: &Path,
    start: NaiveDate,
    end: NaiveDate,
    totals: &mut BTreeMap<(String, String), TokenStats>,
) {
    let Ok(entries) = fs::read_dir(cli_dir) else {
        return;
    };
    for path in entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("jsonl"))
    {
        let Some(session_id) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        let model = kiro_session_model(cli_dir, session_id);
        let mut current_timestamp: Option<Value> = None;
        let mut pending_input = 0.0;
        for_each_json_line(&path, |event| {
            let Some(data) = event.get("data") else {
                return;
            };
            let content = data
                .get("content")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or_default();
            match event.get("kind").and_then(Value::as_str) {
                Some("Prompt") => {
                    if let Some(timestamp) = data.pointer("/meta/timestamp") {
                        current_timestamp = Some(timestamp.clone());
                    }
                    for item in content {
                        pending_input +=
                            if item.get("kind").and_then(Value::as_str) == Some("image") {
                                1_600.0
                            } else {
                                item.get("data")
                                    .map(estimate_json_tokens)
                                    .unwrap_or_default()
                            };
                    }
                }
                Some("ToolResults") => {
                    pending_input += content
                        .iter()
                        .filter_map(|item| item.get("data"))
                        .map(estimate_json_tokens)
                        .sum::<f64>();
                }
                Some("AssistantMessage") => {
                    let output = content
                        .iter()
                        .map(|item| {
                            let data = item.get("data").unwrap_or(&Value::Null);
                            if item.get("kind").and_then(Value::as_str) == Some("thinking") {
                                data.get("text")
                                    .map(estimate_json_tokens)
                                    .unwrap_or_default()
                            } else {
                                estimate_json_tokens(data)
                            }
                        })
                        .sum::<f64>();
                    if let Some(day) = current_timestamp.as_ref().and_then(local_date_from_value) {
                        if in_date_range(day, start, end) {
                            add_tokens_with_requests(
                                totals,
                                day,
                                &model,
                                pending_input,
                                output,
                                0.0,
                                1,
                            );
                        }
                    }
                    pending_input = 0.0;
                }
                Some("Compaction") => pending_input = 0.0,
                _ => {}
            }
        });
    }
}

fn collect_kiro_ide_stats(
    sessions_root: &Path,
    start: NaiveDate,
    end: NaiveDate,
    totals: &mut BTreeMap<(String, String), TokenStats>,
) {
    for path in jsonl_files(sessions_root) {
        if path.file_name().and_then(|value| value.to_str()) != Some("messages.jsonl")
            || path.components().any(|part| part.as_os_str() == "cli")
        {
            continue;
        }
        for_each_json_line(&path, |event| {
            let Some(day) = event.get("timestamp").and_then(local_date_from_value) else {
                return;
            };
            if !in_date_range(day, start, end) {
                return;
            }
            let Some(payload) = event.get("payload") else {
                return;
            };
            match payload.get("type").and_then(Value::as_str) {
                Some("user") | Some("tool_result") => add_tokens_with_requests(
                    totals,
                    day,
                    "kiro-ide",
                    payload
                        .get("content")
                        .map(estimate_json_tokens)
                        .unwrap_or_default(),
                    0.0,
                    0.0,
                    0,
                ),
                Some("tool_call") => add_tokens_with_requests(
                    totals,
                    day,
                    "kiro-ide",
                    payload
                        .get("args")
                        .map(estimate_json_tokens)
                        .unwrap_or_default(),
                    0.0,
                    0.0,
                    0,
                ),
                Some("assistant") => add_tokens_with_requests(
                    totals,
                    day,
                    "kiro-ide",
                    0.0,
                    payload
                        .get("content")
                        .map(estimate_json_tokens)
                        .unwrap_or_default(),
                    0.0,
                    1,
                ),
                _ => {}
            }
        });
    }
}

fn kiro_stats(query: &BTreeMap<String, String>) -> Result<Value, String> {
    let (start, end) = stats_date_range(query)?;
    let sessions_root = home_dir().join(".kiro").join("sessions");
    let mut totals = BTreeMap::new();
    collect_kiro_cli_stats(&sessions_root.join("cli"), start, end, &mut totals);
    collect_kiro_ide_stats(&sessions_root, start, end, &mut totals);
    Ok(token_stats_json(totals))
}

fn qoder_database_paths() -> Vec<PathBuf> {
    #[cfg(target_os = "macos")]
    let base = home_dir().join("Library").join("Application Support");
    #[cfg(target_os = "windows")]
    let base = env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join("AppData").join("Roaming"));
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let base = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".config"));

    ["Qoder", "QoderCN"]
        .into_iter()
        .map(|name| {
            base.join(name)
                .join("SharedClientCache")
                .join("cache")
                .join("db")
                .join("local.db")
        })
        .collect()
}

fn qoder_model(raw: &str) -> String {
    let Ok(value) = serde_json::from_str::<Value>(raw) else {
        return "qoder-ide".to_owned();
    };
    model_name(
        value
            .get("model_key")
            .or_else(|| value.get("model_id"))
            .or_else(|| value.get("model")),
        "qoder-ide",
    )
}

fn collect_qoder_database_stats(
    path: &Path,
    start: NaiveDate,
    end: NaiveDate,
    totals: &mut BTreeMap<(String, String), TokenStats>,
) -> Result<(), String> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| format!("无法读取 {}: {error}", path.display()))?;
    connection
        .busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|error| format!("设置 Qoder 数据库超时失败: {error}"))?;
    let mut statement = connection
        .prepare(
            "SELECT token_info, COALESCE(model_info, ''), CAST(gmt_create AS INTEGER) \
             FROM chat_message WHERE role='assistant' AND token_info IS NOT NULL \
             AND length(token_info) > 2",
        )
        .map_err(|error| format!("Qoder 数据库结构无法读取: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .map_err(|error| format!("查询 Qoder Token 失败: {error}"))?;
    for row in rows.flatten() {
        let (token_info, model_info, timestamp) = row;
        let timestamp = Value::from(timestamp);
        let Some(day) = local_date_from_value(&timestamp) else {
            continue;
        };
        if !in_date_range(day, start, end) {
            continue;
        }
        let Ok(info) = serde_json::from_str::<Value>(&token_info) else {
            continue;
        };
        let prompt = value_number(info.get("prompt_tokens")).max(0.0);
        let completion = value_number(info.get("completion_tokens")).max(0.0);
        let cached = value_number(info.get("cached_tokens")).max(0.0);
        add_tokens_with_requests(
            totals,
            day,
            &qoder_model(&model_info),
            (prompt - cached).max(0.0),
            completion,
            cached,
            1,
        );
    }
    Ok(())
}

fn collect_qoder_cli_stats(
    start: NaiveDate,
    end: NaiveDate,
    totals: &mut BTreeMap<(String, String), TokenStats>,
) {
    for path in jsonl_files(&home_dir().join(".qoder").join("projects")) {
        for_each_json_line(&path, |event| {
            let Some(message) = event.get("message") else {
                return;
            };
            let Some(role) = message.get("role").and_then(Value::as_str) else {
                return;
            };
            if role != "user" && role != "assistant" {
                return;
            }
            let Some(day) = event.get("timestamp").and_then(local_date_from_value) else {
                return;
            };
            if !in_date_range(day, start, end) {
                return;
            }
            let tokens = message
                .get("content")
                .map(estimate_json_tokens)
                .unwrap_or_default();
            let model = model_name(message.get("model"), "qoder-cli");
            add_tokens_with_requests(
                totals,
                day,
                &model,
                if role == "user" { tokens } else { 0.0 },
                if role == "assistant" { tokens } else { 0.0 },
                0.0,
                u64::from(role == "assistant"),
            );
        });
    }
}

fn qoder_stats(query: &BTreeMap<String, String>) -> Result<Value, String> {
    let (start, end) = stats_date_range(query)?;
    let mut totals = BTreeMap::new();
    for path in qoder_database_paths()
        .into_iter()
        .filter(|path| path.is_file())
    {
        collect_qoder_database_stats(&path, start, end, &mut totals)?;
    }
    collect_qoder_cli_stats(start, end, &mut totals);
    Ok(token_stats_json(totals))
}

#[derive(Clone, Copy)]
struct GeminiTotals {
    input: f64,
    cached: f64,
    output: f64,
    total: f64,
}

fn gemini_totals(message: &Value) -> Option<GeminiTotals> {
    if let Some(tokens) = message.get("tokens").filter(|value| value.is_object()) {
        let input = value_number(tokens.get("input")).max(0.0);
        let cached = value_number(tokens.get("cached")).max(0.0);
        let output = value_number(tokens.get("output")).max(0.0)
            + value_number(tokens.get("tool")).max(0.0)
            + value_number(tokens.get("thoughts")).max(0.0);
        let total = value_number(tokens.get("total")).max(input + cached + output);
        return (total > 0.0).then_some(GeminiTotals {
            input,
            cached,
            output,
            total,
        });
    }
    let usage = message
        .get("usageMetadata")
        .or_else(|| message.get("usage"))
        .filter(|value| value.is_object())?;
    let cached = value_number(usage.get("cachedContentTokenCount")).max(0.0);
    let prompt = value_number(
        usage
            .get("promptTokenCount")
            .or_else(|| usage.get("input_tokens")),
    )
    .max(0.0);
    let output = value_number(
        usage
            .get("candidatesTokenCount")
            .or_else(|| usage.get("output_tokens")),
    )
    .max(0.0);
    let input = (prompt - cached).max(0.0);
    let total = input + cached + output;
    (total > 0.0).then_some(GeminiTotals {
        input,
        cached,
        output,
        total,
    })
}

fn diff_gemini_totals(
    current: GeminiTotals,
    previous: Option<GeminiTotals>,
) -> Option<GeminiTotals> {
    let Some(previous) = previous else {
        return Some(current);
    };
    if current.total < previous.total {
        return Some(current);
    }
    let delta = GeminiTotals {
        input: (current.input - previous.input).max(0.0),
        cached: (current.cached - previous.cached).max(0.0),
        output: (current.output - previous.output).max(0.0),
        total: (current.total - previous.total).max(0.0),
    };
    (delta.total > 0.0).then_some(delta)
}

fn visit_gemini_chat_files(path: &Path, depth: usize, files: &mut Vec<PathBuf>) {
    if depth > 2 {
        return;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            visit_gemini_chat_files(&path, depth + 1, files);
        } else if matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("json" | "jsonl")
        ) {
            files.push(path);
        }
    }
}

fn gemini_messages(path: &Path) -> Vec<Value> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    if path.extension().and_then(|value| value.to_str()) == Some("jsonl") {
        return content
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .filter(|value| value.get("type").is_some() || value.get("role").is_some())
            .collect();
    }
    let Ok(document) = serde_json::from_str::<Value>(&content) else {
        return Vec::new();
    };
    document
        .get("messages")
        .or_else(|| document.get("history"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn gemini_stats(query: &BTreeMap<String, String>) -> Result<Value, String> {
    let (start, end) = stats_date_range(query)?;
    let root = home_dir().join(".gemini").join("tmp");
    let mut files = Vec::new();
    if let Ok(hashes) = fs::read_dir(root) {
        for hash in hashes.flatten().filter(|entry| entry.path().is_dir()) {
            visit_gemini_chat_files(&hash.path().join("chats"), 0, &mut files);
        }
    }
    let mut totals = BTreeMap::new();
    for path in files {
        let mut previous = None;
        let mut model = "gemini".to_owned();
        for message in gemini_messages(&path) {
            if let Some(value) = message
                .get("model")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                model = value.to_owned();
            }
            if !matches!(
                message
                    .get("type")
                    .or_else(|| message.get("role"))
                    .and_then(Value::as_str),
                Some("gemini" | "model" | "assistant")
            ) {
                continue;
            }
            let Some(current) = gemini_totals(&message) else {
                continue;
            };
            let delta = diff_gemini_totals(current, previous);
            previous = Some(current);
            let Some(delta) = delta else {
                continue;
            };
            let Some(day) = message
                .get("timestamp")
                .or_else(|| message.get("createTime"))
                .and_then(local_date_from_value)
            else {
                continue;
            };
            if in_date_range(day, start, end) {
                add_tokens_with_requests(
                    &mut totals,
                    day,
                    &model,
                    delta.input + delta.cached,
                    delta.output,
                    delta.cached,
                    1,
                );
            }
        }
    }
    Ok(token_stats_json(totals))
}

fn copilot_stats(query: &BTreeMap<String, String>) -> Result<Value, String> {
    let (start, end) = stats_date_range(query)?;
    let root = home_dir().join(".copilot").join("session-state");
    let mut totals = BTreeMap::new();
    let Ok(sessions) = fs::read_dir(root) else {
        return Ok(token_stats_json(totals));
    };
    for path in sessions
        .flatten()
        .map(|entry| entry.path().join("events.jsonl"))
        .filter(|path| path.is_file())
    {
        for_each_json_line(&path, |event| {
            if event.get("type").and_then(Value::as_str) != Some("session.shutdown") {
                return;
            }
            let Some(day) = event.get("timestamp").and_then(local_date_from_value) else {
                return;
            };
            if !in_date_range(day, start, end) {
                return;
            }
            let Some(metrics) = event
                .pointer("/data/modelMetrics")
                .and_then(Value::as_object)
            else {
                return;
            };
            for (model, metric) in metrics {
                let Some(usage) = metric.get("usage") else {
                    continue;
                };
                add_tokens_with_requests(
                    &mut totals,
                    day,
                    if model.trim().is_empty() {
                        "copilot"
                    } else {
                        model
                    },
                    value_number(usage.get("inputTokens")).max(0.0),
                    value_number(usage.get("outputTokens")).max(0.0),
                    value_number(usage.get("cacheReadTokens")).max(0.0),
                    1,
                );
            }
        });
    }
    Ok(token_stats_json(totals))
}

fn opencode_stats() -> Result<Value, String> {
    let sql = "SELECT date(time_created/1000,'unixepoch','localtime') AS d, \
        COALESCE(json_extract(data,'$.modelID'),json_extract(data,'$.model'),json_extract(data,'$.modelId'),'opencode') AS model, \
        sum(json_extract(data,'$.tokens.input')) AS inp, \
        sum(json_extract(data,'$.tokens.output')) AS outp, \
        sum(COALESCE(json_extract(data,'$.tokens.cache.read'),0)) AS cache \
        FROM message WHERE json_extract(data,'$.role')='assistant' \
        AND json_extract(data,'$.tokens.input') IS NOT NULL GROUP BY d, model ORDER BY d, model";
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

    let mut totals: BTreeMap<(String, String), TokenStats> = BTreeMap::new();
    let sessions_root = home_dir().join(".codex").join("sessions");
    for day in days {
        let folder = sessions_root
            .join(day.format("%Y").to_string())
            .join(day.format("%m").to_string())
            .join(day.format("%d").to_string());
        for file in jsonl_files(&folder) {
            let mut current_model = "codex".to_owned();
            for_each_json_line(&file, |event| {
                if let Some(model) = model_from_event(event) {
                    current_model = model;
                }
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
                    &current_model,
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
// ---- Kimi Code 本地会话 Token（~/.kimi-code/sessions 的 wire.jsonl，真实计数）----
// usage.record 事件按次记录 {inputOther, output, inputCacheRead, inputCacheCreation}。
// 旧版 kimi-cli（~/.kimi/sessions）用 StatusUpdate 事件（snake_case，时间戳为秒）。
// 迁移后两套目录可能并存：kimi-code 有数据时跳过 legacy，避免迁移重复计入。

fn wire_files(root: &Path) -> Vec<PathBuf> {
    jsonl_files(root)
        .into_iter()
        .filter(|path| path.file_name().and_then(|value| value.to_str()) == Some("wire.jsonl"))
        .collect()
}

/// "kimi-code/k3" → "k3"。
fn kimi_model_name(value: Option<&Value>, fallback: &str) -> String {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| name.rsplit('/').next().unwrap_or(name).to_owned())
        .unwrap_or_else(|| fallback.to_owned())
}

fn collect_kimi_code_stats(
    root: &Path,
    start: NaiveDate,
    end: NaiveDate,
    totals: &mut BTreeMap<(String, String), TokenStats>,
) {
    for path in wire_files(root) {
        for_each_json_line(&path, |rec| {
            if rec.get("type").and_then(Value::as_str) != Some("usage.record") {
                return;
            }
            let Some(usage) = rec.get("usage").filter(|value| value.is_object()) else {
                return;
            };
            let input = value_number(usage.get("inputOther"));
            let output = value_number(usage.get("output"));
            let cached = value_number(usage.get("inputCacheRead"))
                + value_number(usage.get("inputCacheCreation"));
            if input == 0.0 && output == 0.0 && cached == 0.0 {
                return;
            }
            let Some(time) = rec.get("time") else { return };
            let Some(day) = local_date_from_value(time) else {
                return;
            };
            if !in_date_range(day, start, end) {
                return;
            }
            add_tokens_with_requests(
                totals,
                day,
                &kimi_model_name(rec.get("model"), "kimi-for-coding"),
                input + cached,
                output,
                cached,
                1,
            );
        });
    }
}

fn collect_kimi_legacy_stats(
    root: &Path,
    start: NaiveDate,
    end: NaiveDate,
    totals: &mut BTreeMap<(String, String), TokenStats>,
) {
    for path in wire_files(root) {
        let mut current_model = "kimi-for-coding".to_owned();
        for_each_json_line(&path, |rec| {
            let Some(message) = rec.get("message") else {
                return;
            };
            if message.get("type").and_then(Value::as_str) != Some("StatusUpdate") {
                return;
            }
            let Some(payload) = message.get("payload").filter(|value| value.is_object()) else {
                return;
            };
            if let Some(model) = payload
                .get("model")
                .and_then(Value::as_str)
                .filter(|model| !model.trim().is_empty())
            {
                current_model = model.to_owned();
            }
            let Some(usage) = payload.get("token_usage").filter(|value| value.is_object()) else {
                return;
            };
            let input = value_number(usage.get("input_other"));
            let output = value_number(usage.get("output"));
            let cached = value_number(usage.get("input_cache_read"))
                + value_number(usage.get("input_cache_creation"));
            if input == 0.0 && output == 0.0 && cached == 0.0 {
                return;
            }
            // legacy 时间戳为 epoch 秒；value_timestamp_millis 自动换算
            let Some(timestamp) = rec.get("timestamp").or_else(|| payload.get("timestamp")) else {
                return;
            };
            let Some(day) = local_date_from_value(timestamp) else {
                return;
            };
            if !in_date_range(day, start, end) {
                return;
            }
            add_tokens_with_requests(
                totals,
                day,
                &current_model,
                input + cached,
                output,
                cached,
                1,
            );
        });
    }
}

fn kimi_stats(query: &BTreeMap<String, String>) -> Result<Value, String> {
    let (start, end) = stats_date_range(query)?;
    let mut totals = BTreeMap::new();
    let code_root = home_dir().join(".kimi-code").join("sessions");
    if wire_files(&code_root).is_empty() {
        collect_kimi_legacy_stats(
            &home_dir().join(".kimi").join("sessions"),
            start,
            end,
            &mut totals,
        );
    } else {
        collect_kimi_code_stats(&code_root, start, end, &mut totals);
    }
    Ok(token_stats_json(totals))
}

fn claude_stats(query: &BTreeMap<String, String>) -> Result<Value, String> {
    let start = requested_date(query, "start")?;
    let end = requested_date(query, "end")?;
    if start > end {
        return Err("start 必须早于或等于 end".to_owned());
    }

    let mut totals: BTreeMap<(String, String), TokenStats> = BTreeMap::new();
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
                message
                    .and_then(|value| value.get("model"))
                    .and_then(Value::as_str)
                    .filter(|model| !model.trim().is_empty())
                    .unwrap_or("claude-code"),
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

// ---- Kimi Code 会员额度 ----
// token 取自 ~/.kimi-code/credentials/kimi-code.json（旧版 kimi-cli 目录 ~/.kimi 作回退）。
// access_token 900 秒过期；过期时用 refresh_token 换新并原子写回
// （服务端会轮换 refresh_token，不写回会导致 CLI 掉登录）。

const KIMI_CLIENT_ID: &str = "17e5f671-d194-4dfb-9706-5516cb48c098";

fn kimi_credentials_path() -> PathBuf {
    for dir in [".kimi-code", ".kimi"] {
        let path = home_dir()
            .join(dir)
            .join("credentials")
            .join("kimi-code.json");
        if path.is_file() {
            return path;
        }
    }
    home_dir()
        .join(".kimi-code")
        .join("credentials")
        .join("kimi-code.json")
}

fn kimi_hosts() -> (&'static str, &'static str) {
    let mut region = String::new();
    for dir in [".kimi-code", ".kimi"] {
        if let Ok(value) = fs::read_to_string(home_dir().join(dir).join("region")) {
            let value = value.trim();
            if !value.is_empty() {
                region = value.to_owned();
                break;
            }
        }
    }
    if region == "global" {
        ("https://api.kimi.ai/coding/v1", "https://auth.kimi.ai")
    } else {
        ("https://api.kimi.com/coding/v1", "https://auth.kimi.com")
    }
}

fn kimi_http_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("xlt-workbench/0.1")
        .build()
        .map_err(|error| format!("创建 Kimi 请求失败: {error}"))
}

fn refresh_kimi_token(path: &Path, credentials: &Value, auth_host: &str) -> Result<Value, String> {
    let refresh_token = credentials
        .get("refresh_token")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Kimi token 已过期且无 refresh_token".to_owned())?;
    let response = kimi_http_client()?
        .post(format!("{auth_host}/api/oauth/token"))
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", KIMI_CLIENT_ID),
        ])
        .send()
        .map_err(|error| format!("Kimi token 刷新请求失败: {error}"))?;
    let status = response.status();
    let body = response
        .text()
        .map_err(|error| format!("读取 Kimi 刷新响应失败: {error}"))?;
    if !status.is_success() {
        return Err(format!("Kimi token 刷新 HTTP {status}: {}", snippet(&body)));
    }
    let data: Value = serde_json::from_str(&body)
        .map_err(|error| format!("Kimi token 刷新响应不是 JSON: {error}"))?;
    let access_token = data
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Kimi token 刷新失败（可能已登出，请重新运行 kimi login）".to_owned())?;
    let expires_in = data
        .get("expires_in")
        .and_then(Value::as_f64)
        .unwrap_or(900.0);
    let mut next = credentials.clone();
    next["access_token"] = Value::from(access_token);
    if let Some(rotated) = data
        .get("refresh_token")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
    {
        next["refresh_token"] = Value::from(rotated);
    }
    next["expires_in"] = json!(expires_in);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs_f64())
        .unwrap_or_default();
    next["expires_at"] = json!(now + expires_in);
    let tmp = PathBuf::from(format!("{}.tmp-{}", path.display(), std::process::id()));
    let serialized = serde_json::to_string_pretty(&next)
        .map_err(|error| format!("序列化 Kimi 凭据失败: {error}"))?;
    fs::write(&tmp, serialized).map_err(|error| format!("写入 Kimi 凭据失败: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
    }
    fs::rename(&tmp, path).map_err(|error| format!("替换 Kimi 凭据失败: {error}"))?;
    Ok(next)
}

fn kimi_window(window: Option<&Value>) -> Option<Value> {
    let window = window?;
    let ratio = window.get("used_ratio").and_then(Value::as_f64)?;
    let reset_time = window.get("reset_time").and_then(Value::as_str)?;
    let resets_at = DateTime::parse_from_rfc3339(reset_time).ok()?;
    Some(json!({
        "usedPercent": (ratio * 1000.0).round() / 10.0,
        "resetsAt": resets_at.to_rfc3339(),
    }))
}

/// 拉取 Kimi Code 会员额度：5h 滚动窗口 + 月 Code 额度 + 月总额度。
/// /me 仅用于取会员等级标签，失败时不阻断额度返回。
fn kimi_usage() -> Result<Value, String> {
    let path = kimi_credentials_path();
    let content = fs::read_to_string(&path)
        .map_err(|error| format!("无法读取 {}: {error}", path.display()))?;
    let mut credentials: Value =
        serde_json::from_str(&content).map_err(|error| format!("Kimi 凭据格式错误: {error}"))?;
    let (api, auth) = kimi_hosts();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs_f64())
        .unwrap_or_default();
    let expires_at = credentials
        .get("expires_at")
        .and_then(Value::as_f64)
        .unwrap_or_default();
    // 预留 30 秒余量，避免请求途中过期
    if expires_at < now + 30.0 {
        credentials = refresh_kimi_token(&path, &credentials, auth)?;
    }
    let access_token = credentials
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Kimi 凭据缺少 access_token（未登录？）".to_owned())?;

    let client = kimi_http_client()?;
    let response = client
        .get(format!("{api}/usages"))
        .bearer_auth(access_token)
        .send()
        .map_err(|error| format!("Kimi 额度请求失败: {error}"))?;
    let status = response.status();
    let body = response
        .text()
        .map_err(|error| format!("读取 Kimi 额度响应失败: {error}"))?;
    if !status.is_success() {
        return Err(format!("Kimi 额度 HTTP {status}: {}", snippet(&body)));
    }
    let usages_payload: Value =
        serde_json::from_str(&body).map_err(|error| format!("Kimi 额度响应不是 JSON: {error}"))?;

    let plan_tag = client
        .get(format!("{api}/me"))
        .bearer_auth(access_token)
        .send()
        .ok()
        .and_then(|response| response.text().ok())
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|me| {
            me.get("user_level_name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
        });

    let usages = usages_payload.get("usages").cloned().unwrap_or(Value::Null);
    let five_hour = kimi_window(usages.get("limit_5h"));
    let monthly_code = kimi_window(usages.get("limit_month_code"));
    let monthly_total = kimi_window(usages.get("limit_month_total"));
    if five_hour.is_none() && monthly_code.is_none() && monthly_total.is_none() {
        return Err("Kimi /usages 响应缺少额度数据".to_owned());
    }
    Ok(json!({
        "planTag": plan_tag,
        "fiveHour": five_hour,
        "monthlyCode": monthly_code,
        "monthlyTotal": monthly_total,
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

#[cfg(test)]
mod kimi_usage_tests {
    use super::kimi_window;
    use serde_json::json;

    #[test]
    fn parses_usages_window_ratio_and_reset_time() {
        let window = kimi_window(Some(&json!({
            "used_ratio": 0.690254,
            "reset_time": "2026-09-20T06:58:09Z"
        })))
        .expect("Kimi 额度窗口应可解析");
        assert_eq!(window["usedPercent"], 69.0);
        assert!(window["resetsAt"]
            .as_str()
            .unwrap()
            .starts_with("2026-09-20T06:58:09"));

        assert!(kimi_window(Some(&json!({"reset_time": "2026-09-20T06:58:09Z"}))).is_none());
        assert!(kimi_window(None).is_none());
    }
}

#[cfg(test)]
mod kimi_stats_tests {
    use super::{collect_kimi_code_stats, kimi_model_name, token_stats_json};
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::fs;

    #[test]
    fn strips_provider_prefix_from_model() {
        assert_eq!(
            kimi_model_name(Some(&json!("kimi-code/k3")), "fallback"),
            "k3"
        );
        assert_eq!(kimi_model_name(Some(&json!("k2")), "fallback"), "k2");
        assert_eq!(kimi_model_name(None, "fallback"), "fallback");
    }

    #[test]
    fn aggregates_usage_record_events_by_day_and_model() {
        let dir = std::env::temp_dir().join(format!("xlt-kimi-stats-{}", std::process::id()));
        let session = dir
            .join("wd_proj_abcd")
            .join("session_x")
            .join("agents")
            .join("main");
        fs::create_dir_all(&session).expect("创建临时 wire 目录");
        // 2026-09-20T02:00:00Z 与 2026-09-19T23:00:00Z（本地时区可能跨年界，取固定 UTC 不便，
        // 直接用本地时间构造两天内的两条记录）
        let today = chrono::Local::now().date_naive();
        let noon = today.and_hms_opt(12, 0, 0).expect("中午时刻有效");
        let ts = noon
            .and_local_timezone(chrono::Local)
            .unwrap()
            .timestamp_millis();
        fs::write(
            session.join("wire.jsonl"),
            format!(
                "{}\n{}\n{}\n",
                json!({"type":"usage.record","model":"kimi-code/k3","usage":{"inputOther":100,"output":50,"inputCacheRead":900,"inputCacheCreation":0},"time":ts}),
                json!({"type":"usage.record","model":"kimi-code/k3","usage":{"inputOther":0,"output":0,"inputCacheRead":0,"inputCacheCreation":0},"time":ts}),
                json!({"type":"metadata","created_at":ts}),
            ),
        )
        .expect("写入临时 wire 文件");

        let mut totals = BTreeMap::new();
        collect_kimi_code_stats(&dir, today, today, &mut totals);
        let rows = token_stats_json(totals);
        let rows = rows.as_array().expect("应为数组");
        assert_eq!(rows.len(), 1, "零用量事件应被跳过: {rows:?}");
        assert_eq!(rows[0]["model"], "k3");
        assert_eq!(rows[0]["inp"], 1000.0, "input 应含缓存");
        assert_eq!(rows[0]["outp"], 50.0);
        assert_eq!(rows[0]["cache"], 900.0);
        assert_eq!(rows[0]["requests"], 1);

        let _ = fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod oil_price_tests {
    use super::parse_current_oil_prices_page;

    #[test]
    fn parses_current_province_prices_and_date() {
        let html = r#"<p>2026年09月13日，福建汽油、柴油每升最新价格为：<a>92号汽油</a>为8.25元，95号汽油为8.81元，98号汽油为10.31元，0号柴油为7.96元。</p>"#;
        let parsed = parse_current_oil_prices_page("福建", "https://example.com/fujian.html", html)
            .expect("今日油价页面应可解析");
        assert_eq!(parsed["data"]["price_date"], "2026-09-13");
        assert_eq!(parsed["data"]["prices"]["92号汽油"], 8.25);
        assert_eq!(parsed["data"]["prices"]["98号汽油"], 10.31);
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
    totals: &mut BTreeMap<(String, String), TokenStats>,
    day: &str,
    model: &str,
    input: f64,
    output: f64,
    cached: f64,
) {
    if input == 0.0 && output == 0.0 && cached == 0.0 {
        return;
    }
    let current = totals
        .entry((day.to_owned(), model.trim().to_owned()))
        .or_default();
    current.input += input;
    current.output += output;
    current.cached += cached;
    current.requests += 1;
}

fn token_stats_json(totals: BTreeMap<(String, String), TokenStats>) -> Value {
    Value::Array(totals.into_iter().map(|((day, model), stats)| {
        json!({ "d": day, "model": model, "inp": stats.input, "outp": stats.output, "cache": stats.cached, "requests": stats.requests })
    }).collect())
}

/** Codex 在 session metadata / turn context 中记录模型；后续 token_count 事件继承最近的名称。 */
fn model_from_event(event: &Value) -> Option<String> {
    let payload = event.get("payload");
    [
        event.get("model"),
        payload.and_then(|value| value.get("model")),
        payload.and_then(|value| value.get("model_id")),
        payload.and_then(|value| value.get("modelId")),
        payload
            .and_then(|value| value.get("turn_context"))
            .and_then(|value| value.get("model")),
        payload
            .and_then(|value| value.get("info"))
            .and_then(|value| value.get("model")),
    ]
    .into_iter()
    .filter_map(|value| value.and_then(Value::as_str))
    .map(str::trim)
    .find(|model| !model.is_empty())
    .map(str::to_owned)
}

#[cfg(test)]
mod model_stats_tests {
    use super::{add_tokens, model_from_event, token_stats_json, TokenStats};
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn reads_codex_turn_context_model_and_keeps_models_separate() {
        let event = json!({
            "payload": { "turn_context": { "model": "gpt-5.6-terra" } }
        });
        assert_eq!(model_from_event(&event).as_deref(), Some("gpt-5.6-terra"));

        let mut totals: BTreeMap<(String, String), TokenStats> = BTreeMap::new();
        add_tokens(
            &mut totals,
            "2026-09-16",
            "gpt-5.6-terra",
            120.0,
            30.0,
            10.0,
        );
        add_tokens(&mut totals, "2026-09-16", "gpt-5.6-sol", 80.0, 20.0, 0.0);
        let rows = token_stats_json(totals);
        assert_eq!(rows.as_array().map(Vec::len), Some(2));
        assert_eq!(rows[0]["model"], "gpt-5.6-sol");
        assert_eq!(rows[1]["model"], "gpt-5.6-terra");
    }
}

#[cfg(test)]
mod desktop_stats_route_tests {
    use super::{connector_get_blocking, diff_gemini_totals, gemini_totals, ConnectorRequest};
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn dispatches_all_desktop_token_stats_routes() {
        for base_url in ["/api/kiro", "/api/qoder", "/api/gemini", "/api/copilot"] {
            let request = ConnectorRequest {
                base_url: base_url.to_owned(),
                path: "/stats".to_owned(),
                query: BTreeMap::from([
                    ("start".to_owned(), "2026-09-18".to_owned()),
                    ("end".to_owned(), "2026-09-17".to_owned()),
                ]),
            };
            assert_eq!(
                connector_get_blocking(request),
                Err("start 必须早于或等于 end".to_owned()),
                "{base_url}/stats 应分发到桌面端聚合器"
            );
        }
    }

    #[test]
    fn diffs_gemini_cumulative_snapshots() {
        let first = gemini_totals(&json!({
            "usageMetadata": {
                "promptTokenCount": 100,
                "cachedContentTokenCount": 20,
                "candidatesTokenCount": 30
            }
        }))
        .expect("首个 Gemini 快照应可解析");
        let second = gemini_totals(&json!({
            "usageMetadata": {
                "promptTokenCount": 160,
                "cachedContentTokenCount": 30,
                "candidatesTokenCount": 50
            }
        }))
        .expect("第二个 Gemini 快照应可解析");
        let delta = diff_gemini_totals(second, Some(first)).expect("快照增量应非空");
        assert_eq!(delta.input, 50.0);
        assert_eq!(delta.cached, 10.0);
        assert_eq!(delta.output, 20.0);
    }
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
