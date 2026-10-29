//! Shell 环境管理领域类型与前后端 DTO。
//!
//! 语义约定（PRD-开发环境管理后续规划 R0-01）：
//! - 环境变量固定「字面量」语义，alias 固定「命令文本」语义，分别序列化；
//! - 含 `$` / 反引号 / 命令替换的值为 Expression，表格编辑只允许 Literal，
//!   Expression 条目必须走源码视图，避免重写引号改变语义。

use serde::{Deserialize, Serialize};

pub const KIND_ALIAS: &str = "alias";
pub const KIND_ZSHRC: &str = "zshrc";

/// 单个文件允许的最大字节数（读 / 写统一预算）。
pub const MAX_FILE_BYTES: usize = 1024 * 1024;

/// 值语义分类。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueKind {    /// 字面量：单引号包裹，或不含展开语法的文本；可安全地在表格中编辑。
    Literal,
    /// 表达式：含 `$`、反引号等展开语法；表格只读，须用源码视图编辑。
    Expression,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AliasEntry {
    /// 稳定条目身份：`alias:<name>`
    pub id: String,
    pub name: String,
    pub value: String,
    pub value_kind: ValueKind,
    /// 分组标题（上方最近的分组注释；被注释掉的 alias 不算分组）
    pub group: Option<String>,
    /// 描述（alias 行尾 `#` 注释，约定见 PRD）
    pub description: Option<String>,
    /// 1-based 行号
    pub line: usize,
    /// 原始行文本
    pub raw: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvEntry {
    /// 托管条目为 `env:<key>`；非托管为 `env:<key>@<line>`（同名可能多处出现）
    pub id: String,
    pub key: String,
    pub value: String,
    pub value_kind: ValueKind,
    /// 是否经 `export` 导出（区别于普通赋值）
    pub exported: bool,
    /// 位于托管区块内，表格可编辑（且仅 Literal 可编辑）
    pub managed: bool,
    pub line: usize,
    pub raw: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockInfo {
    /// 起始标记行（1-based，含）
    pub begin_line: usize,
    /// 结束标记行（1-based，含）
    pub end_line: usize,
    pub entry_count: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSnapshot {
    pub kind: String,
    pub path: String,
    pub exists: bool,
    /// 内容 sha256，作为并发写入的版本基线
    pub version: String,
    /// 原始内容（源码视图的数据源，不擅自规范化）
    pub content: String,
    pub aliases: Vec<AliasEntry>,
    pub envs: Vec<EnvEntry>,
    pub managed_block: Option<BlockInfo>,
    /// 区块结构问题（多标记、不配对等）；非空时托管写操作被拒绝
    pub block_issues: Vec<String>,
    pub line_ending: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellSnapshot {
    pub alias: FileSnapshot,
    pub zshrc: FileSnapshot,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanOp {
    /// 动作名（中文，用于差异确认）
    pub action: String,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellChangePlan {
    pub id: String,
    /// 目标文件："alias" | "zshrc"
    pub kind: String,
    /// 计划基线版本；commit 时与磁盘当前版本比对
    pub base_version: String,
    pub ops: Vec<PlanOp>,
    /// 本次变更对应的会话命令（用户手动复制到已开终端；不再默认自动复制）
    pub session_command: String,
    /// 预计算的整文件后像
    pub postimage: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellCommitResult {
    pub kind: String,
    pub version: String,
    pub content: String,
    pub session_command: String,
    pub ops: Vec<PlanOp>,
    /// 提交后重新解析出的文件快照，前端直接替换对应文件状态
    pub snapshot: FileSnapshot,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellBackup {
    pub id: String,
    pub file: String,
    pub created_at: String,
    pub size: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateResult {
    pub ok: bool,
    pub errors: Vec<String>,
}

/// 变更请求（前端 → preview）。所有写操作（含源码整写与备份恢复）统一走该入口。
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", rename_all_fields = "camelCase")]
pub enum ShellChangeRequest {
    /// 显式新增（同名存在时拒绝，避免与编辑混淆）；分组不存在时自动创建组头注释
    AliasAdd { name: String, value: String, group: Option<String>, description: Option<String> },
    /// 编辑别名命令与描述（仅 Literal 条目）
    AliasEdit { entry_id: String, value: String, description: Option<String> },
    /// 按原身份一次提交的重命名
    AliasRename { entry_id: String, new_name: String },
    AliasDelete { entry_id: String },
    /// 写入托管区；区块不存在时必须携带 confirm_block_create = true
    EnvSet { key: String, value: String, confirm_block_create: Option<bool> },
    EnvDelete { entry_id: String },
    /// 源码视图整文件写入：内容原样保存，不补末尾换行
    SourceWrite { file_kind: String, content: String },
    /// 备份恢复：后像在 preview 时从备份内容固定
    RestoreBackup { backup_id: String },
    /// 旧格式标记规范化为当前标记
    BlockNormalize,
}
