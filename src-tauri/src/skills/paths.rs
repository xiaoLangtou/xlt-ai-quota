use std::path::{Component, Path, PathBuf};

use super::error::SkillsError;

/// 可在线编辑的文本文件扩展名（与 `packages/server/src/paths.ts` 保持一致）。
const TEXT_EXTENSIONS: &[&str] = &[
    ".md", ".markdown", ".txt", ".json", ".yaml", ".yml", ".toml", ".js", ".ts", ".mjs", ".cjs",
    ".py", ".sh", ".bash", ".zsh", ".html", ".css", ".xml", ".csv", ".env", ".gitignore", ".ini",
];

/// 纯词法路径规范化：折叠 `.` 与 `..`，不访问文件系统。
/// 对应 Node 的 `path.resolve` 语义（原实现依赖的正是词法解析，而非 canonicalize）。
pub fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => out.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() && !out.has_root() {
                    out.push("..");
                }
            }
            Component::Normal(part) => out.push(part),
        }
    }
    out
}

/// 按 Node `path.resolve` 的语义把任意路径解析为规范化绝对路径。
pub fn resolve_path(raw: &str) -> PathBuf {
    let path = Path::new(raw);
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_default()
            .join(path)
    };
    normalize(&absolute)
}

/// 判断 `child` 是否严格位于 `base` 内部（含 `base` 自身）。
pub fn is_inside(base: &Path, child: &Path) -> bool {
    let base = normalize(base);
    let target = normalize(child);
    target == base || target.starts_with(&base)
}

/// 将相对路径安全拼接到 `base` 下，越界即报错。
pub fn safe_join(base: &Path, rel: &str) -> Result<PathBuf, SkillsError> {
    let base = normalize(base);
    let target = normalize(&base.join(rel));
    if !is_inside(&base, &target) {
        return Err(SkillsError::bad_request(format!("路径越界：{rel}")));
    }
    Ok(target)
}

/// skill 名称合法性：仅允许字母数字开头，后续为字母数字、`.`、`_`、`-`，且不含 `..`。
pub fn is_valid_skill_name(name: &str) -> bool {
    if name.is_empty() || name.contains("..") {
        return false;
    }
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphanumeric() => {}
        _ => return false,
    }
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// 文件名是否为可在线编辑的文本类型。
pub fn is_editable_file(name: &str) -> bool {
    let lower = name.to_lowercase();
    if lower == "dockerfile" || lower == "makefile" {
        return true;
    }
    match lower.rsplit_once('.') {
        Some((_, extension)) => TEXT_EXTENSIONS
            .iter()
            .any(|item| *item == format!(".{extension}")),
        None => false,
    }
}

/// 系统路径转 POSIX 风格（前端文件树统一用 `/`）。
pub fn to_posix(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// target 相对 base 的 POSIX 路径；不在 base 内时返回 target 的 POSIX 路径。
pub fn relative_posix(base: &Path, target: &Path) -> String {
    let base = normalize(base);
    let target = normalize(target);
    match target.strip_prefix(&base) {
        Ok(rest) => to_posix(rest),
        Err(_) => to_posix(&target),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_dot_and_dotdot() {
        assert_eq!(normalize(Path::new("/a/b/../c")), PathBuf::from("/a/c"));
        assert_eq!(normalize(Path::new("/a/./b")), PathBuf::from("/a/b"));
        assert_eq!(normalize(Path::new("/..")), PathBuf::from("/"));
    }

    #[test]
    fn detects_inside_paths() {
        assert!(is_inside(Path::new("/a"), Path::new("/a/b")));
        assert!(is_inside(Path::new("/a"), Path::new("/a")));
        assert!(!is_inside(Path::new("/a"), Path::new("/ab")));
    }

    #[test]
    fn rejects_traversal_in_safe_join() {
        assert!(safe_join(Path::new("/a"), "b").is_ok());
        assert!(safe_join(Path::new("/a"), "../b").is_err());
        assert!(safe_join(Path::new("/a"), "/etc/passwd").is_err());
    }

    #[test]
    fn validates_skill_names() {
        assert!(is_valid_skill_name("my-skill"));
        assert!(is_valid_skill_name("a.b_c-1"));
        assert!(!is_valid_skill_name(""));
        assert!(!is_valid_skill_name("-bad"));
        assert!(!is_valid_skill_name("../evil"));
    }

    #[test]
    fn detects_editable_files() {
        assert!(is_editable_file("SKILL.md"));
        assert!(is_editable_file("Dockerfile"));
        assert!(!is_editable_file("logo.png"));
    }
}
