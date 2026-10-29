//! 文件链路：版本哈希、字节范围补丁（B04）、安全写入（B06）、备份（B07）、zsh 语法校验。
//!
//! 设计要点：
//! - 补丁只改动目标行的字节范围，未修改区域（含换行风格、行尾注释、末尾换行缺失）原样保留；
//! - 临时文件使用 UUID 唯一名 + `create_new` 排他创建 + 收紧权限，写入前有大小预算；
//! - `zsh -n` 为固定校验程序，stderr 有 64 KiB 上限、总时长 5 秒预算；
//! - 备份使用 UUID 命名避免碰撞，保留 5 份，清理发生在提交完成（rename）之后。

use crate::shellenv::parser::{detect_line_ending, split_lines};
use crate::shellenv::types::{MAX_FILE_BYTES, ShellBackup};
use chrono::{Local, TimeZone};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use uuid::Uuid;

pub const VALIDATE_BUDGET: Duration = Duration::from_secs(5);
pub const VALIDATE_OUTPUT_CAP: u64 = 64 * 1024;
pub const BACKUP_KEEP: usize = 5;

pub fn home_dir() -> Result<PathBuf, String> {
    dirs::home_dir().ok_or_else(|| "无法定位用户主目录".to_string())
}

pub fn alias_path() -> Result<PathBuf, String> {
    Ok(home_dir()?.join(".alias"))
}

pub fn zshrc_path() -> Result<PathBuf, String> {
    Ok(home_dir()?.join(".zshrc"))
}

pub fn kind_path(kind: &str) -> Result<PathBuf, String> {
    match kind {
        crate::shellenv::types::KIND_ALIAS => alias_path(),
        crate::shellenv::types::KIND_ZSHRC => zshrc_path(),
        other => Err(format!("未知的配置文件类型: {other}")),
    }
}

/// 路径白名单：仅允许两个受管配置文件。
pub fn allowed_config_path(path: &str) -> Result<PathBuf, String> {
    let requested = PathBuf::from(path);
    if requested == alias_path()? || requested == zshrc_path()? {
        Ok(requested)
    } else {
        Err("仅允许访问受管理的配置文件".to_string())
    }
}

pub fn content_version(content: &str) -> String {
    let digest = Sha256::digest(content.as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// 读取文件原始内容；不存在时返回 None。
pub fn read_raw(path: &Path) -> Result<Option<String>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read(path).map_err(|error| format!("读取 {} 失败: {error}", path.display()))?;
    if raw.len() > MAX_FILE_BYTES {
        return Err(format!("{} 超过单文件 {} 字节预算", path.display(), MAX_FILE_BYTES));
    }
    String::from_utf8(raw)
        .map(Some)
        .map_err(|_| format!("{} 不是有效的 UTF-8 文本", path.display()))
}

// ---------------------------------------------------------------------------
// 字节范围补丁（B04）
// ---------------------------------------------------------------------------

/// 行级编辑；索引均为 0-based，指向 `split_lines` 的下标。
#[derive(Clone, Debug)]
pub enum LineEdit {
    /// 替换行文本；保留原行的终止符状态（末行无换行则仍无换行）
    Replace { index: usize, text: String },
    /// 删除整行（连同终止符）
    Delete { index: usize },
    /// 在指定行之后插入新行；若该行无终止符，先补一个终止符
    InsertAfter { index: usize, text: String },
    /// 文件末尾追加；`blank_before` 为真时先补一个空行
    Append { text: String, blank_before: bool },
}

/// 应用行级编辑并产出整文件后像：未修改行按原字节复制，换行风格保持一致。
pub fn apply_edits(raw: &str, edits: &[LineEdit]) -> Result<String, String> {
    let lines = split_lines(raw);
    let term = detect_line_ending(raw);
    let mut out = String::with_capacity(raw.len() + 128);
    for (index, line) in lines.iter().enumerate() {
        let mut replaced: Option<&str> = None;
        let mut deleted = false;
        for edit in edits {
            match edit {
                LineEdit::Replace { index: i, text } if *i == index => replaced = Some(text),
                LineEdit::Delete { index: i } if *i == index => deleted = true,
                _ => {}
            }
        }
        if deleted {
            continue;
        }
        match replaced {
            Some(text) => {
                out.push_str(text);
                if line.has_terminator() {
                    out.push_str(line.terminator(raw));
                }
            }
            None => out.push_str(&raw[line.start..line.term_end]),
        }
        for edit in edits {
            if let LineEdit::InsertAfter { index: i, text } = edit {
                if *i == index {
                    if !line.has_terminator() {
                        out.push_str(term);
                    }
                    out.push_str(text);
                    out.push_str(term);
                }
            }
        }
    }
    for edit in edits {
        if let LineEdit::Append { text, blank_before } = edit {
            if !raw.is_empty() {
                // 先确保文件以换行结束；blank_before 再补一个空行分隔
                if !out.ends_with('\n') {
                    out.push_str(term);
                }
                if *blank_before {
                    out.push_str(term);
                }
            }
            out.push_str(text);
            out.push_str(term);
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// 语法校验（固定程序 + 预算）
// ---------------------------------------------------------------------------

/// `zsh -n` 校验：stderr 落临时文件避免管道死锁，读取上限 64 KiB，总时长 5 秒。
pub fn validate_with_zsh(path: &Path) -> Result<(), String> {
    let err_path = std::env::temp_dir().join(format!("xlt-zshn-{}", Uuid::new_v4()));
    let result = (|| -> Result<(), String> {
        let err_file = fs::File::create(&err_path)
            .map_err(|error| format!("创建校验输出文件失败: {error}"))?;
        let mut child = Command::new("zsh")
            .arg("-n")
            .arg(path)
            .stdout(Stdio::null())
            .stderr(Stdio::from(err_file))
            .spawn()
            .map_err(|error| format!("无法执行 zsh -n（请确认 zsh 已安装）: {error}"))?;
        let started = Instant::now();
        let status = loop {
            if let Ok(Some(status)) = child.try_wait() {
                break Ok(status);
            }
            if started.elapsed() > VALIDATE_BUDGET {
                let _ = child.kill();
                let _ = child.wait();
                break Err("zsh -n 校验超过 5 秒预算，已终止".to_string());
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        match status {
            Ok(status) if status.success() => Ok(()),
            Ok(_) => {
                let mut stderr = Vec::new();
                if let Ok(file) = fs::File::open(&err_path) {
                    let _ = file.take(VALIDATE_OUTPUT_CAP).read_to_end(&mut stderr);
                }
                let message = String::from_utf8_lossy(&stderr);
                Err(format!(
                    "zsh 语法校验未通过: {}",
                    message.trim().lines().next().unwrap_or("未知语法错误")
                ))
            }
            Err(message) => Err(message),
        }
    })();
    let _ = fs::remove_file(&err_path);
    result
}

/// 校验一段内容：写入目标同目录临时文件后执行 `zsh -n`。
pub fn validate_content(target: &Path, content: &str) -> Result<(), String> {
    let tmp = temp_path(target)?;
    write_temp(&tmp, content)?;
    let result = validate_with_zsh(&tmp);
    let _ = fs::remove_file(&tmp);
    result
}

// ---------------------------------------------------------------------------
// 临时文件与安全写入（B06）
// ---------------------------------------------------------------------------

fn temp_path(target: &Path) -> Result<PathBuf, String> {
    let name = target
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .trim_start_matches('.');
    Ok(target.with_file_name(format!("{name}.xlt-{}", Uuid::new_v4())))
}

fn write_temp(tmp: &Path, content: &str) -> Result<(), String> {
    if content.len() > MAX_FILE_BYTES {
        return Err(format!("内容超过单文件 {} 字节预算", MAX_FILE_BYTES));
    }
    // create_new 排他创建；权限收紧为 0600（unix），rename 前恢复原文件权限
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(tmp)
        .map_err(|error| format!("创建临时文件失败: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("设置临时文件权限失败: {error}"))?;
    }
    fs::write(tmp, content).map_err(|error| format!("写入临时文件失败: {error}"))
}

fn restore_permissions(tmp: &Path, target: &Path) {
    #[cfg(unix)]
    if let Ok(meta) = fs::metadata(target) {
        use std::os::unix::fs::PermissionsExt;
        let mode = meta.permissions().mode();
        let _ = fs::set_permissions(tmp, fs::Permissions::from_mode(mode));
    }
}

/// 备份当前文件（UUID 命名，不清理）；返回备份文件名。
pub fn backup_file(target: &Path) -> Result<Option<String>, String> {
    if !target.exists() {
        return Ok(None);
    }
    let name = target
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "目标路径无效".to_string())?;
    let backup_name = format!("{name}.bak-{}", Uuid::new_v4());
    fs::copy(target, target.with_file_name(&backup_name))
        .map_err(|error| format!("备份失败: {error}"))?;
    Ok(Some(backup_name))
}

/// 清理旧备份，保留最近 BACKUP_KEEP 份（在提交完成之后调用）。
pub fn prune_backups(target: &Path) {
    let Ok(dir) = fs::read_dir(target.parent().unwrap_or(Path::new("/"))) else {
        return;
    };
    let prefix = format!(
        "{}.bak-",
        target.file_name().and_then(|n| n.to_str()).unwrap_or("file")
    );
    let mut backups: Vec<(PathBuf, Option<std::time::SystemTime>)> = dir
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with(&prefix)))
        .map(|p| {
            let modified = fs::metadata(&p).ok().and_then(|m| m.modified().ok());
            (p, modified)
        })
        .collect();
    backups.sort_by_key(|(_, modified)| *modified);
    while backups.len() > BACKUP_KEEP {
        let (oldest, _) = backups.remove(0);
        let _ = fs::remove_file(oldest);
    }
}

/// 提交写入：写临时文件 → `zsh -n` → 备份当前文件 → 原子 rename → 清理旧备份。
/// 备份清理发生在 rename 成功之后（B07）。
pub fn commit_write(target: &Path, content: &str) -> Result<(), String> {
    let tmp = temp_path(target)?;
    write_temp(&tmp, content)?;
    if let Err(error) = validate_with_zsh(&tmp) {
        let _ = fs::remove_file(&tmp);
        return Err(error);
    }
    backup_file(target)?;
    restore_permissions(&tmp, target);
    if let Err(error) = fs::rename(&tmp, target) {
        let _ = fs::remove_file(&tmp);
        return Err(format!("替换配置文件失败: {error}"));
    }
    prune_backups(target);
    Ok(())
}

// ---------------------------------------------------------------------------
// 备份列表与恢复（B08）
// ---------------------------------------------------------------------------

const BACKUP_ID_PATTERN: &str = r"^\.(alias|zshrc)\.bak-[0-9a-fA-F-]{36}$";

/// 列出两个配置文件的备份（按修改时间倒序）。
pub fn list_backups() -> Result<Vec<ShellBackup>, String> {
    let home = home_dir()?;
    let pattern = regex::Regex::new(BACKUP_ID_PATTERN).expect("valid backup pattern");
    let mut backups = Vec::new();
    for entry in fs::read_dir(&home).map_err(|error| error.to_string())? {
        let Ok(entry) = entry else { continue };
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !pattern.is_match(name) {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let file = if name.starts_with(".alias.") { "alias" } else { "zshrc" };
        let created_at = meta
            .modified()
            .ok()
            .and_then(|time| {
                let secs: i64 = time
                    .duration_since(std::time::UNIX_EPOCH)
                    .ok()?
                    .as_secs()
                    .try_into()
                    .ok()?;
                Local.timestamp_opt(secs, 0).single()
            })
            .map(|time| time.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "未知时间".to_string());
        backups.push(ShellBackup {
            id: name.to_string(),
            file: file.to_string(),
            created_at,
            size: meta.len(),
        });
    }
    backups.sort_by(|a, b| b.id.cmp(&a.id));
    Ok(backups)
}

/// 读取指定备份的内容（B08：preview 时固定后像，不依赖恢复时刻的备份状态之外的因素）。
pub fn read_backup(id: &str) -> Result<(String, String), String> {
    let pattern = regex::Regex::new(BACKUP_ID_PATTERN).expect("valid backup pattern");
    if !pattern.is_match(id) {
        return Err("非法的备份标识".to_string());
    }
    let kind = if id.starts_with(".alias.") {
        crate::shellenv::types::KIND_ALIAS
    } else {
        crate::shellenv::types::KIND_ZSHRC
    };
    let source = home_dir()?.join(id);
    let raw = fs::read(&source).map_err(|error| format!("读取备份失败: {error}"))?;
    if raw.len() > MAX_FILE_BYTES {
        return Err("备份内容超过单文件预算".to_string());
    }
    let content = String::from_utf8(raw).map_err(|_| "备份不是有效的 UTF-8 文本".to_string())?;
    Ok((kind.to_string(), content))
}
