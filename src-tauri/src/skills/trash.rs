use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use super::config::{get_root, SkillRoot, SkillsPaths};
use super::error::SkillsError;
use super::scan::dir_size;

/// 回收站条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashItem {
    pub id: String,
    pub name: String,
    pub root_id: String,
    pub root_label: String,
    pub original_path: String,
    pub deleted_at: String,
    pub size_bytes: u64,
    pub was_symlink: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symlink_target: Option<String>,
}

fn trash_dir(paths: &SkillsPaths) -> PathBuf {
    paths.config_dir.join("trash")
}

fn trash_meta_file(paths: &SkillsPaths) -> PathBuf {
    paths.config_dir.join("trash.json")
}

fn read_meta(paths: &SkillsPaths) -> Vec<TrashItem> {
    fs::read_to_string(trash_meta_file(paths))
        .ok()
        .and_then(|raw| serde_json::from_str::<Vec<TrashItem>>(&raw).ok())
        .unwrap_or_default()
}

fn write_meta(paths: &SkillsPaths, items: &[TrashItem]) -> Result<(), SkillsError> {
    fs::create_dir_all(&paths.config_dir)?;
    let text = serde_json::to_string_pretty(items)
        .map_err(|error| SkillsError::internal(error.to_string()))?;
    fs::write(trash_meta_file(paths), text)?;
    Ok(())
}

fn new_trash_id(original_path: &Path) -> String {
    let seed = format!(
        "{}:{}:{}",
        original_path.to_string_lossy(),
        chrono::Utc::now().timestamp_millis(),
        uuid::Uuid::new_v4()
    );
    let digest = Sha1::digest(seed.as_bytes());
    format!("{digest:x}").chars().take(12).collect()
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// 将 skill 移入回收站（普通目录整体搬迁；软链接仅删除链接本身）。
pub fn move_to_trash(
    paths: &SkillsPaths,
    root: &SkillRoot,
    name: &str,
    dir_path: &Path,
) -> Result<TrashItem, SkillsError> {
    let metadata = fs::symlink_metadata(dir_path)?;
    let was_symlink = metadata.file_type().is_symlink();
    let id = new_trash_id(dir_path);

    let mut size_bytes = 0;
    let mut symlink_target = None;

    if was_symlink {
        symlink_target = fs::read_link(dir_path)
            .ok()
            .map(|target| target.to_string_lossy().to_string());
        fs::remove_file(dir_path)?;
    } else {
        size_bytes = dir_size(dir_path);
        let destination = trash_dir(paths).join(&id);
        fs::create_dir_all(trash_dir(paths))?;
        if fs::rename(dir_path, &destination).is_err() {
            copy_dir_all(dir_path, &destination)?;
            fs::remove_dir_all(dir_path)?;
        }
    }

    let item = TrashItem {
        id,
        name: name.to_owned(),
        root_id: root.id.clone(),
        root_label: root.label.clone(),
        original_path: dir_path.to_string_lossy().to_string(),
        deleted_at: now_iso(),
        size_bytes,
        was_symlink,
        symlink_target,
    };
    let mut items = read_meta(paths);
    items.insert(0, item.clone());
    write_meta(paths, &items)?;
    Ok(item)
}

pub fn list_trash(paths: &SkillsPaths) -> Vec<TrashItem> {
    read_meta(paths)
}

/// 从回收站恢复某个条目，返回其 rootId 与恢复后的名称。
pub fn restore_trash(paths: &SkillsPaths, id: &str) -> Result<(String, String), SkillsError> {
    let items = read_meta(paths);
    let item = items
        .iter()
        .find(|entry| entry.id == id)
        .cloned()
        .ok_or_else(|| SkillsError::not_found("回收站条目不存在"))?;

    let root = get_root(paths, &item.root_id)
        .ok_or_else(|| SkillsError::not_found("原根目录已不存在，无法恢复"))?;
    if !root.writable {
        return Err(SkillsError::forbidden("原根目录不可写，无法恢复"));
    }
    let original = PathBuf::from(&item.original_path);
    if original.exists() {
        return Err(SkillsError::conflict("目标位置已存在同名 skill"));
    }

    fs::create_dir_all(&root.path)?;
    if item.was_symlink {
        let target = item
            .symlink_target
            .as_ref()
            .ok_or_else(|| SkillsError::internal("软链接目标缺失，无法恢复"))?;
        symlink(target, &original)?;
    } else {
        let source = trash_dir(paths).join(id);
        if !source.exists() {
            return Err(SkillsError::internal("回收站数据缺失，无法恢复"));
        }
        if fs::rename(&source, &original).is_err() {
            copy_dir_all(&source, &original)?;
            fs::remove_dir_all(&source)?;
        }
    }

    let remaining: Vec<TrashItem> = items.into_iter().filter(|entry| entry.id != id).collect();
    write_meta(paths, &remaining)?;
    Ok((item.root_id, item.name))
}

/// 永久删除回收站中的某个条目。
pub fn purge_trash(paths: &SkillsPaths, id: &str) -> bool {
    let items = read_meta(paths);
    let Some(item) = items.iter().find(|entry| entry.id == id).cloned() else {
        return false;
    };
    if !item.was_symlink {
        let source = trash_dir(paths).join(id);
        if source.exists() {
            let _ = fs::remove_dir_all(&source);
        }
    }
    let remaining: Vec<TrashItem> = items.into_iter().filter(|entry| entry.id != id).collect();
    write_meta(paths, &remaining).is_ok()
}

/// 递归复制目录，保留软链接（等价 Node 的 `cpSync(..., { verbatimSymlinks: true })`）。
pub fn copy_dir_all(source: &Path, destination: &Path) -> Result<(), SkillsError> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)?.flatten() {
        let file_type = entry.file_type()?;
        let target = destination.join(entry.file_name());
        if file_type.is_symlink() {
            let link = fs::read_link(entry.path())?;
            symlink(link, target)?;
        } else if file_type.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// 创建软链接（跨平台封装）。
pub fn symlink(target: impl AsRef<Path>, link: impl AsRef<Path>) -> Result<(), SkillsError> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)?;
        Ok(())
    }
    #[cfg(windows)]
    {
        let target = target.as_ref();
        // 目录用 junction 语义，文件用文件符号链接。
        let result = if target.is_dir() {
            std::os::windows::fs::symlink_dir(target, link)
        } else {
            std::os::windows::fs::symlink_file(target, link)
        };
        result.map_err(SkillsError::from)
    }
}
