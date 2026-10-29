//! 变更计划管线（B03 / R0-02）：preview（预计算后像）→ commit（版本比对 + 单次消费）→ cancel。
//!
//! 约束：
//! - 所有写入口（表格编辑、源码整写、备份恢复、区块规范化）都经 preview / commit；
//! - 计划 5 分钟过期、最多 16 个、单次消费；
//! - commit 时重新读取目标文件并比对基线版本，外部修改导致版本漂移时拒绝提交（B10）；
//! - 提交互斥由 commands 层的 ShellEnvLock 串行化。
//!
//! 测试策略：`build_change_for_doc` 是纯核心（输入文档 → 编辑 + 后像），测试用合成文档
//! 驱动，不触碰真实用户配置；commit 通过临时目录目标文件验证。

use crate::shellenv::files::{
    self, apply_edits, commit_write, content_version, kind_path, read_backup, validate_content,
    LineEdit,
};
use crate::shellenv::parser::{
    group_title, is_valid_alias_name, is_valid_env_name, managed_block_scan, parse_alias_line,
    scan_alias, scan_zshrc, split_lines,
};
use crate::shellenv::serializer::{
    format_alias_with_description, format_export, literal_edit_conflict, session_command,
    value_kind_label,
    SessionChange, MANAGED_BEGIN, MANAGED_END,
};
use crate::shellenv::types::*;
use chrono::Local;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const PLAN_TTL: Duration = Duration::from_secs(5 * 60);
const PLAN_MAX: usize = 16;

#[derive(Clone)]
pub(crate) struct StoredPlan {
    pub plan: ShellChangePlan,
    pub target_path: PathBuf,
    /// 预计算的整文件后像（preview 时固定；restore 的后像即备份内容）
    pub postimage: String,
    pub created: Instant,
}

fn plans() -> &'static Mutex<HashMap<String, StoredPlan>> {
    static PLANS: OnceLock<Mutex<HashMap<String, StoredPlan>>> = OnceLock::new();
    PLANS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 目标文件当前内容文档。
#[derive(Clone)]
pub(crate) struct Doc {
    pub raw: String,
    pub exists: bool,
    /// 内容 sha256，preview 用作计划基线
    pub version: String,
}

impl Doc {
    pub fn from_raw(raw: impl Into<String>) -> Self {
        let raw = raw.into();
        let version = content_version(&raw);
        Doc { exists: true, version, raw }
    }
}

fn load_doc(kind: &str) -> Result<Doc, String> {
    let path = kind_path(kind)?;
    match files::read_raw(&path)? {
        Some(raw) => Ok(Doc::from_raw(raw)),
        None => Ok(Doc { raw: String::new(), exists: false, version: content_version("") }),
    }
}

/// 由原始内容构建文件快照（UI 数据源）。
pub fn build_snapshot(kind: &str) -> Result<FileSnapshot, String> {
    let path = kind_path(kind)?;
    let raw = files::read_raw(&path)?.unwrap_or_default();
    let exists = path.exists();
    Ok(snapshot_from_content(kind, &path, raw, exists))
}

fn snapshot_from_content(kind: &str, path: &PathBuf, raw: String, exists: bool) -> FileSnapshot {
    let (aliases, envs, managed_block, block_issues);
    if kind == KIND_ALIAS {
        aliases = scan_alias(&raw);
        envs = Vec::new();
        managed_block = None;
        block_issues = Vec::new();
    } else {
        let (env_list, block, issues) = scan_zshrc(&raw);
        aliases = Vec::new();
        envs = env_list;
        managed_block = block;
        block_issues = issues;
    }
    FileSnapshot {
        kind: kind.to_string(),
        path: path.display().to_string(),
        exists,
        version: content_version(&raw),
        line_ending: if raw.contains("\r\n") { "crlf" } else { "lf" }.to_string(),
        content: raw,
        aliases,
        envs,
        managed_block,
        block_issues,
    }
}

fn op(action: impl Into<String>, detail: impl Into<String>) -> PlanOp {
    PlanOp { action: action.into(), detail: detail.into() }
}

// ---------------------------------------------------------------------------
// preview
// ---------------------------------------------------------------------------

pub fn preview(request: ShellChangeRequest) -> Result<ShellChangePlan, String> {
    let kind = request_kind(&request);
    // 只读一次盘：后像与版本基线必须来自同一份内容，否则两次读盘之间
    // 的外部修改会被静默覆盖
    let doc = load_doc(&kind)?;
    let (kind, _edits, postimage, ops, session) = build_change_for_doc(request, doc.clone())?;
    let path = kind_path(&kind)?;

    // 后像预校验：语法不过的计划不允许进入确认环节
    validate_content(&path, &postimage)?;

    let plan = ShellChangePlan {
        id: uuid::Uuid::new_v4().to_string(),
        kind: kind.clone(),
        base_version: doc.version.clone(),
        ops,
        session_command: session,
        postimage: postimage.clone(),
        created_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    };
    store_plan(StoredPlan {
        plan: plan.clone(),
        target_path: path,
        postimage,
        created: Instant::now(),
    })?;
    Ok(plan)
}

fn request_kind(request: &ShellChangeRequest) -> String {
    match request {
        ShellChangeRequest::AliasAdd { .. }
        | ShellChangeRequest::AliasEdit { .. }
        | ShellChangeRequest::AliasRename { .. }
        | ShellChangeRequest::AliasDelete { .. } => KIND_ALIAS.to_string(),
        _ => KIND_ZSHRC.to_string(),
    }
}

/// 组装变更：返回 (目标类型, 行编辑, 后像, 差异描述, 会话命令)。纯核心，供测试复用。
pub(crate) fn build_change_for_doc(
    request: ShellChangeRequest,
    doc: Doc,
) -> Result<(String, Vec<LineEdit>, String, Vec<PlanOp>, String), String> {
    match request {
        ShellChangeRequest::AliasAdd { name, value, group, description } => {
            let name = name.trim().to_string();
            if !is_valid_alias_name(&name) {
                return Err("别名名称无效：仅允许字母、数字、下划线、连字符和点".to_string());
            }
            if value.trim().is_empty() {
                return Err("别名命令不能为空".to_string());
            }
            if literal_edit_conflict(&value) {
                return Err(
                    "别名命令包含 $ 或反引号（展开语义），请改用源码视图编辑以保证语义一致".to_string(),
                );
            }
            let entries = scan_alias(&doc.raw);
            if entries.iter().any(|entry| entry.name == name) {
                return Err(format!("别名 {name} 已存在，请使用编辑而不是新增"));
            }
            let formatted =
                format_alias_with_description(&name, &value, description.as_deref());
            let group = group.as_deref().map(str::trim).filter(|g| !g.is_empty());
            let lines = split_lines(&doc.raw);
            let (edits, detail) = match group {
                Some(group) => {
                    let title_index = lines
                        .iter()
                        .enumerate()
                        .find(|(_, line)| group_title(line.text(&doc.raw)).is_some_and(|t| t == group))
                        .map(|(index, _)| index);
                    match title_index {
                        Some(title) => {
                            // 组尾 = 组内最后一个 alias 行（下一个分组注释前）；空组插在组头之后
                            let mut insert_after = title;
                            for (index, line) in lines.iter().enumerate().skip(title + 1) {
                                let text = line.text(&doc.raw);
                                if group_title(&text).is_some() {
                                    break;
                                }
                                if parse_alias_line(&text).is_some() {
                                    insert_after = index;
                                }
                            }
                            (
                                vec![LineEdit::InsertAfter { index: insert_after, text: formatted.clone() }],
                                format!("alias {name}={formatted}（插入到分组「{group}」末尾）"),
                            )
                        }
                        None => (
                            vec![
                                // 分组不存在：先建组头注释，再写入别名
                                LineEdit::Append { text: format!("# {group}"), blank_before: true },
                                LineEdit::Append { text: formatted.clone(), blank_before: false },
                            ],
                            format!("alias {name}={formatted}（创建分组「{group}」并写入）"),
                        ),
                    }
                }
                None => (
                    vec![LineEdit::Append { text: formatted.clone(), blank_before: !doc.raw.is_empty() }],
                    format!("alias {name}={formatted}（追加到文件末尾）"),
                ),
            };
            Ok((
                KIND_ALIAS.to_string(),
                edits.clone(),
                apply_edits(&doc.raw, &edits)?,
                vec![op("新增别名", detail)],
                session_command(&SessionChange::AliasAdd),
            ))
        }
        ShellChangeRequest::AliasEdit { entry_id, value, description } => {
            if value.trim().is_empty() {
                return Err("别名命令不能为空".to_string());
            }
            if literal_edit_conflict(&value) {
                return Err(
                    "别名命令包含 $ 或反引号（展开语义），请改用源码视图编辑以保证语义一致".to_string(),
                );
            }
            let entries = scan_alias(&doc.raw);
            let entry = entries
                .iter()
                .find(|entry| entry.id == entry_id)
                .ok_or_else(|| format!("别名条目 {entry_id} 不存在，请刷新后重试"))?;
            let description = description.as_deref().map(str::trim).filter(|text| !text.is_empty());
            if entry.value == value && entry.description.as_deref() == description {
                return Err("内容没有变化，无需保存".to_string());
            }
            if entry.value_kind != ValueKind::Literal {
                return Err(format!(
                    "别名 {} 的原值是{}（含展开语法），表格编辑会改变语义，请在源码视图中修改",
                    entry.name,
                    value_kind_label(entry.value_kind)
                ));
            }
            let edit = LineEdit::Replace {
                index: entry.line - 1,
                text: format_alias_with_description(&entry.name, &value, description),
            };
            let postimage = apply_edits(&doc.raw, &[edit.clone()])?;
            let description_note = match (entry.description.as_deref(), description) {
                (None, Some(new_text)) => format!("，描述 → {new_text}"),
                (Some(old_text), None) => format!("，移除描述「{old_text}」"),
                (Some(old_text), Some(new_text)) if old_text != new_text => {
                    format!("，描述 → {new_text}")
                }
                _ => String::new(),
            };
            Ok((
                KIND_ALIAS.to_string(),
                vec![edit],
                postimage,
                vec![op(
                    "修改别名",
                    format!(
                        "{}：{} → {}{}（第 {} 行）",
                        entry.name, entry.value, value, description_note, entry.line
                    ),
                )],
                session_command(&SessionChange::AliasEdit),
            ))
        }
        ShellChangeRequest::AliasRename { entry_id, new_name } => {
            let new_name = new_name.trim().to_string();
            if !is_valid_alias_name(&new_name) {
                return Err("别名名称无效：仅允许字母、数字、下划线、连字符和点".to_string());
            }
            let entries = scan_alias(&doc.raw);
            let entry = entries
                .iter()
                .find(|entry| entry.id == entry_id)
                .ok_or_else(|| format!("别名条目 {entry_id} 不存在，请刷新后重试"))?;
            if entry.name == new_name {
                return Err("名称没有变化".to_string());
            }
            if entries.iter().any(|e| e.name == new_name) {
                return Err(format!("别名 {new_name} 已存在"));
            }
            let edit = LineEdit::Replace {
                index: entry.line - 1,
                // 重命名保留原描述（行尾注释）
                text: format_alias_with_description(&new_name, &entry.value, entry.description.as_deref()),
            };
            let postimage = apply_edits(&doc.raw, &[edit.clone()])?;
            Ok((
                KIND_ALIAS.to_string(),
                vec![edit],
                postimage,
                vec![op(
                    "重命名别名",
                    format!("{} → {}（一次提交完成）", entry.name, new_name),
                )],
                session_command(&SessionChange::AliasRename),
            ))
        }
        ShellChangeRequest::AliasDelete { entry_id } => {
            let entries = scan_alias(&doc.raw);
            let entry = entries
                .iter()
                .find(|entry| entry.id == entry_id)
                .ok_or_else(|| format!("别名条目 {entry_id} 不存在，请刷新后重试"))?;
            let edit = LineEdit::Delete { index: entry.line - 1 };
            let postimage = apply_edits(&doc.raw, &[edit.clone()])?;
            Ok((
                KIND_ALIAS.to_string(),
                vec![edit],
                postimage,
                vec![op("删除别名", format!("{}（第 {} 行）", entry.name, entry.line))],
                session_command(&SessionChange::AliasDelete { name: &entry.name }),
            ))
        }
        ShellChangeRequest::EnvSet { key, value, confirm_block_create } => {
            let key = key.trim().to_string();
            if !is_valid_env_name(&key) {
                return Err("变量名无效：需以字母或下划线开头，仅包含字母、数字、下划线".to_string());
            }
            if literal_edit_conflict(&value) {
                return Err(
                    "变量值包含 $ 或反引号（展开语义），请改用源码视图编辑以保证语义一致".to_string(),
                );
            }
            let (entries, block, issues) = scan_zshrc(&doc.raw);
            if !issues.is_empty() {
                return Err(format!(
                    "托管区块结构异常：{}；请先规范化区块或用源码视图修改",
                    issues.join("；")
                ));
            }
            let formatted = format_export(&key, &value);
            let (edits, ops) = match block {
                Some(block) => {
                    let existing =
                        entries.iter().find(|entry| entry.managed && entry.key == key);
                    let outside = entries.iter().find(|entry| !entry.managed && entry.key == key);
                    let mut ops = Vec::new();
                    if let Some(outside) = outside {
                        ops.push(op(
                            "注意",
                            format!(
                                "区块外第 {} 行已有同名定义 {}，托管区位于文件末尾，生效时以托管值为准",
                                outside.line, outside.key
                            ),
                        ));
                    }
                    let edit = match existing {
                        Some(entry) => {
                            if entry.value == value {
                                return Err("内容没有变化，无需保存".to_string());
                            }
                            if entry.value_kind != ValueKind::Literal {
                                return Err(format!(
                                    "变量 {} 的原值是{}（含展开语法），表格编辑会改变语义，请在源码视图中修改",
                                    key,
                                    value_kind_label(entry.value_kind)
                                ));
                            }
                            LineEdit::Replace {
                                index: entry.line - 1,
                                text: formatted.clone(),
                            }
                        }
                        None => LineEdit::InsertAfter {
                            index: block.end_line - 2,
                            text: formatted.clone(),
                        },
                    };
                    let action = if existing.is_some() { "修改变量" } else { "新增变量" };
                    ops.insert(
                        0,
                        op(
                            action,
                            format!(
                                "export {key}={formatted}（托管区块第 {}—{} 行内）",
                                block.begin_line, block.end_line
                            ),
                        ),
                    );
                    (vec![edit], ops)
                }
                None => {
                    if confirm_block_create != Some(true) {
                        return Err("BLOCK_CREATE_CONFIRM_REQUIRED".to_string());
                    }
                    (
                        vec![
                            LineEdit::Append {
                                text: MANAGED_BEGIN.to_string(),
                                blank_before: true,
                            },
                            LineEdit::Append { text: formatted.clone(), blank_before: false },
                            LineEdit::Append { text: MANAGED_END.to_string(), blank_before: false },
                        ],
                        vec![
                            op(
                                "创建托管区块并写入变量",
                                format!("export {key}={formatted}（追加到 ~/.zshrc 顶层末尾）"),
                            ),
                            op(
                                "说明",
                                "首次创建的区块位于文件末尾顶层；Kiro / SDKMAN 等已有初始化区块保持原顺序不变"
                                    .to_string(),
                            ),
                        ],
                    )
                }
            };
            let postimage = apply_edits(&doc.raw, &edits)?;
            let session = session_command(&SessionChange::EnvSet { assignment: formatted });
            Ok((KIND_ZSHRC.to_string(), edits, postimage, ops, session))
        }
        ShellChangeRequest::EnvDelete { entry_id } => {
            let (entries, block, issues) = scan_zshrc(&doc.raw);
            if !issues.is_empty() {
                return Err(format!(
                    "托管区块结构异常：{}；请先规范化区块或用源码视图修改",
                    issues.join("；")
                ));
            }
            block.as_ref().ok_or_else(|| "托管区块不存在，无可删除的受管变量".to_string())?;
            let entry = entries
                .iter()
                .find(|entry| entry.managed && entry.id == entry_id)
                .ok_or_else(|| format!("托管变量条目 {entry_id} 不存在或不在托管区块内"))?;
            let edit = LineEdit::Delete { index: entry.line - 1 };
            let postimage = apply_edits(&doc.raw, &[edit.clone()])?;
            Ok((
                KIND_ZSHRC.to_string(),
                vec![edit],
                postimage,
                vec![op("删除变量", format!("{}（托管区块第 {} 行）", entry.key, entry.line))],
                session_command(&SessionChange::EnvDelete { key: &entry.key }),
            ))
        }
        ShellChangeRequest::SourceWrite { file_kind, content } => {
            if file_kind != KIND_ALIAS && file_kind != KIND_ZSHRC {
                return Err(format!("未知的配置文件类型: {file_kind}"));
            }
            if content.len() > MAX_FILE_BYTES {
                return Err(format!("内容超过单文件 {} 字节预算", MAX_FILE_BYTES));
            }
            if doc.exists && content == doc.raw {
                return Err("内容没有变化，无需保存".to_string());
            }
            let label = if file_kind == KIND_ALIAS { "alias" } else { "zshrc" };
            Ok((
                file_kind.clone(),
                Vec::new(),
                content.clone(),
                vec![
                    op(
                        "整文件写入",
                        format!("~/{label} 源码编辑保存（不规范化换行与末尾格式）"),
                    ),
                    op("规模", format!("{} → {} 字节", doc.raw.len(), content.len())),
                ],
                session_command(&SessionChange::FullReload),
            ))
        }
        ShellChangeRequest::RestoreBackup { backup_id } => {
            let (kind, content) = read_backup(&backup_id)?;
            if content == doc.raw {
                return Err("备份内容与当前文件一致，无需恢复".to_string());
            }
            Ok((
                kind.clone(),
                Vec::new(),
                content.clone(),
                vec![
                    op("恢复备份", format!("{backup_id}（后像在本次预览时固定）")),
                    op("规模", format!("当前 {} 字节 → 备份 {} 字节", doc.raw.len(), content.len())),
                ],
                session_command(&SessionChange::FullReload),
            ))
        }
        ShellChangeRequest::BlockNormalize => {
            let lines = split_lines(&doc.raw);
            let texts: Vec<&str> = lines.iter().map(|l| l.text(&doc.raw)).collect();
            let scan = managed_block_scan(&texts);
            if scan.old_markers.is_empty() {
                return Err("未发现旧格式标记，无需规范化".to_string());
            }
            let edits: Vec<LineEdit> = scan
                .old_markers
                .iter()
                .map(|&index| {
                    let is_begin = texts[index].trim_start().starts_with("# >>>");
                    let text =
                        if is_begin { MANAGED_BEGIN } else { MANAGED_END }.to_string();
                    LineEdit::Replace { index, text }
                })
                .collect();
            let postimage = apply_edits(&doc.raw, &edits)?;
            let ops = vec![op(
                "规范化区块标记",
                format!("将 {} 行旧格式标记更新为当前格式", edits.len()),
            )];
            Ok((
                KIND_ZSHRC.to_string(),
                edits,
                postimage,
                ops,
                session_command(&SessionChange::FullReload),
            ))
        }
    }
}

fn store_plan(stored: StoredPlan) -> Result<(), String> {
    let mut map = plans().lock().expect("plan store poisoned");
    map.retain(|_, stored| stored.created.elapsed() < PLAN_TTL);
    while map.len() >= PLAN_MAX {
        let oldest = map
            .iter()
            .min_by_key(|(_, stored)| stored.created)
            .map(|(id, _)| id.clone());
        match oldest {
            Some(id) => {
                map.remove(&id);
            }
            None => break,
        }
    }
    map.insert(stored.plan.id.clone(), stored);
    Ok(())
}

// ---------------------------------------------------------------------------
// commit / cancel
// ---------------------------------------------------------------------------

pub fn commit(plan_id: &str, expected_version: &str) -> Result<ShellCommitResult, String> {
    let mut map = plans().lock().expect("plan store poisoned");
    map.retain(|_, stored| stored.created.elapsed() < PLAN_TTL);
    // 单次消费：无论成功失败，计划取出后必须重新预览
    let stored = map
        .remove(plan_id)
        .ok_or_else(|| "计划不存在或已过期，请重新预览".to_string())?;
    if stored.created.elapsed() >= PLAN_TTL {
        return Err("计划已过期，请重新预览".to_string());
    }
    if stored.plan.base_version != expected_version {
        return Err("前端版本与计划基线不一致，请刷新后重新预览".to_string());
    }
    // 与磁盘当前版本比对：外部修改 / 并发提交在此被拒绝
    let current = files::read_raw(&stored.target_path)?.unwrap_or_default();
    if content_version(&current) != stored.plan.base_version {
        return Err("文件已被外部修改（当前版本与计划基线不一致），请重新加载后再试".to_string());
    }
    commit_write(&stored.target_path, &stored.postimage)?;
    let snapshot = snapshot_from_content(
        &stored.plan.kind,
        &stored.target_path,
        stored.postimage.clone(),
        true,
    );
    Ok(ShellCommitResult {
        kind: stored.plan.kind,
        version: content_version(&stored.postimage),
        content: stored.postimage.clone(),
        session_command: stored.plan.session_command,
        ops: stored.plan.ops,
        snapshot,
    })
}

pub fn cancel(plan_id: &str) -> Result<(), String> {
    let mut map = plans().lock().expect("plan store poisoned");
    map.remove(plan_id).map(|_| ()).ok_or_else(|| "计划不存在或已被消费".to_string())
}

#[cfg(test)]
pub(crate) fn put_plan_for_test(stored: StoredPlan) {
    plans().lock().expect("plan store poisoned").insert(stored.plan.id.clone(), stored);
}

#[cfg(test)]
pub(crate) fn clear_plans_for_test() {
    plans().lock().expect("plan store poisoned").clear();
}
