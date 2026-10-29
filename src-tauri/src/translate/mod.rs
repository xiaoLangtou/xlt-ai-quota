//! 翻译：内置多引擎（有道智云 / DeepSeek）+ 独立置顶快捷面板。
//!
//! 配置落在本机 `app_data_dir/translate.json`（0600），请求全部由 Rust 侧发出，
//! 密钥不进前端日志。引擎差异收敛在 `youdao` / `deepseek` 两个子模块里，
//! 新增引擎只需实现 `translate_chunk` 并在 `Engine` 上挂一个分支。

mod deepseek;
pub mod execution;
mod youdao;

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

use crate::clipboard_history;

const CONFIG_FILE: &str = "translate.json";
const DEFAULT_SHORTCUT: &str = "Alt+Space";
const DEFAULT_TIMEOUT_MS: u64 = 30_000;
/// 有道单次上限 5000 字符，留出余量给签名与编码。
const MAX_CHUNK_CHARS: usize = 4_500;
/// 有道对短时并发很敏感（411 访问频率受限），因此顺序发送；DeepSeek 可以并行。
const YOUDAO_CONCURRENCY: usize = 1;
const DEEPSEEK_CONCURRENCY: usize = 3;
/// 命中频率限制后的重试次数与退避基数（第 n 次等待 n × 900ms）。
const RATE_LIMIT_RETRIES: u32 = 2;
const RATE_LIMIT_BACKOFF_MS: u64 = 900;
/// 有道顺序请求之间的最小间隔，进一步压低触发 411 的概率。
const YOUDAO_REQUEST_INTERVAL_MS: u64 = 150;
const PANEL_LABEL: &str = "translate-panel";
const DOMAINS: [&str; 5] = ["general", "computers", "medicine", "finance", "game"];

/// 有道智云文本翻译配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct YoudaoConfig {
    pub base_url: String,
    pub app_key: String,
    pub app_secret: String,
    /// 领域化翻译：general / computers / medicine / finance / game（仅中英互译生效）。
    pub domain: String,
    /// 控制台术语表 ID。
    pub vocab_id: String,
    /// 严格按 from/to 翻译，关闭时有道会自行中英互译。
    pub strict: bool,
    /// 领域翻译失败时不降级到通用翻译。
    pub reject_fallback: bool,
}

impl Default for YoudaoConfig {
    fn default() -> Self {
        Self {
            base_url: "https://openapi.youdao.com/api".to_owned(),
            app_key: String::new(),
            app_secret: String::new(),
            domain: "general".to_owned(),
            vocab_id: String::new(),
            strict: false,
            reject_fallback: false,
        }
    }
}

/// DeepSeek（OpenAI 兼容）翻译配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DeepSeekConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub temperature: f64,
    /// disabled 时不传 thinking 字段，enabled 时按 DeepSeek 的 thinking 参数开启。
    pub thinking: String,
    pub system_prompt: String,
    pub user_prompt: String,
}

impl Default for DeepSeekConfig {
    fn default() -> Self {
        Self {
            base_url: "https://api.deepseek.com".to_owned(),
            api_key: String::new(),
            model: "deepseek-v4-flash".to_owned(),
            temperature: 0.0,
            thinking: "disabled".to_owned(),
            system_prompt: [
                "You are a professional translator. Translate the input text to {target_language}.",
                "Rules:",
                "- Output ONLY the translation, no explanation, no quotes.",
                "- Preserve original formatting, line breaks, and punctuation.",
                "- For technical terms, keep the English original in parentheses.",
            ]
            .join("\n"),
            user_prompt: "Translate into {target_language}:\n\n{text}".to_owned(),
        }
    }
}

/// 翻译服务配置：内置服务 + 各服务的独立参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TranslateConfig {
    /// 当前使用的内置服务：youdao / deepseek。
    pub engine: String,
    pub timeout_ms: u64,
    /// 快捷面板全局快捷键。
    pub shortcut: String,
    pub youdao: YoudaoConfig,
    pub deepseek: DeepSeekConfig,
}

impl Default for TranslateConfig {
    fn default() -> Self {
        Self {
            engine: "youdao".to_owned(),
            timeout_ms: DEFAULT_TIMEOUT_MS,
            shortcut: DEFAULT_SHORTCUT.to_owned(),
            youdao: YoudaoConfig::default(),
            deepseek: DeepSeekConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Engine {
    Youdao,
    DeepSeek,
}

/// 分段请求的错误分类：频率限制值得退避重试，其余直接失败。
#[derive(Debug)]
pub(super) enum ChunkError {
    RateLimited(String),
    Fatal(String),
}

impl ChunkError {
    #[cfg(test)]
    pub(super) fn message(&self) -> &str {
        match self {
            ChunkError::RateLimited(message) | ChunkError::Fatal(message) => message,
        }
    }
}

impl TranslateConfig {
    fn engine(&self) -> Result<Engine, String> {
        match self.engine.as_str() {
            "youdao" => Ok(Engine::Youdao),
            "deepseek" => Ok(Engine::DeepSeek),
            _ => Err("内置服务取值无效".to_owned()),
        }
    }
}

/// 旧版扁平配置：仅用于把已填好的有道凭据迁移到 `youdao` 子对象。
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct LegacyConfig {
    base_url: String,
    app_key: String,
    app_secret: String,
    domain: String,
    vocab_id: String,
    strict: bool,
    reject_fallback: bool,
    timeout_ms: u64,
    shortcut: String,
}

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("无法定位本机数据目录: {error}"))?;
    fs::create_dir_all(&directory).map_err(|error| format!("无法创建配置目录: {error}"))?;
    Ok(directory.join(CONFIG_FILE))
}

pub fn load_config(app: &AppHandle) -> TranslateConfig {
    let Ok(path) = config_path(app) else {
        return TranslateConfig::default();
    };
    let Ok(raw) = fs::read_to_string(path) else {
        return TranslateConfig::default();
    };
    let Ok(value) = serde_json::from_str::<Value>(&raw) else {
        return TranslateConfig::default();
    };
    let mut config: TranslateConfig =
        serde_json::from_value(value.clone()).unwrap_or_default();
    migrate_legacy(&mut config, &value);
    config
}

/// 旧版把有道参数放在顶层；检测到 `youdao` 缺失时把它们搬到子对象，避免凭据丢失。
fn migrate_legacy(config: &mut TranslateConfig, value: &Value) {
    if value.get("youdao").is_some() {
        return;
    }
    let Ok(legacy) = serde_json::from_value::<LegacyConfig>(value.clone()) else {
        return;
    };
    if !legacy.app_key.is_empty() {
        config.youdao.app_key = legacy.app_key;
    }
    if !legacy.app_secret.is_empty() {
        config.youdao.app_secret = legacy.app_secret;
    }
    if !legacy.base_url.is_empty() {
        config.youdao.base_url = legacy.base_url;
    }
    if !legacy.domain.is_empty() {
        config.youdao.domain = legacy.domain;
    }
    if !legacy.vocab_id.is_empty() {
        config.youdao.vocab_id = legacy.vocab_id;
    }
    config.youdao.strict = legacy.strict;
    config.youdao.reject_fallback = legacy.reject_fallback;
    if legacy.timeout_ms > 0 {
        config.timeout_ms = legacy.timeout_ms;
    }
    if !legacy.shortcut.is_empty() {
        config.shortcut = legacy.shortcut;
    }
}

fn write_config(app: &AppHandle, config: &TranslateConfig) -> Result<(), String> {
    let path = config_path(app)?;
    let raw = serde_json::to_string_pretty(config)
        .map_err(|error| format!("无法序列化翻译配置: {error}"))?;
    let temporary_path = path.with_extension("tmp");
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temporary_path)
        .map_err(|error| format!("翻译配置写入失败: {error}"))?;
    file.write_all(raw.as_bytes())
        .map_err(|error| format!("翻译配置写入失败: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("翻译配置同步失败: {error}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temporary_path, fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("无法设置翻译配置权限: {error}"))?;
    }

    fs::rename(&temporary_path, &path).map_err(|error| format!("翻译配置替换失败: {error}"))
}

fn normalize_config(mut config: TranslateConfig) -> Result<TranslateConfig, String> {
    config.engine = config.engine.trim().to_lowercase();
    if config.engine.is_empty() {
        config.engine = "youdao".to_owned();
    }
    config.engine()?;
    config.timeout_ms = config.timeout_ms.clamp(3_000, 120_000);
    config.shortcut = config.shortcut.trim().to_owned();
    if config.shortcut.is_empty() {
        config.shortcut = DEFAULT_SHORTCUT.to_owned();
    }
    clipboard_history::validate_shortcut(&config.shortcut)?;

    normalize_youdao(&mut config.youdao)?;
    normalize_deepseek(&mut config.deepseek)?;
    Ok(config)
}

fn normalize_youdao(config: &mut YoudaoConfig) -> Result<(), String> {
    config.base_url = config.base_url.trim().to_owned();
    config.app_key = config.app_key.trim().to_owned();
    config.app_secret = config.app_secret.trim().to_owned();
    config.domain = config.domain.trim().to_owned();
    config.vocab_id = config.vocab_id.trim().to_owned();
    if config.base_url.is_empty() {
        config.base_url = YoudaoConfig::default().base_url;
    }
    require_http(&config.base_url)?;
    if config.domain.is_empty() {
        config.domain = "general".to_owned();
    }
    if !DOMAINS.contains(&config.domain.as_str()) {
        return Err("领域取值无效".to_owned());
    }
    Ok(())
}

fn normalize_deepseek(config: &mut DeepSeekConfig) -> Result<(), String> {
    config.base_url = config.base_url.trim().to_owned();
    config.api_key = config.api_key.trim().to_owned();
    config.model = config.model.trim().to_owned();
    config.thinking = config.thinking.trim().to_lowercase();
    if config.base_url.is_empty() {
        config.base_url = DeepSeekConfig::default().base_url;
    }
    require_http(&config.base_url)?;
    if config.model.is_empty() {
        config.model = DeepSeekConfig::default().model;
    }
    if config.thinking.is_empty() {
        config.thinking = "disabled".to_owned();
    }
    if config.thinking != "disabled" && config.thinking != "enabled" {
        return Err("思考模式取值无效".to_owned());
    }
    config.temperature = config.temperature.clamp(0.0, 2.0);
    if config.system_prompt.trim().is_empty() {
        config.system_prompt = DeepSeekConfig::default().system_prompt;
    }
    if config.user_prompt.trim().is_empty() {
        config.user_prompt = DeepSeekConfig::default().user_prompt;
    }
    Ok(())
}

fn require_http(url: &str) -> Result<(), String> {
    if url.starts_with("https://") || url.starts_with("http://") {
        Ok(())
    } else {
        Err("基础 URL 必须以 https:// 或 http:// 开头".to_owned())
    }
}

/// 面板快捷键的注册结果，供设置页显示「是否真的生效」。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutStatus {
    pub shortcut: String,
    pub registered: bool,
    pub error: String,
}

/// 全局快捷键注册记录：启动时注册失败只写日志没人看得到，这里留给设置页展示。
#[derive(Default)]
pub struct ShortcutState(Mutex<ShortcutRecord>);

#[derive(Default)]
struct ShortcutRecord {
    shortcut: String,
    error: String,
}

impl ShortcutState {
    fn record(&self, shortcut: &str, error: &str) {
        if let Ok(mut record) = self.0.lock() {
            record.shortcut = shortcut.to_owned();
            record.error = error.to_owned();
        }
    }

    fn snapshot(&self) -> (String, String) {
        match self.0.lock() {
            Ok(record) => (record.shortcut.clone(), record.error.clone()),
            Err(_) => (String::new(), String::new()),
        }
    }
}

/// 面板快捷键与剪贴板面板共用一套全局注册，注册时不清理其他快捷键。
pub fn register_shortcut(app: &AppHandle, shortcut: &str) -> Result<(), String> {
    let result = clipboard_history::validate_shortcut(shortcut).and_then(|_| {
        app.global_shortcut()
            .register(shortcut)
            .map_err(|error| format!("快捷键已被占用或无法注册: {error}"))
    });
    if let Some(state) = app.try_state::<ShortcutState>() {
        state.record(shortcut, result.as_ref().err().map(String::as_str).unwrap_or_default());
    }
    result
}

/// 已注册就跳过；启动时被占用导致失败的情况下，重新保存同值也能重试。
fn ensure_shortcut(app: &AppHandle, shortcut: &str) -> Result<(), String> {
    if app.global_shortcut().is_registered(shortcut) {
        return Ok(());
    }
    register_shortcut(app, shortcut)
}

fn replace_shortcut(app: &AppHandle, current: &str, next: &str) -> Result<(), String> {
    if current == next {
        return ensure_shortcut(app, next);
    }
    register_shortcut(app, next)?;
    if let Err(error) = app.global_shortcut().unregister(current) {
        let _ = app.global_shortcut().unregister(next);
        return Err(format!("无法取消原快捷键: {error}"));
    }
    Ok(())
}

/// 设置页用来确认快捷键是否真的注册到系统。
#[tauri::command]
pub fn translate_shortcut_status(app: AppHandle) -> ShortcutStatus {
    let config = load_config(&app);
    let error = app
        .try_state::<ShortcutState>()
        .map(|state| state.snapshot().1)
        .unwrap_or_default();
    ShortcutStatus {
        registered: app.global_shortcut().is_registered(config.shortcut.as_str()),
        shortcut: config.shortcut,
        error,
    }
}

/// 判断按下的全局快捷键是否属于翻译面板。
pub fn owns_shortcut(app: &AppHandle, pressed: &Shortcut) -> bool {
    match Shortcut::from_str(&load_config(app).shortcut) {
        Ok(shortcut) => shortcut.mods == pressed.mods && shortcut.key == pressed.key,
        Err(_) => false,
    }
}

pub fn toggle_panel(app: &AppHandle) -> Result<(), String> {
    let panel = app
        .get_webview_window(PANEL_LABEL)
        .ok_or_else(|| "未找到翻译快捷面板".to_owned())?;
    if panel.is_visible().map_err(|error| error.to_string())? {
        panel.hide().map_err(|error| error.to_string())?;
    } else {
        panel.show().map_err(|error| error.to_string())?;
        panel.set_focus().map_err(|error| error.to_string())?;
        // 显示后复核：透明无边框窗口若未真正上屏，用户只会看到「按了没反应」。
        if !panel.is_visible().map_err(|error| error.to_string())? {
            return Err("翻译面板未能显示".to_owned());
        }
    }
    Ok(())
}

#[tauri::command]
pub fn translate_show_panel(app: AppHandle) -> Result<(), String> {
    let panel = app
        .get_webview_window(PANEL_LABEL)
        .ok_or_else(|| "未找到翻译快捷面板".to_owned())?;
    panel.show().map_err(|error| error.to_string())?;
    panel.set_focus().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn translate_hide_panel(app: AppHandle) -> Result<(), String> {
    let panel = app
        .get_webview_window(PANEL_LABEL)
        .ok_or_else(|| "未找到翻译快捷面板".to_owned())?;
    panel.hide().map_err(|error| error.to_string())
}

/// 只更新快捷键并立即注册：录制完就生效，避免「录了没保存」的歧义。
#[tauri::command]
pub fn translate_set_shortcut(app: AppHandle, shortcut: String) -> Result<ShortcutStatus, String> {
    let mut config = load_config(&app);
    let next = shortcut.trim().to_owned();
    if next.is_empty() {
        return Err("快捷键不能为空".to_owned());
    }
    clipboard_history::validate_shortcut(&next)?;
    replace_shortcut(&app, &config.shortcut, &next)?;
    config.shortcut = next;
    write_config(&app, &config)?;
    Ok(translate_shortcut_status(app))
}

/// 翻译页切换内置服务：立即持久化，后续请求按新引擎走。
#[tauri::command]
pub fn translate_set_engine(app: AppHandle, engine: String) -> Result<TranslateConfig, String> {
    let mut config = load_config(&app);
    config.engine = engine.trim().to_lowercase();
    let normalized = normalize_config(config)?;
    write_config(&app, &normalized)?;
    Ok(normalized)
}

#[tauri::command]
pub fn translate_config_get(app: AppHandle) -> TranslateConfig {
    load_config(&app)
}

#[tauri::command]
pub fn translate_config_save(
    app: AppHandle,
    config: TranslateConfig,
) -> Result<TranslateConfig, String> {
    let next = normalize_config(config)?;
    let current = load_config(&app);
    replace_shortcut(&app, &current.shortcut, &next.shortcut)?;
    write_config(&app, &next)?;
    Ok(next)
}

/// 用固定短句验证凭据。传入草稿配置时只用于测试，不落盘。
#[tauri::command]
pub async fn translate_test(
    app: AppHandle,
    config: Option<TranslateConfig>,
) -> Result<String, String> {
    let config = match config {
        Some(draft) => normalize_config(draft)?,
        None => load_config(&app),
    };
    ensure_ready(&config)?;
    let client = build_client(&config)?;
    let (translated, _) =
        translate_chunk_with_retry(&client, &config, "Hello world", "en", "zh-CHS").await?;
    Ok(translated)
}

fn ensure_ready(config: &TranslateConfig) -> Result<(), String> {
    match config.engine()? {
        Engine::Youdao => {
            if config.youdao.app_key.is_empty() || config.youdao.app_secret.is_empty() {
                return Err("尚未配置有道翻译：请填写应用 ID 与应用密钥".to_owned());
            }
        }
        Engine::DeepSeek => {
            if config.deepseek.api_key.is_empty() {
                return Err("尚未配置 DeepSeek：请填写 API 密钥".to_owned());
            }
        }
    }
    Ok(())
}

fn build_client(config: &TranslateConfig) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_millis(config.timeout_ms))
        .build()
        .map_err(|error| format!("无法初始化网络客户端: {error}"))
}

fn concurrency(engine: Engine) -> usize {
    match engine {
        Engine::Youdao => YOUDAO_CONCURRENCY,
        Engine::DeepSeek => DEEPSEEK_CONCURRENCY,
    }
}

/// 命中 411 / 412 时退避重试，避免整篇文档因为一次限流直接失败。
async fn translate_chunk_with_retry(
    client: &reqwest::Client,
    config: &TranslateConfig,
    text: &str,
    from: &str,
    to: &str,
) -> Result<(String, String), String> {
    let mut attempt = 0;
    loop {
        match translate_chunk(client, config, text, from, to).await {
            Ok(value) => return Ok(value),
            Err(ChunkError::RateLimited(message)) => {
                if attempt >= RATE_LIMIT_RETRIES {
                    return Err(message);
                }
                attempt += 1;
                tokio::time::sleep(Duration::from_millis(
                    RATE_LIMIT_BACKOFF_MS * attempt as u64,
                ))
                .await;
            }
            Err(ChunkError::Fatal(message)) => return Err(message),
        }
    }
}

async fn translate_chunk(
    client: &reqwest::Client,
    config: &TranslateConfig,
    text: &str,
    from: &str,
    to: &str,
) -> Result<(String, String), ChunkError> {
    match config.engine() {
        Ok(Engine::Youdao) => youdao::translate_chunk(client, &config.youdao, text, from, to).await,
        Ok(Engine::DeepSeek) => {
            deepseek::translate_chunk(client, &config.deepseek, text, from, to).await
        }
        Err(message) => Err(ChunkError::Fatal(message)),
    }
}

fn normalize_lang(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "auto".to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn unix_timestamp() -> Result<String, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .map_err(|error| format!("无法获取系统时间: {error}"))
}

/// 按空行分段；单段超过接口上限时再按句末标点切分。
fn split_paragraphs(text: &str) -> Vec<String> {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut paragraphs = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    for line in normalized.lines() {
        if line.trim().is_empty() {
            push_paragraph(&mut paragraphs, &mut current);
        } else {
            current.push(line.trim_end());
        }
    }
    push_paragraph(&mut paragraphs, &mut current);
    paragraphs
        .into_iter()
        .flat_map(|paragraph| split_oversized(&paragraph))
        .collect()
}

fn push_paragraph(paragraphs: &mut Vec<String>, current: &mut Vec<&str>) {
    if current.is_empty() {
        return;
    }
    let text = current.join("\n").trim().to_owned();
    current.clear();
    if !text.is_empty() {
        paragraphs.push(text);
    }
}

fn split_oversized(paragraph: &str) -> Vec<String> {
    if paragraph.chars().count() <= MAX_CHUNK_CHARS {
        return vec![paragraph.to_owned()];
    }
    let mut chunks = Vec::new();
    let mut current = String::new();
    for sentence in split_sentences(paragraph) {
        for part in hard_split(&sentence, MAX_CHUNK_CHARS) {
            if !current.is_empty()
                && current.chars().count() + part.chars().count() > MAX_CHUNK_CHARS
            {
                chunks.push(std::mem::take(&mut current));
            }
            current.push_str(&part);
        }
    }
    if !current.trim().is_empty() {
        chunks.push(current);
    }
    chunks
}

fn split_sentences(text: &str) -> Vec<String> {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    let pattern = PATTERN.get_or_init(|| {
        Regex::new(r"[^。！？.!?]+[。！？.!?]+|[^。！？.!?]+$").expect("句末标点正则固定可用")
    });
    pattern
        .find_iter(text)
        .map(|item| item.as_str().to_owned())
        .collect()
}

fn hard_split(text: &str, limit: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= limit {
        return vec![text.to_owned()];
    }
    chars
        .chunks(limit)
        .map(|chunk| chunk.iter().collect())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        migrate_legacy, normalize_config, split_oversized, split_paragraphs, TranslateConfig,
        MAX_CHUNK_CHARS,
    };
    use serde_json::json;

    #[test]
    fn splits_paragraphs_on_blank_lines() {
        let text = "First paragraph.\n\nSecond line one.\nSecond line two.\n\n\nThird.";
        let paragraphs = split_paragraphs(text);
        assert_eq!(paragraphs.len(), 3);
        assert_eq!(paragraphs[0], "First paragraph.");
        assert_eq!(paragraphs[1], "Second line one.\nSecond line two.");
        assert_eq!(paragraphs[2], "Third.");
    }

    #[test]
    fn ignores_blank_input() {
        assert!(split_paragraphs("   \n\n  \n").is_empty());
    }

    #[test]
    fn splits_oversized_paragraph_under_limit() {
        let paragraph = "句子。".repeat(MAX_CHUNK_CHARS);
        let chunks = split_oversized(&paragraph);
        assert!(chunks.len() > 1, "超长段落必须被切分");
        assert!(
            chunks
                .iter()
                .all(|chunk| chunk.chars().count() <= MAX_CHUNK_CHARS),
            "切分后每段都不能超过接口上限"
        );
        assert_eq!(chunks.concat(), paragraph, "切分不能丢字");
    }

    #[test]
    fn migrates_legacy_flat_config() {
        let value = json!({
            "appKey": "legacy-key",
            "appSecret": "legacy-secret",
            "timeoutMs": 15000,
            "shortcut": "Alt+T"
        });
        let mut config = TranslateConfig::default();
        migrate_legacy(&mut config, &value);
        assert_eq!(config.youdao.app_key, "legacy-key");
        assert_eq!(config.youdao.app_secret, "legacy-secret");
        assert_eq!(config.timeout_ms, 15000);
        assert_eq!(config.shortcut, "Alt+T");
    }

    #[test]
    fn keeps_nested_config_untouched() {
        let value = json!({ "youdao": { "appKey": "nested" } });
        let mut config = TranslateConfig::default();
        config.youdao.app_key = "nested".to_owned();
        migrate_legacy(&mut config, &value);
        assert_eq!(config.youdao.app_key, "nested");
    }

    #[test]
    fn rejects_unknown_engine() {
        let mut config = TranslateConfig::default();
        config.engine = "baidu".to_owned();
        assert!(normalize_config(config).is_err());
    }

    #[test]
    fn normalizes_engine_specific_defaults() {
        let mut config = TranslateConfig::default();
        config.engine = " DeepSeek ".to_owned();
        config.deepseek.model = "  ".to_owned();
        config.deepseek.base_url = "".to_owned();
        let normalized = normalize_config(config).expect("默认值应当补齐");
        assert_eq!(normalized.engine, "deepseek");
        assert_eq!(normalized.deepseek.model, "deepseek-v4-flash");
        assert_eq!(normalized.deepseek.base_url, "https://api.deepseek.com");
    }
}
