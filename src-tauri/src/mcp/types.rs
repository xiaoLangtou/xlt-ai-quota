use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha1::{Digest, Sha1};

/// 传输类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpTransport {
    Stdio,
    Http,
    Sse,
}

impl McpTransport {
    pub fn as_str(self) -> &'static str {
        match self {
            McpTransport::Stdio => "stdio",
            McpTransport::Http => "http",
            McpTransport::Sse => "sse",
        }
    }

    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_lowercase().as_str() {
            "http" | "streamable-http" | "streamablehttp" | "streamable_http" => McpTransport::Http,
            "sse" => McpTransport::Sse,
            _ => McpTransport::Stdio,
        }
    }
}

/// 归一化后的服务定义。一个 `McpServer` 可被多个 Agent / 范围引用。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServer {
    pub name: String,
    pub transport: McpTransport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
    /// 未识别字段，原样保留，写回时不丢。
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub extra: Map<String, Value>,
}

impl Default for McpServer {
    fn default() -> Self {
        Self {
            name: String::new(),
            transport: McpTransport::Stdio,
            command: None,
            args: Vec::new(),
            env: BTreeMap::new(),
            url: None,
            headers: BTreeMap::new(),
            extra: Map::new(),
        }
    }
}

impl McpServer {
    /// 用于合并与去重的规范化指纹（名称 + 类型 + 命令 / URL + 环境与头）。
    pub fn fingerprint(&self) -> String {
        let canonical = serde_json::json!({
            "name": self.name,
            "transport": self.transport.as_str(),
            "command": self.command,
            "args": self.args,
            "env": self.env,
            "url": self.url,
            "headers": self.headers,
            "extra": self.extra,
        });
        let digest = Sha1::digest(
            serde_json::to_string(&canonical)
                .unwrap_or_default()
                .as_bytes(),
        );
        format!("{digest:x}")
    }

    /// 合并来自不同落点的同一服务：补齐缺失字段，保留并集。
    pub fn merge_from(&mut self, other: &McpServer) {
        if self.command.is_none() {
            self.command = other.command.clone();
        }
        if self.args.is_empty() {
            self.args = other.args.clone();
        }
        if self.url.is_none() {
            self.url = other.url.clone();
        }
        for (key, value) in &other.env {
            self.env.entry(key.clone()).or_insert_with(|| value.clone());
        }
        for (key, value) in &other.headers {
            self.headers.entry(key.clone()).or_insert_with(|| value.clone());
        }
        for (key, value) in &other.extra {
            self.extra.entry(key.clone()).or_insert_with(|| value.clone());
        }
    }
}

/// 服务在某处的落点状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstanceStatus {
    Ok,
    Disabled,
    ConfigError,
    CommandMissing,
    Unchecked,
}

impl InstanceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            InstanceStatus::Ok => "ok",
            InstanceStatus::Disabled => "disabled",
            InstanceStatus::ConfigError => "config_error",
            InstanceStatus::CommandMissing => "command_missing",
            InstanceStatus::Unchecked => "unchecked",
        }
    }

    /// 合并多个落点时的严重度排序（越大越优先展示）。
    pub fn severity(self) -> u8 {
        match self {
            InstanceStatus::Ok => 0,
            InstanceStatus::Unchecked => 1,
            InstanceStatus::Disabled => 2,
            InstanceStatus::CommandMissing => 3,
            InstanceStatus::ConfigError => 4,
        }
    }
}

/// 该服务在某 Agent / 范围下的落点。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpInstance {
    pub agent: String,
    pub agent_label: String,
    pub scope: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_path: Option<String>,
    pub config_file: String,
    pub enabled: bool,
    pub status: InstanceStatus,
    /// 读取该落点时是否发现解析 / 探测问题。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 合并后的服务（列表行）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpService {
    pub key: String,
    pub name: String,
    pub transport: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    pub headers: BTreeMap<String, String>,
    pub instances: Vec<McpInstance>,
    pub agents: Vec<String>,
    pub scopes: Vec<String>,
    pub enabled: bool,
    pub status: String,
    pub note: String,
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
}

/// 写入操作。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ServerOp {
    Upsert { server: McpServer },
    Remove { name: String },
    SetEnabled { name: String, enabled: bool },
}

/// 一个写入目标（Agent + 范围 + 项目）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteTarget {
    pub agent: String,
    pub scope: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_path: Option<String>,
}

/// 单个目标文件的变更计划。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetPlan {
    pub agent: String,
    pub agent_label: String,
    pub scope: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_path: Option<String>,
    pub config_file: String,
    pub exists: bool,
    pub before: String,
    pub after: String,
    pub diff: String,
    /// 目标不支持某字段时的提示（如不支持 headers）。
    pub warnings: Vec<String>,
}

/// 变更计划：一次写入对应多个目标文件的 diff，需用户确认后提交。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpPlan {
    pub id: String,
    pub summary: String,
    pub targets: Vec<TargetPlan>,
    pub warnings: Vec<String>,
}

/// 提交计划请求。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyPlanRequest {
    /// 用户确认时把计划里的目标文件 mtime 回传，写前比对，冲突即中止。
    #[serde(default)]
    pub expected_mtimes: BTreeMap<String, i64>,
}

/// 变更类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanKind {
    Upsert,
    Remove,
    Enable,
    Disable,
}

/// 生成变更计划的请求。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanRequest {
    pub kind: PlanKind,
    /// 服务定义：upsert / disable / enable（无原生停用时）需要。
    #[serde(default)]
    pub server: Option<McpServer>,
    /// 服务名（remove / enable / disable 需要）。
    pub name: String,
    pub targets: Vec<WriteTarget>,
}

/// 暂存区动作（无原生停用字段的 Agent）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StageAction {
    pub agent: String,
    pub scope: String,
    pub project_path: String,
    pub server: McpServer,
    /// stage / unstage
    pub action: String,
}

/// 写入结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyResult {
    pub written: Vec<WrittenFile>,
    pub rolled_back: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WrittenFile {
    pub config_file: String,
    pub backup_dir: String,
}

/// 探测结果（测试连接）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeResult {
    pub ok: bool,
    pub transport: String,
    pub tools: Vec<ProbeTool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_info: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeTool {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// 适配器解析出的单条服务及其（原生）启停状态。
#[derive(Debug, Clone)]
pub struct AdapterEntry {
    pub server: McpServer,
    /// 仅当适配器 `supports_disable()` 为真时有意义。
    pub enabled: bool,
}

/// 扫描过程中发现的文件级问题（配置不可解析等）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpScanIssue {
    pub agent: String,
    pub agent_label: String,
    pub config_file: String,
    pub message: String,
}

/// 扫描结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpScanResult {
    pub services: Vec<McpService>,
    pub issues: Vec<McpScanIssue>,
    pub scanned_files: Vec<String>,
}

/// 解析添加抽屉粘贴内容的结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedServer {
    pub server: McpServer,
    pub source: String,
}

// ---------- MCP 库 ----------

/// 可一键安装的运行方式。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Runtime {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// 安装所需环境变量声明。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvVar {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub required: bool,
    pub secret: bool,
}

/// 安装档位。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "level", rename_all = "snake_case")]
pub enum InstallInfo {
    /// 可一键安装。
    Ready {
        #[serde(default)]
        runtimes: Vec<Runtime>,
        #[serde(default, rename = "envSpec")]
        env_spec: Vec<EnvVar>,
    },
    /// 只有仓库信息，需读 README。
    Manual,
}

impl Default for InstallInfo {
    fn default() -> Self {
        InstallInfo::Manual
    }
}

/// 库内统一条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpPackage {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: String,
    pub homepage: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<LanguageInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(default)]
    pub platforms: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stars: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forks: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub verified: bool,
    #[serde(default)]
    pub mirror: bool,
    /// server / client / other；列表默认只展示 server。
    pub kind: String,
    pub install: InstallInfo,
    /// 已安装到的 Agent（运行时计算，不落库）。
    #[serde(default)]
    pub installed_agents: Vec<String>,
    #[serde(default)]
    pub favorite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageInfo {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// 数据源配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSource {
    pub id: String,
    pub label: String,
    /// builtin / registry / gitcode / custom
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_channel_id: Option<String>,
    pub enabled: bool,
    pub builtin: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_sync_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

/// 库列表筛选。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogFilter {
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub query: Option<String>,
    /// all / star / updated / favorites
    #[serde(default)]
    pub sort: Option<String>,
    /// server / client / other；为空时按 show_clients 决定。
    #[serde(default)]
    pub kind: Option<String>,
    /// npx / uvx / docker / remote
    #[serde(default)]
    pub runtime: Option<String>,
    #[serde(default)]
    pub show_clients: bool,
    #[serde(default)]
    pub favorites_only: bool,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub offset: Option<usize>,
}

/// README 摘要与从其中解析出的安装片段。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadmeResult {
    pub available: bool,
    pub text: String,
    pub snippets: Vec<ParsedServer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 适配器能力（供前端渲染安装目标与筛选）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpAgentInfo {
    pub id: String,
    pub label: String,
    pub supports_disable: bool,
    pub supports_project: bool,
    pub supports_remote: bool,
    pub global_path: Option<String>,
    pub available: bool,
}

/// 自定义源保存请求。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSaveRequest {
    #[serde(default)]
    pub id: Option<String>,
    pub label: String,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub channel_id: Option<String>,
    #[serde(default)]
    pub sub_channel_id: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}


