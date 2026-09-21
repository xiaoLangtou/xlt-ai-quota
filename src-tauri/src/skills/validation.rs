use std::fs;
use std::path::Path;

use serde::Serialize;
use serde_json::{Map, Value};

use super::paths::is_valid_skill_name;

pub const SKILL_FILE: &str = "SKILL.md";

/// 超大文件告警阈值：5MB。
const LARGE_FILE_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ValidationLevel {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationIssue {
    pub level: ValidationLevel,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

impl ValidationIssue {
    fn new(level: ValidationLevel, code: &str, message: impl Into<String>) -> Self {
        Self {
            level,
            code: code.to_owned(),
            message: message.into(),
            file: None,
            fix: None,
        }
    }

    fn with_file(mut self, file: impl Into<String>) -> Self {
        self.file = Some(file.into());
        self
    }

    fn with_fix(mut self, fix: impl Into<String>) -> Self {
        self.fix = Some(fix.into());
        self
    }
}

/// SKILL.md 解析结果。
pub struct ParsedSkillMd {
    /// frontmatter 解析结果（可能是对象、数组或标量）。
    pub frontmatter: Value,
    /// frontmatter 之后的正文（已去除前导空行）。
    pub body: String,
}

/// 拆分 SKILL.md 的 frontmatter 与正文（对应 gray-matter 行为）。
fn split_frontmatter(raw: &str) -> Option<(&str, &str)> {
    let raw = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    let rest = raw.strip_prefix("---")?;
    let rest = rest
        .strip_prefix("\r\n")
        .or_else(|| rest.strip_prefix('\n'))?;
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\n', '\r']);
        if trimmed == "---" {
            let frontmatter = &rest[..offset];
            let body = &rest[offset + line.len()..];
            return Some((frontmatter, body));
        }
        offset += line.len();
    }
    // 结尾没有关闭分隔符时，按无 frontmatter 处理（与 gray-matter 的宽松行为接近）。
    None
}

/// 解析 SKILL.md 文本为 frontmatter 与正文。
pub fn parse_skill_md(raw: &str) -> Result<ParsedSkillMd, String> {
    match split_frontmatter(raw) {
        Some((yaml, body)) => {
            let frontmatter = if yaml.trim().is_empty() {
                Value::Object(Map::new())
            } else {
                serde_yml::from_str::<Value>(yaml).map_err(|error| error.to_string())?
            };
            Ok(ParsedSkillMd {
                frontmatter,
                body: body.trim_start_matches('\n').to_owned(),
            })
        }
        None => Ok(ParsedSkillMd {
            frontmatter: Value::Object(Map::new()),
            body: raw.trim_start_matches('\n').to_owned(),
        }),
    }
}

/// 将 frontmatter 归一为键值对象；非对象时给出错误说明。
pub fn frontmatter_object(value: &Value) -> Result<Map<String, Value>, &'static str> {
    match value {
        Value::Object(map) => Ok(map.clone()),
        Value::Null => Ok(Map::new()),
        Value::Array(_) => Err("frontmatter 必须是 YAML 键值对象"),
        _ => Err("frontmatter 必须是 YAML 键值对象"),
    }
}

/// 结构化校验结果。
pub struct ValidationResult {
    pub issues: Vec<ValidationIssue>,
    pub frontmatter: Map<String, Value>,
    /// YAML 是否解析成功且为对象（对象缺失时为 true）。
    pub frontmatter_valid: bool,
}

/// 对单个 skill 目录做结构化校验（FR-VALID-01 / FR-VALID-02）。
pub fn validate_skill(dir_path: &Path, dir_name: &str) -> ValidationResult {
    let mut issues = Vec::new();
    let mut frontmatter = Map::new();
    let skill_file = dir_path.join(SKILL_FILE);

    if !skill_file.exists() {
        issues.push(
            ValidationIssue::new(ValidationLevel::Error, "missing_skill_md", "SKILL.md 不存在")
                .with_file(SKILL_FILE)
                .with_fix("在 skill 目录下创建包含 frontmatter 的 SKILL.md"),
        );
        scan_files(dir_path, "", &mut issues);
        return ValidationResult {
            issues,
            frontmatter,
            frontmatter_valid: true,
        };
    }

    let raw = match fs::read_to_string(&skill_file) {
        Ok(text) => text,
        Err(error) => {
            issues.push(
                ValidationIssue::new(
                    ValidationLevel::Error,
                    "unreadable_skill_md",
                    format!("SKILL.md 不可读取：{error}"),
                )
                .with_file(SKILL_FILE),
            );
            return ValidationResult {
                issues,
                frontmatter,
                frontmatter_valid: true,
            };
        }
    };

    let parsed = match parse_skill_md(&raw) {
        Ok(parsed) => parsed,
        Err(message) => {
            issues.push(
                ValidationIssue::new(
                    ValidationLevel::Error,
                    "invalid_yaml",
                    format!("frontmatter YAML 解析失败：{message}"),
                )
                .with_file(SKILL_FILE)
                .with_fix("检查 --- 分隔符之间的 YAML 语法"),
            );
            return ValidationResult {
                issues,
                frontmatter,
                frontmatter_valid: false,
            };
        }
    };

    let mut frontmatter_valid = true;
    match frontmatter_object(&parsed.frontmatter) {
        Ok(map) => frontmatter = map,
        Err(message) => {
            frontmatter_valid = false;
            issues.push(
                ValidationIssue::new(ValidationLevel::Error, "frontmatter_not_object", message)
                    .with_file(SKILL_FILE),
            );
        }
    }

    // name 字段
    match frontmatter.get("name") {
        None | Some(Value::Null) => {
            issues.push(
                ValidationIssue::new(ValidationLevel::Error, "missing_name", "frontmatter 缺少 name 字段")
                    .with_file(SKILL_FILE)
                    .with_fix("在 frontmatter 中添加 name 字段"),
            );
        }
        Some(Value::String(name)) if name.is_empty() => {
            issues.push(
                ValidationIssue::new(ValidationLevel::Error, "missing_name", "frontmatter 缺少 name 字段")
                    .with_file(SKILL_FILE)
                    .with_fix("在 frontmatter 中添加 name 字段"),
            );
        }
        Some(Value::String(name)) => {
            if !is_valid_skill_name(name) {
                issues.push(
                    ValidationIssue::new(
                        ValidationLevel::Warning,
                        "invalid_name_format",
                        format!("name（{name}）不符合命名规则（仅字母数字、-、_、.）"),
                    )
                    .with_file(SKILL_FILE),
                );
            }
            if name != dir_name {
                issues.push(
                    ValidationIssue::new(
                        ValidationLevel::Warning,
                        "name_dir_mismatch",
                        format!("frontmatter name（{name}）与文件夹名（{dir_name}）不一致"),
                    )
                    .with_file(SKILL_FILE)
                    .with_fix("重命名目录或修改 frontmatter name 使两者一致"),
                );
            }
        }
        Some(_) => {
            issues.push(
                ValidationIssue::new(ValidationLevel::Error, "name_not_string", "name 字段必须是字符串")
                    .with_file(SKILL_FILE),
            );
        }
    }

    // description 字段
    match frontmatter.get("description") {
        None | Some(Value::Null) => {
            issues.push(
                ValidationIssue::new(
                    ValidationLevel::Warning,
                    "missing_description",
                    "frontmatter 缺少 description 字段",
                )
                .with_file(SKILL_FILE)
                .with_fix("添加 description 以便 skill 被发现"),
            );
        }
        Some(Value::String(text)) if text.is_empty() => {
            issues.push(
                ValidationIssue::new(
                    ValidationLevel::Warning,
                    "missing_description",
                    "frontmatter 缺少 description 字段",
                )
                .with_file(SKILL_FILE)
                .with_fix("添加 description 以便 skill 被发现"),
            );
        }
        Some(Value::String(_)) => {}
        Some(_) => {
            issues.push(
                ValidationIssue::new(
                    ValidationLevel::Warning,
                    "description_not_string",
                    "description 字段必须是字符串",
                )
                .with_file(SKILL_FILE),
            );
        }
    }

    scan_files(dir_path, "", &mut issues);
    ValidationResult {
        issues,
        frontmatter,
        frontmatter_valid,
    }
}

/// 递归扫描超大文件与异常文件名。
fn scan_files(dir: &Path, rel: &str, issues: &mut Vec<ValidationIssue>) {
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
        if name.chars().any(|c| c < '\u{20}') {
            issues.push(
                ValidationIssue::new(
                    ValidationLevel::Warning,
                    "bad_filename",
                    format!("文件名包含控制字符：{name}"),
                )
                .with_file(rel_path.clone()),
            );
        }
        let full = entry.path();
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => scan_files(&full, &rel_path, issues),
            Ok(kind) if kind.is_file() => {
                if let Ok(metadata) = fs::metadata(&full) {
                    let size = metadata.len();
                    if size > LARGE_FILE_BYTES {
                        issues.push(
                            ValidationIssue::new(
                                ValidationLevel::Warning,
                                "large_file",
                                format!("文件过大（{:.1} MB）", size as f64 / 1024.0 / 1024.0),
                            )
                            .with_file(rel_path)
                            .with_fix("考虑拆分或移出该文件"),
                        );
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontmatter_and_body() {
        let raw = "---\nname: demo\nversion: 1.0.0\ndescription: hi\n---\n\n# Demo\n";
        let parsed = parse_skill_md(raw).unwrap();
        assert_eq!(parsed.frontmatter["name"], Value::String("demo".to_owned()));
        assert_eq!(parsed.body, "# Demo\n");
    }

    #[test]
    fn handles_missing_frontmatter() {
        let parsed = parse_skill_md("# No FM\n").unwrap();
        assert!(parsed.frontmatter.as_object().unwrap().is_empty());
        assert_eq!(parsed.body, "# No FM\n");
    }

    #[test]
    fn preserves_frontmatter_key_order() {
        let raw = "---\nzebra: 1\nalpha: 2\n---\nbody";
        let parsed = parse_skill_md(raw).unwrap();
        let keys: Vec<&String> = parsed.frontmatter.as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["zebra", "alpha"]);
    }
}
