use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::config::{get_root, SkillRoot, SkillsPaths};
use super::error::SkillsError;
use super::install::records::{remove_record_by_dir, rename_record_dir};
use super::paths::{is_inside, is_valid_skill_name, safe_join};
use super::scan::{build_file_tree, read_summary, SkillFileNode, SkillSummary};
use super::trash::move_to_trash;
use super::validation::{parse_skill_md, SKILL_FILE};

/// frontmatter 中由结构化字段承载的键，其余键归入 `extraYaml`。
const STRUCTURED_KEYS: &[&str] = &["name", "version", "description"];

/// skill 详情。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillDetail {
    #[serde(flatten)]
    pub summary: SkillSummary,
    pub frontmatter: Map<String, Value>,
    pub frontmatter_yaml: String,
    pub body: String,
    pub files: Vec<SkillFileNode>,
}

fn extra_yaml(frontmatter: &Map<String, Value>) -> String {
    let mut extra = Map::new();
    for (key, value) in frontmatter {
        if !STRUCTURED_KEYS.contains(&key.as_str()) {
            extra.insert(key.clone(), value.clone());
        }
    }
    if extra.is_empty() {
        return String::new();
    }
    serde_yml::to_string(&extra)
        .map(|text| text.trim_end().to_owned())
        .unwrap_or_default()
}

/// 解析并校验 skill 目录，返回根目录与绝对路径。
pub fn resolve_skill_dir(
    paths: &SkillsPaths,
    root_id: &str,
    name: &str,
) -> Result<(SkillRoot, PathBuf), SkillsError> {
    if !is_valid_skill_name(name) {
        return Err(SkillsError::bad_request("非法的 skill 名称"));
    }
    let root = get_root(paths, root_id).ok_or_else(|| SkillsError::not_found("根目录不存在"))?;
    let dir_path = safe_join(&PathBuf::from(&root.path), name)?;
    Ok((root, dir_path))
}

/// 读取单个 skill 详情。
pub fn get_skill_detail(
    paths: &SkillsPaths,
    root_id: &str,
    name: &str,
) -> Result<SkillDetail, SkillsError> {
    let (root, dir_path) = resolve_skill_dir(paths, root_id, name)?;
    let summary = read_summary(&root, name)
        .map_err(SkillsError::internal)?
        .ok_or_else(|| SkillsError::not_found("skill 不存在"))?;

    let raw = fs::read_to_string(dir_path.join(SKILL_FILE)).unwrap_or_default();
    // 详情页对非法 YAML 保持可用：解析失败时回退为空 frontmatter + 全文正文。
    let (frontmatter, body) = match parse_skill_md(&raw) {
        Ok(parsed) => {
            let map = super::validation::frontmatter_object(&parsed.frontmatter)
                .unwrap_or_default();
            (map, parsed.body)
        }
        Err(_) => (Map::new(), raw.clone()),
    };
    let frontmatter_yaml = extra_yaml(&frontmatter);
    let files = build_file_tree(&dir_path, "");

    Ok(SkillDetail {
        summary,
        frontmatter,
        frontmatter_yaml,
        body,
        files,
    })
}

/// 读取 skill 内某个子文件的文本内容（解析软链接后仍须落在 skill 目录内）。
pub fn read_skill_file(
    paths: &SkillsPaths,
    root_id: &str,
    name: &str,
    rel_path: &str,
) -> Result<String, SkillsError> {
    let (_root, dir_path) = resolve_skill_dir(paths, root_id, name)?;
    let abs = safe_join(&dir_path, rel_path)?;
    if !abs.exists() || !abs.is_file() {
        return Err(SkillsError::not_found("文件不存在"));
    }
    let real_base = fs::canonicalize(&dir_path)
        .map_err(|error| SkillsError::internal(format!("解析 skill 路径失败：{error}")))?;
    let real_target = fs::canonicalize(&abs)
        .map_err(|error| SkillsError::internal(format!("解析文件路径失败：{error}")))?;
    if !is_inside(&real_base, &real_target) {
        return Err(SkillsError::forbidden("禁止访问 skill 目录外的文件"));
    }
    Ok(fs::read_to_string(&abs)?)
}

// ============ 编辑（保存 / 新建 / 重命名 / 删除） ============

/// 保存 skill 的请求体。
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSkillRequest {
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub extra_yaml: Option<String>,
    pub body: String,
    #[serde(default)]
    pub files: Option<HashMap<String, String>>,
}

/// 新建 skill 请求体。
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSkillRequest {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub version: Option<String>,
    pub root_id: String,
}

/// 删除结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteResult {
    pub deleted: bool,
    pub was_symlink: bool,
}

/// 组装 SKILL.md 文本（frontmatter + 正文）。
fn compose_skill_md(request: &SaveSkillRequest) -> Result<String, SkillsError> {
    let mut data = Map::new();
    data.insert("name".to_owned(), Value::String(request.name.clone()));
    if let Some(version) = request.version.as_ref().filter(|value| !value.is_empty()) {
        data.insert("version".to_owned(), Value::String(version.clone()));
    }
    if let Some(description) = &request.description {
        data.insert("description".to_owned(), Value::String(description.clone()));
    }
    if let Some(extra) = request
        .extra_yaml
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        let parsed = serde_yml::from_str::<Value>(extra)
            .map_err(|error| SkillsError::bad_request(format!("附加 YAML 解析失败：{error}")))?;
        match parsed {
            Value::Object(map) => {
                for (key, value) in map {
                    data.insert(key, value);
                }
            }
            Value::Null => {}
            _ => return Err(SkillsError::bad_request("附加 YAML 必须是键值对象")),
        }
    }
    let frontmatter = serde_yml::to_string(&data)
        .map_err(|error| SkillsError::internal(format!("序列化 frontmatter 失败：{error}")))?
        .trim_end()
        .to_owned();
    let body = request.body.trim_start_matches('\n').trim_end();
    Ok(format!("---\n{frontmatter}\n---\n\n{body}\n"))
}

/// 保存 skill（写回 SKILL.md 及被编辑的子文件）。
pub fn save_skill(
    paths: &SkillsPaths,
    root_id: &str,
    name: &str,
    request: &SaveSkillRequest,
) -> Result<SkillDetail, SkillsError> {
    let (root, dir_path) = resolve_skill_dir(paths, root_id, name)?;
    if !root.writable {
        return Err(SkillsError::forbidden("该根目录不可写"));
    }
    if !dir_path.exists() {
        return Err(SkillsError::not_found("skill 不存在"));
    }
    if request.name.is_empty() || !is_valid_skill_name(&request.name) {
        return Err(SkillsError::bad_request("name 非法或为空"));
    }

    let content = compose_skill_md(request)?;
    fs::write(dir_path.join(SKILL_FILE), content)?;

    if let Some(files) = &request.files {
        for (rel_path, text) in files {
            let abs = safe_join(&dir_path, rel_path)?;
            if let Some(parent) = abs.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(abs, text)?;
        }
    }
    get_skill_detail(paths, root_id, name)
}

fn skill_template(name: &str, description: &str, version: &str) -> String {
    let escaped = description.replace('"', "\\\"");
    format!(
        "---\nname: {name}\nversion: {version}\ndescription: \"{escaped}\"\n---\n\n# {name}\n\n{description}\n\n## 使用说明\n\n在此填写 skill 的详细指令。\n"
    )
}

/// 新建 skill（脚手架）。
pub fn create_skill(
    paths: &SkillsPaths,
    request: &CreateSkillRequest,
) -> Result<SkillDetail, SkillsError> {
    if !is_valid_skill_name(&request.name) {
        return Err(SkillsError::bad_request("name 非法"));
    }
    let root = get_root(paths, &request.root_id)
        .ok_or_else(|| SkillsError::not_found("根目录不存在"))?;
    if !root.writable {
        return Err(SkillsError::forbidden("该根目录不可写"));
    }
    let dir_path = safe_join(&PathBuf::from(&root.path), &request.name)?;
    if dir_path.exists() {
        return Err(SkillsError::conflict("同名 skill 已存在"));
    }
    fs::create_dir_all(&dir_path)?;
    fs::write(
        dir_path.join(SKILL_FILE),
        skill_template(
            &request.name,
            &request.description,
            request.version.as_deref().unwrap_or("0.1.0"),
        ),
    )?;
    get_skill_detail(paths, &request.root_id, &request.name)
}

/// 安全重命名 skill：同步目录名与 frontmatter.name，失败回滚。
pub fn rename_skill(
    paths: &SkillsPaths,
    root_id: &str,
    old_name: &str,
    new_name: &str,
) -> Result<SkillDetail, SkillsError> {
    let (root, dir_path) = resolve_skill_dir(paths, root_id, old_name)?;
    if !root.writable {
        return Err(SkillsError::forbidden("该根目录不可写"));
    }
    if !dir_path.exists() {
        return Err(SkillsError::not_found("skill 不存在"));
    }
    if !is_valid_skill_name(new_name) {
        return Err(SkillsError::bad_request("name 非法或为空"));
    }
    if new_name == old_name {
        return get_skill_detail(paths, root_id, old_name);
    }

    let destination = safe_join(&PathBuf::from(&root.path), new_name)?;
    if destination.exists() {
        return Err(SkillsError::conflict("同名 skill 已存在"));
    }

    let is_symlink = fs::symlink_metadata(&dir_path)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false);
    let skill_file = dir_path.join(SKILL_FILE);

    // 先更新 frontmatter.name（软链接目标为外部目录，不改写其内容以免污染源）。
    let mut original_content: Option<String> = None;
    if !is_symlink && skill_file.exists() {
        let text = fs::read_to_string(&skill_file)?;
        original_content = Some(text.clone());
        let mut frontmatter = match parse_skill_md(&text) {
            Ok(parsed) => super::validation::frontmatter_object(&parsed.frontmatter).unwrap_or_default(),
            Err(_) => Map::new(),
        };
        frontmatter.insert("name".to_owned(), Value::String(new_name.to_owned()));
        let body = parse_skill_md(&text).map(|parsed| parsed.body).unwrap_or_default();
        let yaml = serde_yml::to_string(&frontmatter)
            .map_err(|error| SkillsError::internal(format!("序列化 frontmatter 失败：{error}")))?
            .trim_end()
            .to_owned();
        fs::write(
            &skill_file,
            format!("---\n{yaml}\n---\n\n{}\n", body.trim_start_matches('\n').trim_end()),
        )?;
    }

    if let Err(error) = fs::rename(&dir_path, &destination) {
        if let Some(content) = original_content {
            let _ = fs::write(&skill_file, content);
        }
        return Err(SkillsError::internal(format!("重命名失败：{error}")));
    }
    rename_record_dir(
        paths,
        root_id,
        old_name,
        new_name,
        &destination.to_string_lossy(),
    );
    get_skill_detail(paths, root_id, new_name)
}

/// 删除 skill（移入回收站，可恢复；软链接仅移除链接不删目标）。
pub fn delete_skill(
    paths: &SkillsPaths,
    root_id: &str,
    name: &str,
) -> Result<DeleteResult, SkillsError> {
    let (root, dir_path) = resolve_skill_dir(paths, root_id, name)?;
    if !root.writable {
        return Err(SkillsError::forbidden("该根目录不可写"));
    }
    if !dir_path.exists() {
        return Err(SkillsError::not_found("skill 不存在"));
    }
    let item = move_to_trash(paths, &root, name, &dir_path)?;
    remove_record_by_dir(paths, root_id, name);
    Ok(DeleteResult {
        deleted: true,
        was_symlink: item.was_symlink,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::config::list_roots;
    use crate::skills::trash::{list_trash, restore_trash};

    fn setup() -> (SkillsPaths, PathBuf) {
        let base = std::env::temp_dir().join(format!("xlt-skills-io-{}", uuid::Uuid::new_v4()));
        let root = base.join("root");
        fs::create_dir_all(&root).unwrap();
        let paths = SkillsPaths {
            home: base.join("home"),
            config_dir: base.join("config"),
            staging_dir: base.join("staging"),
            default_roots: vec![root.clone()],
            discover_default_roots: false,
        };
        (paths, root)
    }

    #[test]
    fn creates_saves_renames_deletes_and_restores() {
        let (paths, _root) = setup();
        let root_id = list_roots(&paths)[0].id.clone();

        let created = create_skill(
            &paths,
            &CreateSkillRequest {
                name: "demo".to_owned(),
                description: "hi".to_owned(),
                version: None,
                root_id: root_id.clone(),
            },
        )
        .unwrap();
        assert_eq!(created.summary.name, "demo");
        assert_eq!(created.summary.version.as_deref(), Some("0.1.0"));

        let saved = save_skill(
            &paths,
            &root_id,
            "demo",
            &SaveSkillRequest {
                name: "demo".to_owned(),
                version: Some("2.0.0".to_owned()),
                description: Some("updated".to_owned()),
                extra_yaml: Some("tags:\n  - alpha".to_owned()),
                body: "# Hi".to_owned(),
                files: Some(HashMap::from([("notes.md".to_owned(), "note".to_owned())])),
            },
        )
        .unwrap();
        assert_eq!(saved.summary.version.as_deref(), Some("2.0.0"));
        assert_eq!(saved.frontmatter.get("tags").and_then(Value::as_array).map(|v| v.len()), Some(1));
        assert!(saved.files.iter().any(|node| node.path == "notes.md"));

        let renamed = rename_skill(&paths, &root_id, "demo", "demo2").unwrap();
        assert_eq!(renamed.summary.name, "demo2");
        assert!(renamed.summary.dir_path.ends_with("demo2"));

        let deleted = delete_skill(&paths, &root_id, "demo2").unwrap();
        assert!(deleted.deleted);
        assert!(!deleted.was_symlink);

        let trash_items = list_trash(&paths);
        assert_eq!(trash_items.len(), 1);
        assert_eq!(trash_items[0].name, "demo2");

        let (_restored_root, restored_name) = restore_trash(&paths, &trash_items[0].id).unwrap();
        assert_eq!(restored_name, "demo2");
        get_skill_detail(&paths, &root_id, "demo2").unwrap();
    }
}
