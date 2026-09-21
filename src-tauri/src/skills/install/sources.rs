use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::download::download;
use super::extract::{
    extract_archive_buffer, extract_tar_gz_file, extract_zip_file, looks_like_gzip, looks_like_zip,
};
use super::github::{fetch_github_to_dir, resolve_github_location};
use super::inspector::{inspect_staging_content, InspectedSkill};
use super::records::SourceMetadata;
use super::staging::{
    create_staging, remove_staging, set_staging_status, staging_content_dir, update_staging,
    StagingRecord, StagingRegistry,
};
use super::super::config::SkillsPaths;
use super::super::error::SkillsError;
use super::super::paths::{is_inside, is_valid_skill_name, safe_join};
use super::super::skillssh::download_skills_sh;
use super::super::trash::copy_dir_all;
use super::super::validation::parse_skill_md;

/// 准备来源接口（local/upload/remote）统一响应。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareSourceResponse {
    pub staging_id: String,
    pub expires_at: String,
    pub source: SourceMetadata,
    pub skills: Vec<InspectedSkill>,
}

/// 检查候选并置 ready，产出统一响应。
fn finalize(
    paths: &SkillsPaths,
    registry: &StagingRegistry,
    record: &StagingRecord,
    search_subpath: Option<String>,
) -> Result<PrepareSourceResponse, SkillsError> {
    let content_dir = staging_content_dir(paths, &record.id);
    if let Some(subpath) = &search_subpath {
        update_staging(paths, registry, &record.id, None, Some(subpath.clone()))?;
    }
    let search_root = match &search_subpath {
        Some(subpath) => Some(safe_join(&content_dir, subpath)?),
        None => None,
    };
    let skills = inspect_staging_content(&content_dir, search_root.as_deref())?;
    let ready = set_staging_status(paths, registry, &record.id, "ready")?;
    Ok(PrepareSourceResponse {
        staging_id: ready.id,
        expires_at: ready.expires_at,
        source: ready.source,
        skills,
    })
}

fn finish_or_cleanup(
    paths: &SkillsPaths,
    registry: &StagingRegistry,
    record: &StagingRecord,
    fill: Result<Option<String>, SkillsError>,
) -> Result<PrepareSourceResponse, SkillsError> {
    match fill {
        Ok(search_subpath) => match finalize(paths, registry, record, search_subpath) {
            Ok(response) => Ok(response),
            Err(error) => {
                remove_staging(paths, registry, &record.id);
                Err(error)
            }
        },
        Err(error) => {
            remove_staging(paths, registry, &record.id);
            Err(error)
        }
    }
}

fn real_or_original(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// 本地目录准备：复制进 staging 快照。
pub fn prepare_local_source(
    paths: &SkillsPaths,
    registry: &StagingRegistry,
    source_path: &str,
) -> Result<PrepareSourceResponse, SkillsError> {
    let input = source_path.trim();
    if input.is_empty() {
        return Err(SkillsError::bad_request("缺少 sourcePath"));
    }
    let source = Path::new(input);
    if !source.exists() || !source.is_dir() {
        return Err(SkillsError::bad_request("源路径不存在或不是目录"));
    }
    let real = real_or_original(source);
    for guarded in [paths.staging_dir.as_path(), paths.config_dir.as_path()] {
        let guard = real_or_original(guarded);
        if is_inside(&guard, &real) || is_inside(&real, &guard) {
            return Err(SkillsError::bad_request(format!(
                "源目录不可位于工具内部目录：{}",
                real.to_string_lossy()
            )));
        }
    }

    let metadata = SourceMetadata {
        source_type: "local".to_owned(),
        reference: real.to_string_lossy().to_string(),
        repository: None,
        revision: None,
        branch: None,
        subpath: None,
    };
    let record = create_staging(paths, registry, metadata)?;
    let content_dir = staging_content_dir(paths, &record.id);
    let fill = copy_dir_all(&real, &content_dir).map(|_| None);
    finish_or_cleanup(paths, registry, &record, fill)
}

/// 读文件头嗅探归档类型，扩展名兜底。
fn sniff_archive_kind(archive_path: &Path, filename: &str) -> Result<&'static str, SkillsError> {
    let mut head = [0u8; 8];
    let read = fs::File::open(archive_path)
        .and_then(|mut file| file.read(&mut head))
        .unwrap_or(0);
    let lower = filename.to_lowercase();
    if looks_like_zip(&head[..read]) || lower.ends_with(".zip") {
        return Ok("zip");
    }
    if looks_like_gzip(&head[..read]) || lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
        return Ok("tar.gz");
    }
    Err(SkillsError::new(415, "仅支持 .zip 或 .tar.gz 归档"))
}

/// 上传归档准备：限额解压进 staging。
pub fn prepare_upload_source(
    paths: &SkillsPaths,
    registry: &StagingRegistry,
    archive_path: &str,
    filename: &str,
) -> Result<PrepareSourceResponse, SkillsError> {
    let path = Path::new(archive_path);
    if !path.exists() || !path.is_file() {
        return Err(SkillsError::bad_request("归档文件不存在"));
    }
    let kind = sniff_archive_kind(path, filename)?;
    let reference = if filename.is_empty() {
        path.file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default()
    } else {
        filename.to_owned()
    };
    let metadata = SourceMetadata {
        source_type: "archive".to_owned(),
        reference,
        repository: None,
        revision: None,
        branch: None,
        subpath: None,
    };
    let record = create_staging(paths, registry, metadata)?;
    let content_dir = staging_content_dir(paths, &record.id);
    let fill = if kind == "zip" {
        extract_zip_file(path, &content_dir).map(|_| None)
    } else {
        extract_tar_gz_file(path, &content_dir).map(|_| None)
    };
    finish_or_cleanup(paths, registry, &record, fill)
}

/// 远程来源准备：github / skills-sh / http / market。
pub async fn prepare_remote_source(
    paths: &SkillsPaths,
    registry: &StagingRegistry,
    source: &str,
    reference: &str,
) -> Result<PrepareSourceResponse, SkillsError> {
    let reference = reference.trim();
    if reference.is_empty() {
        return Err(SkillsError::bad_request("缺少 ref"));
    }

    if source == "github" {
        let location = resolve_github_location(reference).await?;
        let metadata = SourceMetadata {
            source_type: "github".to_owned(),
            reference: reference.to_owned(),
            repository: Some(format!("{}/{}", location.owner, location.repo)),
            branch: location.revision.clone(),
            subpath: location.subpath.clone(),
            revision: None,
        };
        let record = create_staging(paths, registry, metadata.clone())?;
        let content_dir = staging_content_dir(paths, &record.id);
        match fetch_github_to_dir(&location, &content_dir).await {
            Ok(result) => {
                let updated = SourceMetadata {
                    branch: result.revision.clone().or(metadata.branch),
                    revision: result.commit_sha.clone(),
                    ..metadata
                };
                update_staging(paths, registry, &record.id, Some(updated), None)?;
                finalize(paths, registry, &record, result.search_subpath)
            }
            Err(error) => {
                remove_staging(paths, registry, &record.id);
                Err(error)
            }
        }
    } else if source == "skills-sh" {
        let metadata = SourceMetadata {
            source_type: "skills-sh".to_owned(),
            reference: reference.to_owned(),
            repository: None,
            revision: None,
            branch: None,
            subpath: None,
        };
        let record = create_staging(paths, registry, metadata)?;
        let content_dir = staging_content_dir(paths, &record.id);
        match download_skills_sh(reference, &content_dir).await {
            Ok(dir_name) => finish_or_cleanup(paths, registry, &record, Ok(Some(dir_name))),
            Err(error) => finish_or_cleanup(paths, registry, &record, Err(error)),
        }
    } else {
        if !reference.to_lowercase().starts_with("http://") && !reference.to_lowercase().starts_with("https://") {
            return Err(SkillsError::bad_request(format!(
                "{source} 来源需要以 http(s):// 开头的 URL"
            )));
        }
        let source_type = if source == "market" { "market" } else { "http" };
        let metadata = SourceMetadata {
            source_type: source_type.to_owned(),
            reference: reference.to_owned(),
            repository: None,
            revision: None,
            branch: None,
            subpath: None,
        };
        let record = create_staging(paths, registry, metadata)?;
        let content_dir = staging_content_dir(paths, &record.id);
        let lower = reference.to_lowercase();
        let fill = async {
            let result = download(reference).await?;
            if looks_like_zip(&result.buffer)
                || lower.ends_with(".zip")
                || result.content_type.contains("zip")
            {
                extract_archive_buffer(&result.buffer, "zip", &content_dir)?;
                return Ok(None);
            }
            if looks_like_gzip(&result.buffer)
                || lower.ends_with(".tar.gz")
                || lower.ends_with(".tgz")
                || result.content_type.contains("gzip")
            {
                extract_archive_buffer(&result.buffer, "tar.gz", &content_dir)?;
                return Ok(None);
            }
            let text = String::from_utf8_lossy(&result.buffer).to_string();
            let name = parse_skill_md(&text)
                .ok()
                .and_then(|parsed| {
                    parsed
                        .frontmatter
                        .as_object()
                        .and_then(|map| map.get("name"))
                        .and_then(|value| value.as_str())
                        .filter(|value| is_valid_skill_name(value))
                        .map(ToOwned::to_owned)
                })
                .unwrap_or_else(|| "downloaded-skill".to_owned());
            let skill_dir = safe_join(&content_dir, &name)?;
            fs::create_dir_all(&skill_dir)?;
            fs::write(skill_dir.join("SKILL.md"), text)?;
            Ok(None)
        }
        .await;
        finish_or_cleanup(paths, registry, &record, fill)
    }
}
