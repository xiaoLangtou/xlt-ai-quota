use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;
use sha1::{Digest, Sha1};

use super::super::error::SkillsError;
use super::super::paths::{is_inside, relative_posix};
use super::super::validation::{validate_skill, ValidationIssue, ValidationLevel};

const SKILL_FILE: &str = "SKILL.md";
/// 候选搜索上限（防御异常目录树）。
const MAX_CANDIDATES: usize = 50;

const SCRIPT_EXTENSIONS: &[&str] = &[
    ".sh", ".bash", ".zsh", ".py", ".rb", ".pl", ".js", ".mjs", ".cjs", ".ts",
];
const BINARY_EXTENSIONS: &[&str] = &[
    ".exe", ".dll", ".so", ".dylib", ".bin", ".node", ".wasm", ".zip", ".tar", ".gz", ".tgz",
    ".7z", ".rar", ".png", ".jpg", ".jpeg", ".gif", ".webp", ".ico", ".pdf", ".woff", ".woff2",
    ".ttf",
];

/// 来源中检出的一个候选 Skill。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectedSkill {
    pub id: String,
    pub relative_path: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub content_hash: String,
    pub size_bytes: u64,
    pub file_count: usize,
    pub has_scripts: bool,
    pub has_binary_files: bool,
    pub issues: Vec<ValidationIssue>,
    pub installable: bool,
}

pub struct FileEntry {
    pub rel: String,
    pub abs: PathBuf,
    pub symlink: bool,
}

fn issue(level: ValidationLevel, code: &str, message: String, file: Option<String>, fix: Option<&str>) -> ValidationIssue {
    ValidationIssue {
        level,
        code: code.to_owned(),
        message,
        file,
        fix: fix.map(ToOwned::to_owned),
    }
}

fn extension_of(name: &str) -> String {
    match name.rfind('.') {
        Some(index) => name[index..].to_lowercase(),
        None => String::new(),
    }
}

/// 读取文件权限中的可执行位（非 unix 平台返回 0）。
fn executable_mode(metadata: &fs::Metadata) -> u32 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode()
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        0
    }
}

/// 内容嗅探：前 8KB 含 NUL 字节视为二进制。
fn sniff_binary(path: &Path) -> bool {
    let Ok(mut file) = File::open(path) else {
        return false;
    };
    let mut buffer = [0u8; 8192];
    match file.read(&mut buffer) {
        Ok(read) => buffer[..read].contains(&0),
        Err(_) => false,
    }
}

/// BFS 收集全部含 SKILL.md 的目录（skill 目录视为叶子，不再深入）。
pub fn find_skill_dirs(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut queue: VecDeque<PathBuf> = VecDeque::new();
    queue.push_back(root.to_path_buf());
    while let Some(dir) = queue.pop_front() {
        if found.len() >= MAX_CANDIDATES {
            break;
        }
        if dir.join(SKILL_FILE).is_file() {
            found.push(dir);
            continue;
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() && !kind.is_symlink() {
                queue.push_back(entry.path());
            }
        }
    }
    found
}

/// 递归收集目录下全部文件（相对 POSIX 路径）。
pub fn list_files(dir: &Path) -> Vec<FileEntry> {
    let mut out = Vec::new();
    collect_files(dir, "", &mut out);
    out
}

fn collect_files(dir: &Path, rel: &str, out: &mut Vec<FileEntry>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".DS_Store" {
            continue;
        }
        let rel_path = if rel.is_empty() {
            name.clone()
        } else {
            format!("{rel}/{name}")
        };
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            out.push(FileEntry {
                rel: rel_path,
                abs: entry.path(),
                symlink: true,
            });
        } else if kind.is_dir() {
            collect_files(&entry.path(), &rel_path, out);
        } else if kind.is_file() {
            out.push(FileEntry {
                rel: rel_path,
                abs: entry.path(),
                symlink: false,
            });
        }
    }
}

/// 计算目录内容哈希：按相对路径排序，逐个纳入「路径 + 内容」。
pub fn hash_dir(dir: &Path) -> String {
    let mut files = list_files(dir);
    files.sort_by(|a, b| a.rel.cmp(&b.rel));
    let mut hasher = Sha1::new();
    for file in files {
        hasher.update(file.rel.as_bytes());
        hasher.update(b"\0");
        if file.symlink {
            if let Ok(target) = fs::read_link(&file.abs) {
                hasher.update(format!("link:{}", target.to_string_lossy()).as_bytes());
            }
        } else if let Ok(bytes) = fs::read(&file.abs) {
            hasher.update(&bytes);
        }
        hasher.update(b"\0");
    }
    format!("{:x}", hasher.finalize()).chars().take(16).collect()
}

fn inspect_one(content_dir: &Path, skill_dir: &Path) -> InspectedSkill {
    let relative_path = relative_posix(content_dir, skill_dir);
    let dir_name = skill_dir
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "skill".to_owned());
    let validation = validate_skill(skill_dir, &dir_name);
    let mut issues = validation.issues;

    let files = list_files(skill_dir);
    let mut size_bytes = 0u64;
    let mut has_scripts = false;
    let mut has_binary_files = false;
    for file in &files {
        if file.symlink {
            let target = fs::read_link(&file.abs)
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_default();
            let absolute = Path::new(&target).is_absolute();
            let resolved = if absolute {
                PathBuf::from(&target)
            } else {
                file.abs.parent().unwrap_or(skill_dir).join(&target)
            };
            if target.is_empty() || absolute || !is_inside(skill_dir, &resolved) {
                issues.push(issue(
                    ValidationLevel::Error,
                    "external_symlink",
                    format!(
                        "包含指向 skill 目录外的软链接：{}{}",
                        file.rel,
                        if target.is_empty() {
                            String::new()
                        } else {
                            format!(" → {target}")
                        }
                    ),
                    Some(file.rel.clone()),
                    Some("移除该软链接或替换为实际文件"),
                ));
            } else {
                issues.push(issue(
                    ValidationLevel::Info,
                    "internal_symlink",
                    format!("包含目录内软链接：{}", file.rel),
                    Some(file.rel.clone()),
                    None,
                ));
            }
            continue;
        }
        let metadata = fs::metadata(&file.abs).ok();
        let size = metadata.as_ref().map(|meta| meta.len()).unwrap_or(0);
        let mode = metadata.as_ref().map(executable_mode).unwrap_or(0);
        size_bytes += size;
        let extension = extension_of(&file.rel);
        if SCRIPT_EXTENSIONS.contains(&extension.as_str()) || (mode & 0o111 != 0 && !file.rel.ends_with(".md")) {
            has_scripts = true;
        }
        if BINARY_EXTENSIONS.contains(&extension.as_str())
            || (!SCRIPT_EXTENSIONS.contains(&extension.as_str())
                && extension != ".md"
                && sniff_binary(&file.abs))
        {
            has_binary_files = true;
        }
    }
    if has_scripts {
        issues.push(issue(
            ValidationLevel::Warning,
            "has_scripts",
            "包含可执行脚本，安装前请确认来源可信".to_owned(),
            None,
            None,
        ));
    }
    if has_binary_files {
        issues.push(issue(
            ValidationLevel::Warning,
            "has_binary_files",
            "包含二进制文件，无法在线预览其内容".to_owned(),
            None,
            None,
        ));
    }

    let name = validation
        .frontmatter
        .get("name")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or(&dir_name)
        .to_owned();
    let version = validation
        .frontmatter
        .get("version")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let description = validation
        .frontmatter
        .get("description")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let installable = !issues.iter().any(|item| item.level == ValidationLevel::Error);
    let id = {
        let digest = Sha1::digest(if relative_path.is_empty() { "." } else { relative_path.as_str() }.as_bytes());
        format!("{digest:x}").chars().take(12).collect::<String>()
    };

    InspectedSkill {
        id,
        relative_path,
        name,
        version,
        description,
        content_hash: hash_dir(skill_dir),
        size_bytes,
        file_count: files.len(),
        has_scripts,
        has_binary_files,
        issues,
        installable,
    }
}

/// 检查 staging 内容目录，返回全部候选 Skill；无候选时报错。
pub fn inspect_staging_content(
    content_dir: &Path,
    search_root: Option<&Path>,
) -> Result<Vec<InspectedSkill>, SkillsError> {
    let root = search_root.unwrap_or(content_dir);
    let dirs = find_skill_dirs(root);
    if dirs.is_empty() {
        return Err(SkillsError::new(422, "未找到包含 SKILL.md 的目录"));
    }
    let mut skills: Vec<InspectedSkill> = dirs
        .iter()
        .map(|dir| inspect_one(content_dir, dir))
        .collect();
    skills.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(skills)
}
