use crate::database;
use crate::productivity::types::{
    CalendarEvent, CalendarEventDraft, CommitInput, CommitResult, DecompositionInput,
    DecompositionResult, DocumentContent, MoveEventInput, TodoDraft, TodoFilter, TodoTask,
};
use crate::productivity::{decomposition, document, store, zentao};
use tauri::{AppHandle, Emitter};

#[tauri::command]
pub fn calendar_delete_event(app: AppHandle, id: String) -> Result<(), String> {
    let connection = database::open(&app)?;
    store::delete_event(&connection, &id)
}

#[tauri::command]
pub fn calendar_list_range(
    app: AppHandle,
    start: String,
    end: String,
) -> Result<Vec<CalendarEvent>, String> {
    let connection = database::open(&app)?;
    store::list_events(&connection, &start, &end)
}

#[tauri::command]
pub fn calendar_save_event(
    app: AppHandle,
    draft: CalendarEventDraft,
) -> Result<CalendarEvent, String> {
    let mut connection = database::open(&app)?;
    store::save_event(&mut connection, draft)
}

#[tauri::command]
pub fn calendar_move_event(
    app: AppHandle,
    input: MoveEventInput,
) -> Result<CalendarEvent, String> {
    let mut connection = database::open(&app)?;
    store::move_event(&mut connection, input)
}

#[tauri::command]
pub fn todo_list(app: AppHandle, filter: TodoFilter) -> Result<Vec<TodoTask>, String> {
    let connection = database::open(&app)?;
    store::list_tasks(&connection, filter)
}

#[tauri::command]
pub fn todo_get(app: AppHandle, id: String) -> Result<TodoTask, String> {
    let connection = database::open(&app)?;
    store::read_task(&connection, &id)
}

#[tauri::command]
pub fn todo_save(app: AppHandle, draft: TodoDraft) -> Result<TodoTask, String> {
    let mut connection = database::open(&app)?;
    store::save_task(&mut connection, draft)
}

#[tauri::command]
pub fn todo_set_status(app: AppHandle, id: String, status: String) -> Result<TodoTask, String> {
    let mut connection = database::open(&app)?;
    store::set_task_status(&mut connection, &id, &status)
}

#[tauri::command]
pub fn todo_delete(app: AppHandle, id: String) -> Result<(), String> {
    let connection = database::open(&app)?;
    store::delete_task(&connection, &id)
}

#[tauri::command]
pub async fn document_read(path: String) -> Result<DocumentContent, String> {
    // 文件读取与 PDF/DOCX 解析可能较慢，放到阻塞线程池，避免占用 IPC 主线程。
    tauri::async_runtime::spawn_blocking(move || {
        let (name, content, char_count) = document::read_document(&path)?;
        Ok(DocumentContent {
            name,
            content,
            char_count,
        })
    })
    .await
    .map_err(|error| format!("读取文档失败：{error}"))?
}

#[tauri::command]
pub async fn document_decomposition_generate(
    app: AppHandle,
    input: DecompositionInput,
) -> Result<DecompositionResult, String> {
    // AI 调用是网络阻塞操作（最长 120 秒），必须离开主线程，否则界面会无响应。
    // 同时把调用阶段通过事件流回前端，让用户看到过程（不含正文、密钥或模型）。
    let started = std::time::Instant::now();
    let progress_app = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut tick = move |stage: &str, detail: String| {
            let _ = progress_app.emit(
                "decomposition-progress",
                serde_json::json!({
                    "stage": stage,
                    "detail": detail,
                    "elapsedMs": started.elapsed().as_millis() as u64,
                }),
            );
        };
        decomposition::generate(input, &mut tick)
    })
    .await
    .map_err(|error| format!("拆解任务执行失败：{error}"))?;
    if let Err(error) = &result {
        let _ = app.emit(
            "decomposition-progress",
            serde_json::json!({
                "stage": "error",
                "detail": error,
                "elapsedMs": started.elapsed().as_millis() as u64,
            }),
        );
    }
    result
}

#[tauri::command]
pub fn document_decomposition_commit(
    app: AppHandle,
    input: CommitInput,
) -> Result<CommitResult, String> {
    let mut connection = database::open(&app)?;
    store::commit_decomposition(&mut connection, input)
}

#[tauri::command]
pub fn zentao_status() -> zentao::ZentaoStatus {
    zentao::status()
}

#[tauri::command]
pub async fn zentao_login(url: String, account: String, password: String) -> Result<zentao::ZentaoStatus, String> {
    // 登录是网络阻塞操作，离开主线程；密码只在本次请求中使用，不落日志。
    tauri::async_runtime::spawn_blocking(move || zentao::login(&url, &account, &password))
        .await
        .map_err(|error| format!("登录任务执行失败：{error}"))?
}

#[tauri::command]
pub async fn zentao_logout() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(zentao::logout)
        .await
        .map_err(|error| format!("退出任务执行失败：{error}"))?
}

#[tauri::command]
pub async fn zentao_sync(app: AppHandle) -> Result<zentao::ZentaoSyncResult, String> {
    // 同步要遍历禅道产品/执行并发多个请求，整体放阻塞线程池执行。
    tauri::async_runtime::spawn_blocking(move || {
        let mut connection = database::open(&app)?;
        zentao::sync(&mut connection)
    })
    .await
    .map_err(|error| format!("禅道同步执行失败：{error}"))?
}

#[tauri::command]
pub async fn zentao_task_detail(
    external_type: String,
    external_id: i64,
) -> Result<zentao::ZentaoDetail, String> {
    tauri::async_runtime::spawn_blocking(move || zentao::detail(&external_type, external_id))
        .await
        .map_err(|error| format!("读取详情执行失败：{error}"))?
}

#[tauri::command]
pub async fn zentao_download_file(
    file_id: i64,
    extension: String,
    title: String,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || zentao::download_file(file_id, &extension, &title))
        .await
        .map_err(|error| format!("附件下载执行失败：{error}"))?
}

#[tauri::command]
pub async fn zentao_read_file(
    file_id: i64,
    extension: String,
) -> Result<zentao::ZentaoFileContent, String> {
    tauri::async_runtime::spawn_blocking(move || zentao::read_file(file_id, &extension))
        .await
        .map_err(|error| format!("附件读取执行失败：{error}"))?
}

#[tauri::command]
pub async fn zentao_read_archive(file_id: i64) -> Result<Vec<zentao::ZentaoArchiveEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || zentao::read_archive(file_id))
        .await
        .map_err(|error| format!("压缩包读取执行失败：{error}"))?
}

#[tauri::command]
pub async fn zentao_read_archive_entry(
    file_id: i64,
    index: usize,
) -> Result<zentao::ZentaoFileContent, String> {
    tauri::async_runtime::spawn_blocking(move || zentao::read_archive_entry(file_id, index))
        .await
        .map_err(|error| format!("压缩包条目读取执行失败：{error}"))?
}

#[tauri::command]
pub async fn zentao_action_context(app: AppHandle, task_id: String, operation: String) -> Result<zentao::ZentaoActionContext, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let connection = database::open(&app)?;
        zentao::action_context(&connection, &task_id, &operation)
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn zentao_action(app: AppHandle, input: zentao::ZentaoActionInput) -> Result<zentao::ZentaoActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut connection = database::open(&app)?;
        zentao::action(&mut connection, input)
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn zentao_comment(app: AppHandle, input: zentao::ZentaoCommentInput) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let connection = database::open(&app)?;
        zentao::comment(&connection, input)
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn zentao_batch_resolve(
    app: AppHandle,
    input: zentao::ZentaoBatchResolveInput,
) -> Result<zentao::ZentaoBatchResolveResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut connection = database::open(&app)?;
        zentao::batch_resolve(&mut connection, input)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn zentao_summary_generate(input: super::zentao_summary::SummaryInput) -> Result<super::zentao_summary::Summary, String> {
    tauri::async_runtime::spawn_blocking(move || super::zentao_summary::generate(input))
        .await.map_err(|error| error.to_string())?
}
