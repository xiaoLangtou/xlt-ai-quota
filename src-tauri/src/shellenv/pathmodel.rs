//! R1 只读 PATH 模型：条目分类、诊断、静态合并展示。
//!
//! 边界（PRD 7.2）：
//! - 条目区分 literal / home-relative / variable-expression / dynamic / relative /
//!   empty / reference；home-relative 是类型化路径，由后端用已知主目录解析，
//!   不使用 eval；
//! - 合并结果只包含经静态证明的顶层定义与系统清单；条件块 / 函数体内的添加语句
//!   单独列出并标记「动态未求值」，不混入最终列表；
//! - 重复比较先以原文 / 可确定的规范路径判断，不把大小写或软链接视为同一目录；
//! - 目录检查只是存在性检查，不是程序健康检查，不执行目录内程序；
//! - 诊断措辞保守：基于「当前扫描文件」的事实，不声称所有终端的结果。

use crate::shellenv::parser::{scan_definitions, split_lines, top_level_statement_lines};
use crate::shellenv::source::{literal_source_target, ScannedSet};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// `$PATH` / `${PATH}` / `$path` 引用段：合并的拼接点。
fn is_reference(text: &str) -> bool {
    matches!(text, "$PATH" | "${PATH}" | "$path" | "${path}" | "${=path}" | "${=PATH}")
}

/// 单段分类。`quote` 为定义该值时使用的引号：单引号内一律字面量文本。
fn classify_segment(text: &str, quote: char, home: &Path) -> (String, Option<String>) {
    if text.is_empty() {
        return ("empty".to_string(), None);
    }
    if is_reference(text) {
        return ("reference".to_string(), None);
    }
    if quote == '\'' {
        // 单引号：不展开，按字面量文本处理
        return ("literal".to_string(), Some(text.to_string()));
    }
    // home-relative：~/ 与 $HOME/ 两种写法等价，均为类型化路径
    if text.starts_with("~/")
        || text == "~"
        || text.starts_with("$HOME/")
        || text == "$HOME"
    {
        let normalized = if let Some(rest) = text.strip_prefix("$HOME") {
            format!("~{rest}")
        } else {
            text.to_string()
        };
        let resolved = crate::shellenv::source::resolve_path_text(&normalized, home);
        return ("home-relative".to_string(), Some(resolved));
    }
    if text.contains('$') || text.contains('`') {
        return ("variable-expression".to_string(), None);
    }
    if text.starts_with('/') {
        return ("literal".to_string(), Some(text.to_string()));
    }
    if text.contains('(') || text.contains('*') || text.contains('?') {
        return ("dynamic".to_string(), None);
    }
    ("relative".to_string(), None)
}

// ---------------------------------------------------------------------------
// DTO
// ---------------------------------------------------------------------------

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathSegment {
    pub text: String,
    /// literal / home-relative / variable-expression / dynamic / relative / empty / reference
    pub kind: String,
    pub resolved: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathContribution {
    pub file_id: String,
    pub path: String,
    pub line: usize,
    /// 归属工具（同来源模型规则），未知为 unknown
    pub owner: String,
    /// static（顶层）/ dynamic（条件块等，未纳入合并）/ system（系统清单）
    pub context: String,
    /// prepend / append / replace（相对 $PATH 引用点）
    pub mode: String,
    /// 原始语句（数组为整段）
    pub raw: String,
    pub segments: Vec<PathSegment>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathDiagnostic {
    /// duplicate / relative / empty-entry / missing / dynamic-note
    pub code: String,
    /// info / warn
    pub severity: String,
    pub message: String,
    /// 证据位置列表：`file:line`
    pub sources: Vec<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergedPathEntry {
    pub text: String,
    pub kind: String,
    /// 可确定的解析路径（literal / home-relative），用于重复比较
    pub resolved: Option<String>,
    /// static / system
    pub certainty: String,
    /// 添加位置：`file:line`
    pub via: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathSummary {
    pub contribution_count: usize,
    pub dynamic_count: usize,
    pub merged_count: usize,
    pub duplicate_groups: usize,
    pub missing_count: usize,
    pub relative_count: usize,
    pub empty_count: usize,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathReport {
    pub contributions: Vec<PathContribution>,
    /// 静态合并结果（只读展示；动态语句未纳入，见 diagnostics 与 dynamic_count）
    pub merged: Vec<MergedPathEntry>,
    pub diagnostics: Vec<PathDiagnostic>,
    pub summary: PathSummary,
}

// ---------------------------------------------------------------------------
// 解析
// ---------------------------------------------------------------------------

/// `path=(a b $path c)` 单行数组解析：返回元素列表；跨行数组返回 None（不支持并跳过）。
fn parse_path_array(line: &str) -> Option<Vec<String>> {
    let trimmed = line.trim();
    let body = trimmed
        .strip_prefix("export ")
        .map(|rest| rest.trim_start())
        .unwrap_or(trimmed);
    let body = body.strip_prefix("path=(")?;
    let end = body.rfind(')')?;
    let inner = &body[..end];
    if body[end + 1..].trim().contains(')') {
        return None;
    }
    // 逐元素切分（尊重单双引号）
    let mut elements = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    for ch in inner.chars() {
        match quote {
            Some(q) => {
                if ch == q {
                    quote = None;
                } else {
                    current.push(ch);
                }
            }
            None => {
                if ch == '\'' || ch == '"' {
                    quote = Some(ch);
                } else if ch.is_whitespace() {
                    if !current.is_empty() {
                        elements.push(current.clone());
                        current.clear();
                    }
                } else {
                    current.push(ch);
                }
            }
        }
    }
    if !current.is_empty() {
        elements.push(current);
    }
    Some(elements)
}

fn segments_from_value(value: &str, quote: char, home: &Path) -> Vec<PathSegment> {
    value
        .split(':')
        .map(|segment| {
            let (kind, resolved) = classify_segment(segment, quote, home);
            PathSegment { text: segment.to_string(), kind, resolved }
        })
        .collect()
}

fn mode_of(segments: &[PathSegment]) -> String {
    let reference_at = segments.iter().position(|segment| segment.kind == "reference");
    match reference_at {
        Some(0) => "append".to_string(),
        Some(_) => "prepend".to_string(),
        None => {
            if segments.is_empty() {
                "append".to_string()
            } else {
                "replace".to_string()
            }
        }
    }
}

/// 从扫描集合解析全部 PATH 贡献语句（含动态上下文）。
fn collect_contributions(set: &ScannedSet, home: &Path) -> Vec<PathContribution> {
    let mut contributions = Vec::new();
    for scanned in &set.files {
        if !scanned.meta.included || scanned.is_path_list {
            continue;
        }
        for def in scan_definitions(&scanned.raw) {
            let line_text = split_lines(&scanned.raw)[def.index].text(&scanned.raw).to_string();
            // path=( ...) 数组
            if let Some(elements) = parse_path_array(&line_text) {
                let segments: Vec<PathSegment> = elements
                    .into_iter()
                    .map(|element| {
                        let quote = if line_text.contains('\'') { '\'' } else { '"' };
                        let (kind, resolved) = classify_segment(&element, quote, home);
                        PathSegment { text: element, kind, resolved }
                    })
                    .collect();
                contributions.push(PathContribution {
                    file_id: scanned.meta.id.clone(),
                    path: scanned.meta.path.clone(),
                    line: def.index + 1,
                    owner: owner_for_line(&scanned.raw, def.index, &line_text),
                    context: if def.top_level { "static" } else { "dynamic" }.to_string(),
                    mode: mode_of(&segments),
                    raw: line_text,
                    segments,
                });
                continue;
            }
            if def.key != "PATH" && def.key != "path" {
                continue;
            }
            let segments = segments_from_value(&def.value, def.quote, home);
            contributions.push(PathContribution {
                file_id: scanned.meta.id.clone(),
                path: scanned.meta.path.clone(),
                line: def.index + 1,
                owner: owner_for_line(&scanned.raw, def.index, &line_text),
                context: if def.top_level { "static" } else { "dynamic" }.to_string(),
                mode: mode_of(&segments),
                raw: line_text,
                segments,
            });
        }
    }
    contributions
}

fn owner_for_line(raw: &str, index: usize, line: &str) -> String {
    let (owner, _) = crate::shellenv::source::detect_owner(raw, index, line);
    owner
}

// ---------------------------------------------------------------------------
// 静态合并
// ---------------------------------------------------------------------------

struct MergeState {
    merged: Vec<MergedPathEntry>,
}

impl MergeState {
    fn apply(&mut self, contribution: &PathContribution) {
        let via = format!("{}:{}", contribution.path, contribution.line);
        let new_entries: Vec<MergedPathEntry> = contribution
            .segments
            .iter()
            .filter(|segment| segment.kind != "reference" && segment.kind != "empty")
            .map(|segment| MergedPathEntry {
                text: segment.text.clone(),
                kind: segment.kind.clone(),
                resolved: segment.resolved.clone(),
                certainty: if contribution.context == "system" { "system" } else { "static" }.to_string(),
                via: via.clone(),
            })
            .collect();
        match contribution.mode.as_str() {
            "prepend" => {
                // 新段在前、原有列表接在引用点之后
                let mut next = new_entries;
                next.extend(self.merged.drain(..));
                self.merged = next;
            }
            "replace" => {
                self.merged = new_entries;
            }
            _ => {
                self.merged.extend(new_entries);
            }
        }
    }
}

/// 静态合并：按 zsh 近似加载顺序处理顶层语句，source 引用内联展开；
/// 动态（条件块内）语句不参与合并，仅计数并在诊断中说明。
fn merge_static(set: &ScannedSet, contributions: &[PathContribution], home: &Path) -> Vec<MergedPathEntry> {
    let by_path: HashMap<&str, usize> = set
        .files
        .iter()
        .enumerate()
        .filter(|(_, scanned)| scanned.meta.included && !scanned.is_path_list)
        .map(|(index, scanned)| (scanned.meta.path.as_str(), index))
        .collect();
    // 静态贡献索引：(file_path, line) → contribution
    let static_by_location: HashMap<(&str, usize), &PathContribution> = contributions
        .iter()
        .filter(|contribution| contribution.context == "static")
        .map(|contribution| (contribution.path.as_str(), contribution.line, contribution))
        .map(|(path, line, contribution)| ((path, line), contribution))
        .collect();
    // source 引用目标（静态、顶层、字面量）
    let source_targets: HashMap<(&str, usize), PathBuf> = set
        .files
        .iter()
        .filter(|scanned| scanned.meta.included && !scanned.is_path_list)
        .flat_map(|scanned| {
            top_level_statement_lines(&scanned.raw)
                .into_iter()
                .filter_map(|index| {
                    let line = split_lines(&scanned.raw)[index].text(&scanned.raw).to_string();
                    literal_source_target(&line).map(|token| ((scanned.meta.path.as_str(), index + 1), PathBuf::from(crate::shellenv::source::resolve_path_text(&token, home))))
                })
                .collect::<Vec<_>>()
        })
        .collect();

    let mut state = MergeState { merged: Vec::new() };
    fn walk(
        state: &mut MergeState,
        set: &ScannedSet,
        by_path: &HashMap<&str, usize>,
        static_by_location: &HashMap<(&str, usize), &PathContribution>,
        source_targets: &HashMap<(&str, usize), PathBuf>,
        file_index: usize,
        depth: usize,
        visited: &mut HashSet<usize>,
    ) {
        if depth > crate::shellenv::source::MAX_DEPTH || !visited.insert(file_index) {
            return;
        }
        let scanned = &set.files[file_index];
        // 系统清单文件：逐行目录，按顺序追加
        if scanned.is_path_list {
            for (index, line) in split_lines(&scanned.raw).iter().enumerate() {
                let text = line.text(&scanned.raw).trim().to_string();
                if text.is_empty() || text.starts_with('#') {
                    continue;
                }
                state.apply(&PathContribution {
                    file_id: scanned.meta.id.clone(),
                    path: scanned.meta.path.clone(),
                    line: index + 1,
                    owner: String::new(),
                    context: "system".to_string(),
                    mode: "append".to_string(),
                    raw: text.clone(),
                    segments: vec![PathSegment { text, kind: "system".to_string(), resolved: None }],
                });
            }
            return;
        }
        for index in top_level_statement_lines(&scanned.raw) {
            let line_number = index + 1;
            if let Some(target) = source_targets.get(&(scanned.meta.path.as_str(), line_number)) {
                if let Some(&target_index) = by_path.get(target.to_string_lossy().as_ref()) {
                    walk(state, set, by_path, static_by_location, source_targets, target_index, depth + 1, visited);
                    continue;
                }
            }
            if let Some(contribution) = static_by_location.get(&(scanned.meta.path.as_str(), line_number)) {
                state.apply(contribution);
            }
        }
    }

    let mut visited: HashSet<usize> = HashSet::new();
    // 加载顺序：zshenv → 系统清单 → zprofile → zshrc（alias 无 PATH 语义但保持引用内联）
    let order_roles = ["zshenv", "system-paths", "system-paths-d", "zprofile", "zshrc"];
    for role in order_roles {
        for (index, scanned) in set.files.iter().enumerate() {
            if scanned.meta.included && scanned.meta.role == role {
                walk(&mut state, set, &by_path, &static_by_location, &source_targets, index, 1, &mut visited);
            }
        }
    }
    // sourced 文件若未被内联（如目标存在但引用方未被扫描），按发现顺序兜底追加
    for (index, scanned) in set.files.iter().enumerate() {
        if scanned.meta.included && scanned.meta.role == "sourced" && !visited.contains(&index) {
            walk(&mut state, set, &by_path, &static_by_location, &source_targets, index, 1, &mut visited);
        }
    }
    state.merged
}

// ---------------------------------------------------------------------------
// 诊断
// ---------------------------------------------------------------------------

pub fn build_diagnostics(
    contributions: &[PathContribution],
    merged: &[MergedPathEntry],
) -> Vec<PathDiagnostic> {
    let mut diagnostics = Vec::new();

    // 动态语句说明
    let dynamic: Vec<&PathContribution> =
        contributions.iter().filter(|contribution| contribution.context == "dynamic").collect();
    if !dynamic.is_empty() {
        diagnostics.push(PathDiagnostic {
            code: "dynamic-note".to_string(),
            severity: "info".to_string(),
            message: format!(
                "当前扫描文件含 {} 处条件 / 动态上下文中的 PATH 修改语句，未纳入合并结果（运行时是否生效取决于条件）",
                dynamic.len()
            ),
            sources: dynamic
                .iter()
                .map(|contribution| format!("{}:{}", contribution.path, contribution.line))
                .collect(),
        });
    }

    // 重复：合并结果中同 key 出现多次（literal/home-relative 以解析路径；其余按原文）
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();
    for entry in merged {
        groups
            .entry(dedup_key(entry))
            .or_default()
            .push(format!("{}（{}）", entry.via, entry.text));
    }
    let mut duplicates: Vec<(String, Vec<String>)> = groups
        .into_iter()
        .filter(|(_, sources)| sources.len() > 1)
        .collect();
    duplicates.sort();
    for (key, sources) in duplicates {
        diagnostics.push(PathDiagnostic {
            code: "duplicate".to_string(),
            severity: "warn".to_string(),
            message: format!("目录 {key} 在当前扫描范围内出现 {} 次（重复不改变查找结果，仅增加查找成本）", sources.len()),
            sources,
        });
    }

    // 相对目录 / 空条目 / 缺失（对贡献语句的段逐一检查，存在性检查有缓存）
    let mut missing_seen: HashMap<String, Vec<String>> = HashMap::new();
    for contribution in contributions {
        for segment in &contribution.segments {
            let at = format!("{}:{}", contribution.path, contribution.line);
            match segment.kind.as_str() {
                "relative" => {
                    diagnostics.push(PathDiagnostic {
                        code: "relative".to_string(),
                        severity: "warn".to_string(),
                        message: format!(
                            "相对目录「{}」——相对当前工作目录解析，通常为误写",
                            segment.text
                        ),
                        sources: vec![at],
                    });
                }
                "empty" => {
                    diagnostics.push(PathDiagnostic {
                        code: "empty-entry".to_string(),
                        severity: "warn".to_string(),
                        message: "空 PATH 条目（连续冒号或首尾冒号）——zsh 将其解释为当前目录".to_string(),
                        sources: vec![at],
                    });
                }
                "literal" | "home-relative" => {
                    if let Some(resolved) = &segment.resolved {
                        if !Path::new(resolved).is_dir() {
                            missing_seen
                                .entry(resolved.clone())
                                .or_default()
                                .push(at);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    let mut missing: Vec<(String, Vec<String>)> = missing_seen.into_iter().collect();
    missing.sort();
    for (path, sources) in missing {
        diagnostics.push(PathDiagnostic {
            code: "missing".to_string(),
            severity: "warn".to_string(),
            message: format!("目录 {path} 在当前扫描时刻不存在（可能是条件安装或拼写错误）"),
            sources,
        });
    }
    diagnostics
}

fn dedup_key(entry: &MergedPathEntry) -> String {
    // 可确定的解析路径优先；其余按原文
    entry.resolved.clone().unwrap_or_else(|| entry.text.clone())
}

// ---------------------------------------------------------------------------
// 报告入口
// ---------------------------------------------------------------------------

pub fn build_path_report(set: &ScannedSet, home: &Path) -> PathReport {
    let contributions = collect_contributions(set, home);
    let merged = merge_static(set, &contributions, home);
    let diagnostics = build_diagnostics(&contributions, &merged);
    let summary = PathSummary {
        contribution_count: contributions.len(),
        dynamic_count: contributions.iter().filter(|c| c.context == "dynamic").count(),
        merged_count: merged.len(),
        duplicate_groups: diagnostics.iter().filter(|d| d.code == "duplicate").count(),
        missing_count: diagnostics.iter().filter(|d| d.code == "missing").count(),
        relative_count: diagnostics.iter().filter(|d| d.code == "relative").count(),
        empty_count: diagnostics.iter().filter(|d| d.code == "empty-entry").count(),
    };
    PathReport { contributions, merged, diagnostics, summary }
}
