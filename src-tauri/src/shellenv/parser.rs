//! 解析层：行切分（保留字节偏移）、别名 / 环境变量条目解析、值语义分类、
//! 结构上下文（条件块 / heredoc / 续行）与托管区块扫描。
//!
//! 设计要点（R0-01 / B02）：
//! - 不做通用 Shell 解释器；只识别可静态证明的简单定义；
//! - 区分导出变量与普通赋值，条件块 / heredoc / 续行中的行不做可编辑判定；
//! - 托管区块标记唯一性不足时产出 issues，阻止结构化写操作。

use crate::shellenv::serializer::{MANAGED_BEGIN, MANAGED_END};
use crate::shellenv::types::{AliasEntry, BlockInfo, EnvEntry, ValueKind};

/// 单行字节范围：`text = raw[start..end]`，终止符为 `raw[end..term_end]`。
#[derive(Clone, Copy, Debug)]
pub struct DocLine {
    pub start: usize,
    pub end: usize,
    pub term_end: usize,
}

impl DocLine {
    pub fn text<'a>(&self, raw: &'a str) -> &'a str {
        &raw[self.start..self.end]
    }
    pub fn has_terminator(&self) -> bool {
        self.term_end > self.end
    }
    pub fn terminator<'a>(&self, raw: &'a str) -> &'a str {
        &raw[self.end..self.term_end]
    }
}

/// 按字节切分行，保留 CRLF：文本不含终止符，终止符字节完整记录。
pub fn split_lines(raw: &str) -> Vec<DocLine> {
    let bytes = raw.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'\n' {
            let end = if i > start && bytes[i - 1] == b'\r' { i - 1 } else { i };
            lines.push(DocLine { start, end, term_end: i + 1 });
            start = i + 1;
        }
        i += 1;
    }
    if start < bytes.len() {
        lines.push(DocLine { start, end: bytes.len(), term_end: bytes.len() });
    }
    lines
}

/// 新插入行使用的终止符：文件中出现过 CRLF 则沿用 CRLF。
pub fn detect_line_ending(raw: &str) -> &'static str {
    if raw.contains("\r\n") { "\r\n" } else { "\n" }
}

/// 值语义：单引号字面量或无展开语法的文本为 Literal；含 `$` / 反引号为 Expression。
pub fn classify_value(value: &str, quote: char) -> ValueKind {
    if quote == '\'' {
        ValueKind::Literal
    } else if value.contains('$') || value.contains('`') {
        ValueKind::Expression
    } else {
        ValueKind::Literal
    }
}

/// shell 别名允许 `-` 和 `.`，甚至数字开头；拒绝 `-` 前缀（与 `alias -g` 等选项冲突）。
pub fn is_valid_alias_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().next().is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}

/// 环境变量名：字母或下划线开头，仅字母数字下划线。
pub fn is_valid_env_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// 解析单行 alias，返回 (name, value, 引号字符, 值语义)。
pub fn parse_alias_line(line: &str) -> Option<(String, String, char, ValueKind)> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix("alias")?;
    if !rest.starts_with(|c: char| c.is_whitespace()) {
        return None;
    }
    let rest = rest.trim();
    let eq = rest.find('=')?;
    let name = rest[..eq].trim();
    if !is_valid_alias_name(name) {
        return None;
    }
    let (value, quote) = unquote(rest[eq + 1..].trim());
    let kind = classify_value(&value, quote);
    Some((name.to_string(), value, quote, kind))
}

/// 解析单行赋值：`export KEY=v`（exported=true）或顶层 `KEY=v`（exported=false）。
/// 返回 (key, value, 引号字符, 值语义, exported)。
pub fn parse_assignment_line(line: &str) -> Option<(String, String, char, ValueKind, bool)> {
    let trimmed = line.trim_start();
    let (rest, exported) = match trimmed.strip_prefix("export ") {
        Some(r) => (r.trim_start(), true),
        None => (trimmed, false),
    };
    let eq = rest.find('=')?;
    let key = rest[..eq].trim();
    if !is_valid_env_name(key) {
        return None;
    }
    let (value, quote) = unquote(rest[eq + 1..].trim());
    let kind = classify_value(&value, quote);
    Some((key.to_string(), value, quote, kind, exported))
}

/// 去引号：单引号原样；双引号还原 shell 转义（`\"` → `"`、`\\` → `\`）。
fn unquote(raw: &str) -> (String, char) {
    let bytes = raw.as_bytes();
    if bytes.len() >= 2 && (raw.starts_with('\'') || raw.starts_with('"')) && raw.ends_with(bytes[0] as char) {
        let quote = bytes[0] as char;
        let inner = &raw[1..raw.len() - 1];
        let value = if quote == '"' {
            inner.replace("\\\"", "\"").replace("\\\\", "\\")
        } else {
            inner.to_string()
        };
        (value, quote)
    } else {
        (raw.to_string(), '\0')
    }
}

/// 从注释行提取分组标题；被注释掉的 alias（如 `#alias ng="nest generate"`）不是分组。
pub fn group_title(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let text = trimmed.strip_prefix('#')?.trim();
    if text.is_empty() {
        return None;
    }
    let inner = text.trim_start();
    if inner.starts_with("alias") && inner.contains('=') {
        return None;
    }
    Some(text.to_string())
}

/// 拆分行尾注释：取引号外的首个 `#`，返回（别名部分, 描述文本）。
/// 单引号内的 `#` 属于值本身不参与切分；描述去除首个 `#` 并 trim。
pub fn split_trailing_comment(line: &str) -> (&str, Option<String>) {
    let mut quote: Option<char> = None;
    for (pos, ch) in line.char_indices() {
        match quote {
            Some(q) => {
                if ch == q {
                    quote = None;
                }
            }
            None => {
                if ch == '\'' || ch == '"' {
                    quote = Some(ch);
                } else if ch == '#' {
                    let comment = line[pos..].trim_start_matches('#').trim();
                    let description = if comment.is_empty() { None } else { Some(comment.to_string()) };
                    return (&line[..pos], description);
                }
            }
        }
    }
    (line, None)
}

/// 别名条目扫描：分组 = 最近一条分组注释（跨空行保留）；描述 = alias 行尾 `#` 注释。
pub fn scan_alias(raw: &str) -> Vec<AliasEntry> {
    let mut group: Option<String> = None;
    let mut entries = Vec::new();
    for (index, line) in split_lines(raw).iter().enumerate() {
        let text = line.text(raw);
        if let Some(title) = group_title(&text) {
            group = Some(title);
            continue;
        }
        let (alias_part, description) = split_trailing_comment(&text);
        if let Some((name, value, _, kind)) = parse_alias_line(alias_part) {
            entries.push(AliasEntry {
                id: format!("alias:{name}"),
                name,
                value,
                value_kind: kind,
                description,
                group: group.clone(),
                line: index + 1,
                raw: text.to_string(),
            });
        }
    }
    entries
}

/// 托管区块扫描结果。
pub struct BlockScan {
    /// (begin_index, end_index) 0-based，含标记行；仅在一对标记且配对时给出
    pub range: Option<(usize, usize)>,
    /// 结构问题：多标记、不配对；非空时托管写操作被拒绝
    pub issues: Vec<String>,
    /// 使用旧格式标记的行索引（可经 block_normalize 规范化）
    pub old_markers: Vec<usize>,
}

fn is_begin_marker(line: &str) -> bool {
    line.trim_start().starts_with("# >>> xlt-managed env")
}

fn is_end_marker(line: &str) -> bool {
    line.trim_start().starts_with("# <<< xlt-managed env")
}

pub fn managed_block_scan(texts: &[&str]) -> BlockScan {
    let begins: Vec<usize> = texts
        .iter()
        .enumerate()
        .filter(|(_, l)| is_begin_marker(l))
        .map(|(i, _)| i)
        .collect();
    let ends: Vec<usize> = texts
        .iter()
        .enumerate()
        .filter(|(_, l)| is_end_marker(l))
        .map(|(i, _)| i)
        .collect();
    let mut issues = Vec::new();
    if begins.len() > 1 || ends.len() > 1 {
        issues.push(format!("发现 {} 个起始标记、{} 个结束标记，托管区块必须唯一", begins.len(), ends.len()));
    }
    let range = if begins.len() == 1 && ends.len() == 1 {
        if ends[0] > begins[0] {
            Some((begins[0], ends[0]))
        } else {
            issues.push("结束标记出现在起始标记之前".to_string());
            None
        }
    } else {
        if begins.len() == 1 && ends.is_empty() {
            issues.push("起始标记缺少对应的结束标记".to_string());
        }
        if ends.len() == 1 && begins.is_empty() {
            issues.push("结束标记缺少对应的起始标记".to_string());
        }
        None
    };
    let old_markers: Vec<usize> = texts
        .iter()
        .enumerate()
        .filter(|(_, l)| {
            let t = l.trim();
            (is_begin_marker(l) && t != MANAGED_BEGIN) || (is_end_marker(l) && t != MANAGED_END)
        })
        .map(|(i, _)| i)
        .collect();
    BlockScan { range, issues, old_markers }
}

/// 区块 opener / closer 关键字（启发式顶层判定，仅用于标注，不做解释）。
fn is_block_opener(trimmed: &str) -> bool {
    let first = trimmed.split_whitespace().next().unwrap_or("");
    matches!(first, "if" | "for" | "while" | "until" | "case" | "select") || trimmed.ends_with('{')
}

fn is_block_closer(trimmed: &str) -> bool {
    let first = trimmed.split_whitespace().next().unwrap_or("");
    matches!(first, "fi" | "done" | "esac") || trimmed == "}" || trimmed.starts_with("}")
}

/// 通用赋值定义扫描结果（供所有来源文件复用）。
#[derive(Clone, Debug)]
pub struct RawDefinition {
    /// 0-based 行索引
    pub index: usize,
    pub key: String,
    pub value: String,
    pub quote: char,
    pub value_kind: ValueKind,
    pub exported: bool,
    /// 顶层（非条件块 / 函数体 / heredoc / 续行）
    pub top_level: bool,
}

/// 上下文扫描：返回候选语句行的 (行索引, 是否顶层)。
/// 候选 = 非空、非注释、非 heredoc 体、非续行；是否顶层由条件块深度判定。
fn context_scan(raw: &str) -> Vec<(usize, bool)> {
    let lines = split_lines(raw);
    let texts: Vec<&str> = lines.iter().map(|l| l.text(raw)).collect();
    let mut result = Vec::new();
    let mut depth = 0usize;
    let mut heredoc: Option<String> = None;
    let mut continuation = false;

    for (index, line) in texts.iter().enumerate() {
        let trimmed = line.trim();

        // heredoc 体：直到定界符独占一行
        if let Some(delim) = &heredoc {
            if trimmed == delim.as_str() {
                heredoc = None;
            }
            continue;
        }
        // 上一行以 `\` 结尾：本行是续行
        if continuation {
            continuation = line.ends_with('\\');
            continue;
        }

        let top_level = depth == 0;
        let is_statement = !trimmed.is_empty() && !trimmed.starts_with('#');
        if is_statement {
            result.push((index, top_level));
        }

        // 更新上下文
        if line.ends_with('\\') {
            continuation = true;
        }
        if is_block_closer(trimmed) {
            depth = depth.saturating_sub(1);
        } else if is_block_opener(trimmed) {
            depth += 1;
        }
        if let Some(pos) = trimmed.find("<<") {
            if !trimmed[pos..].starts_with("<<<") {
                let rest = &trimmed[pos + 2..];
                let rest = rest.strip_prefix('-').unwrap_or(rest);
                let delim: String = rest
                    .trim_start_matches(['"', '\''])
                    .chars()
                    .take_while(|c| !c.is_whitespace() && *c != '"' && *c != '\'')
                    .collect();
                if !delim.is_empty() {
                    heredoc = Some(delim);
                }
            }
        }
    }
    result
}

/// 通用赋值定义扫描：带条件块深度、heredoc、续行的顶层上下文判定。
/// 条件块 / 函数体内的定义是动态上下文，值语义强制 Expression（不可表格编辑）。
pub fn scan_definitions(raw: &str) -> Vec<RawDefinition> {
    let lines = split_lines(raw);
    let texts: Vec<&str> = lines.iter().map(|l| l.text(raw)).collect();
    let mut definitions = Vec::new();

    for (index, top_level) in context_scan(raw) {
        let line = texts[index];
        if let Some((key, value, quote, mut kind, exported)) = parse_assignment_line(line) {
            if !top_level {
                kind = ValueKind::Expression;
            }
            definitions.push(RawDefinition {
                index,
                key,
                value,
                quote,
                value_kind: kind,
                exported,
                top_level,
            });
        }
    }
    definitions
}

/// 顶层语句行索引（供 `source` 引用等非赋值语句的静态识别）。
pub fn top_level_statement_lines(raw: &str) -> Vec<usize> {
    context_scan(raw)
        .into_iter()
        .filter(|(_, top_level)| *top_level)
        .map(|(index, _)| index)
        .collect()
}

/// `.zshrc` 环境条目扫描：在通用定义扫描之上叠加托管区块标记。
pub fn scan_zshrc(raw: &str) -> (Vec<EnvEntry>, Option<BlockInfo>, Vec<String>) {
    let lines = split_lines(raw);
    let texts: Vec<&str> = lines.iter().map(|l| l.text(raw)).collect();
    let scan = managed_block_scan(&texts);
    let mut entries = Vec::new();

    for def in scan_definitions(raw) {
        let managed = scan.range.is_some_and(|(b, e)| def.index > b && def.index < e);
        let id = if managed {
            format!("env:{}", def.key)
        } else {
            format!("env:{}@{}", def.key, def.index)
        };
        entries.push(EnvEntry {
            id,
            key: def.key,
            value: def.value,
            value_kind: def.value_kind,
            exported: def.exported,
            managed,
            line: def.index + 1,
            raw: texts[def.index].to_string(),
        });
    }

    let block = scan.range.map(|(b, e)| BlockInfo {
        begin_line: b + 1,
        end_line: e + 1,
        entry_count: entries.iter().filter(|entry| entry.managed).count(),
    });
    (entries, block, scan.issues)
}
