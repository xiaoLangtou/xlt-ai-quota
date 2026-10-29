//! 禅道同步：经 REST API v2 拉取指派给我的任务 / Bug / 需求，转换为本地待办。
//!
//! 凭证策略与 arkcli/kiro-cli 一致：登录态保存在本机 `~/.zentao-token.json`，
//! 应用只读取该缓存文件，代码与日志不接触密码；token 过期（401）时提示重新登录。
//! 显式操作经禅道 API 提交并回读核实；同步状态以禅道为准。

use crate::productivity::store::{now_ms, sync_task_event};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use uuid::Uuid;

mod operations;
pub use operations::{
    action, action_context, batch_resolve, comment, ZentaoCommentInput, ZentaoActionContext, ZentaoActionInput,
    ZentaoActionResult, ZentaoBatchResolveInput, ZentaoBatchResolveResult,
};

const API_PATH: &str = "/api.php/v2";
const SOURCE_NAME: &str = "禅道";
const REC_PER_PAGE: usize = 500;
const MAX_PAGES: usize = 20;
/// 遍历上限：超出部分跳过并在结果中说明，避免单次同步请求失控。
const MAX_PRODUCTS: usize = 50;
const MAX_PROJECTS: usize = 50;
const MAX_EXECUTIONS: usize = 200;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZentaoStatus {
    pub configured: bool,
    pub url: String,
    pub account: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZentaoSyncResult {
    pub task_count: usize,
    pub bug_count: usize,
    pub created: usize,
    pub updated: usize,
    pub closed: usize,
    pub skipped: String,
    pub elapsed_ms: u64,
    pub request_count: usize,
    pub history_deferred: usize,
    pub synced_at: i64,
    /// 逐条同步明细，供同步完成弹窗的表格展示。
    pub items: Vec<ZentaoSyncItem>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZentaoFile {
    pub id: i64,
    pub title: String,
    pub extension: String,
    pub size: i64,
    /// 附件挂载位置：task = 任务本体，story = 关联需求（禅道页面的“关联材料”）。
    pub origin: String,
}

/// 附件内容：用于预览。kind ∈ text | image | binary。
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZentaoFileContent {
    pub kind: String,
    /// text = UTF-8 文本；image = data URL（base64）；binary 为空。
    pub content: String,
}

/// zip 附件内的一个文件条目。path 为包内相对路径（'/' 分隔），目录由前端按路径推导；
/// index 对应 zip 内条目序号，按索引读取可绕开文件名编码差异。
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZentaoArchiveEntry {
    pub index: usize,
    pub path: String,
    pub name: String,
    pub size: u64,
    pub extension: String,
}

/// 同步明细中的单条条目：action ∈ created | updated | unchanged | closed。
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZentaoSyncItem {
    pub zentao_id: i64,
    pub external_type: String,
    pub title: String,
    pub priority: String,
    pub status: String,
    pub action: String,
    pub external_url: String,
    pub due: Option<String>,
    pub estimate_minutes: Option<i64>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZentaoDetail {
    pub external_type: String,
    pub external_id: i64,
    pub description: String,
    pub status: String,
    pub assigned_to: String,
    pub files: Vec<ZentaoFile>,
    /// 描述中的内嵌截图（data URL），按出现顺序。
    pub images: Vec<String>,
    /// 处理历史（禅道 actions），旧到新。
    pub actions: Vec<ZentaoAction>,
}

/// 历史记录里的一条字段变更。
/// rich=true 表示富文本字段（如 steps）：用 newText/newImages 展示变更后的内容，
/// 不直接抛原文 HTML。
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZentaoActionChange {
    pub field: String,
    pub old: String,
    pub new: String,
    pub rich: bool,
    /// 富文本变更后的可读文本（含【截图n】占位符，按原位置）。
    pub new_text: String,
    /// newText 中占位符对应的截图（data URL，按序）。
    pub new_images: Vec<String>,
}

/// 一条处理历史：desc 是禅道预格式化的中文描述行；images 为该次操作附带/引用的截图。
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZentaoAction {
    pub date: String,
    pub actor: String,
    pub desc: String,
    pub comment: String,
    pub changes: Vec<ZentaoActionChange>,
    pub images: Vec<String>,
}

/// 规范化后的禅道条目，与 todo_task 的 zentao 来源字段一一对应。
#[derive(Clone)]
struct ZentaoItem {
    external_id: String,
    external_type: String,
    title: String,
    description: Option<String>,
    priority: String,
    status: String,
    external_status: String,
    assigned_to: String,
    transferred: bool,
    due: Option<String>,
    updated_date: Option<String>,
    estimate_minutes: Option<i64>,
    external_url: String,
}

#[derive(Deserialize)]
struct TokenFile {
    url: String,
    token: String,
    account: String,
}

fn token_path() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".zentao-token.json")
}

fn read_token() -> Result<TokenFile, String> {
    let text = std::fs::read_to_string(token_path())
        .map_err(|_| "尚未登录禅道，请先在设置页配置禅道账号".to_owned())?;
    let file: TokenFile =
        serde_json::from_str(&text).map_err(|_| "禅道登录缓存无效，请重新登录".to_owned())?;
    if file.url.trim().is_empty() || file.token.trim().is_empty() {
        return Err("禅道登录缓存无效，请重新登录".to_owned());
    }
    Ok(TokenFile {
        url: file.url.trim().trim_end_matches('/').to_owned(),
        token: file.token.trim().to_owned(),
        account: file.account.trim().to_owned(),
    })
}

pub fn status() -> ZentaoStatus {
    match read_token() {
        Ok(file) => ZentaoStatus {
            configured: true,
            url: file.url,
            account: file.account,
        },
        Err(_) => ZentaoStatus {
            configured: false,
            url: String::new(),
            account: String::new(),
        },
    }
}

/// 用账号密码登录禅道，成功后把 {url, token, account} 写入本机缓存文件。
/// 密码只用于这一次请求，不落盘、不写日志。
pub fn login(url: &str, account: &str, password: &str) -> Result<ZentaoStatus, String> {
    let url = url.trim().trim_end_matches('/');
    let account = account.trim();
    if url.is_empty() || account.is_empty() || password.is_empty() {
        return Err("请填写禅道地址、账号和密码".to_owned());
    }
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("禅道地址应以 http:// 或 https:// 开头".to_owned());
    }
    let client = http_client()?;
    let response = client
        .post(format!("{url}{API_PATH}/users/login"))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "account": account, "password": password }))
        .send()
        .map_err(|error| format!("无法连接禅道服务器: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("登录失败：服务器返回 HTTP {}", response.status()));
    }
    let body: Value = response
        .json()
        .map_err(|_| "登录响应不是合法 JSON".to_owned())?;
    let token = body["token"]
        .as_str()
        .or_else(|| body["data"]["token"].as_str())
        .unwrap_or("")
        .trim()
        .to_owned();
    if token.is_empty() {
        return Err("登录失败：账号或密码错误".to_owned());
    }
    let path = token_path();
    std::fs::write(
        &path,
        serde_json::json!({ "url": url, "token": token, "account": account }).to_string(),
    )
    .map_err(|error| format!("无法写入禅道登录缓存: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(ZentaoStatus {
        configured: true,
        url: url.to_owned(),
        account: account.to_owned(),
    })
}

/// 退出登录：删除本机 token 缓存（不动禅道侧数据）。
pub fn logout() -> Result<(), String> {
    match std::fs::remove_file(token_path()) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("无法清除禅道登录缓存: {error}")),
    }
}

fn http_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("xlt-workbench")
        .build()
        .map_err(|error| format!("无法创建 HTTP 客户端: {error}"))
}

#[derive(Clone)]
struct ZentaoClient {
    url: String,
    token: String,
    account: String,
    http: reqwest::blocking::Client,
    requests: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

impl ZentaoClient {
    fn from_token(file: TokenFile) -> Result<Self, String> {
        Ok(Self {
            url: file.url,
            token: file.token,
            account: file.account,
            http: http_client()?,
            requests: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        })
    }

    fn get(&self, path: &str, query: &[(&str, &str)]) -> Result<Value, String> {
        self.get_raw(&format!("{API_PATH}{path}"), query)
    }

    /// 按完整 API 路径请求（可指定 v1/v2 版本）。
    /// 禅道实例在短时间请求过多时会偶发 403（fail2ban），这里对只读请求做两次退避重试；
    /// 写请求不经过此路径，避免重复提交。
    fn get_raw(&self, api_path: &str, query: &[(&str, &str)]) -> Result<Value, String> {
        const ATTEMPTS: usize = 3;
        let mut last_error = String::new();
        for attempt in 0..ATTEMPTS {
            let mut request = self
                .http
                .get(format!("{}{api_path}", self.url))
                .header("token", &self.token);
            if !query.is_empty() {
                request = request.query(query);
            }
            self.requests.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let response = match request.send() {
                Ok(response) => response,
                Err(error) => {
                    last_error = format!("请求禅道失败: {error}");
                    if attempt + 1 < ATTEMPTS {
                        std::thread::sleep(Duration::from_millis(200 * (attempt as u64 + 1)));
                        continue;
                    }
                    return Err(last_error);
                }
            };
            let status = response.status();
            if status.as_u16() == 401 {
                return Err("禅道登录已失效，请在设置页重新登录".to_owned());
            }
            // 403 在禅道侧通常是频率限制；按 500ms / 1.5s 退避后重试。
            if (status.as_u16() == 403 || status.as_u16() == 429 || status.is_server_error())
                && attempt + 1 < ATTEMPTS
            {
                std::thread::sleep(Duration::from_millis(500 * (attempt as u64 + 1)));
                continue;
            }
            if status.as_u16() == 403 || status.as_u16() == 429 {
                return Err(format!(
                    "禅道暂时拒绝访问（HTTP {}）：可能是请求过于频繁或登录态失效，已自动重试，请稍后再试；仍失败请在设置页重新登录禅道",
                    status.as_u16()
                ));
            }
            if !status.is_success() {
                return Err(format!("禅道返回 HTTP {status}"));
            }
            return operations::check_body(
                response.json().map_err(|_| "禅道响应不是合法 JSON".to_owned())?,
            );
        }
        Err(if last_error.is_empty() {
            "禅道请求失败".to_owned()
        } else {
            last_error
        })
    }

    /// 拉取分页列表（自动翻页）。不同禅道版本列表包裹形状不一，
    /// 统一按「顶层数组 / {key:[]} / {data:[...]} / {data:{key:[...]}}」解析。
    fn list(&self, path: &str, key: &str, query: &[(&str, &str)]) -> Result<Vec<Value>, String> {
        let mut items = Vec::new();
        for page in 1..=MAX_PAGES {
            let mut page_query: Vec<(&str, String)> = query
                .iter()
                .map(|(name, value)| (*name, (*value).to_owned()))
                .collect();
            page_query.push(("recPerPage", REC_PER_PAGE.to_string()));
            page_query.push(("pageID", page.to_string()));
            let references: Vec<(&str, &str)> = page_query
                .iter()
                .map(|(name, value)| (*name, value.as_str()))
                .collect();
            let body = self.get(path, &references)?;
            let page_items = extract_items(&body, key);
            let done = page_items.len() < REC_PER_PAGE;
            items.extend(page_items);
            if done {
                break;
            }
        }
        Ok(items)
    }
}

/// 从列表响应中提取数组，兼容禅道各版本的包裹形状。
fn extract_items(body: &Value, key: &str) -> Vec<Value> {
    let candidates = [
        body.as_array().cloned(),
        body.get(key).and_then(Value::as_array).cloned(),
        body.get("data").and_then(Value::as_array).cloned(),
        body.get("data").and_then(|data| data.get(key)).and_then(Value::as_array).cloned(),
        // 兜底：对象里第一个值为对象数组的字段。
        body.as_object().and_then(|object| {
            object
                .values()
                .find(|value| {
                    value
                        .as_array()
                        .is_some_and(|list| list.first().is_some_and(Value::is_object))
                })
                .and_then(Value::as_array)
                .cloned()
        }),
    ];
    candidates
        .into_iter()
        .flatten()
        .max_by_key(Vec::len)
        .unwrap_or_default()
}

fn text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty() && *text != "0")
        .map(ToOwned::to_owned)
}

/// 禅道实例的数值字段多为字符串（如 "14927"、"0.00"），数字与字符串都要兼容。
fn number(value: &Value, key: &str) -> Option<i64> {
    match value.get(key)? {
        Value::Number(number) => number.as_i64(),
        Value::String(raw) => raw.trim().parse::<i64>().ok(),
        _ => None,
    }
}

fn number_f64(value: &Value, key: &str) -> Option<f64> {
    match value.get(key)? {
        Value::Number(number) => number.as_f64(),
        Value::String(raw) => raw.trim().parse::<f64>().ok(),
        _ => None,
    }
}

/// 禅道截止日期可能是 "0000-00-00" 等占位值，只接受合法日期。
fn deadline(value: &Value, key: &str) -> Option<String> {
    let raw = text(value, key)?;
    let date = raw.get(..10)?;
    chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .ok()
        .map(|parsed| parsed.format("%Y-%m-%d").to_string())
}

fn map_priority(pri: Option<i64>, severity: Option<i64>) -> &'static str {
    // 优先级映射：pri 1 → 高，2/3 → 中，4 → 低；Bug 严重级别 1/2 至少提到高。
    let base = match pri.unwrap_or(3) {
        1 => "high",
        2 | 3 => "medium",
        _ => "low",
    };
    if matches!(severity, Some(1 | 2)) {
        "high"
    } else {
        base
    }
}

fn map_status(external_type: &str, status: &str) -> &'static str {
    match external_type {
        "task" => match status {
            "doing" => "doing",
            "done" | "closed" | "cancel" => "done",
            _ => "open",
        },
        "bug" => match status {
            "resolved" | "closed" => "done",
            _ => "open",
        },
        _ => match status {
            "closed" => "done",
            _ => "open",
        },
    }
}

/// 禅道描述字段是富文本 HTML，转换为纯文本供待办描述展示。
fn html_to_text(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut in_hidden_block = false; // <style>/<script> 内容整块跳过
    let mut chars = raw.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if ch == '<' {
            let rest = raw[index..].to_lowercase();
            if rest.starts_with("<style") || rest.starts_with("<script") {
                in_hidden_block = true;
            }
            if rest.starts_with("</style") || rest.starts_with("</script") {
                in_hidden_block = false;
            }
            let is_block_break = !in_hidden_block
                && ["</p>", "</div>", "</li>", "</h1>", "</h2>", "</h3>", "</tr>", "<br"]
                    .iter()
                    .any(|tag| rest.starts_with(tag));
            if is_block_break {
                out.push('\n');
            }
            // 跳过整个标签直到 '>'
            for (_, c) in chars.by_ref() {
                if c == '>' {
                    break;
                }
            }
            continue;
        }
        if !in_hidden_block {
            out.push(ch);
        }
    }
    // 解码常见 HTML 实体，逐行重建以压缩空白与连续空行。
    let decoded = out
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&");
    decoded
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// 双保险：服务端已按 assignedtome 过滤，客户端再校验一遍指派对象。
fn assigned_to_me(value: &Value, account: &str) -> bool {
    match value.get("assignedTo").and_then(Value::as_str) {
        Some(assignee) => assignee.trim() == account,
        // 字段缺失时不拦截（老版本兼容），存在即严格比对。
        None => true,
    }
}

fn normalize(external_type: &str, value: &Value, base_url: &str) -> Option<ZentaoItem> {
    let id = number(value, "id")?;
    let raw_title = text(value, if external_type == "task" { "name" } else { "title" })?;
    // 标题带禅道编号，便于在待办/日历里直接对上条目：【#14925】xxx。
    let title = format!("【#{id}】{raw_title}");
    let status = text(value, "status").unwrap_or_else(|| "open".to_owned());
    let estimate_hours = number_f64(value, "estimate");
    let estimate_minutes = estimate_hours
        .filter(|hours| *hours > 0.0)
        .map(|hours| (hours * 60.0).round() as i64);
    // 任务描述在 desc，Bug 描述在 steps，均为富文本 HTML。
    let description = text(value, if external_type == "task" { "desc" } else { "steps" })
        .map(|raw| truncate_chars(&html_to_text(&raw), crate::productivity::types::MAX_DESCRIPTION))
        .filter(|text| !text.is_empty());
    Some(ZentaoItem {
        external_id: format!("zentao:{external_type}:{id}"),
        external_type: external_type.to_owned(),
        title: truncate_chars(&title, crate::productivity::types::MAX_TITLE),
        description,
        priority: map_priority(number(value, "pri"), number(value, "severity")).to_owned(),
        status: map_status(external_type, &status).to_owned(),
        external_status: status,
        assigned_to: operations::account_field(value, "assignedTo"),
        transferred: false,
        updated_date: text(value, "lastEditedDate").filter(|date| !date.starts_with("0000") && !date.is_empty()),
        due: deadline(value, if external_type == "story" { "" } else { "deadline" }),
        estimate_minutes: if external_type == "task" {
            estimate_minutes
        } else {
            None
        },
        external_url: format!("{base_url}/{external_type}-view-{id}.html"),
    })
}

fn truncate_chars(value: &str, max: usize) -> String {
    value.chars().take(max).collect()
}

/// 拉取两个来源：bug 按产品遍历，task 按项目 → 执行遍历；均为指派给当前登录人的条目。
/// 服务端 assignedtome 过滤之外，客户端再按账号二次校验。
const SYNC_READ_CONCURRENCY: usize = 3;
const HISTORY_RECHECK_MS: i64 = 30 * 60 * 1000;

/// 每次同步共享最多三个只读 worker；结果按输入顺序归并，不在 worker 内写数据库。
fn parallel_read<T: Sync, R: Send>(items: &[T], operation: impl Fn(&T) -> R + Sync) -> Result<Vec<R>, String> {
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..SYNC_READ_CONCURRENCY.min(items.len())).map(|_| {
            let operation = &operation;
            let next = &next;
            scope.spawn(move || {
                let mut results = Vec::new();
                loop {
                    let index = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if index >= items.len() { break; }
                    results.push((index, operation(&items[index])));
                }
                results
            })
        }).collect();
        let mut results = Vec::with_capacity(items.len());
        for handle in handles {
            results.extend(handle.join().map_err(|_| "禅道只读同步任务异常".to_owned())?);
        }
        results.sort_by_key(|(index, _)| *index);
        Ok(results.into_iter().map(|(_, value)| value).collect())
    })
}

fn append_sync_issue(skipped: &mut String, message: String) {
    if !skipped.is_empty() { skipped.push('；'); }
    skipped.push_str(&message);
}

fn fetch_my_items(client: &ZentaoClient) -> Result<(Vec<ZentaoItem>, String), String> {
    let roots = parallel_read(&["products", "projects"], |kind| {
        client.list(&format!("/{kind}"), kind, if *kind == "projects" { &[("browseType", "all")] } else { &[] })
    })?;
    let mut roots = roots.into_iter();
    let products = roots.next().ok_or("产品查询结果缺失")??;
    let projects = roots.next().ok_or("项目查询结果缺失")??;
    let mut skipped = String::new();
    if products.len() > MAX_PRODUCTS { append_sync_issue(&mut skipped, "产品数量达到遍历上限".to_owned()); }
    if projects.len() > MAX_PROJECTS { append_sync_issue(&mut skipped, "项目数量达到遍历上限".to_owned()); }
    let product_ids: Vec<i64> = products.iter().take(MAX_PRODUCTS).filter_map(|p| number(p, "id")).collect();
    let project_ids: Vec<i64> = projects.iter().take(MAX_PROJECTS).filter_map(|p| number(p, "id")).collect();
    let mut items = Vec::new();
    for (id, result) in parallel_read(&product_ids, |id| (*id, client.list(&format!("/products/{id}/bugs"), "bugs", &[("browseType", "assigntome")])))? {
        match result {
            Ok(list) => items.extend(list.iter().filter(|v| assigned_to_me(v, &client.account)).filter_map(|v| normalize("bug", v, &client.url))),
            Err(error) => append_sync_issue(&mut skipped, format!("产品 {id} 的 Bug 未同步：{error}")),
        }
    }
    let mut execution_ids = Vec::new();
    for (id, result) in parallel_read(&project_ids, |id| (*id, client.list(&format!("/projects/{id}/executions"), "executions", &[("browseType", "undone")])))? {
        match result {
            Ok(list) => execution_ids.extend(list.iter().filter_map(|v| number(v, "id"))),
            Err(error) => append_sync_issue(&mut skipped, format!("项目 {id} 的执行未同步：{error}")),
        }
    }
    execution_ids.sort_unstable();
    execution_ids.dedup();
    if execution_ids.len() > MAX_EXECUTIONS { append_sync_issue(&mut skipped, "执行数量达到遍历上限，部分执行未同步".to_owned()); }
    execution_ids.truncate(MAX_EXECUTIONS);
    for (id, result) in parallel_read(&execution_ids, |id| (*id, client.list(&format!("/executions/{id}/tasks"), "tasks", &[("status", "assignedtome")])))? {
        match result {
            Ok(list) => items.extend(list.iter().filter(|v| assigned_to_me(v, &client.account)).filter_map(|v| normalize("task", v, &client.url))),
            Err(error) => append_sync_issue(&mut skipped, format!("执行 {id} 的任务未同步：{error}")),
        }
    }
    Ok((items, skipped))
}

/// 幂等 upsert：新条目仅导入未完成项；已同步条目继续跟随远端状态，保留完成历史，
/// 但已完成条目不进入同步明细（同步明细只展示未完成条目）。
/// 禅道条目的状态以远端为准；
/// planned_date 仅在禅道侧截止日期变化时跟随更新，保留本地手动调整。
/// 每条的写入结果（created/updated/unchanged）追加到 report，供同步弹窗展示。
fn upsert_items(
    transaction: &Transaction<'_>,
    items: &[ZentaoItem],
    counts: &mut (usize, usize, usize),
    report: &mut Vec<ZentaoSyncItem>,
) -> Result<(), String> {
    let now = now_ms();
    for item in items {
        let previous = transaction
            .query_row(
                "SELECT id, status, planned_date, external_due, title, description, priority,
                        external_status, external_assigned_to, external_transferred
                 FROM todo_task WHERE external_id = ?1",
                params![&item.external_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, Option<String>>(7)?,
                        row.get::<_, Option<String>>(8)?,
                        row.get::<_, bool>(9)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| format!("无法读取禅道待办: {error}"))?;
        let (id, status, planned_date, action) = match previous {
            Some((
                id,
                local_status,
                local_planned,
                external_due,
                local_title,
                local_description,
                local_priority,
                local_external_status,
                local_assigned_to,
                local_transferred,
            )) => {
                let status = item.status.clone();
                // 截止日期未变化时保留本地排期；变化时跟随禅道更新。
                let planned_date =
                    if external_due.as_deref() == item.due.as_deref() && local_planned.is_some() {
                        local_planned.clone()
                    } else {
                        item.due.clone()
                    };
                let changed = local_title != item.title
                    || local_description != item.description
                    || local_priority != item.priority
                    || status != local_status
                    || planned_date != local_planned
                    || local_external_status.as_deref() != Some(item.external_status.as_str())
                    || local_assigned_to.as_deref() != Some(item.assigned_to.as_str())
                    || local_transferred != item.transferred;
                transaction
                    .execute(
                        "UPDATE todo_task SET title = ?2, description = ?3, status = ?4, priority = ?5,
                            planned_date = ?6, updated_at = ?7, completed_at = CASE WHEN ?4 = 'done' THEN COALESCE(completed_at, ?8) ELSE NULL END,
                            external_type = ?9, external_url = ?10, external_due = ?11,
                            synced_at = ?7
                         WHERE id = ?1",
                        params![
                            &id,
                            &item.title,
                            &item.description,
                            &status,
                            &item.priority,
                            &planned_date,
                            now,
                            if status == "done" { Some(now) } else { None },
                            &item.external_type,
                            &item.external_url,
                            &item.due,
                        ],
                    )
                    .map_err(|error| format!("无法更新禅道待办: {error}"))?;
                // 已完成条目不计入更新数：写入仅为跟随远端状态（如标记本地完成），不进同步明细。
                if changed && status != "done" {
                    counts.1 += 1;
                }
                (
                    id,
                    status,
                    planned_date,
                    if changed { "updated" } else { "unchanged" },
                )
            }
            None => {
                // 已完成、关闭、取消的历史条目不首次导入，也不作为新增出现在同步明细。
                if item.status == "done" { continue; }
                let id = Uuid::new_v4().to_string();
                transaction
                    .execute(
                        "INSERT INTO todo_task (id, title, description, status, priority, category,
                            planned_date, planned_time, estimate_minutes, source_type, source_name,
                            created_at, updated_at, completed_at,
                            external_id, external_type, external_url, external_due, synced_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, 'work', ?6, NULL, ?7, 'zentao', ?8,
                            ?9, ?9, ?10, ?11, ?12, ?13, ?14, ?9)",
                        params![
                            &id,
                            &item.title,
                            &item.description,
                            &item.status,
                            &item.priority,
                            &item.due,
                            &item.estimate_minutes,
                            SOURCE_NAME,
                            now,
                            if item.status == "done" { Some(now) } else { None },
                            &item.external_id,
                            &item.external_type,
                            &item.external_url,
                            &item.due,
                        ],
                    )
                    .map_err(|error| format!("无法创建禅道待办: {error}"))?;
                counts.0 += 1;
                (id, item.status.clone(), item.due.clone(), "created")
            }
        };
        transaction.execute(
            "UPDATE todo_task SET external_status = ?2, external_assigned_to = ?3,
                external_transferred = ?4, external_updated_date = ?5 WHERE id = ?1",
            params![&id, &item.external_status, &item.assigned_to, item.transferred, &item.updated_date],
        ).map_err(|e| format!("无法保存禅道原始状态: {e}"))?;
        // 只同步未完成条目：已完成（完成/关闭/取消/已解决）的条目不再出现在同步明细，
        // 避免指派回开单人的历史任务每次同步都重复展示；状态仍落库以保留完成历史。
        if status != "done" {
            report.push(ZentaoSyncItem {
                zentao_id: item
                    .external_id
                    .rsplit(':')
                    .next()
                    .and_then(|raw| raw.parse().ok())
                    .unwrap_or(0),
                external_type: item.external_type.clone(),
                title: item.title.clone(),
                priority: item.priority.clone(),
                status,
                action: action.to_owned(),
                external_url: item.external_url.clone(),
                due: planned_date,
                estimate_minutes: item.estimate_minutes,
            });
        }
        sync_task_event(transaction, &id)?;
    }
    Ok(())
}

/// 列表缺失不代表完成；逐项读详情，核实转交、解决及重新激活。
fn should_recheck_history(status: &str, synced_at: Option<i64>, now: i64) -> bool {
    if !matches!(status, "done" | "closed" | "cancel" | "resolved") { return true; }
    !synced_at.is_some_and(|time| time <= now && now - time < HISTORY_RECHECK_MS)
}

fn refresh_missing(
    client: &ZentaoClient, connection: &Connection,
    items: &mut Vec<ZentaoItem>, skipped: &mut String,
) -> Result<usize, String> {
    let mut statement = connection.prepare(
        "SELECT external_type, external_id, external_status, synced_at FROM todo_task WHERE source_type = 'zentao' ORDER BY id"
    ).map_err(|e| e.to_string())?;
    let rows = statement.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<String>>(2)?, row.get::<_, Option<i64>>(3)?)))
        .map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    let present: std::collections::HashSet<String> = items.iter().map(|item| item.external_id.clone()).collect();
    let now = now_ms();
    let mut deferred = 0;
    let mut missing = Vec::new();
    for (kind, key, status, synced_at) in rows {
        if present.contains(&key) { continue; }
        if !should_recheck_history(status.as_deref().unwrap_or(""), synced_at, now) {
            deferred += 1;
            continue;
        }
        let id = key.rsplit(':').next().and_then(|v| v.parse::<i64>().ok()).ok_or_else(|| format!("禅道来源编号无效: {key}"))?;
        missing.push((kind, key, id));
    }
    for (key, result) in parallel_read(&missing, |(kind, key, id)| {
        let result = operations::read_object(client, kind, *id).and_then(|object| {
            let mut item = normalize(kind, &object, &client.url).ok_or("禅道详情缺少编号或标题")?;
            item.transferred = item.assigned_to != client.account;
            Ok(item)
        });
        (key.clone(), result)
    })? {
        match result {
            Ok(item) => items.push(item),
            Err(error) => append_sync_issue(skipped, format!("{key} 状态未更新：{error}")),
        }
    }
    Ok(deferred)
}

pub fn detail(external_type: &str, external_id: i64) -> Result<ZentaoDetail, String> {
    let token = read_token()?;
    let client = ZentaoClient::from_token(token)?;
    let path = if external_type == "bug" {
        format!("/bugs/{external_id}")
    } else {
        format!("/tasks/{external_id}")
    };
    let (body, raw_actions) = match client.get_raw(&format!("/api.php/v1{path}"), &[]) {
        Ok(v1_body) => {
            let actions = ["task", "bug", "data"].iter().find_map(|key| v1_body.get(*key).and_then(|object| object.get("actions"))).or_else(|| v1_body.get("actions")).cloned();
            (v1_body, actions)
        }
        Err(_) => {
            let v2_body = client.get(&path, &[])?;
            (v2_body, None)
        }
    };
    // 详情可能是裸对象、{task: {...}} / {bug: {...}} 或 {data: {...}}。
    let object = ["task", "bug", "data"]
        .iter()
        .find_map(|key| body.get(*key).filter(|value| value.is_object()))
        .unwrap_or(&body);
    let raw_html = text(object, if external_type == "bug" { "steps" } else { "desc" })
        .unwrap_or_default();
    // 描述与历史共享同一份图片缓存（同一文件只下载一次）。
    let mut image_cache = ImageCache::new();
    // 内嵌截图转 data URL，并在描述文本里按原位置留【截图n】占位符，前端按序插回。
    let (description, images) = render_description(&client, &raw_html, &mut image_cache);
    let mut files = Vec::new();
    // 任务本体附件 + 关联需求附件（禅道页面显示为“关联材料”的 PRD/原型等）。
    for (origin, key) in [("task", "files"), ("story", "storyFiles")] {
        let Some(map) = object.get(key).and_then(Value::as_object) else {
            continue;
        };
        for (key, value) in map {
            let id = number(value, "id").or_else(|| key.parse::<i64>().ok());
            let Some(id) = id else { continue };
            // 需求附件与任务附件可能重复（任务页同时挂载），按 id 去重。
            if files.iter().any(|file: &ZentaoFile| file.id == id) {
                continue;
            }
            files.push(ZentaoFile {
                id,
                title: text(value, "title").unwrap_or_else(|| format!("附件-{id}")),
                extension: text(value, "extension").unwrap_or_default(),
                size: number(value, "size").unwrap_or(0),
                origin: origin.to_owned(),
            });
        }
    }
    files.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(ZentaoDetail {
        external_type: external_type.to_owned(),
        external_id,
        description,
        status: text(object, "status").unwrap_or_default(),
        assigned_to: operations::account_field(object, "assignedTo"),
        files,
        images,
        actions: parse_actions(&client, raw_actions.as_ref(), &mut image_cache),
    })
}

/// 描述渲染：把 HTML 里的内嵌截图（`file-read-{id}.{ext}`）逐张拉取为 data URL，
/// 原 `<img>` 位置替换为【截图n】占位符后转纯文本。成功图片按出现顺序返回。
/// 单张失败静默移除；最多 12 张、总量 8MB。
fn render_description(
    client: &ZentaoClient,
    html: &str,
    cache: &mut ImageCache,
) -> (String, Vec<String>) {
    let img_tag = regex::Regex::new(r"(?i)<img[^>]*>").expect("合法正则");
    let src_attr = regex::Regex::new(r#"(?i)src\s*=\s*"([^"]+)""#).expect("合法正则");
    let file_read = regex::Regex::new(r"file-read-(\d+)\.([A-Za-z0-9]+)").expect("合法正则");
    let mut images: Vec<String> = Vec::new();
    let mut out = String::with_capacity(html.len());
    let mut last = 0usize;
    for matched in img_tag.find_iter(html) {
        out.push_str(&html[last..matched.start()]);
        last = matched.end();
        let tag = &html[matched.start()..matched.end()];
        let src = src_attr.captures(tag).map(|c| c[1].to_owned());
        let Some(src) = src else { continue };
        let Some(cap) = file_read.captures(&src) else { continue };
        let Ok(id) = cap[1].parse::<i64>() else { continue };
        if images.len() >= 12 {
            continue;
        }
        let Some(data_url) = cached_image(client, id, &cap[2].to_lowercase(), cache) else {
            continue;
        };
        images.push(data_url);
        out.push_str(&format!("\n【截图{}】\n", images.len()));
    }
    out.push_str(&html[last..]);
    let text = truncate_chars(&html_to_text(&out), crate::productivity::types::MAX_DESCRIPTION);
    (text, images)
}

/// 图片缓存：同一个文件 ID 只下载一次，描述与各条历史共享。
type ImageCache = std::collections::HashMap<i64, String>;

/// 取图片 data URL（优先缓存）。总量超过 24 张或 12MB 后不再新增。
fn cached_image(
    client: &ZentaoClient,
    file_id: i64,
    extension: &str,
    cache: &mut ImageCache,
) -> Option<String> {
    if let Some(existing) = cache.get(&file_id) {
        return Some(existing.clone());
    }
    if cache.len() >= 24 {
        return None;
    }
    let bytes = fetch_file_bytes(client, file_id).ok()?;
    let mime = image_mime(extension, &bytes)?;
    use base64::Engine as _;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let data_url = format!("data:{mime};base64,{encoded}");
    cache.insert(file_id, data_url.clone());
    Some(data_url)
}

/// 从 HTML 片段里收集截图（按文件 ID 去重），追加到 images；单条最多 8 张。
fn collect_images(
    client: &ZentaoClient,
    html: &str,
    cache: &mut ImageCache,
    images: &mut Vec<String>,
) {
    let src_attr = regex::Regex::new(r#"(?i)src\s*=\s*"([^"]+)""#).expect("合法正则");
    let file_read = regex::Regex::new(r"file-read-(\d+)\.([A-Za-z0-9]+)").expect("合法正则");
    let mut seen_here = std::collections::HashSet::new();
    for capture in src_attr.captures_iter(html) {
        if images.len() >= 8 {
            break;
        }
        let Some(file) = file_read.captures(&capture[1]) else { continue };
        let Ok(id) = file[1].parse::<i64>() else { continue };
        if !seen_here.insert(id) {
            continue;
        }
        if let Some(data_url) = cached_image(client, id, &file[2].to_lowercase(), cache) {
            images.push(data_url);
        }
    }
}

/// 经 API v2 通道拉取文件本体（已验证该实例可用）。
fn fetch_file_bytes(client: &ZentaoClient, file_id: i64) -> Result<Vec<u8>, String> {
    let response = client
        .http
        .get(format!("{}/api.php/v2/files/{file_id}/download", client.url))
        .header("token", &client.token)
        .send()
        .map_err(|error| format!("无法读取附件：{error}"))?;
    if !response.status().is_success() {
        return Err(format!("无法读取附件：HTTP {}", response.status()));
    }
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_lowercase();
    if content_type.contains("application/json") || content_type.contains("text/html") {
        return Err("禅道返回的是错误页而不是文件内容".to_owned());
    }
    let bytes = response
        .bytes()
        .map_err(|error| format!("无法读取附件：{error}"))?;
    if bytes.is_empty() {
        return Err("附件内容为空".to_owned());
    }
    Ok(bytes.to_vec())
}

/// 摘要读取附件，不写入下载目录；读取期间限制十 MB。
pub(super) fn summary_attachment_bytes(file_id: i64) -> Result<Vec<u8>, String> {
    if file_id <= 0 { return Err("附件编号无效".into()); }
    let client = ZentaoClient::from_token(read_token()?)?;
    let response = client.http.get(format!("{}/api.php/v2/files/{file_id}/download", client.url))
        .header("token", &client.token).send().map_err(|_| "无法读取摘要附件".to_owned())?;
    if !response.status().is_success() { return Err(format!("附件读取返回 HTTP {}", response.status())); }
    let mut bytes = Vec::new();
    response.take(10 * 1024 * 1024 + 1).read_to_end(&mut bytes).map_err(|_| "附件读取中断".to_owned())?;
    if bytes.is_empty() || bytes.len() > 10 * 1024 * 1024 { return Err("摘要附件须为 1 字节到 10MB".into()); }
    Ok(bytes)
}

/// 图片 MIME 判断：优先扩展名，无扩展名时按魔数嗅探。
fn image_mime(extension: &str, bytes: &[u8]) -> Option<&'static str> {
    match extension {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "svg" => Some("image/svg+xml"),
        "bmp" => Some("image/bmp"),
        "ico" => Some("image/x-icon"),
        "" | "bin" => {
            if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
                Some("image/png")
            } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
                Some("image/jpeg")
            } else if bytes.starts_with(b"GIF8") {
                Some("image/gif")
            } else {
                None
            }
        }
        _ => None,
    }
}

/// 解析 v1 详情返回的 actions 历史数组（可能缺失或形状不符，容错为空）。
/// 富文本字段（steps/desc 等）的 diff 转纯文本截断展示；历史里的截图按条目收集。
fn parse_actions(
    client: &ZentaoClient,
    raw: Option<&Value>,
    cache: &mut ImageCache,
) -> Vec<ZentaoAction> {
    let Some(list) = raw.and_then(Value::as_array) else {
        return Vec::new();
    };
    list.iter()
        .take(100)
        .map(|entry| {
            let mut images = Vec::new();
            let changes = entry
                .get("history")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .map(|item| {
                            let old_raw = text(item, "old").unwrap_or_default();
                            let new_raw = text(item, "new").unwrap_or_default();
                            let rich = old_raw.contains('<') || new_raw.contains('<');
                            // 富文本 diff：把变更后的内容渲染成可读文本（含截图占位符），
                            // 前端折叠展示，避免大段 HTML 直接铺开。
                            let (new_text, new_images) = if rich {
                                render_description(client, &new_raw, cache)
                            } else {
                                (String::new(), Vec::new())
                            };
                            ZentaoActionChange {
                                field: text(item, "field").unwrap_or_default(),
                                old: truncate_chars(&html_to_text(&old_raw).trim().to_owned(), 200),
                                new: truncate_chars(&html_to_text(&new_raw).trim().to_owned(), 200),
                                rich,
                                new_text,
                                new_images,
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();
            let comment_raw = text(entry, "comment").unwrap_or_default();
            collect_images(client, &comment_raw, cache, &mut images);
            ZentaoAction {
                date: text(entry, "date").unwrap_or_default(),
                actor: text(entry, "actor").unwrap_or_default(),
                desc: text(entry, "desc")
                    .map(|raw| html_to_text(&raw).trim().to_owned())
                    .unwrap_or_default(),
                comment: if comment_raw.trim().is_empty() {
                    String::new()
                } else {
                    truncate_chars(&html_to_text(&comment_raw).trim().to_owned(), 2000)
                },
                changes,
                images,
            }
        })
        .collect()
}

/// 下载附件到本机「下载」目录。主通道为 API v2 `/files/{id}/download`（实测该实例可用）；
/// 其余为老版本兜底；全部失败才报错。
pub fn download_file(file_id: i64, extension: &str, title: &str) -> Result<String, String> {
    let token = read_token()?;
    let client = ZentaoClient::from_token(token)?;
    let extension = extension.trim().trim_start_matches('.').to_owned();
    let attempts: Vec<String> = vec![
        format!("/api.php/v2/files/{file_id}/download"),
        format!("/api.php/v2/files/{file_id}"),
        format!("/file-read-{file_id}.{}", if extension.is_empty() { "bin" } else { &extension }),
    ];
    for path in attempts {
        let response = match client
            .http
            .get(format!("{}{path}", client.url))
            .header("token", &client.token)
            .send()
        {
            Ok(response) => response,
            Err(_) => continue,
        };
        if !response.status().is_success() {
            continue;
        }
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_lowercase();
        let bytes = match response.bytes() {
            Ok(bytes) => bytes,
            Err(_) => continue,
        };
        if content_type.contains("application/json") || content_type.contains("text/html") {
            // 返回 JSON/HTML 说明是错误页或元数据而非文件本体，继续尝试下一通道。
            continue;
        }
        if bytes.is_empty() {
            continue;
        }
        return save_download(&bytes, &extension, title);
    }
    Err("无法下载附件：禅道版本不支持 API 下载，请从「在禅道中打开」的页面手动下载".to_owned())
}

const PREVIEW_TEXT_MAX: usize = 512 * 1024;
const PREVIEW_IMAGE_MAX: usize = 8 * 1024 * 1024;

/// 可按 UTF-8 文本预览的扩展名（附件与压缩包内条目共用）。
const PREVIEWABLE_TEXT_EXT: [&str; 22] = [
    "md", "txt", "log", "json", "csv", "tsv", "xml", "yml", "yaml", "html", "htm", "css", "js",
    "ts", "py", "java", "go", "rs", "sql", "sh", "ini", "conf",
];

fn extension_of(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((_, ext)) => ext.to_lowercase(),
        None => String::new(),
    }
}

fn file_content_from_bytes(extension: &str, bytes: &[u8]) -> Result<ZentaoFileContent, String> {
    if let Some(mime) = image_mime(extension, bytes) {
        if bytes.len() > PREVIEW_IMAGE_MAX {
            return Err("图片超过 8MB，请下载后查看".to_owned());
        }
        use base64::Engine as _;
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
        return Ok(ZentaoFileContent {
            kind: "image".to_owned(),
            content: format!("data:{mime};base64,{encoded}"),
        });
    }
    // 含 NUL 字节视为二进制（AppleDouble 资源 fork、可执行文件等），避免渲染成乱码。
    let sniff = &bytes[..bytes.len().min(8192)];
    if sniff.contains(&0) {
        return Err("该文件是二进制格式，暂不支持预览，请下载后查看".to_owned());
    }
    if PREVIEWABLE_TEXT_EXT.contains(&extension) {
        if bytes.len() > PREVIEW_TEXT_MAX {
            return Err("文件超过 512KB，请下载后查看".to_owned());
        }
        return Ok(ZentaoFileContent {
            kind: "text".to_owned(),
            content: String::from_utf8_lossy(bytes).to_string(),
        });
    }
    Err("该类型暂不支持预览，请下载后查看".to_owned())
}

/// 读取附件内容用于预览：文本类返回 UTF-8 文本，图片类返回 data URL，其余仅可下载。
pub fn read_file(file_id: i64, extension: &str) -> Result<ZentaoFileContent, String> {
    let token = read_token()?;
    let client = ZentaoClient::from_token(token)?;
    let extension = extension.trim().trim_start_matches('.').to_lowercase();
    let bytes = fetch_file_bytes(&client, file_id)?;
    file_content_from_bytes(&extension, &bytes)
}

const ARCHIVE_MAX_BYTES: usize = 64 * 1024 * 1024;
/// 条目数上限：超出部分直接截断，防止界面卡顿。
const ARCHIVE_MAX_ENTRIES: usize = 2000;
const ARCHIVE_CACHE_TTL: Duration = Duration::from_secs(10 * 60);

/// 进程内压缩包缓存：列完目录再点开条目时复用包体，避免重复下载。
static ARCHIVE_CACHE: std::sync::Mutex<Option<HashMap<i64, (Instant, Vec<u8>)>>> =
    std::sync::Mutex::new(None);

fn cached_archive_bytes(client: &ZentaoClient, file_id: i64) -> Result<Vec<u8>, String> {
    if let Ok(guard) = ARCHIVE_CACHE.lock() {
        if let Some((at, bytes)) = guard.as_ref().and_then(|map| map.get(&file_id)) {
            if at.elapsed() < ARCHIVE_CACHE_TTL {
                return Ok(bytes.clone());
            }
        }
    }
    let bytes = fetch_file_bytes(client, file_id)?;
    if bytes.len() > ARCHIVE_MAX_BYTES {
        return Err("压缩包超过 64MB，暂不支持在线预览，请下载后查看".to_owned());
    }
    if let Ok(mut guard) = ARCHIVE_CACHE.lock() {
        let map = guard.get_or_insert_with(HashMap::new);
        map.retain(|_, (at, _)| at.elapsed() < ARCHIVE_CACHE_TTL);
        map.insert(file_id, (Instant::now(), bytes.clone()));
    }
    Ok(bytes)
}

/// macOS 打包产物（__MACOSX 目录、.DS_Store、资源 fork ._ 文件）对预览是噪音。
fn is_noise_entry(name: &str) -> bool {
    name.split('/').any(|segment| {
        segment == "__MACOSX" || segment == ".DS_Store" || segment.starts_with("._")
    })
}

/// 规范化 zip 条目名：反斜杠转正斜杠、去掉开头斜杠与目录条目。
fn normalize_entry_name(raw: &str) -> Option<String> {
    let name = raw.replace('\\', "/");
    let name = name.trim_start_matches('/').to_owned();
    if name.is_empty() || name.ends_with('/') || name.contains("..") {
        return None;
    }
    Some(name)
}

/// 解码 zip 条目名：UTF-8 优先，失败按 GBK（Windows 中文压缩工具常见）兜底，
/// 否则中文文件名会显示乱码。
fn decode_entry_name(raw: &[u8]) -> String {
    match std::str::from_utf8(raw) {
        Ok(name) => name.to_owned(),
        Err(_) => {
            let (decoded, _, _) = encoding_rs::GBK.decode(raw);
            decoded.into_owned()
        }
    }
}

/// zip 附件：读取文件条目列表（不含目录节点，目录树由前端按路径推导）。
pub fn read_archive(file_id: i64) -> Result<Vec<ZentaoArchiveEntry>, String> {
    let token = read_token()?;
    let client = ZentaoClient::from_token(token)?;
    let bytes = cached_archive_bytes(&client, file_id)?;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&bytes))
        .map_err(|error| format!("无法解析压缩包：{error}"))?;
    let mut entries = Vec::new();
    for index in 0..archive.len() {
        // by_index_raw 只读元数据不实际解压。
        let Ok(file) = archive.by_index_raw(index) else {
            continue;
        };
        // 条目名自解码：UTF-8 失败回退 GBK，避免依赖 crate 内部解码行为。
        let Some(name) = normalize_entry_name(&decode_entry_name(file.name_raw())) else {
            continue;
        };
        if is_noise_entry(&name) {
            continue;
        }
        if entries.len() >= ARCHIVE_MAX_ENTRIES {
            break;
        }
        let entry_name = name.rsplit('/').next().unwrap_or(&name).to_owned();
        entries.push(ZentaoArchiveEntry {
            index,
            path: name.clone(),
            name: entry_name,
            size: file.size(),
            extension: extension_of(&name),
        });
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(entries)
}

/// zip 附件：按条目索引读取单个条目内容，文本/图片预览规则与附件一致。
pub fn read_archive_entry(file_id: i64, index: usize) -> Result<ZentaoFileContent, String> {
    let token = read_token()?;
    let client = ZentaoClient::from_token(token)?;
    let bytes = cached_archive_bytes(&client, file_id)?;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&bytes))
        .map_err(|error| format!("无法解析压缩包：{error}"))?;
    let max_preview = PREVIEW_TEXT_MAX.max(PREVIEW_IMAGE_MAX);
    let file = archive
        .by_index(index)
        .map_err(|_| "压缩包内未找到该文件".to_owned())?;
    if file.is_dir() {
        return Err("该路径是目录".to_owned());
    }
    let extension = extension_of(&decode_entry_name(file.name_raw()));
    let mut buf = Vec::new();
    // 多读 1 字节用于判断超限。
    file.take((max_preview + 1) as u64)
        .read_to_end(&mut buf)
        .map_err(|error| format!("无法读取压缩包条目：{error}"))?;
    file_content_from_bytes(&extension, &buf)
}

fn save_download(bytes: &[u8], extension: &str, title: &str) -> Result<String, String> {
    let directory = dirs::download_dir()
        .or_else(dirs::home_dir)
        .ok_or_else(|| "无法定位下载目录".to_owned())?;
    let sanitize = |raw: &str| -> String {
        raw.chars()
            .map(|ch| match ch {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                other => other,
            })
            .collect()
    };
    let mut file_name = sanitize(title);
    if file_name.is_empty() {
        file_name = format!("zentao-attachment-{}", now_ms());
    }
    if !extension.is_empty() && !file_name.to_lowercase().ends_with(&format!(".{extension}")) {
        file_name.push('.');
        file_name.push_str(extension);
    }
    let mut path = directory.join(&file_name);
    // 同名文件追加时间戳，避免覆盖。
    if path.exists() {
        let stem = if extension.is_empty() {
            file_name.clone()
        } else {
            file_name
                .trim_end_matches(&format!(".{extension}"))
                .to_owned()
        };
        let name = if extension.is_empty() {
            format!("{stem}-{}", now_ms())
        } else {
            format!("{stem}-{}.{extension}", now_ms())
        };
        path = directory.join(name);
    }
    std::fs::write(&path, bytes).map_err(|error| format!("无法写入附件文件: {error}"))?;
    Ok(path.to_string_lossy().to_string())
}

/// 进程内互斥：手动触发与轮询并发时只跑一个同步。
static SYNC_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn sync(connection: &mut Connection) -> Result<ZentaoSyncResult, String> {
    let _guard = SYNC_LOCK
        .try_lock()
        .map_err(|_| "禅道同步正在进行中".to_owned())?;
    let started = std::time::Instant::now();
    let token = read_token()?;
    let client = ZentaoClient::from_token(token)?;
    let (mut items, mut skipped) = fetch_my_items(&client)?;
    let history_deferred = refresh_missing(&client, connection, &mut items, &mut skipped)?;
    let transaction = connection
        .transaction()
        .map_err(|error| format!("无法开始同步事务: {error}"))?;
    let mut counts = (0usize, 0usize, 0usize);
    let mut report = Vec::new();
    upsert_items(&transaction, &items, &mut counts, &mut report)?;
    let task_count = report.iter().filter(|item| item.external_type == "task").count();
    let bug_count = report.iter().filter(|item| item.external_type == "bug").count();
    let closed = 0usize;
    transaction
        .commit()
        .map_err(|error| format!("无法提交同步事务: {error}"))?;

    Ok(ZentaoSyncResult {
        task_count,
        bug_count,
        created: counts.0,
        updated: counts.1,
        closed,
        skipped,
        elapsed_ms: started.elapsed().as_millis() as u64,
        request_count: client.requests.load(std::sync::atomic::Ordering::Relaxed),
        history_deferred,
        synced_at: now_ms(),
        items: report,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        assigned_to_me, deadline, decode_entry_name, extension_of, extract_items, html_to_text,
        is_noise_entry, map_priority, map_status, normalize, normalize_entry_name,
    };
    use serde_json::json;

    #[test]
    fn normalizes_zip_entry_names() {
        assert_eq!(normalize_entry_name("docs/readme.md"), Some("docs/readme.md".to_owned()));
        // 反斜杠分隔与开头斜杠统一规范化。
        assert_eq!(normalize_entry_name("/a\\b/c.txt"), Some("a/b/c.txt".to_owned()));
        // 目录条目、空名与路径穿越直接丢弃。
        assert_eq!(normalize_entry_name("docs/"), None);
        assert_eq!(normalize_entry_name(""), None);
        assert_eq!(normalize_entry_name("../secret"), None);
    }

    #[test]
    fn extracts_extensions_from_entry_paths() {
        assert_eq!(extension_of("a/b/c.PNG"), "png");
        assert_eq!(extension_of("no-ext"), "");
        assert_eq!(extension_of(".gitignore"), "gitignore");
    }

    #[test]
    fn decodes_gbk_zip_entry_names() {
        assert_eq!(decode_entry_name("docs/readme.md".as_bytes()), "docs/readme.md");
        // 「需求文档.zip」的 GBK 字节序列（Windows 中文压缩工具常见编码）。
        let gbk = [0xD0u8, 0xE8, 0xC7, 0xF3, 0xCE, 0xC4, 0xB5, 0xB5, 0x2E, 0x7A, 0x69, 0x70];
        assert_eq!(decode_entry_name(&gbk), "需求文档.zip");
    }

    #[test]
    fn filters_macos_noise_entries() {
        assert!(is_noise_entry("__MACOSX/._readme.html"));
        assert!(is_noise_entry("docs/.DS_Store"));
        assert!(is_noise_entry("._人证配置.html"));
        assert!(!is_noise_entry("docs/readme.md"));
        assert!(!is_noise_entry("normal_file.txt"));
    }

    #[test]
    fn extracts_items_from_all_response_shapes() {
        let task = json!({ "id": 1, "name": "任务" });
        let shapes = vec![
            json!([task]),
            json!({ "tasks": [task] }),
            json!({ "data": [task] }),
            json!({ "data": { "tasks": [task] } }),
        ];
        for body in shapes {
            assert_eq!(extract_items(&body, "tasks").len(), 1, "shape: {body}");
        }
        assert!(extract_items(&json!({ "total": 0 }), "tasks").is_empty());
    }

    #[test]
    fn normalizes_task_fields() {
        let value = json!({
            "id": 42, "name": "联调接口", "pri": 1, "status": "doing",
            "deadline": "2026-10-12", "estimate": 2.5,
            "desc": "<p>完成联调</p><p>覆盖异常分支</p>",
        });
        let item = normalize("task", &value, "https://zentao.example.com").expect("normalize");
        assert_eq!(item.external_id, "zentao:task:42");
        assert_eq!(item.title, "【#42】联调接口");
        assert_eq!(item.priority, "high");
        assert_eq!(item.status, "doing");
        assert_eq!(item.due.as_deref(), Some("2026-10-12"));
        assert_eq!(item.estimate_minutes, Some(150));
        assert_eq!(item.description.as_deref(), Some("完成联调\n覆盖异常分支"));
        assert_eq!(item.external_url, "https://zentao.example.com/task-view-42.html");
    }

    /// 真实实例（zentao.goktech.cn）的数值字段是字符串，deadline 可能为 null。
    #[test]
    fn normalizes_string_typed_fields() {
        let value = json!({
            "id": "14927", "name": "实施空白名称提示", "pri": "3", "status": "wait",
            "deadline": null, "estimate": "0.00",
            "desc": "<p>在隔离验收 fixture 中实现并测试空白名称提示</p>",
        });
        let item = normalize("task", &value, "https://zentao.example.com").expect("normalize");
        assert_eq!(item.external_id, "zentao:task:14927");
        assert_eq!(item.priority, "medium");
        assert_eq!(item.status, "open");
        assert_eq!(item.due, None);
        assert_eq!(item.estimate_minutes, None);
        assert_eq!(item.description.as_deref(), Some("在隔离验收 fixture 中实现并测试空白名称提示"));
    }

    #[test]
    fn keeps_only_items_assigned_to_the_account() {
        let account = "weipc";
        assert!(assigned_to_me(&json!({ "assignedTo": "weipc" }), account));
        assert!(!assigned_to_me(&json!({ "assignedTo": "zengzh" }), account));
        assert!(!assigned_to_me(&json!({ "assignedTo": "" }), account));
        // 字段缺失时不过滤（老版本兼容）。
        assert!(assigned_to_me(&json!({}), account));
    }

    #[test]
    fn strips_html_from_descriptions() {
        assert_eq!(
            html_to_text("<p>第一步</p><br/><b>加粗</b>&nbsp;&lt;标签&gt;<script>evil()</script>"),
            "第一步\n加粗 <标签>",
        );
        assert_eq!(html_to_text(""), "");
    }

    #[test]
    fn normalizes_bug_priority_with_severity() {
        let value = json!({ "id": 7, "title": "崩溃", "pri": 4, "severity": 1, "status": "active" });
        let item = normalize("bug", &value, "https://z.example.com").expect("normalize");
        assert_eq!(item.title, "【#7】崩溃");
        assert_eq!(item.priority, "high");
        assert_eq!(item.status, "open");
        assert_eq!(item.estimate_minutes, None);
    }

    #[test]
    fn maps_statuses_and_filters_invalid_deadlines() {
        assert_eq!(map_status("task", "wait"), "open");
        assert_eq!(map_status("task", "cancel"), "done");
        assert_eq!(map_status("bug", "resolved"), "done");
        assert_eq!(map_status("story", "closed"), "done");
        assert_eq!(map_priority(None, None), "medium");
        assert_eq!(map_priority(Some(4), None), "low");
        assert!(deadline(&json!({ "deadline": "0000-00-00" }), "deadline").is_none());
        assert_eq!(
            deadline(&json!({ "deadline": "2026-10-12 00:00:00" }), "deadline").as_deref(),
            Some("2026-10-12"),
        );
    }
}


#[cfg(test)]
mod import_tests {
    use super::*;
    use serde_json::json;

    fn write(connection: &mut Connection, items: &[ZentaoItem]) -> ((usize, usize, usize), Vec<ZentaoSyncItem>) {
        let transaction = connection.transaction().unwrap();
        let mut counts = (0, 0, 0);
        let mut report = Vec::new();
        upsert_items(&transaction, items, &mut counts, &mut report).unwrap();
        transaction.commit().unwrap();
        (counts, report)
    }

    fn item(kind: &str, id: i64, status: &str) -> ZentaoItem {
        normalize(kind, &json!({
            "id": id, "name": "任务", "title": "Bug", "status": status, "assignedTo": "me"
        }), "https://example.com").unwrap()
    }

    fn connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        crate::database::initialize(&connection).unwrap();
        connection
    }

    #[test]
    fn first_import_excludes_terminal_tasks_and_bugs_from_storage_and_report() {
        let mut connection = connection();
        let items = [
            item("task", 1, "done"), item("task", 2, "closed"), item("task", 3, "cancel"),
            item("bug", 4, "resolved"), item("bug", 5, "closed"),
            item("task", 6, "wait"), item("task", 7, "doing"), item("bug", 8, "active"),
        ];
        let (counts, report) = write(&mut connection, &items);
        assert_eq!(counts, (3, 0, 0));
        assert_eq!(report.iter().map(|row| row.zentao_id).collect::<Vec<_>>(), vec![6, 7, 8]);
        let stored: i64 = connection.query_row("SELECT COUNT(*) FROM todo_task", [], |r| r.get(0)).unwrap();
        assert_eq!(stored, 3);
        let (counts, report) = write(&mut connection, &items);
        assert_eq!(counts, (0, 0, 0));
        assert_eq!(report.len(), 3);
        assert!(report.iter().all(|row| row.action == "unchanged"));
    }

    #[test]
    fn known_task_completion_is_retained_and_reactivation_keeps_same_identity() {
        let mut connection = connection();
        write(&mut connection, &[item("task", 42, "doing")]);
        let id: String = connection.query_row("SELECT id FROM todo_task", [], |r| r.get(0)).unwrap();
        let (counts, report) = write(&mut connection, &[item("task", 42, "done")]);
        // 完成条目仍跟随远端状态落库，但不计入更新数、也不进同步明细。
        assert_eq!(counts, (0, 0, 0));
        assert!(report.is_empty());
        let completed = crate::productivity::store::read_task(&connection, &id).unwrap();
        assert_eq!(completed.status, "done");
        assert!(completed.completed_at.is_some());
        let (counts, report) = write(&mut connection, &[item("task", 42, "done")]);
        assert_eq!(counts, (0, 0, 0));
        assert!(report.is_empty());
        let unchanged = crate::productivity::store::read_task(&connection, &id).unwrap();
        assert_eq!(unchanged.completed_at, completed.completed_at);
        let (counts, report) = write(&mut connection, &[item("task", 42, "doing")]);
        assert_eq!(counts, (0, 1, 0));
        assert_eq!(report[0].action, "updated");
        let active = crate::productivity::store::read_task(&connection, &id).unwrap();
        assert_eq!(active.status, "doing");
        assert!(active.completed_at.is_none());
        let stored: i64 = connection.query_row("SELECT COUNT(*) FROM todo_task", [], |r| r.get(0)).unwrap();
        assert_eq!(stored, 1);
    }

    #[test]
    fn assignment_only_change_counts_as_update_instead_of_unchanged() {
        let mut connection = connection();
        let mut bug = item("bug", 42, "active");
        write(&mut connection, &[bug.clone()]);
        bug.assigned_to = "tester".to_owned();
        bug.transferred = true;
        let (counts, report) = write(&mut connection, &[bug]);
        assert_eq!(counts, (0, 1, 0));
        assert_eq!(report[0].action, "updated");
    }
}

#[cfg(test)]
mod sync_performance_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn read_workers_are_bounded_and_keep_input_order() {
        let active = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let values: Vec<_> = (0..12).collect();
        let result = parallel_read(&values, |value| {
            let count = active.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(count, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(2 * (4 - value % 4)));
            active.fetch_sub(1, Ordering::SeqCst);
            value * 2
        }).unwrap();
        assert_eq!(result, values.iter().map(|v| v * 2).collect::<Vec<_>>());
        assert!(peak.load(Ordering::SeqCst) <= SYNC_READ_CONCURRENCY);
        assert_eq!(active.load(Ordering::SeqCst), 0);
        assert!(parallel_read::<i32, i32>(&[], |v| *v).unwrap().is_empty());
    }

    #[test]
    fn only_recent_terminal_history_defers_rechecks() {
        let now = HISTORY_RECHECK_MS * 4;
        for status in ["active", "wait", "doing", "pause", ""] {
            assert!(should_recheck_history(status, Some(now), now));
        }
        for status in ["done", "resolved", "closed", "cancel"] {
            assert!(!should_recheck_history(status, Some(now - 1), now));
            assert!(should_recheck_history(status, Some(now - HISTORY_RECHECK_MS), now));
            assert!(should_recheck_history(status, None, now));
            assert!(should_recheck_history(status, Some(now + 1), now));
        }
    }
}
