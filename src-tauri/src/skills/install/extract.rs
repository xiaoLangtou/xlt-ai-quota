use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use uuid::Uuid;
use zip::ZipArchive;

use super::security::{
    MAX_COMPRESSION_RATIO, MAX_DEPTH, MAX_EXTRACTED_BYTES, MAX_FILE_BYTES, MAX_FILE_COUNT,
    MAX_PACKAGE_BYTES,
};
use super::super::error::SkillsError;
use super::super::paths::{is_inside, normalize, safe_join};
use super::super::trash::symlink;

/// 归档魔数嗅探。
pub fn looks_like_zip(buffer: &[u8]) -> bool {
    buffer.len() > 4 && buffer[0] == 0x50 && buffer[1] == 0x4b
}

pub fn looks_like_gzip(buffer: &[u8]) -> bool {
    buffer.len() > 2 && buffer[0] == 0x1f && buffer[1] == 0x8b
}

fn assert_archive_size(bytes: u64) -> Result<(), SkillsError> {
    if bytes > MAX_PACKAGE_BYTES {
        return Err(SkillsError::new(
            413,
            format!("压缩包过大，上限 {}MB", MAX_PACKAGE_BYTES / 1024 / 1024),
        ));
    }
    Ok(())
}

fn assert_depth(rel_path: &str) -> Result<(), SkillsError> {
    let depth = rel_path.split('/').filter(|part| !part.is_empty()).count();
    if depth > MAX_DEPTH {
        return Err(SkillsError::new(
            413,
            format!("目录层级过深（>{MAX_DEPTH}）：{rel_path}"),
        ));
    }
    Ok(())
}

/// 共享的解压累计器：总量 / 数量 / 压缩比。
struct ExtractBudget {
    total_bytes: u64,
    file_count: usize,
    archive_bytes: u64,
}

impl ExtractBudget {
    fn new(archive_bytes: u64) -> Self {
        Self {
            total_bytes: 0,
            file_count: 0,
            archive_bytes,
        }
    }

    fn add_file(&mut self) -> Result<(), SkillsError> {
        self.file_count += 1;
        if self.file_count > MAX_FILE_COUNT {
            return Err(SkillsError::new(
                413,
                format!("文件数量超限（>{MAX_FILE_COUNT}）"),
            ));
        }
        Ok(())
    }

    fn add_bytes(&mut self, bytes: u64) -> Result<(), SkillsError> {
        self.total_bytes += bytes;
        if self.total_bytes > MAX_EXTRACTED_BYTES {
            return Err(SkillsError::new(
                413,
                format!(
                    "解压后总大小超限（>{}MB）",
                    MAX_EXTRACTED_BYTES / 1024 / 1024
                ),
            ));
        }
        if self.archive_bytes > 0
            && self.total_bytes > self.archive_bytes * MAX_COMPRESSION_RATIO
        {
            return Err(SkillsError::new(
                413,
                format!("压缩比异常（>{MAX_COMPRESSION_RATIO}:1），疑似压缩炸弹"),
            ));
        }
        Ok(())
    }
}

/// 解压 ZIP（逐 entry 写入，safeJoin 防 Zip Slip）。
pub fn extract_zip_file(archive: &Path, dest: &Path) -> Result<(), SkillsError> {
    let archive_bytes = fs::metadata(archive)?.len();
    assert_archive_size(archive_bytes)?;
    let file = File::open(archive)?;
    let mut zip =
        ZipArchive::new(file).map_err(|error| SkillsError::bad_request(format!("ZIP 解析失败：{error}")))?;
    let mut budget = ExtractBudget::new(archive_bytes);
    for index in 0..zip.len() {
        let mut entry = zip
            .by_index(index)
            .map_err(|error| SkillsError::bad_request(format!("ZIP 解析失败：{error}")))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_owned();
        assert_depth(&name)?;
        budget.add_file()?;
        let size = entry.size();
        if size > MAX_FILE_BYTES {
            return Err(SkillsError::new(
                413,
                format!(
                    "单文件过大（>{}MB）：{name}",
                    MAX_FILE_BYTES / 1024 / 1024
                ),
            ));
        }
        budget.add_bytes(size)?;
        let out_path = safe_join(dest, &name)?;
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = File::create(&out_path)?;
        io::copy(&mut entry, &mut out)?;
    }
    Ok(())
}

/// 解压 tar.gz（entry 层累计限制；拒绝设备 / FIFO / 硬链接 / 外部软链接）。
pub fn extract_tar_gz_file(archive: &Path, dest: &Path) -> Result<(), SkillsError> {
    let archive_bytes = fs::metadata(archive)?.len();
    assert_archive_size(archive_bytes)?;
    let file = File::open(archive)?;
    let mut archive = tar::Archive::new(GzDecoder::new(file));
    let mut budget = ExtractBudget::new(archive_bytes);
    let result = extract_tar_entries(&mut archive, dest, &mut budget);
    if result.is_err() {
        // 已落盘的部分内容一并清理，避免半成品残留。
        let _ = fs::remove_dir_all(dest);
        let _ = fs::create_dir_all(dest);
    }
    result
}

fn extract_tar_entries<R: Read>(
    archive: &mut tar::Archive<R>,
    dest: &Path,
    budget: &mut ExtractBudget,
) -> Result<(), SkillsError> {
    let entries = archive
        .entries()
        .map_err(|error| SkillsError::bad_request(format!("tar 解析失败：{error}")))?;
    for entry in entries {
        let mut entry = entry.map_err(|error| SkillsError::bad_request(format!("tar 解析失败：{error}")))?;
        let entry_type = entry.header().entry_type();
        let path = entry
            .path()
            .map_err(|error| SkillsError::bad_request(format!("tar 条目路径非法：{error}")))?
            .into_owned();
        let rel_path = path.to_string_lossy().replace('\\', "/");
        assert_depth(&rel_path)?;
        let out_path = safe_join(dest, &rel_path)?;

        if entry_type.is_dir() {
            fs::create_dir_all(&out_path)?;
            continue;
        }
        if entry_type.is_symlink() {
            let link = entry
                .link_name()
                .map_err(|error| SkillsError::bad_request(format!("tar 软链接非法：{error}")))?
                .ok_or_else(|| SkillsError::bad_request(format!("tar 软链接缺少目标：{rel_path}")))?;
            if link.is_absolute() {
                return Err(SkillsError::bad_request(format!(
                    "压缩包包含指向外部的软链接：{rel_path}"
                )));
            }
            let base = out_path.parent().unwrap_or(dest).to_path_buf();
            let resolved = normalize(&base.join(&link));
            if !is_inside(dest, &resolved) {
                return Err(SkillsError::bad_request(format!(
                    "压缩包包含指向外部的软链接：{rel_path} → {}",
                    link.to_string_lossy()
                )));
            }
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }
            symlink(&link, &out_path)?;
            continue;
        }
        if entry_type.is_file() {
            budget.add_file()?;
            let size = entry.header().size().unwrap_or(0);
            if size > MAX_FILE_BYTES {
                return Err(SkillsError::new(
                    413,
                    format!(
                        "单文件过大（>{}MB）：{rel_path}",
                        MAX_FILE_BYTES / 1024 / 1024
                    ),
                ));
            }
            budget.add_bytes(size)?;
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut out = File::create(&out_path)?;
            io::copy(&mut entry, &mut out)?;
            continue;
        }
        return Err(SkillsError::bad_request(format!(
            "压缩包包含不允许的条目类型（{entry_type:?}）：{rel_path}"
        )));
    }
    Ok(())
}

/// 将内存中的归档 buffer 落盘为临时文件后解压。
pub fn extract_archive_buffer(
    buffer: &[u8],
    kind: &str,
    dest_dir: &Path,
) -> Result<(), SkillsError> {
    assert_archive_size(buffer.len() as u64)?;
    let tmp_file: PathBuf = dest_dir.join(format!(".archive-{}", Uuid::new_v4()));
    fs::write(&tmp_file, buffer)?;
    let result = if kind == "zip" {
        extract_zip_file(&tmp_file, dest_dir)
    } else {
        extract_tar_gz_file(&tmp_file, dest_dir)
    };
    let _ = fs::remove_file(&tmp_file);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("xlt-extract-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn sniffs_magic_bytes() {
        assert!(looks_like_zip(&[0x50, 0x4b, 0x03, 0x04, 0x00]));
        assert!(looks_like_gzip(&[0x1f, 0x8b, 0x08]));
        assert!(!looks_like_zip(b"hello world"));
    }

    #[test]
    fn extracts_zip_and_rejects_zip_slip() {
        let dir = temp_dir();
        let archive = dir.join("ok.zip");
        {
            let file = File::create(&archive).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            zip.start_file("skill/SKILL.md", zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"---\nname: demo\n---\n").unwrap();
            zip.finish().unwrap();
        }
        let dest = dir.join("dest");
        fs::create_dir_all(&dest).unwrap();
        extract_zip_file(&archive, &dest).unwrap();
        assert!(dest.join("skill/SKILL.md").exists());

        let evil = dir.join("evil.zip");
        {
            let file = File::create(&evil).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            zip.start_file("../escape.md", zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"x").unwrap();
            zip.finish().unwrap();
        }
        let dest2 = dir.join("dest2");
        fs::create_dir_all(&dest2).unwrap();
        assert!(extract_zip_file(&evil, &dest2).is_err());
    }
}
