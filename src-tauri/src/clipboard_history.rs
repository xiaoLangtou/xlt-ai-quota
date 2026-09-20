use crate::database;
use auto_launch::AutoLaunchBuilder;
use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::{Duration, SecondsFormat, Utc};
use clipboard_rs::{
    common::RustImage, Clipboard, ClipboardContext, ClipboardHandler, ClipboardWatcher,
    ClipboardWatcherContext,
};
use enigo::{Direction, Enigo, Key, Keyboard, Settings as EnigoSettings};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
    sync::{Mutex, OnceLock},
    thread,
    time::Duration as StdDuration,
};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
use uuid::Uuid;

const MAX_TEXT_BYTES: usize = 200 * 1024;
const MAX_FILE_TEXT_PREVIEW_BYTES: u64 = 2 * 1024 * 1024;
const MAX_FILE_BINARY_PREVIEW_BYTES: u64 = 20 * 1024 * 1024;
static INTERNAL_WRITE_HASH: OnceLock<Mutex<Option<String>>> = OnceLock::new();
static PASTE_SEQUENCE: OnceLock<Mutex<Option<PasteSequence>>> = OnceLock::new();

struct PasteSequence {
    ids: Vec<String>,
    next_index: usize,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardItem {
    pub id: String,
    pub kind: String,
    pub content: String,
    pub hash: String,
    pub source_app: Option<String>,
    pub pinned: bool,
    pub copy_count: i64,
    pub size_bytes: i64,
    pub image_width: Option<i64>,
    pub image_height: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardQuery {
    pub query: Option<String>,
    pub kind: Option<String>,
    pub pinned_only: Option<bool>,
    pub limit: Option<i64>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardSettings {
    pub enabled: bool,
    pub launch_at_login: bool,
    pub max_items: i64,
    pub ttl_days: i64,
    pub shortcut: String,
    pub excluded_apps: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardStatus {
    pub settings: ClipboardSettings,
    pub total: i64,
    pub pinned: i64,
    pub text_count: i64,
    pub image_count: i64,
    pub file_count: i64,
    pub accessibility_granted: bool,
    pub accessibility_target: String,
    pub storage_path: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardSequenceStatus {
    pub active: bool,
    pub total: usize,
    pub next_index: usize,
    pub remaining: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardSequenceStep {
    pub item: ClipboardItem,
    pub status: ClipboardSequenceStatus,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardFilePreview {
    pub path: String,
    pub name: String,
    pub extension: Option<String>,
    pub kind: String,
    pub size_bytes: u64,
    pub content: Option<String>,
    pub data_url: Option<String>,
    pub message: Option<String>,
}

struct CapturedData {
    kind: &'static str,
    content: String,
    hash: String,
    size_bytes: i64,
    image_width: Option<i64>,
    image_height: Option<i64>,
    image_path: Option<PathBuf>,
}

struct WatchHandler {
    app: AppHandle,
    clipboard: ClipboardContext,
}

impl ClipboardHandler for WatchHandler {
    fn on_clipboard_change(&mut self) {
        if let Err(error) = capture_current(&self.app, &self.clipboard) {
            eprintln!("剪贴板采集失败: {error}");
        }
    }
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn hash_bytes(kind: &str, bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(kind.as_bytes());
    digest.update([0]);
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

fn row_to_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<ClipboardItem> {
    Ok(ClipboardItem {
        id: row.get(0)?,
        kind: row.get(1)?,
        content: row.get(2)?,
        hash: row.get(3)?,
        source_app: row.get(4)?,
        pinned: row.get::<_, i64>(5)? != 0,
        copy_count: row.get(6)?,
        size_bytes: row.get(7)?,
        image_width: row.get(8)?,
        image_height: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

fn read_item(connection: &Connection, id: &str) -> Result<ClipboardItem, String> {
    connection
        .query_row(
            "SELECT id, kind, content, hash, source_app, pinned, copy_count, size_bytes,
                    image_width, image_height, created_at, updated_at
             FROM clipboard_items WHERE id = ?1",
            params![id],
            row_to_item,
        )
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => "剪贴板记录不存在或已删除".to_owned(),
            _ => format!("无法读取剪贴板记录: {error}"),
        })
}

fn config(connection: &Connection, key: &str) -> Result<String, String> {
    connection
        .query_row(
            "SELECT value FROM clipboard_config WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .map_err(|error| format!("无法读取剪贴板设置: {error}"))
}

fn read_settings(connection: &Connection) -> Result<ClipboardSettings, String> {
    let excluded = config(connection, "excluded_apps")?;
    Ok(ClipboardSettings {
        enabled: config(connection, "enabled")? == "true",
        launch_at_login: config(connection, "launch_at_login")? == "true",
        max_items: config(connection, "max_items")?
            .parse()
            .map_err(|error| format!("剪贴板容量设置无效: {error}"))?,
        ttl_days: config(connection, "ttl_days")?
            .parse()
            .map_err(|error| format!("剪贴板保留时长设置无效: {error}"))?,
        shortcut: config(connection, "shortcut")?,
        excluded_apps: serde_json::from_str(&excluded)
            .map_err(|error| format!("无法解析应用排除名单: {error}"))?,
    })
}

fn write_config(connection: &Connection, key: &str, value: &str) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO clipboard_config (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )
        .map(|_| ())
        .map_err(|error| format!("无法保存剪贴板设置: {error}"))
}

fn image_directory(app: &AppHandle) -> Result<PathBuf, String> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("无法定位本机数据目录: {error}"))?
        .join("clipboard-images");
    fs::create_dir_all(&directory).map_err(|error| format!("无法创建图片目录: {error}"))?;
    Ok(directory)
}

fn capture_data(
    app: &AppHandle,
    clipboard: &ClipboardContext,
) -> Result<Option<CapturedData>, String> {
    if let Ok(files) = clipboard.get_files() {
        if !files.is_empty() {
            let content = serde_json::to_string(&files)
                .map_err(|error| format!("无法序列化文件列表: {error}"))?;
            let size_bytes = files
                .iter()
                .filter_map(|path| fs::metadata(path).ok())
                .map(|metadata| metadata.len() as i64)
                .sum();
            return Ok(Some(CapturedData {
                kind: "files",
                hash: hash_bytes("files", content.as_bytes()),
                content,
                size_bytes,
                image_width: None,
                image_height: None,
                image_path: None,
            }));
        }
    }

    if let Ok(image) = clipboard.get_image() {
        let (width, height) = image.get_size();
        if width > 0 && height > 0 {
            let path = image_directory(app)?.join(format!("{}.png", Uuid::new_v4()));
            image
                .save_to_path(path.to_string_lossy().as_ref())
                .map_err(|error| format!("无法保存剪贴板图片: {error}"))?;
            let bytes = fs::read(&path).map_err(|error| format!("无法读取剪贴板图片: {error}"))?;
            return Ok(Some(CapturedData {
                kind: "image",
                content: path.to_string_lossy().into_owned(),
                hash: hash_bytes("image", &bytes),
                size_bytes: bytes.len() as i64,
                image_width: Some(width as i64),
                image_height: Some(height as i64),
                image_path: Some(path),
            }));
        }
    }

    if let Ok(text) = clipboard.get_text() {
        if !text.trim().is_empty() && text.len() <= MAX_TEXT_BYTES {
            return Ok(Some(CapturedData {
                kind: "text",
                hash: hash_bytes("text", text.as_bytes()),
                size_bytes: text.len() as i64,
                content: text,
                image_width: None,
                image_height: None,
                image_path: None,
            }));
        }
    }

    Ok(None)
}

fn consume_internal_write(hash: &str) -> bool {
    let mut state = INTERNAL_WRITE_HASH
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("内部剪贴板写入状态锁已损坏");
    if state.as_deref() == Some(hash) {
        *state = None;
        return true;
    }
    false
}

fn mark_internal_write(hash: String) {
    let mut state = INTERNAL_WRITE_HASH
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("内部剪贴板写入状态锁已损坏");
    *state = Some(hash);
}

pub(crate) fn write_plain_text(content: &str) -> Result<(), String> {
    if content.trim().is_empty() {
        return Err("不能复制空文本".to_owned());
    }
    if content.len() > MAX_TEXT_BYTES {
        return Err("文本超过 200 KB，无法写入剪贴板".to_owned());
    }
    let clipboard =
        ClipboardContext::new().map_err(|error| format!("无法打开系统剪贴板: {error}"))?;
    mark_internal_write(hash_bytes("text", content.as_bytes()));
    clipboard
        .set_text(content.to_owned())
        .map_err(|error| format!("无法复制文本: {error}"))
}

fn write_item(item: &ClipboardItem) -> Result<(), String> {
    if item.kind == "text" {
        return write_plain_text(&item.content);
    }
    let clipboard =
        ClipboardContext::new().map_err(|error| format!("无法打开系统剪贴板: {error}"))?;
    mark_internal_write(item.hash.clone());
    match item.kind.as_str() {
        "files" => {
            let files = serde_json::from_str::<Vec<String>>(&item.content)
                .map_err(|error| format!("无法读取文件列表: {error}"))?;
            clipboard
                .set_files(files)
                .map_err(|error| format!("无法复制文件: {error}"))
        }
        "image" => {
            let image = clipboard_rs::RustImageData::from_path(&item.content)
                .map_err(|error| format!("无法读取图片: {error}"))?;
            clipboard
                .set_image(image)
                .map_err(|error| format!("无法复制图片: {error}"))
        }
        _ => Err("不支持的剪贴板类型".to_owned()),
    }
}

fn sequence_status(sequence: Option<&PasteSequence>) -> ClipboardSequenceStatus {
    match sequence {
        Some(value) => ClipboardSequenceStatus {
            active: value.next_index < value.ids.len(),
            total: value.ids.len(),
            next_index: value.next_index,
            remaining: value.ids.len().saturating_sub(value.next_index),
        },
        None => ClipboardSequenceStatus {
            active: false,
            total: 0,
            next_index: 0,
            remaining: 0,
        },
    }
}

fn current_source_app() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("osascript")
            .args([
                "-e",
                "tell application \"System Events\" to get name of first application process whose frontmost is true",
            ])
            .output()
            .ok()?;
        let name = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        return (!name.is_empty()).then_some(name);
    }

    #[cfg(target_os = "windows")]
    {
        let script = r#"Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class ForegroundProcess {
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);
}
'@; $id = 0; [ForegroundProcess]::GetWindowThreadProcessId([ForegroundProcess]::GetForegroundWindow(), [ref]$id) | Out-Null; (Get-Process -Id $id).ProcessName"#;
        let output = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .output()
            .ok()?;
        let name = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        return (!name.is_empty()).then_some(name);
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    None
}

fn is_excluded(source: Option<&str>, excluded_apps: &[String]) -> bool {
    let Some(source) = source else { return false };
    let source = source.to_lowercase();
    excluded_apps
        .iter()
        .any(|app| source.contains(&app.trim().to_lowercase()))
}

fn has_sensitive_clipboard_semantics(clipboard: &ClipboardContext) -> bool {
    clipboard
        .available_formats()
        .unwrap_or_default()
        .iter()
        .any(|format| {
            let normalized = format.to_lowercase();
            normalized.contains("concealed") || normalized.contains("transient")
        })
}

fn remove_paths(paths: &[String]) {
    for path in paths {
        let _ = fs::remove_file(path);
    }
}

fn paths_for_where(
    connection: &Connection,
    clause: &str,
    value: &str,
) -> Result<Vec<String>, String> {
    let sql = format!("SELECT content FROM clipboard_items WHERE kind = 'image' AND {clause}");
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("无法读取待清理图片: {error}"))?;
    let rows = statement
        .query_map(params![value], |row| row.get::<_, String>(0))
        .map_err(|error| format!("无法读取待清理图片: {error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("无法读取待清理图片: {error}"))
}

fn prune(connection: &Connection, settings: &ClipboardSettings) -> Result<(), String> {
    let cutoff = (Utc::now() - Duration::days(settings.ttl_days))
        .to_rfc3339_opts(SecondsFormat::Millis, true);
    let expired_paths = paths_for_where(connection, "pinned = 0 AND updated_at < ?1", &cutoff)?;
    connection
        .execute(
            "DELETE FROM clipboard_items WHERE pinned = 0 AND updated_at < ?1",
            params![cutoff],
        )
        .map_err(|error| format!("无法清理过期剪贴板记录: {error}"))?;
    remove_paths(&expired_paths);

    let mut statement = connection
        .prepare(
            "SELECT id, CASE WHEN kind = 'image' THEN content ELSE NULL END
             FROM clipboard_items WHERE pinned = 0
             ORDER BY updated_at DESC LIMIT -1 OFFSET ?1",
        )
        .map_err(|error| format!("无法检查剪贴板容量: {error}"))?;
    let overflow = statement
        .query_map(params![settings.max_items], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })
        .map_err(|error| format!("无法检查剪贴板容量: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("无法检查剪贴板容量: {error}"))?;
    for (id, path) in overflow {
        connection
            .execute("DELETE FROM clipboard_items WHERE id = ?1", params![id])
            .map_err(|error| format!("无法清理超量剪贴板记录: {error}"))?;
        if let Some(path) = path {
            let _ = fs::remove_file(path);
        }
    }
    Ok(())
}

fn capture_current(app: &AppHandle, clipboard: &ClipboardContext) -> Result<(), String> {
    let connection = database::open(app)?;
    let settings = read_settings(&connection)?;
    if !settings.enabled {
        return Ok(());
    }
    if has_sensitive_clipboard_semantics(clipboard) {
        return Ok(());
    }
    let source_app = current_source_app();
    if is_excluded(source_app.as_deref(), &settings.excluded_apps) {
        return Ok(());
    }
    let Some(data) = capture_data(app, clipboard)? else {
        return Ok(());
    };
    if consume_internal_write(&data.hash) {
        if let Some(path) = data.image_path {
            let _ = fs::remove_file(path);
        }
        return Ok(());
    }

    let existing = connection
        .query_row(
            "SELECT id FROM clipboard_items WHERE hash = ?1",
            params![&data.hash],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("无法执行剪贴板去重: {error}"))?;
    let timestamp = now();
    if let Some(id) = existing {
        connection
            .execute(
                "UPDATE clipboard_items
                 SET source_app = ?2, copy_count = copy_count + 1, updated_at = ?3
                 WHERE id = ?1",
                params![id, source_app, timestamp],
            )
            .map_err(|error| format!("无法更新剪贴板记录: {error}"))?;
        if let Some(path) = data.image_path {
            let _ = fs::remove_file(path);
        }
    } else {
        connection
            .execute(
                "INSERT INTO clipboard_items
                 (id, kind, content, hash, source_app, pinned, copy_count, size_bytes,
                  image_width, image_height, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 0, 1, ?6, ?7, ?8, ?9, ?9)",
                params![
                    Uuid::new_v4().to_string(),
                    data.kind,
                    data.content,
                    data.hash,
                    source_app,
                    data.size_bytes,
                    data.image_width,
                    data.image_height,
                    timestamp
                ],
            )
            .map_err(|error| format!("无法保存剪贴板记录: {error}"))?;
    }
    prune(&connection, &settings)?;
    let _ = app.emit("clipboard-history-updated", ());
    Ok(())
}

pub fn start_listener(app: AppHandle) {
    thread::spawn(move || loop {
        let clipboard = match ClipboardContext::new() {
            Ok(value) => value,
            Err(error) => {
                eprintln!("无法初始化剪贴板读取器: {error}");
                thread::sleep(StdDuration::from_secs(2));
                continue;
            }
        };
        let mut watcher = match ClipboardWatcherContext::new() {
            Ok(value) => value,
            Err(error) => {
                eprintln!("无法初始化剪贴板监听器: {error}");
                thread::sleep(StdDuration::from_secs(2));
                continue;
            }
        };
        watcher.add_handler(WatchHandler {
            app: app.clone(),
            clipboard,
        });
        watcher.start_watch();
        thread::sleep(StdDuration::from_secs(1));
    });
}

pub fn register_shortcut(app: &AppHandle, shortcut: &str) -> Result<(), String> {
    validate_shortcut(shortcut)?;
    app.global_shortcut()
        .unregister_all()
        .map_err(|error| format!("无法取消原快捷键: {error}"))?;
    app.global_shortcut()
        .register(shortcut)
        .map_err(|error| format!("快捷键已被占用或无法注册: {error}"))
}

fn replace_shortcut(app: &AppHandle, current: &str, next: &str) -> Result<(), String> {
    validate_shortcut(next)?;
    if current == next {
        return Ok(());
    }
    app.global_shortcut()
        .register(next)
        .map_err(|error| format!("快捷键已被占用或无法注册: {error}"))?;
    if let Err(error) = app.global_shortcut().unregister(current) {
        let _ = app.global_shortcut().unregister(next);
        return Err(format!("无法取消原快捷键: {error}"));
    }
    Ok(())
}

fn validate_shortcut(shortcut: &str) -> Result<(), String> {
    Shortcut::from_str(shortcut).map_err(|_| "快捷键格式无效".to_owned())?;
    let parts = shortcut.split('+').collect::<Vec<_>>();
    if parts.len() == 1 {
        return matches!(
            parts[0],
            "F1" | "F2" | "F3" | "F4" | "F5" | "F6" | "F7" | "F8" | "F9" | "F10" | "F11" | "F12"
        )
        .then_some(())
        .ok_or_else(|| "F1–F12 可单独使用，其他按键需要组合键".to_owned());
    }
    let has_modifier = parts[..parts.len() - 1].iter().any(|part| {
        matches!(
            *part,
            "CommandOrControl" | "Command" | "Control" | "Alt" | "Shift" | "Meta" | "Super"
        )
    });
    has_modifier
        .then_some(())
        .ok_or_else(|| "快捷键必须包含修饰键".to_owned())
}

pub fn configured_shortcut(app: &AppHandle) -> Result<String, String> {
    database::open(app)
        .and_then(|connection| read_settings(&connection))
        .map(|settings| settings.shortcut)
}

pub fn toggle_quick_panel(app: &AppHandle) -> Result<(), String> {
    let panel = app
        .get_webview_window("clipboard-panel")
        .ok_or_else(|| "未找到剪贴板快捷面板".to_owned())?;
    if panel.is_visible().map_err(|error| error.to_string())? {
        panel.hide().map_err(|error| error.to_string())?;
    } else {
        panel.show().map_err(|error| error.to_string())?;
        panel.set_focus().map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn clipboard_show_panel(app: AppHandle) -> Result<(), String> {
    let panel = app
        .get_webview_window("clipboard-panel")
        .ok_or_else(|| "未找到剪贴板快捷面板".to_owned())?;
    panel.show().map_err(|error| error.to_string())?;
    panel.set_focus().map_err(|error| error.to_string())
}

fn auto_launcher() -> Result<auto_launch::AutoLaunch, String> {
    let executable =
        std::env::current_exe().map_err(|error| format!("无法定位当前应用: {error}"))?;
    let executable = executable
        .to_str()
        .ok_or_else(|| "应用路径包含无法识别的字符".to_owned())?;
    let mut builder = AutoLaunchBuilder::new();
    builder
        .set_app_name("XLT Workbench")
        .set_app_path(executable)
        .set_args(&["--hidden"]);
    builder
        .build()
        .map_err(|error| format!("无法创建登录启动配置: {error}"))
}

fn set_launch_at_login(enabled: bool) -> Result<(), String> {
    let launcher = auto_launcher()?;
    if enabled {
        launcher
            .enable()
            .map_err(|error| format!("无法开启登录时启动: {error}"))
    } else {
        launcher
            .disable()
            .map_err(|error| format!("无法关闭登录时启动: {error}"))
    }
}

pub fn initialize_launch_at_login(app: &AppHandle) -> Result<(), String> {
    let connection = database::open(app)?;
    if read_settings(&connection)?.launch_at_login {
        set_launch_at_login(true)?;
    }
    Ok(())
}

fn simulate_paste() -> Result<(), String> {
    let mut enigo = Enigo::new(&EnigoSettings::default())
        .map_err(|error| format!("无法初始化系统输入控制: {error}"))?;
    #[cfg(target_os = "macos")]
    let modifier = Key::Meta;
    #[cfg(target_os = "windows")]
    let modifier = Key::Control;
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    return Err("当前系统不支持自动粘贴".to_owned());

    enigo
        .key(modifier, Direction::Press)
        .map_err(|error| format!("无法按下系统粘贴快捷键: {error}"))?;
    let paste_result = enigo.key(Key::Unicode('v'), Direction::Click);
    let release_result = enigo.key(modifier, Direction::Release);
    paste_result.map_err(|error| format!("无法执行系统粘贴: {error}"))?;
    release_result.map_err(|error| format!("无法释放系统粘贴快捷键: {error}"))
}

fn hide_quick_panel(app: &AppHandle) -> Result<(), String> {
    app.get_webview_window("clipboard-panel")
        .ok_or_else(|| "未找到剪贴板快捷面板".to_owned())?
        .hide()
        .map_err(|error| error.to_string())
}

fn bump_copy_count(connection: &Connection, id: &str) -> Result<(), String> {
    connection
        .execute(
            "UPDATE clipboard_items SET copy_count = copy_count + 1, updated_at = ?2 WHERE id = ?1",
            params![id, now()],
        )
        .map(|_| ())
        .map_err(|error| format!("无法更新使用次数: {error}"))
}

fn merge_text_items(
    connection: &Connection,
    ids: &[String],
    separator: &str,
) -> Result<String, String> {
    if ids.len() < 2 {
        return Err("合并操作至少需要 2 条文本记录".to_owned());
    }
    let mut contents = Vec::with_capacity(ids.len());
    for id in ids {
        let item = read_item(connection, id)?;
        if item.kind != "text" {
            return Err("合并操作仅支持文本记录".to_owned());
        }
        contents.push(item.content);
    }
    let merged = contents.join(separator);
    if merged.len() > MAX_TEXT_BYTES {
        return Err("合并后的文本超过 200 KB".to_owned());
    }
    Ok(merged)
}

#[cfg(target_os = "macos")]
fn accessibility_granted() -> bool {
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrustedWithOptions(options: *const std::ffi::c_void) -> u8;
    }
    unsafe { AXIsProcessTrustedWithOptions(std::ptr::null()) != 0 }
}

#[cfg(not(target_os = "macos"))]
fn accessibility_granted() -> bool {
    true
}

#[tauri::command]
pub fn clipboard_list(
    app: AppHandle,
    filter: ClipboardQuery,
) -> Result<Vec<ClipboardItem>, String> {
    let connection = database::open(&app)?;
    let query = filter
        .query
        .map(|value| format!("%{}%", value.trim().replace('%', "\\%").replace('_', "\\_")))
        .filter(|value| value != "%%");
    let kind = filter
        .kind
        .filter(|value| matches!(value.as_str(), "text" | "image" | "files"));
    let pinned_only = filter.pinned_only.unwrap_or(false);
    let limit = filter.limit.unwrap_or(500).clamp(1, 5000);
    let mut statement = connection
        .prepare(
            "SELECT id, kind, content, hash, source_app, pinned, copy_count, size_bytes,
                    image_width, image_height, created_at, updated_at
             FROM clipboard_items
             WHERE (?1 IS NULL OR content LIKE ?1 ESCAPE '\\' COLLATE NOCASE
                                OR source_app LIKE ?1 ESCAPE '\\' COLLATE NOCASE)
               AND (?2 IS NULL OR kind = ?2)
               AND (?3 = 0 OR pinned = 1)
             ORDER BY pinned DESC, updated_at DESC
             LIMIT ?4",
        )
        .map_err(|error| format!("无法搜索剪贴板历史: {error}"))?;
    let rows = statement
        .query_map(params![query, kind, pinned_only, limit], row_to_item)
        .map_err(|error| format!("无法搜索剪贴板历史: {error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("无法搜索剪贴板历史: {error}"))
}

#[tauri::command]
pub fn clipboard_status(app: AppHandle) -> Result<ClipboardStatus, String> {
    let connection = database::open(&app)?;
    let settings = read_settings(&connection)?;
    let (total, pinned, text_count, image_count, file_count) = connection
        .query_row(
            "SELECT COUNT(*),
                    COALESCE(SUM(pinned), 0),
                    COALESCE(SUM(CASE WHEN kind = 'text' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN kind = 'image' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN kind = 'files' THEN 1 ELSE 0 END), 0)
             FROM clipboard_items",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .map_err(|error| format!("无法读取剪贴板统计: {error}"))?;
    Ok(ClipboardStatus {
        settings,
        total,
        pinned,
        text_count,
        image_count,
        file_count,
        accessibility_granted: accessibility_granted(),
        accessibility_target: std::env::current_exe()
            .map_err(|error| format!("无法定位当前应用: {error}"))?
            .to_string_lossy()
            .into_owned(),
        storage_path: database::database_path(&app)?
            .to_string_lossy()
            .into_owned(),
    })
}

#[tauri::command]
pub fn clipboard_set_pinned(
    app: AppHandle,
    id: String,
    pinned: bool,
) -> Result<ClipboardItem, String> {
    let connection = database::open(&app)?;
    let affected = connection
        .execute(
            "UPDATE clipboard_items SET pinned = ?2 WHERE id = ?1",
            params![&id, pinned],
        )
        .map_err(|error| format!("无法更新收藏状态: {error}"))?;
    if affected == 0 {
        return Err("剪贴板记录不存在或已删除".to_owned());
    }
    let item = read_item(&connection, &id)?;
    let _ = app.emit("clipboard-history-updated", ());
    Ok(item)
}

#[tauri::command]
pub fn clipboard_copy(app: AppHandle, id: String) -> Result<ClipboardItem, String> {
    let connection = database::open(&app)?;
    let item = read_item(&connection, &id)?;
    write_item(&item)?;
    bump_copy_count(&connection, &id)?;
    let updated = read_item(&connection, &id)?;
    let _ = app.emit("clipboard-history-updated", ());
    Ok(updated)
}

#[tauri::command]
pub fn clipboard_copy_plain(app: AppHandle, id: String) -> Result<ClipboardItem, String> {
    let connection = database::open(&app)?;
    let item = read_item(&connection, &id)?;
    if item.kind != "text" {
        return Err("纯文本模式仅支持文本记录".to_owned());
    }
    write_plain_text(&item.content)?;
    bump_copy_count(&connection, &id)?;
    let updated = read_item(&connection, &id)?;
    let _ = app.emit("clipboard-history-updated", ());
    Ok(updated)
}

#[tauri::command]
pub fn clipboard_write_text(content: String) -> Result<(), String> {
    write_plain_text(&content)
}

#[tauri::command]
pub fn clipboard_paste(app: AppHandle, id: String, plain: bool) -> Result<ClipboardItem, String> {
    let connection = database::open(&app)?;
    let item = read_item(&connection, &id)?;
    if plain {
        if item.kind != "text" {
            return Err("纯文本粘贴仅支持文本记录".to_owned());
        }
        write_plain_text(&item.content)?;
    } else {
        write_item(&item)?;
    }
    bump_copy_count(&connection, &id)?;
    hide_quick_panel(&app)?;
    thread::sleep(StdDuration::from_millis(90));
    simulate_paste()?;
    let updated = read_item(&connection, &id)?;
    let _ = app.emit("clipboard-history-updated", ());
    Ok(updated)
}

#[tauri::command]
pub fn clipboard_copy_merged(
    app: AppHandle,
    ids: Vec<String>,
    separator: String,
) -> Result<usize, String> {
    let connection = database::open(&app)?;
    let merged = merge_text_items(&connection, &ids, &separator)?;
    write_plain_text(&merged)?;
    for id in &ids {
        bump_copy_count(&connection, id)?;
    }
    let _ = app.emit("clipboard-history-updated", ());
    Ok(ids.len())
}

#[tauri::command]
pub fn clipboard_paste_merged(
    app: AppHandle,
    ids: Vec<String>,
    separator: String,
) -> Result<usize, String> {
    let count = clipboard_copy_merged(app.clone(), ids, separator)?;
    hide_quick_panel(&app)?;
    thread::sleep(StdDuration::from_millis(90));
    simulate_paste()?;
    Ok(count)
}

#[tauri::command]
pub fn clipboard_sequence_start(
    app: AppHandle,
    ids: Vec<String>,
) -> Result<ClipboardSequenceStatus, String> {
    let mut unique = Vec::new();
    for id in ids {
        let id = id.trim().to_owned();
        if !id.is_empty() && !unique.contains(&id) {
            unique.push(id);
        }
    }
    if unique.len() < 2 {
        return Err("粘贴序列至少需要 2 条记录".to_owned());
    }
    let connection = database::open(&app)?;
    for id in &unique {
        read_item(&connection, id)?;
    }
    let sequence = PasteSequence {
        ids: unique,
        next_index: 0,
    };
    let status = sequence_status(Some(&sequence));
    *PASTE_SEQUENCE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("粘贴序列状态锁已损坏") = Some(sequence);
    Ok(status)
}

#[tauri::command]
pub fn clipboard_sequence_status() -> ClipboardSequenceStatus {
    let state = PASTE_SEQUENCE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("粘贴序列状态锁已损坏");
    sequence_status(state.as_ref())
}

#[tauri::command]
pub fn clipboard_sequence_next(app: AppHandle) -> Result<ClipboardSequenceStep, String> {
    let id = {
        let state = PASTE_SEQUENCE
            .get_or_init(|| Mutex::new(None))
            .lock()
            .expect("粘贴序列状态锁已损坏");
        let sequence = state
            .as_ref()
            .filter(|value| value.next_index < value.ids.len())
            .ok_or_else(|| "当前没有正在执行的粘贴序列".to_owned())?;
        sequence.ids[sequence.next_index].clone()
    };
    let item = clipboard_copy(app, id)?;
    let status = {
        let mut state = PASTE_SEQUENCE
            .get_or_init(|| Mutex::new(None))
            .lock()
            .expect("粘贴序列状态锁已损坏");
        if let Some(sequence) = state.as_mut() {
            sequence.next_index += 1;
            let status = sequence_status(Some(sequence));
            if !status.active {
                *state = None;
            }
            status
        } else {
            sequence_status(None)
        }
    };
    Ok(ClipboardSequenceStep { item, status })
}

#[tauri::command]
pub fn clipboard_sequence_paste_next(app: AppHandle) -> Result<ClipboardSequenceStep, String> {
    let step = clipboard_sequence_next(app.clone())?;
    hide_quick_panel(&app)?;
    thread::sleep(StdDuration::from_millis(90));
    simulate_paste()?;
    Ok(step)
}

#[tauri::command]
pub fn clipboard_sequence_cancel() {
    *PASTE_SEQUENCE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("粘贴序列状态锁已损坏") = None;
}

#[tauri::command]
pub fn clipboard_delete(app: AppHandle, id: String) -> Result<(), String> {
    let connection = database::open(&app)?;
    let item = read_item(&connection, &id)?;
    connection
        .execute("DELETE FROM clipboard_items WHERE id = ?1", params![id])
        .map_err(|error| format!("无法删除剪贴板记录: {error}"))?;
    if item.kind == "image" {
        let _ = fs::remove_file(item.content);
    }
    let _ = app.emit("clipboard-history-updated", ());
    Ok(())
}

#[tauri::command]
pub fn clipboard_clear(app: AppHandle, include_pinned: bool) -> Result<i64, String> {
    let connection = database::open(&app)?;
    let where_clause = if include_pinned {
        "1 = 1"
    } else {
        "pinned = 0"
    };
    let mut statement = connection
        .prepare(&format!(
            "SELECT content FROM clipboard_items WHERE kind = 'image' AND {where_clause}"
        ))
        .map_err(|error| format!("无法读取待删除图片: {error}"))?;
    let paths = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| format!("无法读取待删除图片: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("无法读取待删除图片: {error}"))?;
    drop(statement);
    let deleted = connection
        .execute(
            &format!("DELETE FROM clipboard_items WHERE {where_clause}"),
            [],
        )
        .map_err(|error| format!("无法清空剪贴板历史: {error}"))? as i64;
    remove_paths(&paths);
    let _ = app.emit("clipboard-history-updated", ());
    Ok(deleted)
}

#[tauri::command]
pub fn clipboard_update_settings(
    app: AppHandle,
    mut settings: ClipboardSettings,
) -> Result<ClipboardStatus, String> {
    settings.max_items = settings.max_items.clamp(20, 5000);
    settings.ttl_days = settings.ttl_days.clamp(1, 365);
    settings.shortcut = settings.shortcut.trim().to_owned();
    if settings.shortcut.is_empty() {
        return Err("请输入全局快捷键".to_owned());
    }
    settings.excluded_apps = settings
        .excluded_apps
        .into_iter()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .collect();
    let connection = database::open(&app)?;
    let current_settings = read_settings(&connection)?;
    let launch_changed = current_settings.launch_at_login != settings.launch_at_login;
    if launch_changed {
        set_launch_at_login(settings.launch_at_login)?;
    }
    if let Err(error) = replace_shortcut(&app, &current_settings.shortcut, &settings.shortcut) {
        if launch_changed {
            let _ = set_launch_at_login(current_settings.launch_at_login);
        }
        return Err(error);
    }
    write_config(
        &connection,
        "enabled",
        if settings.enabled { "true" } else { "false" },
    )?;
    write_config(
        &connection,
        "launch_at_login",
        if settings.launch_at_login {
            "true"
        } else {
            "false"
        },
    )?;
    write_config(&connection, "max_items", &settings.max_items.to_string())?;
    write_config(&connection, "ttl_days", &settings.ttl_days.to_string())?;
    write_config(&connection, "shortcut", &settings.shortcut)?;
    write_config(
        &connection,
        "excluded_apps",
        &serde_json::to_string(&settings.excluded_apps)
            .map_err(|error| format!("无法保存应用排除名单: {error}"))?,
    )?;
    prune(&connection, &settings)?;
    clipboard_status(app)
}

#[tauri::command]
pub fn clipboard_save_as_snippet(app: AppHandle, id: String) -> Result<(), String> {
    let connection = database::open(&app)?;
    let item = read_item(&connection, &id)?;
    if item.kind != "text" {
        return Err("只有文本记录可以保存为片段".to_owned());
    }
    let title = item
        .content
        .lines()
        .next()
        .unwrap_or("剪贴板片段")
        .chars()
        .take(40)
        .collect::<String>();
    let snippet_id = Uuid::new_v4().to_string();
    let timestamp = now();
    connection
        .execute(
            "INSERT INTO snippets
             (id, title, kind, content, language, description, pinned, use_count, last_used_at, created_at, updated_at)
             VALUES (?1, ?2, 'text', ?3, NULL, '来自剪贴板历史', 0, 0, NULL, ?4, ?4)",
            params![&snippet_id, title, item.content, timestamp],
        )
        .map_err(|error| format!("无法保存为片段: {error}"))?;
    connection
        .execute(
            "INSERT INTO snippets_fts (id, title, content, description, language, tags)
             VALUES (?1, ?2, ?3, '来自剪贴板历史', '', '')",
            params![snippet_id, title, item.content],
        )
        .map_err(|error| format!("无法更新片段搜索索引: {error}"))?;
    Ok(())
}

#[tauri::command]
pub fn clipboard_image_data_url(app: AppHandle, id: String) -> Result<String, String> {
    let connection = database::open(&app)?;
    let item = read_item(&connection, &id)?;
    if item.kind != "image" {
        return Err("该记录不是图片".to_owned());
    }
    let bytes = fs::read(Path::new(&item.content))
        .map_err(|error| format!("无法读取剪贴板图片: {error}"))?;
    Ok(format!("data:image/png;base64,{}", STANDARD.encode(bytes)))
}

#[tauri::command]
pub fn clipboard_image_thumbnail_data_url(
    app: AppHandle,
    id: String,
    size: u32,
) -> Result<String, String> {
    let connection = database::open(&app)?;
    let item = read_item(&connection, &id)?;
    if item.kind != "image" {
        return Err("该记录不是图片".to_owned());
    }
    let image = image::open(Path::new(&item.content))
        .map_err(|error| format!("无法读取剪贴板图片: {error}"))?;
    let thumbnail = image.thumbnail(size.clamp(24, 256), size.clamp(24, 256));
    let mut bytes = Cursor::new(Vec::new());
    thumbnail
        .write_to(&mut bytes, image::ImageFormat::Png)
        .map_err(|error| format!("无法生成图片缩略图: {error}"))?;
    Ok(format!(
        "data:image/png;base64,{}",
        STANDARD.encode(bytes.into_inner())
    ))
}

fn preview_image_mime(extension: &str) -> Option<&'static str> {
    match extension {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        _ => None,
    }
}

#[tauri::command]
pub fn clipboard_file_preview(
    app: AppHandle,
    id: String,
    index: usize,
) -> Result<ClipboardFilePreview, String> {
    let connection = database::open(&app)?;
    let item = read_item(&connection, &id)?;
    if item.kind != "files" {
        return Err("该记录不是文件".to_owned());
    }

    let paths = serde_json::from_str::<Vec<String>>(&item.content)
        .map_err(|error| format!("无法读取文件列表: {error}"))?;
    let path_value = paths
        .get(index)
        .ok_or_else(|| "文件序号超出范围".to_owned())?;
    let path = Path::new(path_value);
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(path_value)
        .to_owned();
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());
    let metadata = fs::metadata(path).map_err(|error| format!("无法读取文件信息: {error}"))?;

    let mut preview = ClipboardFilePreview {
        path: path_value.to_owned(),
        name,
        extension: extension.clone(),
        kind: "unsupported".to_owned(),
        size_bytes: metadata.len(),
        content: None,
        data_url: None,
        message: None,
    };

    if !metadata.is_file() {
        preview.message = Some("文件夹不支持内容预览".to_owned());
        return Ok(preview);
    }

    if let Some(mime) = extension.as_deref().and_then(preview_image_mime) {
        if metadata.len() > MAX_FILE_BINARY_PREVIEW_BYTES {
            preview.message = Some("图片超过 20 MB，无法在面板中预览".to_owned());
            return Ok(preview);
        }
        let bytes = fs::read(path).map_err(|error| format!("无法读取图片: {error}"))?;
        preview.kind = "image".to_owned();
        preview.data_url = Some(format!("data:{mime};base64,{}", STANDARD.encode(bytes)));
        return Ok(preview);
    }

    if extension.as_deref() == Some("pdf") {
        if metadata.len() > MAX_FILE_BINARY_PREVIEW_BYTES {
            preview.message = Some("PDF 超过 20 MB，无法在面板中预览".to_owned());
            return Ok(preview);
        }
        let bytes = fs::read(path).map_err(|error| format!("无法读取 PDF: {error}"))?;
        preview.kind = "pdf".to_owned();
        preview.data_url = Some(format!(
            "data:application/pdf;base64,{}",
            STANDARD.encode(bytes)
        ));
        return Ok(preview);
    }

    if metadata.len() > MAX_FILE_TEXT_PREVIEW_BYTES {
        preview.message = Some("文件超过 2 MB，无法在面板中预览".to_owned());
        return Ok(preview);
    }

    let bytes = fs::read(path).map_err(|error| format!("无法读取文件: {error}"))?;
    match String::from_utf8(bytes) {
        Ok(content) => {
            preview.kind = "text".to_owned();
            preview.content = Some(content);
        }
        Err(_) => preview.message = Some("该文件不是可预览的文本、图片或 PDF".to_owned()),
    }
    Ok(preview)
}

#[tauri::command]
pub fn clipboard_open_special(kind: String, value: String) -> Result<(), String> {
    let value = value.trim();
    let target = match kind.as_str() {
        "url"
            if regex::Regex::new(r"^https?://[^\s]+$")
                .expect("链接校验表达式无效")
                .is_match(value) =>
        {
            value.to_owned()
        }
        "email"
            if regex::Regex::new(r"^[^\s@]+@[^\s@]+\.[^\s@]+$")
                .expect("邮箱校验表达式无效")
                .is_match(value) =>
        {
            format!("mailto:{value}")
        }
        _ => return Err("不支持打开该内容".to_owned()),
    };
    #[cfg(target_os = "macos")]
    Command::new("open")
        .arg(target)
        .spawn()
        .map_err(|error| format!("无法打开内容: {error}"))?;
    #[cfg(target_os = "windows")]
    Command::new("explorer")
        .arg(&target)
        .spawn()
        .map_err(|error| format!("无法打开内容: {error}"))?;
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    return Err("当前系统不支持打开该内容".to_owned());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{hash_bytes, is_excluded, sequence_status, validate_shortcut, PasteSequence};
    use std::str::FromStr;
    use tauri_plugin_global_shortcut::Shortcut;

    #[test]
    fn hashes_kind_and_content() {
        assert_eq!(hash_bytes("text", b"hello"), hash_bytes("text", b"hello"));
        assert_ne!(hash_bytes("text", b"hello"), hash_bytes("files", b"hello"));
    }

    #[test]
    fn matches_excluded_app_without_case_sensitivity() {
        assert!(is_excluded(Some("1Password 8"), &["1password".to_owned()]));
        assert!(!is_excluded(
            Some("Visual Studio Code"),
            &["Bitwarden".to_owned()]
        ));
    }

    #[test]
    fn parses_default_global_shortcut() {
        assert!(Shortcut::from_str("CommandOrControl+Shift+V").is_ok());
    }

    #[test]
    fn validates_standalone_function_keys_and_requires_combinations_otherwise() {
        assert!(validate_shortcut("F3").is_ok());
        assert!(validate_shortcut("Shift+V").is_ok());
        assert!(validate_shortcut("V").is_err());
    }

    #[test]
    fn reports_remaining_sequence_items() {
        let sequence = PasteSequence {
            ids: vec!["a".to_owned(), "b".to_owned(), "c".to_owned()],
            next_index: 1,
        };
        let status = sequence_status(Some(&sequence));
        assert!(status.active);
        assert_eq!(status.total, 3);
        assert_eq!(status.next_index, 1);
        assert_eq!(status.remaining, 2);
    }
}
