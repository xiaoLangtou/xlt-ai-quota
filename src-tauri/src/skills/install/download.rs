use std::time::Duration;

use futures_util::StreamExt;
use reqwest::redirect::Policy;
use url::Url;

use super::security::{assert_public_host_resolved, MAX_PACKAGE_BYTES};
use super::super::error::SkillsError;

const MAX_REDIRECTS: usize = 5;
/// 空闲超时：连接建立或数据块之间超过 30s 无进展才中断。
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);
/// 整体下载上限兜底。
const TOTAL_TIMEOUT: Duration = Duration::from_secs(5 * 60);

pub struct DownloadResult {
    pub buffer: Vec<u8>,
    pub content_type: String,
}

/// 安全下载器：手动重定向逐跳做字符串层 + DNS 层主机校验，流式读取并强制大小上限。
pub async fn download(url: &str) -> Result<DownloadResult, SkillsError> {
    match tokio::time::timeout(TOTAL_TIMEOUT, download_inner(url)).await {
        Ok(result) => result,
        Err(_) => Err(SkillsError::new(502, "下载超时（总时长超限）")),
    }
}

async fn download_inner(url: &str) -> Result<DownloadResult, SkillsError> {
    let client = reqwest::Client::builder()
        .redirect(Policy::none())
        .build()
        .map_err(|error| SkillsError::internal(error.to_string()))?;

    let mut current = url.to_owned();
    let mut response: Option<reqwest::Response> = None;
    for hop in 0..=MAX_REDIRECTS {
        assert_public_host_resolved(&current).await?;
        let resp = client
            .get(&current)
            .send()
            .await
            .map_err(|error| SkillsError::new(502, format!("下载失败：{error}")))?;
        if resp.status().is_redirection() {
            if hop == MAX_REDIRECTS {
                return Err(SkillsError::new(502, "重定向次数过多"));
            }
            let location = resp
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .map(ToOwned::to_owned);
            match location {
                Some(location) => {
                    current = Url::parse(&current)
                        .and_then(|base| base.join(&location))
                        .map_err(|_| SkillsError::new(502, format!("非法重定向地址：{location}")))?
                        .to_string();
                    continue;
                }
                None => {
                    response = Some(resp);
                    break;
                }
            }
        }
        response = Some(resp);
        break;
    }

    let resp = response.ok_or_else(|| SkillsError::new(502, "下载失败"))?;
    if !resp.status().is_success() {
        return Err(SkillsError::new(
            502,
            format!(
                "下载失败：{} {}",
                resp.status().as_u16(),
                resp.status().canonical_reason().unwrap_or("")
            ),
        ));
    }
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    if let Some(length) = resp.content_length() {
        if length > MAX_PACKAGE_BYTES {
            return Err(SkillsError::new(
                413,
                format!("文件过大，上限 {}MB", MAX_PACKAGE_BYTES / 1024 / 1024),
            ));
        }
    }

    let mut stream = resp.bytes_stream();
    let mut buffer: Vec<u8> = Vec::new();
    loop {
        let next = tokio::time::timeout(IDLE_TIMEOUT, stream.next())
            .await
            .map_err(|_| SkillsError::new(502, "下载超时（网络无响应）"))?;
        match next {
            None => break,
            Some(Err(error)) => {
                return Err(SkillsError::new(502, format!("下载失败：{error}")));
            }
            Some(Ok(chunk)) => {
                if buffer.len() as u64 + chunk.len() as u64 > MAX_PACKAGE_BYTES {
                    return Err(SkillsError::new(
                        413,
                        format!("文件过大，上限 {}MB", MAX_PACKAGE_BYTES / 1024 / 1024),
                    ));
                }
                buffer.extend_from_slice(&chunk);
            }
        }
    }
    Ok(DownloadResult {
        buffer,
        content_type,
    })
}
