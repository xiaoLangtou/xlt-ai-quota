//! 每窗口单个可取消请求；取消丢弃请求 Future，停止网络等待与后续段落调度。
use super::*;
use futures_util::{
    future::{AbortHandle, AbortRegistration, Abortable},
    stream::{self, StreamExt},
};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Instant,
};
use tauri::{Emitter, WebviewWindow};

#[derive(Clone, Default)]
pub struct TranslationRequests(Arc<Mutex<HashMap<String, (String, AbortHandle)>>>);
struct RequestGuard {
    state: TranslationRequests,
    window: String,
    id: String,
}
impl Drop for RequestGuard {
    fn drop(&mut self) {
        if let Ok(mut requests) = self.state.0.lock() {
            if requests
                .get(&self.window)
                .is_some_and(|(id, _)| id == &self.id)
            {
                requests.remove(&self.window);
            }
        }
    }
}
impl TranslationRequests {
    fn begin(&self, window: &str, id: &str) -> Result<(AbortRegistration, RequestGuard), String> {
        if id.is_empty() || id.len() > 80 {
            return Err("翻译请求标识无效".into());
        }
        let mut requests = self.0.lock().map_err(|_| "无法读取翻译请求状态")?;
        if requests.contains_key(window) {
            return Err("当前窗口仍有翻译请求，请先取消或等待结束".into());
        }
        let (handle, registration) = AbortHandle::new_pair();
        requests.insert(window.into(), (id.into(), handle));
        Ok((
            registration,
            RequestGuard {
                state: self.clone(),
                window: window.into(),
                id: id.into(),
            },
        ))
    }
    fn cancel(&self, window: &str, id: &str) -> Result<bool, String> {
        let mut requests = self.0.lock().map_err(|_| "无法读取翻译请求状态")?;
        if !requests.get(window).is_some_and(|(active, _)| active == id) {
            return Ok(false);
        }
        let (_, handle) = requests.remove(window).ok_or("翻译请求已结束")?;
        handle.abort();
        Ok(true)
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Segment {
    pub index: usize,
    pub original: String,
    pub translated: String,
    pub status: String,
    pub error: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub segments: Vec<Segment>,
    pub lang: String,
    pub ms: u64,
    pub requests: usize,
    pub cancelled: bool,
    pub engine: String,
    pub config_version: String,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    request_id: String,
    done: usize,
    total: usize,
    config_version: String,
    #[serde(flatten)]
    segment: Segment,
}
fn config_version(config: &TranslateConfig) -> Result<String, String> {
    let bytes = serde_json::to_vec(config).map_err(|_| "无法识别当前翻译配置")?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

#[tauri::command]
pub fn translate_cancel(
    window: WebviewWindow,
    app: AppHandle,
    request_id: String,
) -> Result<bool, String> {
    app.state::<TranslationRequests>()
        .cancel(window.label(), &request_id)
}
#[tauri::command]
pub async fn translate_run(
    window: WebviewWindow,
    app: AppHandle,
    text: String,
    from: String,
    to: String,
    request_id: String,
) -> Result<Outcome, String> {
    if text.chars().count() > 100_000 {
        return Err("单次翻译不能超过十万字符".into());
    }
    let chunks: Vec<_> = split_paragraphs(&text).into_iter().enumerate().collect();
    execute(window, app, chunks, from, to, request_id, None).await
}
#[tauri::command]
pub async fn translate_retry(
    window: WebviewWindow,
    app: AppHandle,
    text: String,
    index: usize,
    from: String,
    to: String,
    request_id: String,
    expected_config: String,
) -> Result<Outcome, String> {
    if text.trim().is_empty() || text.chars().count() > MAX_CHUNK_CHARS || index >= 200 {
        return Err("重试段落超出范围".into());
    }
    execute(
        window,
        app,
        vec![(index, text)],
        from,
        to,
        request_id,
        Some(expected_config),
    )
    .await
}
async fn execute(
    window: WebviewWindow,
    app: AppHandle,
    chunks: Vec<(usize, String)>,
    from: String,
    to: String,
    request_id: String,
    expected: Option<String>,
) -> Result<Outcome, String> {
    if chunks.is_empty() || chunks.len() > 200 {
        return Err("翻译段落须为一到二百段".into());
    }
    let config = load_config(&app);
    ensure_ready(&config)?;
    let version = config_version(&config)?;
    if expected.is_some_and(|expected| expected != version) {
        return Err("翻译配置已变化，请重新翻译全文".into());
    }
    let client = build_client(&config)?;
    let from = normalize_lang(&from);
    let to = normalize_lang(&to);
    if config.engine()? == Engine::Youdao && config.youdao.strict && from == "auto" {
        return Err("严格翻译需要指定源语言".into());
    }
    let (registration, _guard) = app
        .state::<TranslationRequests>()
        .begin(window.label(), &request_id)?;
    let started = Instant::now();
    let total = chunks.len();
    let limit = concurrency(config.engine()?);
    let paced = matches!(config.engine(), Ok(Engine::Youdao));
    let completed = Arc::new(Mutex::new(BTreeMap::<usize, Segment>::new()));
    let finished = Arc::new(AtomicUsize::new(0));
    let submitted = Arc::new(AtomicUsize::new(0));
    let lang = Arc::new(Mutex::new(String::new()));
    let notify = |segment: Segment, done: usize| {
        let _ = window.emit(
            "translate:progress",
            Progress {
                request_id: request_id.clone(),
                done,
                total,
                config_version: version.clone(),
                segment,
            },
        );
    };
    for (index, original) in &chunks {
        notify(
            Segment {
                index: *index,
                original: original.clone(),
                translated: String::new(),
                status: "queued".into(),
                error: None,
            },
            0,
        );
    }
    let work = stream::iter(chunks.iter().cloned().enumerate())
        .map(|(position, (index, original))| {
            let config = &config;
            let client = &client;
            let from = &from;
            let to = &to;
            let notify = &notify;
            let completed = completed.clone();
            let finished = finished.clone();
            let submitted = submitted.clone();
            let lang = lang.clone();
            async move {
                if paced && position > 0 {
                    tokio::time::sleep(Duration::from_millis(YOUDAO_REQUEST_INTERVAL_MS)).await;
                }
                notify(
                    Segment {
                        index,
                        original: original.clone(),
                        translated: String::new(),
                        status: "running".into(),
                        error: None,
                    },
                    finished.load(Ordering::Relaxed),
                );
                submitted.fetch_add(1, Ordering::Relaxed);
                let result = translate_chunk_with_retry(client, config, &original, from, to).await;
                let mut segment = Segment {
                    index,
                    original,
                    translated: String::new(),
                    status: "failed".into(),
                    error: None,
                };
                match result {
                    Ok((text, detected)) => {
                        segment.translated = text;
                        segment.status = "done".into();
                        if let Ok(mut language) = lang.lock() {
                            if language.is_empty() {
                                *language = detected;
                            }
                        }
                    }
                    Err(error) => segment.error = Some(error),
                }
                completed
                    .lock()
                    .expect("translation results mutex")
                    .insert(index, segment.clone());
                notify(segment, finished.fetch_add(1, Ordering::Relaxed) + 1);
            }
        })
        .buffer_unordered(limit)
        .collect::<Vec<_>>();
    let cancelled = Abortable::new(work, registration).await.is_err();
    let mut results = completed.lock().map_err(|_| "无法读取翻译结果")?;
    let segments = chunks
        .into_iter()
        .map(|(index, original)| {
            results.remove(&index).unwrap_or(Segment {
                index,
                original,
                translated: String::new(),
                status: "cancelled".into(),
                error: None,
            })
        })
        .collect();
    let detected = lang.lock().map_err(|_| "无法读取语言结果")?.clone();
    Ok(Outcome {
        segments,
        lang: detected,
        ms: started.elapsed().as_millis() as u64,
        requests: submitted.load(Ordering::Relaxed),
        cancelled,
        engine: config.engine.clone(),
        config_version: version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_is_owned_by_window_and_request() {
        let state = TranslationRequests::default();
        let (registration, guard) = state.begin("main", "one").unwrap();
        assert!(state.begin("main", "two").is_err());
        assert!(!state.cancel("translate-panel", "one").unwrap());
        assert!(!state.cancel("main", "other").unwrap());
        assert!(state.cancel("main", "one").unwrap());
        drop(registration);
        drop(guard);
        assert!(state.begin("main", "next").is_ok());
    }
    #[test]
    fn old_guard_does_not_remove_new_request() {
        let state = TranslationRequests::default();
        let (_, old) = state.begin("main", "old").unwrap();
        state.cancel("main", "old").unwrap();
        let (_, current) = state.begin("main", "new").unwrap();
        drop(old);
        assert!(state.cancel("main", "new").unwrap());
        drop(current);
    }
    #[test]
    fn abort_drops_inflight_work_and_stops_queued_segments() {
        use std::{
            future::{pending, Future},
            task::{Context, Poll},
        };
        struct Dropped(Arc<AtomicUsize>);
        impl Drop for Dropped {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }
        let submitted = Arc::new(AtomicUsize::new(0));
        let dropped = Arc::new(AtomicUsize::new(0));
        let (handle, registration) = AbortHandle::new_pair();
        let work = stream::iter(0..10)
            .map(|_| {
                let submitted = submitted.clone();
                let dropped = dropped.clone();
                async move {
                    submitted.fetch_add(1, Ordering::SeqCst);
                    let _drop = Dropped(dropped);
                    pending::<()>().await;
                }
            })
            .buffer_unordered(1)
            .collect::<Vec<_>>();
        {
            let mut task = Box::pin(Abortable::new(work, registration));
            let waker = futures_util::task::noop_waker();
            let mut context = Context::from_waker(&waker);
            assert!(task.as_mut().poll(&mut context).is_pending());
            handle.abort();
            assert!(matches!(
                task.as_mut().poll(&mut context),
                Poll::Ready(Err(_))
            ));
        }
        assert_eq!(submitted.load(Ordering::SeqCst), 1);
        assert_eq!(dropped.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn retry_version_changes_with_credentials_or_prompt() {
        let mut config = TranslateConfig::default();
        let original = config_version(&config).unwrap();
        config.deepseek.api_key = "synthetic-key".into();
        let changed = config_version(&config).unwrap();
        assert_ne!(original, changed);
        assert!(!changed.contains("synthetic-key"));
        config.deepseek.user_prompt.push_str(" preserve names");
        assert_ne!(changed, config_version(&config).unwrap());
    }
}
