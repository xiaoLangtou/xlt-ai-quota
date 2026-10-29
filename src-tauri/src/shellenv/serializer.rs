//! 序列化层：字面量引号规则、行格式化、会话命令生成、托管区块标记。
//!
//! 语义约定（B01）：值一律按「字面量」写入——单引号包裹、内嵌单引号用 `'\''` 转义，
//! 不产生 `$` 展开语义。含展开语法的新值应由用户在源码视图中书写。

use crate::shellenv::types::ValueKind;

/// 托管区块标记（当前规范格式；旧格式由 block_normalize 迁移）。
pub const MANAGED_BEGIN: &str = "# >>> xlt-managed env >>>";
pub const MANAGED_END: &str = "# <<< xlt-managed env <<<";

/// 单引号字面量：`'value'`，内嵌单引号写作 `'\''`。
pub fn shell_single_quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// alias 行：命令文本语义，字面量写入。
pub fn format_alias(name: &str, value: &str) -> String {
    format!("alias {}={}", name, shell_single_quoted(value))
}

/// alias 行 + 可选行尾注释描述。换行符折叠为空格（注释必须单行）；空描述不追加。
pub fn format_alias_with_description(name: &str, value: &str, description: Option<&str>) -> String {
    let base = format_alias(name, value);
    match description.map(str::trim).filter(|text| !text.is_empty()) {
        Some(text) => {
            let one_line: String = text.chars().map(|c| if c == '\n' || c == '\r' { ' ' } else { c }).collect();
            format!("{base} # {}", one_line.trim())
        }
        None => base,
    }
}

/// 环境变量行：字面量语义，字面量写入。
pub fn format_export(key: &str, value: &str) -> String {
    format!("export {}={}", key, shell_single_quoted(value))
}

/// 判断新输入的表格值是否适合字面量写入：任何文本都可以安全字面量化，
/// 但值本身含 `$` / 反引号时用户多半想要展开语义，应引导走源码视图。
pub fn literal_edit_conflict(value: &str) -> bool {
    value.contains('$') || value.contains('`')
}

/// 依据值语义生成描述（用于差异确认与表格提示）。
pub fn value_kind_label(kind: ValueKind) -> &'static str {
    match kind {
        ValueKind::Literal => "字面量",
        ValueKind::Expression => "表达式",
    }
}

/// 会话命令（B09）：针对本次变更生成最小命令，由用户手动复制；
/// 删除类变更必须包含 unalias / unset，不能只靠 source 清理。
pub enum SessionChange<'a> {
    AliasAdd,
    AliasEdit,
    AliasRename,
    AliasDelete { name: &'a str },
    EnvSet { assignment: String },
    EnvDelete { key: &'a str },
    FullReload,
}

pub fn session_command(change: &SessionChange<'_>) -> String {
    match change {
        SessionChange::AliasAdd | SessionChange::AliasEdit | SessionChange::AliasRename => {
            "source ~/.alias".to_string()
        }
        SessionChange::AliasDelete { name } => {
            format!("unalias {name} 2>/dev/null; source ~/.alias")
        }
        SessionChange::EnvSet { assignment } => {
            format!("{assignment}  # 已开终端直接执行本行即可生效")
        }
        SessionChange::EnvDelete { key } => {
            format!("unset {key}; source ~/.zshrc")
        }
        SessionChange::FullReload => "source ~/.alias && source ~/.zshrc".to_string(),
    }
}
