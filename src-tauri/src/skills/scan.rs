use std::fs;
use std::path::Path;
use std::time::SystemTime;

use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;
use serde_json::Value;
use sha1::{Digest, Sha1};

use super::config::{list_roots, SkillRoot, SkillScope, SkillsPaths};
use super::paths::{is_editable_file, to_posix};
use super::validation::{validate_skill, ValidationIssue};

const SKILL_FILE: &str = "SKILL.md";

/// skill 内的文件树节点。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillFileNode {
    /// 相对 skill 目录的 POSIX 路径
    pub path: String,
    pub name: String,
    #[serde(rename = "type")]
    pub node_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub editable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_symlink: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<SkillFileNode>>,
}

/// skill 概览（列表用）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSummary {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub root_id: String,
    pub root_label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    pub dir_path: String,
    pub content_hash: String,
    pub scope: SkillScope,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_label: Option<String>,
    pub is_symlink: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symlink_target: Option<String>,
    pub size_bytes: u64,
    pub mtime: String,
    pub valid: bool,
    pub warnings: Vec<String>,
    pub issues: Vec<ValidationIssue>,
}

/// 扫描 / 解析失败项。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanFailure {
    pub root_id: String,
    pub root_label: String,
    pub dir_name: String,
    pub reason: String,
}

fn iso_millis(time: SystemTime) -> String {
    let datetime: DateTime<Utc> = time.into();
    datetime.to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// 递归统计目录大小（不跟随软链接）。
pub fn dir_size(dir: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    let mut total = 0;
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            total += dir_size(&entry.path());
        } else if kind.is_file() {
            if let Ok(metadata) = entry.metadata() {
                total += metadata.len();
            }
        }
    }
    total
}

/// 构建 skill 子文件树（相对 skill 目录，目录在前再按名称排序）。
pub fn build_file_tree(dir: &Path, rel: &str) -> Vec<SkillFileNode> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut nodes = Vec::new();
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
        if kind.is_dir() {
            nodes.push(SkillFileNode {
                path: rel_path.clone(),
                name,
                node_type: "dir",
                size_bytes: None,
                editable: None,
                is_symlink: None,
                children: Some(build_file_tree(&entry.path(), &rel_path)),
            });
        } else if kind.is_symlink() {
            let size = entry.metadata().map(|meta| meta.len()).unwrap_or(0);
            nodes.push(SkillFileNode {
                path: rel_path,
                name,
                node_type: "file",
                size_bytes: Some(size),
                editable: Some(false),
                is_symlink: Some(true),
                children: None,
            });
        } else if kind.is_file() {
            let size = entry.metadata().map(|meta| meta.len()).unwrap_or(0);
            nodes.push(SkillFileNode {
                path: rel_path,
                name: name.clone(),
                node_type: "file",
                size_bytes: Some(size),
                editable: Some(is_editable_file(&name)),
                is_symlink: None,
                children: None,
            });
        }
    }
    nodes.sort_by(|a, b| {
        let a_dir = a.node_type == "dir";
        let b_dir = b.node_type == "dir";
        b_dir.cmp(&a_dir).then_with(|| a.name.cmp(&b.name))
    });
    nodes
}

/// 读取单个 skill 的概览；目录缺少 SKILL.md 时返回 None。
pub fn read_summary(root: &SkillRoot, dir_name: &str) -> Result<Option<SkillSummary>, String> {
    let dir_path = Path::new(&root.path).join(dir_name);
    let skill_file = dir_path.join(SKILL_FILE);
    if !skill_file.exists() {
        return Ok(None);
    }

    let is_symlink = fs::symlink_metadata(&dir_path)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false);
    let symlink_target = if is_symlink {
        fs::read_link(&dir_path)
            .ok()
            .map(|target| target.to_string_lossy().to_string())
    } else {
        None
    };

    let validation = validate_skill(&dir_path, dir_name);
    let name = validation
        .frontmatter
        .get("name")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or(dir_name)
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
    let warnings: Vec<String> = validation
        .issues
        .iter()
        .filter(|issue| issue.level != super::validation::ValidationLevel::Info)
        .map(|issue| issue.message.clone())
        .collect();
    let valid = !validation
        .issues
        .iter()
        .any(|issue| issue.level == super::validation::ValidationLevel::Error);

    let size_bytes = dir_size(&dir_path);
    let content_hash = fs::read(&skill_file)
        .map(|bytes| {
            let digest = Sha1::digest(&bytes);
            format!("{digest:x}").chars().take(16).collect::<String>()
        })
        .unwrap_or_default();
    let mtime = fs::metadata(&dir_path)
        .and_then(|meta| meta.modified())
        .map(iso_millis)
        .unwrap_or_default();

    Ok(Some(SkillSummary {
        id: format!("{}::{dir_name}", root.id),
        name,
        version,
        description,
        root_id: root.id.clone(),
        root_label: root.label.clone(),
        agent: root.agent.clone(),
        dir_path: to_posix(&dir_path),
        content_hash,
        scope: root.scope,
        project_label: root.project_label.clone(),
        is_symlink,
        symlink_target,
        size_bytes,
        mtime,
        valid,
        warnings,
        issues: validation.issues,
    }))
}

/// 扫描所有根目录，返回 skill 概览与失败项（按名称排序）。
pub fn scan_all_skills(paths: &SkillsPaths) -> (Vec<SkillSummary>, Vec<ScanFailure>) {
    let mut skills = Vec::new();
    let mut failures = Vec::new();
    for root in list_roots(paths) {
        if !root.exists {
            continue;
        }
        let Ok(entries) = fs::read_dir(&root.path) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if !kind.is_dir() && !kind.is_symlink() {
                continue;
            }
            match read_summary(&root, &name) {
                Ok(Some(summary)) => skills.push(summary),
                Ok(None) => failures.push(ScanFailure {
                    root_id: root.id.clone(),
                    root_label: root.label.clone(),
                    dir_name: name,
                    reason: "缺少 SKILL.md".to_owned(),
                }),
                Err(reason) => failures.push(ScanFailure {
                    root_id: root.id.clone(),
                    root_label: root.label.clone(),
                    dir_name: name,
                    reason,
                }),
            }
        }
    }
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    (skills, failures)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("xlt-skills-scan-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn builds_sorted_file_tree() {
        let dir = temp_dir();
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("b.txt"), "b").unwrap();
        fs::write(dir.join("a.md"), "a").unwrap();
        fs::write(dir.join(".DS_Store"), "x").unwrap();
        let tree = build_file_tree(&dir, "");
        let names: Vec<&str> = tree.iter().map(|node| node.name.as_str()).collect();
        assert_eq!(names, vec!["sub", "a.md", "b.txt"]);
        assert_eq!(tree[0].node_type, "dir");
    }

    #[test]
    fn reads_summary_from_skill_dir() {
        let dir = temp_dir();
        let skill = dir.join("demo");
        fs::create_dir_all(&skill).unwrap();
        fs::write(
            skill.join("SKILL.md"),
            "---\nname: demo\nversion: 1.0.0\ndescription: hi\n---\n\nbody\n",
        )
        .unwrap();
        let root = SkillRoot {
            id: "root1".to_owned(),
            label: "~/.qoder/skills".to_owned(),
            path: dir.to_string_lossy().to_string(),
            writable: true,
            is_default: true,
            exists: true,
            agent: Some("Qoder".to_owned()),
            skill_count: Some(1),
            scope: SkillScope::Global,
            project_id: None,
            project_label: None,
        };
        let summary = read_summary(&root, "demo").unwrap().unwrap();
        assert_eq!(summary.name, "demo");
        assert_eq!(summary.version.as_deref(), Some("1.0.0"));
        assert!(summary.valid);
        assert_eq!(summary.agent.as_deref(), Some("Qoder"));
    }
}
