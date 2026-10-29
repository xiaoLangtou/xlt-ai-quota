//! 显式状态流转：一次提交、回读验证，再更新本地镜像。
use super::*;
use crate::productivity::{store, types::TodoTask};
use chrono::NaiveDateTime;
use serde_json::json;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ZentaoActionInput {
    pub task_id: String,
    pub operation: String,
    pub assigned_to: String,
    pub resolution: String,
    pub duplicate_bug: Option<i64>,
    pub resolved_build: String,
    pub action_date: String,
    pub real_started: String,
    pub consumed: f64,
    pub left: f64,
    pub comment: String,
}

#[derive(Serialize)]
pub struct Choice {
    pub value: String,
    pub label: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZentaoActionContext {
    pub status: String,
    pub opened_by: String,
    pub assigned_to: String,
    pub account: String,
    pub real_started: String,
    pub consumed: f64,
    pub left: f64,
    pub users: Vec<Choice>,
    pub builds: Vec<Choice>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZentaoActionResult {
    pub task: Option<TodoTask>,
    pub warning: String,
}

pub(super) fn account_field(object: &Value, field: &str) -> String {
    let value = &object[field];
    if value.is_object() {
        text(value, "account").unwrap_or_default()
    } else {
        text(object, field).unwrap_or_default()
    }
}

pub(super) fn check_body(body: Value) -> Result<Value, String> {
    if body.get("status").and_then(Value::as_str) == Some("fail")
        || body.get("error").is_some_and(|value| {
            !value.is_null() && value != &json!(false) && value != &json!("") && value != &json!(0)
        })
    {
        let message = body
            .get("message")
            .or_else(|| body.get("error"))
            .map(|value| truncate_chars(&value.to_string(), 600))
            .unwrap_or_else(|| "禅道拒绝了本次操作".to_owned());
        return Err(message);
    }
    Ok(body)
}

pub(super) fn read_object(client: &ZentaoClient, kind: &str, id: i64) -> Result<Value, String> {
    let collection = match kind {
        "bug" => "bugs",
        "task" => "tasks",
        _ => return Err("不支持的禅道类型".to_owned()),
    };
    let body = check_body(client.get_raw(&format!("/api.php/v1/{collection}/{id}"), &[])?)?;
    let object = [kind, "data"]
        .iter()
        .find_map(|key| body.get(*key).filter(|value| value.is_object()))
        .unwrap_or(&body);
    if number(object, "id") != Some(id) || text(object, "status").is_none() {
        return Err("禅道详情缺少匹配的编号或状态".to_owned());
    }
    Ok(object.clone())
}

fn target(connection: &Connection, task_id: &str) -> Result<(String, i64), String> {
    let task = store::read_task(connection, task_id)?;
    if task.source_type != "zentao" {
        return Err("该待办不是禅道来源".to_owned());
    }
    let kind = task.external_type.ok_or("禅道类型缺失")?;
    if !matches!(kind.as_str(), "task" | "bug") {
        return Err("不支持的禅道类型".to_owned());
    }
    let id = task
        .external_id
        .and_then(|key| {
            key.strip_prefix(&format!("zentao:{kind}:"))?
                .parse::<i64>()
                .ok()
        })
        .filter(|id| *id > 0)
        .ok_or("禅道编号无效")?;
    Ok((kind, id))
}

pub fn action_context(
    connection: &Connection,
    task_id: &str,
    _operation: &str,
) -> Result<ZentaoActionContext, String> {
    let client = ZentaoClient::from_token(read_token()?)?;
    action_context_with_client(connection, task_id, &client)
}

fn action_context_with_client(
    connection: &Connection,
    task_id: &str,
    client: &ZentaoClient,
) -> Result<ZentaoActionContext, String> {
    let (kind, id) = target(connection, task_id)?;
    let object = read_object(client, &kind, id)?;
    let mut users = Vec::new();
    let user_list = if kind == "bug" {
        client.list("/users", "users", &[("browseType", "inside")])?
    } else {
        Vec::new()
    };
    for user in user_list {
        let Some(account) = text(&user, "account").filter(|value| !value.is_empty()) else {
            continue;
        };
        if text(&user, "deleted").as_deref() == Some("1") {
            continue;
        }
        let name = text(&user, "realname").unwrap_or_default();
        users.push(Choice {
            label: format!("{name} ({account})"),
            value: account,
        });
    }
    // 解决使用明确的主干版本，不读取项目版本列表。
    let builds = vec![Choice {
        value: "trunk".to_owned(),
        label: "主干（trunk）".to_owned(),
    }];
    Ok(ZentaoActionContext {
        opened_by: account_field(&object, "openedBy"),
        status: text(&object, "status").unwrap_or_default(),
        assigned_to: account_field(&object, "assignedTo"),
        account: client.account.clone(),
        real_started: text(&object, "realStarted")
            .filter(|date| !date.starts_with("0000"))
            .unwrap_or_default(),
        consumed: number_f64(&object, "consumed").unwrap_or(0.0),
        left: number_f64(&object, "left").unwrap_or(0.0),
        users,
        builds,
    })
}

fn timestamp(value: &str) -> Result<(), String> {
    NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
        .map(|_| ())
        .map_err(|_| "请填写有效的日期和时间".to_owned())
}

fn payload(
    input: &ZentaoActionInput,
    kind: &str,
    id: i64,
    object: &Value,
) -> Result<(reqwest::Method, String, Value), String> {
    if input.comment.chars().count() > 4000 {
        return Err("备注不能超过 4000 字".to_owned());
    }
    if !input.consumed.is_finite()
        || input.consumed < 0.0
        || !input.left.is_finite()
        || input.left < 0.0
    {
        return Err("工时必须为非负数".to_owned());
    }
    let status = text(object, "status").ok_or("禅道状态缺失")?;
    let comment = input.comment.trim();
    let (method, path, body) = match (kind, input.operation.as_str()) {
        ("bug", "assign") => {
            if status == "closed" {
                return Err("已关闭的 Bug 不能指派".to_owned());
            }
            if input.assigned_to.trim().is_empty() {
                return Err("请选择指派人".to_owned());
            }
            if account_field(object, "assignedTo") == input.assigned_to.trim() {
                return Err("该 Bug 已指派给此人，无需重复提交".to_owned());
            }
            (
                reqwest::Method::POST,
                format!("/bugs/{id}/assign"),
                json!({
                    "assignedTo": input.assigned_to.trim(), "comment": comment,
                }),
            )
        }
        ("bug", "resolve") => {
            if status != "active" {
                return Err("只有激活状态的 Bug 可以解决，请刷新详情".to_owned());
            }
            if !matches!(
                input.resolution.as_str(),
                "bydesign"
                    | "duplicate"
                    | "external"
                    | "fixed"
                    | "notrepro"
                    | "postponed"
                    | "willnotfix"
            ) {
                return Err("请选择有效的解决方案".to_owned());
            }
            timestamp(&input.action_date)?;
            if input.resolution == "duplicate"
                && !input
                    .duplicate_bug
                    .is_some_and(|other| other > 0 && other != id)
            {
                return Err("请填写其他 Bug 的有效编号".to_owned());
            }
            if input.resolution == "fixed" && input.resolved_build.trim().is_empty() {
                return Err("请选择解决版本".to_owned());
            }
            let mut body = json!({"resolution": input.resolution, "resolvedDate": input.action_date, "comment": comment});
            if input.resolution == "duplicate" {
                body["duplicateBug"] = json!(input.duplicate_bug);
            }
            if !input.resolved_build.is_empty() {
                body["resolvedBuild"] = json!(input.resolved_build);
            }
            if !input.assigned_to.is_empty() {
                body["assignedTo"] = json!(input.assigned_to);
            }
            (reqwest::Method::POST, format!("/bugs/{id}/resolve"), body)
        }
        ("task", "start") => {
            if status != "wait" {
                return Err("只有未开始的任务可以开始，请刷新详情".to_owned());
            }
            timestamp(&input.real_started)?;
            (
                reqwest::Method::POST,
                format!("/tasks/{id}/start"),
                json!({
                    "realStarted": input.real_started, "consumed": input.consumed, "left": input.left, "comment": comment,
                }),
            )
        }
        ("task", "finish") => {
            if !matches!(status.as_str(), "wait" | "doing" | "pause") {
                return Err("当前任务状态不能完成，请刷新详情".to_owned());
            }
            timestamp(&input.action_date)?;
            timestamp(&input.real_started)?;
            if input.action_date < input.real_started {
                return Err("完成时间不能早于开始时间".to_owned());
            }
            (
                reqwest::Method::POST,
                format!("/tasks/{id}/finish"),
                json!({
                    "currentConsumed": input.consumed, "realStarted": input.real_started,
                    "finishedDate": input.action_date, "comment": comment,
                }),
            )
        }
        _ => return Err("该禅道类型不支持此操作".to_owned()),
    };
    Ok((method, format!("/api.php/v1{path}"), body))
}

fn verify(input: &ZentaoActionInput, object: &Value) -> bool {
    match input.operation.as_str() {
        "assign" => account_field(object, "assignedTo") == input.assigned_to.trim(),
        "resolve" => {
            text(object, "status").as_deref() == Some("resolved")
                && text(object, "resolution").as_deref() == Some(input.resolution.as_str())
        }
        "start" => text(object, "status").as_deref() == Some("doing"),
        "finish" => text(object, "status").as_deref() == Some("done"),
        _ => false,
    }
}

pub fn action(
    connection: &mut Connection,
    input: ZentaoActionInput,
) -> Result<ZentaoActionResult, String> {
    let _guard = SYNC_LOCK
        .try_lock()
        .map_err(|_| "禅道正在同步或处理其他操作，请稍后再试".to_owned())?;
    let client = ZentaoClient::from_token(read_token()?)?;
    execute_action(connection, &client, input)
}

fn execute_action(
    connection: &mut Connection,
    client: &ZentaoClient,
    input: ZentaoActionInput,
) -> Result<ZentaoActionResult, String> {
    let (kind, id) = target(connection, &input.task_id)?;
    let before = read_object(client, &kind, id)?;
    let (method, path, body) = payload(&input, &kind, id, &before)?;
    let response = client
        .http
        .request(method, format!("{}{path}", client.url))
        .header("token", &client.token)
        .json(&body)
        .send()
        .map_err(|_| "操作请求已发送但未能收到结果，请刷新禅道状态核实，勿重复提交".to_owned())?;
    if !response.status().is_success() {
        return Err(format!(
            "禅道操作返回 HTTP {}，请刷新状态核实结果",
            response.status()
        ));
    }
    let body = response
        .json::<Value>()
        .map_err(|_| "禅道返回非 JSON 结果，请刷新状态核实，勿重复提交".to_owned())?;
    check_body(body)?;
    // 写入成功后所有不确定结果都单独返回，避免把本地刷新失败提示成“提交失败”。
    let after = match read_object(client, &kind, id) {
        Ok(object) => object,
        Err(error) => {
            return Ok(ZentaoActionResult {
                task: None,
                warning: format!("请求已提交，回读未完成：{error}。请刷新核实，勿重复提交。"),
            })
        }
    };
    if !verify(&input, &after) {
        return Ok(ZentaoActionResult {
            task: None,
            warning: "请求已提交，但禅道当前状态未确认操作生效。请刷新核实，勿重复提交。"
                .to_owned(),
        });
    }
    let local = (|| {
        store::read_task(connection, &input.task_id)?;
        let mut item = normalize(&kind, &after, &client.url).ok_or("禅道返回的详情不完整")?;
        item.transferred = item.assigned_to != client.account;
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        upsert_items(&transaction, &[item], &mut (0, 0, 0), &mut Vec::new())?;
        transaction.commit().map_err(|e| e.to_string())?;
        store::read_task(connection, &input.task_id)
    })();
    match local {
        Ok(task) => Ok(ZentaoActionResult {
            task: Some(task),
            warning: String::new(),
        }),
        Err(error) => Ok(ZentaoActionResult {
            task: None,
            warning: format!("禅道操作已生效，本地刷新失败：{error}。请同步禅道。"),
        }),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ZentaoCommentInput {
    pub task_id: String,
    pub comment: String,
    pub password: String,
}

pub fn comment(connection: &Connection, input: ZentaoCommentInput) -> Result<(), String> {
    let _guard = SYNC_LOCK.try_lock().map_err(|_| "禅道正在同步或处理其他操作，请稍后再试".to_owned())?;
    let client = ZentaoClient::from_token(read_token()?)?;
    let web = reqwest::blocking::Client::builder().timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none()).build().map_err(|_| "无法创建禅道会话".to_owned())?;
    execute_comment(connection, &client, &web, input)
}

fn parse_web_response(response: reqwest::blocking::Response, action: &str) -> Result<Value, String> {
    if !response.status().is_success() { return Err(format!("{action}：服务器返回 HTTP {}", response.status())); }
    let body: Value = response.json().map_err(|_| format!("{action}：服务器响应不是 JSON"))?;
    if body.get("status").and_then(Value::as_str) != Some("success") {
        return Err(format!("{action}未成功，请检查禅道账号、权限及实例版本"));
    }
    Ok(body)
}

fn comment_actions(object: &Value) -> Result<&Vec<Value>, String> {
    object.get("actions").and_then(Value::as_array).ok_or_else(|| "无法读取禅道动态，不能核实评论是否生效".to_owned())
}

fn execute_comment(connection: &Connection, client: &ZentaoClient, web: &reqwest::blocking::Client, input: ZentaoCommentInput) -> Result<(), String> {
    let message = input.comment.trim();
    if message.is_empty() || message.chars().count() > 4000 { return Err("评论须为 1 到 4000 字".to_owned()); }
    if input.password.is_empty() || client.account.is_empty() { return Err("请输入禅道密码".to_owned()); }
    let (kind, id) = target(connection, &input.task_id)?;
    let before = read_object(client, &kind, id)?;
    let previous = comment_actions(&before)?;
    if previous.iter().any(|action| number(action, "id").is_none()) {
        return Err("禅道动态缺少记录编号，不能核实评论是否生效".to_owned());
    }
    let old_ids: std::collections::HashSet<i64> = previous.iter().filter_map(|a| number(a, "id")).collect();
    let session = parse_web_response(web.get(format!("{}/api-getsessionid.json", client.url)).send()
        .map_err(|_| "无法建立禅道网页会话".to_owned())?, "获取网页会话")?;
    let data = &session["data"];
    let session_data: Value = if let Some(raw) = data.as_str() { serde_json::from_str(raw).map_err(|_| "网页会话数据无效".to_owned())? } else { data.clone() };
    let session_name = session_data["sessionName"].as_str().filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')).ok_or("网页会话名称无效")?;
    let session_id = session_data["sessionID"].as_str().filter(|s| !s.is_empty()).ok_or("网页会话编号无效")?;
    let login = parse_web_response(web.post(format!("{}/user-login.json", client.url)).query(&[(session_name, session_id)])
        .form(&[("account", client.account.as_str()), ("password", input.password.as_str())]).send()
        .map_err(|_| "无法登录禅道网页会话".to_owned())?, "网页会话登录")?;
    let authenticated_id = login["token"].as_str().filter(|s| !s.is_empty()).ok_or("网页会话登录未返回有效会话")?;
    let result = web.post(format!("{}/action-comment-{kind}-{id}.json", client.url)).query(&[(session_name, authenticated_id)])
        .form(&[("comment", message)]).send()
        .map_err(|_| "评论请求已发送但未收到结果，请刷新动态核实，勿重复提交".to_owned())?;
    parse_web_response(result, "评论提交").map_err(|error| format!("{error}；请刷新动态核实，勿重复提交"))?;
    let after = read_object(client, &kind, id).map_err(|_| "评论已提交，但动态回读失败，请刷新核实，勿重复提交".to_owned())?;
    let found = comment_actions(&after).map_err(|_| "评论已提交，但动态缺失，请刷新核实，勿重复提交".to_owned())?.iter().any(|a| {
        number(a, "id").is_some_and(|id| !old_ids.contains(&id))
            && text(a, "comment").is_some_and(|raw| html_to_text(&raw).trim() == message)
    });
    if !found { return Err("评论已提交，但未能在禅道动态中核实，请刷新核实，勿重复提交".to_owned()); }
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ZentaoBatchResolveInput {
    pub task_ids: Vec<String>,
    pub resolution: String,
    pub resolved_build: String,
    pub action_date: String,
    pub comment: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZentaoBatchItemResult {
    pub task_id: String,
    pub title: String,
    pub ok: bool,
    pub error: String,
    pub task: Option<TodoTask>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZentaoBatchResolveResult {
    pub items: Vec<ZentaoBatchItemResult>,
    pub succeeded: usize,
    pub failed: usize,
}

const BATCH_MARGIN_MS: u64 = 150;
const BATCH_MAX_ITEMS: usize = 50;

/// 批量解决 Bug：逐条复用单条「提交 → 回读核实 → 更新本地镜像」流程。
/// 单条失败不影响其余条目；整批共用一次同步锁，避免与轮询同步交叉。
pub fn batch_resolve(
    connection: &mut Connection,
    input: ZentaoBatchResolveInput,
) -> Result<ZentaoBatchResolveResult, String> {
    if input.task_ids.is_empty() {
        return Err("请选择要解决的 Bug".to_owned());
    }
    if input.task_ids.len() > BATCH_MAX_ITEMS {
        return Err(format!("单次最多解决 {BATCH_MAX_ITEMS} 个 Bug"));
    }
    if !matches!(
        input.resolution.as_str(),
        "bydesign"
            | "duplicate"
            | "external"
            | "fixed"
            | "notrepro"
            | "postponed"
            | "willnotfix"
    ) {
        return Err("请选择有效的解决方案".to_owned());
    }
    timestamp(&input.action_date)?;
    let _guard = SYNC_LOCK
        .try_lock()
        .map_err(|_| "禅道正在同步或处理其他操作，请稍后再试".to_owned())?;
    let client = ZentaoClient::from_token(read_token()?)?;
    batch_resolve_with_client(connection, &client, input)
}

fn batch_resolve_with_client(
    connection: &mut Connection,
    client: &ZentaoClient,
    input: ZentaoBatchResolveInput,
) -> Result<ZentaoBatchResolveResult, String> {
    let mut items = Vec::with_capacity(input.task_ids.len());
    for (index, task_id) in input.task_ids.iter().enumerate() {
        let title = store::read_task(connection, task_id)
            .map(|task| task.title)
            .unwrap_or_default();
        let action = ZentaoActionInput {
            task_id: task_id.clone(),
            operation: "resolve".to_owned(),
            assigned_to: String::new(),
            resolution: input.resolution.clone(),
            duplicate_bug: None,
            resolved_build: input.resolved_build.clone(),
            action_date: input.action_date.clone(),
            real_started: String::new(),
            consumed: 0.0,
            left: 0.0,
            comment: input.comment.clone(),
        };
        let (ok, error, task) = match execute_action(connection, client, action) {
            Ok(result) => match result.task {
                Some(task) => (true, String::new(), Some(task)),
                None => (false, result.warning, None),
            },
            Err(error) => (false, error, None),
        };
        items.push(ZentaoBatchItemResult {
            task_id: task_id.clone(),
            title,
            ok,
            error,
            task,
        });
        if index + 1 < input.task_ids.len() {
            // 控制请求节奏，降低触发禅道频率限制（403）的概率。
            std::thread::sleep(std::time::Duration::from_millis(BATCH_MARGIN_MS));
        }
    }
    let succeeded = items.iter().filter(|item| item.ok).count();
    let failed = items.len() - succeeded;
    Ok(ZentaoBatchResolveResult {
        items,
        succeeded,
        failed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(operation: &str) -> ZentaoActionInput {
        ZentaoActionInput {
            task_id: "local".to_owned(),
            operation: operation.to_owned(),
            assigned_to: "tester".to_owned(),
            resolution: "fixed".to_owned(),
            duplicate_bug: None,
            resolved_build: "trunk".to_owned(),
            action_date: "2026-10-23 12:00:00".to_owned(),
            real_started: "2026-10-23 09:00:00".to_owned(),
            consumed: 1.5,
            left: 2.0,
            comment: "已验证".to_owned(),
        }
    }

    #[test]
    fn task_actions_use_workflow_endpoints_and_distinguish_total_from_current_hours() {
        let (method, path, start) =
            payload(&input("start"), "task", 42, &json!({"status":"wait"})).unwrap();
        assert_eq!(method, reqwest::Method::POST);
        assert_eq!(path, "/api.php/v1/tasks/42/start");
        assert_eq!(start["consumed"], 1.5);
        assert_eq!(start["left"], 2.0);
        let (_, path, finish) =
            payload(&input("finish"), "task", 42, &json!({"status":"doing"})).unwrap();
        assert_eq!(path, "/api.php/v1/tasks/42/finish");
        assert_eq!(finish["currentConsumed"], 1.5);
        assert!(finish.get("consumed").is_none());
    }

    #[test]
    fn rejects_invalid_transitions_dates_hours_and_duplicate_targets() {
        let mut action = input("resolve");
        action.resolution = "duplicate".to_owned();
        for duplicate in [None, Some(0), Some(42)] {
            action.duplicate_bug = duplicate;
            assert!(payload(&action, "bug", 42, &json!({"status":"active"})).is_err());
        }
        action.duplicate_bug = Some(99);
        assert_eq!(
            payload(&action, "bug", 42, &json!({"status":"active"}))
                .unwrap()
                .2["duplicateBug"],
            99
        );
        assert!(payload(&action, "bug", 42, &json!({"status":"resolved"})).is_err());
        action = input("finish");
        action.action_date = "2026-10-23 08:00:00".to_owned();
        assert!(payload(&action, "task", 42, &json!({"status":"doing"})).is_err());
        action = input("start");
        action.left = f64::NAN;
        assert!(payload(&action, "task", 42, &json!({"status":"wait"})).is_err());
        action = input("start");
        assert!(payload(&action, "bug", 42, &json!({"status":"active"})).is_err());
        assert!(payload(&action, "task", 42, &json!({"status":"done"})).is_err());
    }

    #[test]
    fn business_failure_and_unconfirmed_remote_state_are_not_success() {
        assert!(check_body(json!({"status":"fail","message":"无权限"})).is_err());
        assert!(check_body(json!({"error":"permission denied"})).is_err());
        assert!(!verify(&input("resolve"), &json!({"status":"active"})));
        assert!(!verify(
            &input("resolve"),
            &json!({"status":"resolved","resolution":"duplicate"})
        ));
        assert!(verify(
            &input("resolve"),
            &json!({"status":"resolved","resolution":"fixed"})
        ));
        assert!(verify(
            &input("assign"),
            &json!({"assignedTo":{"account":"tester"}})
        ));
    }

    #[test]
    fn remote_reactivation_and_transfer_preserve_local_schedule() {
        let mut connection = Connection::open_in_memory().unwrap();
        crate::database::initialize(&connection).unwrap();
        // 迁移必须可重复运行。
        crate::database::initialize(&connection).unwrap();
        let mut item = normalize(
            "bug",
            &json!({"id":42,"title":"Bug","status":"active","assignedTo":"me"}),
            "https://example.com",
        )
        .unwrap();
        fn write(connection: &mut Connection, item: ZentaoItem) {
            let tx = connection.transaction().unwrap();
            upsert_items(&tx, &[item], &mut (0, 0, 0), &mut Vec::new()).unwrap();
            tx.commit().unwrap();
        }
        write(&mut connection, item.clone());
        item.status = "done".to_owned();
        item.external_status = "resolved".to_owned();
        write(&mut connection, item.clone());
        let id: String = connection
            .query_row("SELECT id FROM todo_task", [], |r| r.get(0))
            .unwrap();
        connection
            .execute(
                "UPDATE todo_task SET planned_date='2026-10-24' WHERE id=?1",
                [&id],
            )
            .unwrap();
        item.status = "open".to_owned();
        item.external_status = "active".to_owned();
        item.assigned_to = "tester".to_owned();
        item.transferred = true;
        write(&mut connection, item);
        let task = store::read_task(&connection, &id).unwrap();
        assert_eq!(task.status, "open");
        assert!(task.completed_at.is_none());
        assert_eq!(task.external_status.as_deref(), Some("active"));
        assert_eq!(task.external_assigned_to.as_deref(), Some("tester"));
        assert!(task.external_transferred);
        assert_eq!(task.planned_date.as_deref(), Some("2026-10-24"));
        assert!(store::set_task_status(&mut connection, &id, "done").is_err());
    }
}

#[cfg(test)]
mod request_tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    fn server(responses: Vec<Value>) -> (ZentaoClient, thread::JoinHandle<Vec<(String, Value)>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let handle = thread::spawn(move || {
            let mut requests = Vec::new();
            for response in responses {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut chunk = [0u8; 4096];
                let (header_end, length) = loop {
                    let read = stream.read(&mut chunk).unwrap();
                    assert!(read > 0);
                    bytes.extend_from_slice(&chunk[..read]);
                    if let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                        let header = String::from_utf8_lossy(&bytes[..end]);
                        let length = header
                            .lines()
                            .find_map(|line| {
                                let (name, value) = line.split_once(':')?;
                                name.eq_ignore_ascii_case("content-length")
                                    .then(|| value.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        break (end + 4, length);
                    }
                };
                while bytes.len() < header_end + length {
                    let read = stream.read(&mut chunk).unwrap();
                    assert!(read > 0);
                    bytes.extend_from_slice(&chunk[..read]);
                }
                let header = String::from_utf8_lossy(&bytes[..header_end]);
                let body = if length == 0 {
                    Value::Null
                } else if header.to_ascii_lowercase().contains("application/json") {
                    serde_json::from_slice(&bytes[header_end..header_end + length]).unwrap()
                } else {
                    Value::String(String::from_utf8_lossy(&bytes[header_end..header_end + length]).to_string())
                };
                requests.push((header.lines().next().unwrap().to_owned(), body));
                let body = response.to_string();
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
            }
            requests
        });
        let client = ZentaoClient {
            url,
            token: "test-token".to_owned(),
            account: "me".to_owned(),
            requests: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            http: reqwest::blocking::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(3))
                .build()
                .unwrap(),
        };
        (client, handle)
    }

    fn fixture(client: &ZentaoClient) -> (Connection, ZentaoActionInput) {
        let mut connection = Connection::open_in_memory().unwrap();
        crate::database::initialize(&connection).unwrap();
        let tx = connection.transaction().unwrap();
        let item = normalize("bug", &bug("active", "me"), &client.url).unwrap();
        upsert_items(&tx, &[item], &mut (0, 0, 0), &mut Vec::new()).unwrap();
        tx.commit().unwrap();
        let task_id = connection
            .query_row("SELECT id FROM todo_task", [], |r| r.get(0))
            .unwrap();
        let input = ZentaoActionInput {
            task_id,
            operation: "assign".to_owned(),
            assigned_to: "tester".to_owned(),
            resolution: String::new(),
            duplicate_bug: None,
            resolved_build: String::new(),
            action_date: String::new(),
            real_started: String::new(),
            consumed: 0.0,
            left: 0.0,
            comment: "请协助处理".to_owned(),
        };
        (connection, input)
    }
    fn bug(status: &str, account: &str) -> Value {
        bug_at(42, status, account)
    }
    fn bug_at(id: i64, status: &str, account: &str) -> Value {
        json!({"id":id,"title":format!("Bug {id}"),"status":status,"assignedTo":account,"steps":"<p>original</p>"})
    }
    fn resolved_bug(id: i64, account: &str) -> Value {
        json!({"id":id,"title":format!("Bug {id}"),"status":"resolved","resolution":"fixed","assignedTo":account,"steps":"<p>original</p>"})
    }

    #[test]
    fn comment_uses_session_and_verifies_new_history_once() {
        let mut before = bug("active", "me");
        before["actions"] = json!([{ "id": 8, "comment": "旧评论" }]);
        let mut after = before.clone();
        after["actions"] = json!([{ "id": 8, "comment": "旧评论" }, { "id": 9, "comment": "<p>请确认</p>" }]);
        let (client, handle) = server(vec![before, json!({"status":"success", "data":{"sessionName":"zentaosid", "sessionID":"initial"}}),
            json!({"status":"success", "token":"authenticated"}), json!({"status":"success", "data":9}), after]);
        let (connection, action_input) = fixture(&client);
        let input = ZentaoCommentInput { task_id: action_input.task_id, comment: "请确认".into(), password: "test-only".into() };
        execute_comment(&connection, &client, &client.http, input).unwrap();
        let requests = handle.join().unwrap();
        assert_eq!(requests.len(), 5);
        assert!(requests[3].0.starts_with("POST /action-comment-bug-42.json?zentaosid=authenticated "));
        assert!(requests[3].1.as_str().unwrap().starts_with("comment="));
    }

    #[test]
    fn comment_that_is_not_confirmed_is_never_reported_successful() {
        let mut before = bug("active", "me");
        before["actions"] = json!([{ "id": 8, "comment": "同一文本" }]);
        let (client, handle) = server(vec![before.clone(), json!({"status":"success", "data":{"sessionName":"zentaosid", "sessionID":"initial"}}),
            json!({"status":"success", "token":"authenticated"}), json!({"status":"success"}), before]);
        let (connection, action_input) = fixture(&client);
        let input = ZentaoCommentInput { task_id: action_input.task_id, comment: "同一文本".into(), password: "test-only".into() };
        let error = execute_comment(&connection, &client, &client.http, input).unwrap_err();
        assert!(error.contains("勿重复提交"));
        assert_eq!(handle.join().unwrap().len(), 5);
    }

    #[test]
    fn resolve_context_does_not_request_project_builds() {
        let mut detail = bug("active", "me");
        detail["project"] = json!(243);
        detail["openedBy"] = json!({"account":"creator"});
        let (client, handle) = server(vec![detail, json!({"users":[{"account":"creator","realname":"创建人"}]})]);
        let (connection, input) = fixture(&client);
        let context = action_context_with_client(&connection, &input.task_id, &client).unwrap();
        assert_eq!(context.opened_by, "creator");
        assert_eq!(context.builds.len(), 1);
        assert_eq!(context.builds[0].value, "trunk");
        let requests = handle.join().unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].0, "GET /api.php/v1/bugs/42 HTTP/1.1");
        assert!(requests[1].0.starts_with("GET /api.php/v2/users?"));
        assert!(requests.iter().all(|(path, _)| !path.contains("/builds")));
    }

    #[test]
    fn assignment_submits_once_then_verifies_and_updates_local_mirror() {
        let (client, handle) = server(vec![
            bug("active", "me"),
            json!({"status":"success"}),
            bug("active", "tester"),
        ]);
        let (mut connection, input) = fixture(&client);
        let result = execute_action(&mut connection, &client, input).unwrap();
        assert!(result.warning.is_empty());
        let task = result.task.unwrap();
        assert_eq!(task.status, "open");
        assert!(task.external_transferred);
        assert_eq!(task.description.as_deref(), Some("original"));
        let requests = handle.join().unwrap();
        assert_eq!(requests.len(), 3);
        assert_eq!(requests[1].0, "POST /api.php/v1/bugs/42/assign HTTP/1.1");
        assert_eq!(
            requests[1].1,
            json!({"assignedTo":"tester","comment":"请协助处理"})
        );
    }

    #[test]
    fn http_200_business_rejection_does_not_change_local_state() {
        let (client, handle) = server(vec![
            bug("active", "me"),
            json!({"status":"fail","message":"无权限"}),
        ]);
        let (mut connection, input) = fixture(&client);
        let id = input.task_id.clone();
        assert!(execute_action(&mut connection, &client, input)
            .err()
            .unwrap()
            .contains("无权限"));
        assert_eq!(
            store::read_task(&connection, &id)
                .unwrap()
                .external_assigned_to
                .as_deref(),
            Some("me")
        );
        assert_eq!(handle.join().unwrap().len(), 2);
    }

    #[test]
    fn unconfirmed_write_returns_warning_and_leaves_local_state_unchanged() {
        let (client, handle) = server(vec![
            bug("active", "me"),
            json!({"status":"success"}),
            bug("active", "me"),
        ]);
        let (mut connection, input) = fixture(&client);
        let id = input.task_id.clone();
        let result = execute_action(&mut connection, &client, input).unwrap();
        assert!(result.task.is_none());
        assert!(!result.warning.is_empty());
        assert!(
            !store::read_task(&connection, &id)
                .unwrap()
                .external_transferred
        );
        assert_eq!(handle.join().unwrap().len(), 3);
    }

    #[test]
    fn missing_from_my_list_uses_remote_status_instead_of_marking_done() {
        let (client, handle) = server(vec![bug("active", "tester")]);
        let (connection, _) = fixture(&client);
        let mut items = Vec::new();
        let mut skipped = String::new();
        refresh_missing(&client, &connection, &mut items, &mut skipped).unwrap();
        assert!(skipped.is_empty());
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].status, "open");
        assert!(items[0].transferred);
        handle.join().unwrap();
    }

    #[test]
    fn batch_resolve_continues_after_a_failed_item() {
        let (client, handle) = server(vec![
            bug_at(41, "active", "me"),
            json!({"status":"success"}),
            resolved_bug(41, "me"),
            bug_at(42, "active", "me"),
            json!({"status":"fail","message":"无权限"}),
        ]);
        let mut connection = Connection::open_in_memory().unwrap();
        crate::database::initialize(&connection).unwrap();
        let tx = connection.transaction().unwrap();
        let items = vec![
            normalize("bug", &bug_at(41, "active", "me"), &client.url).unwrap(),
            normalize("bug", &bug_at(42, "active", "me"), &client.url).unwrap(),
        ];
        upsert_items(&tx, &items, &mut (0, 0, 0), &mut Vec::new()).unwrap();
        tx.commit().unwrap();
        let ids: Vec<String> = connection
            .prepare("SELECT id FROM todo_task ORDER BY external_id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        let result = batch_resolve_with_client(
            &mut connection,
            &client,
            ZentaoBatchResolveInput {
                task_ids: ids.clone(),
                resolution: "fixed".to_owned(),
                resolved_build: "trunk".to_owned(),
                action_date: "2026-10-23 12:00:00".to_owned(),
                comment: "批量验证".to_owned(),
            },
        )
        .unwrap();
        assert_eq!(result.succeeded, 1);
        assert_eq!(result.failed, 1);
        assert!(result.items[0].ok);
        assert!(result.items[1].error.contains("无权限"));
        let resolved = store::read_task(&connection, &ids[0]).unwrap();
        assert_eq!(resolved.status, "done");
        assert_eq!(resolved.external_status.as_deref(), Some("resolved"));
        let requests = handle.join().unwrap();
        assert_eq!(requests.len(), 5);
        assert_eq!(requests[1].0, "POST /api.php/v1/bugs/41/resolve HTTP/1.1");
        assert_eq!(requests[4].0, "POST /api.php/v1/bugs/42/resolve HTTP/1.1");
    }

    #[test]
    fn batch_resolve_rejects_invalid_selection_before_touching_the_network() {
        let mut connection = Connection::open_in_memory().unwrap();
        crate::database::initialize(&connection).unwrap();
        let input = |task_ids: Vec<String>, resolution: &str, action_date: &str| ZentaoBatchResolveInput {
            task_ids,
            resolution: resolution.to_owned(),
            resolved_build: "trunk".to_owned(),
            action_date: action_date.to_owned(),
            comment: String::new(),
        };
        assert!(batch_resolve(&mut connection, input(Vec::new(), "fixed", "2026-10-23 12:00:00")).is_err());
        assert!(batch_resolve(&mut connection, input(vec!["local".to_owned()], "bogus", "2026-10-23 12:00:00")).is_err());
        assert!(batch_resolve(&mut connection, input(vec!["local".to_owned()], "fixed", "not-a-date")).is_err());
    }
}
