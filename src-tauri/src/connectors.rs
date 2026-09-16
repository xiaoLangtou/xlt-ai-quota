use base64::{
    engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD},
    Engine as _,
};
use chrono::{Datelike, DateTime, Local, NaiveDate, Utc};
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
        "北京" => Some("beijing"), "天津" => Some("tianjin"), "河北" => Some("hebei"),
        "山西" => Some("shanxi"), "内蒙古" => Some("neimenggu"), "辽宁" => Some("liaoning"),
        "吉林" => Some("jilin"), "黑龙江" => Some("heilongjiang"), "上海" => Some("shanghai"),
        "江苏" => Some("jiangsu"), "浙江" => Some("zhejiang"), "安徽" => Some("anhui"),
        "福建" => Some("fujian"), "江西" => Some("jiangxi"), "山东" => Some("shandong"),
        "河南" => Some("henan"), "湖北" => Some("hubei"), "湖南" => Some("hunan"),
        "广东" => Some("guangdong"), "广西" => Some("guangxi"), "海南" => Some("hainan"),
        "重庆" => Some("chongqing"), "四川" => Some("sichuan"), "贵州" => Some("guizhou"),
        "云南" => Some("yunnan"), "西藏" => Some("xizang"), "陕西" => Some("shannxi"),
        "甘肃" => Some("gansu"), "青海" => Some("qinghai"), "宁夏" => Some("ningxia"),
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
    let html = response.text().map_err(|error| format!("读取今日油价页面失败: {error}"))?;
    parse_current_oil_prices_page(province, &source_url, &html)
}

fn parse_current_oil_prices_page(province: &str, source_url: &str, html: &str) -> Result<Value, String> {
    let blocks_re = Regex::new(r"(?is)<script.*?</script>|<style.*?</style>")
        .map_err(|error| error.to_string())?;
    let tags_re = Regex::new(r"<[^>]+>").map_err(|error| error.to_string())?;
    let spaces_re = Regex::new(r"\s+").map_err(|error| error.to_string())?;
    let without_blocks = blocks_re.replace_all(&html, " ");
    let without_tags = tags_re.replace_all(&without_blocks, " ");
    let plain = spaces_re
        .replace_all(
            &without_tags.replace("&nbsp;", " ").replace("&#160;", " ").replace("&yen;", " "),
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
        prices.get(1).map(|value| value.as_str()).unwrap_or_default(),
        prices.get(2).map(|value| value.as_str()).unwrap_or_default(),
        prices.get(3).map(|value| value.as_str()).unwrap_or_default(),
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
        .query(&[("action", "forecast"), ("province", province), ("year", year.as_str())]);
    if let Some(api_key) = query.get("apiKey").map(|value| value.trim()).filter(|value| !value.is_empty()) {
        request = request.bearer_auth(api_key);
    }
    let response = request.send().map_err(|error| format!("油价数据源请求失败: {error}"))?;
    let status = response.status();
    let body = response.text().map_err(|error| format!("读取油价响应失败: {error}"))?;
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
    let entry_re = Regex::new(
        r#"<li><a href="([^"]+)"[^>]*>([^<]*成品油价格[^<]*)</a><span>([^<]+)</span>"#,
    )
    .map_err(|error| error.to_string())?;
    let entry = entry_re
        .captures(&list_html)
        .ok_or_else(|| "国家发改委新闻列表中未找到成品油调价公告".to_owned())?;
    let href = entry.get(1).map(|value| value.as_str()).unwrap_or_default();
    let title = entry.get(2).map(|value| value.as_str().trim()).unwrap_or_default();
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
        .replace_all(&without_tags.replace("&nbsp;", " ").replace("&#160;", " "), " ")
        .into_owned();
    let actual_re = Regex::new(r"调控后实际(上调|下调)\s*(\d+)元[、，]\s*(\d+)元")
        .map_err(|error| error.to_string())?;
    let normal_re = Regex::new(
        r"汽、柴油[^。]{0,80}?价格每吨分别(?:应)?(上调|下调)\s*(\d+)元[、，]\s*(\d+)元",
    )
    .map_err(|error| error.to_string())?;
    let amounts = actual_re
        .captures(&plain)
        .or_else(|| normal_re.captures(&plain))
        .map(|captures| {
            let action = captures.get(1).map(|value| value.as_str()).unwrap_or_default();
            let gasoline = captures.get(2).and_then(|value| value.as_str().parse::<i64>().ok());
            let diesel = captures.get(3).and_then(|value| value.as_str().parse::<i64>().ok());
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
        add_tokens(&mut totals, "2026-09-16", "gpt-5.6-terra", 120.0, 30.0, 10.0);
        add_tokens(&mut totals, "2026-09-16", "gpt-5.6-sol", 80.0, 20.0, 0.0);
        let rows = token_stats_json(totals);
        assert_eq!(rows.as_array().map(Vec::len), Some(2));
        assert_eq!(rows[0]["model"], "gpt-5.6-sol");
        assert_eq!(rows[1]["model"], "gpt-5.6-terra");
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
