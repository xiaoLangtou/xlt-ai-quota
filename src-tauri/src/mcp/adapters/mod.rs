use std::path::{Path, PathBuf};

use super::error::McpError;
use super::paths::McpPaths;
use super::types::{AdapterEntry, ServerOp};

pub mod claude;
pub mod codex;
pub mod gemini;
pub mod json;
pub mod kiro;
pub mod opencode;

/// Agent 适配器统一接口：读 / 写 / 格式化，保留未知字段。
pub trait McpAdapter: Send + Sync {
    /// 稳定标识（小写），用于前端与落点记录。
    fn id(&self) -> &'static str;
    /// 展示名。
    fn label(&self) -> &'static str;
    /// 全局配置文件（不存在则 None）。
    fn global_path(&self, paths: &McpPaths) -> Option<PathBuf>;
    /// 项目级配置文件（相对项目根）。
    fn project_path(&self, _project: &Path) -> Option<PathBuf> {
        None
    }
    /// 是否原生支持停用字段（否则走本地暂存区）。
    fn supports_disable(&self) -> bool {
        false
    }
    /// 是否支持远程（http / sse）传输。
    fn supports_remote(&self) -> bool {
        true
    }
    /// 解析配置文件原文为归一化服务列表。
    fn parse(&self, raw: &str) -> Result<Vec<AdapterEntry>, McpError>;
    /// 对配置文件原文应用操作，返回新原文（保留未知字段 / 注释）。
    fn apply(&self, raw: &str, ops: &[ServerOp]) -> Result<String, McpError>;

    /// 该适配器支持写入（只读适配器返回 false）。
    fn writable(&self) -> bool {
        true
    }
}

/// 全部已注册适配器。
pub fn all_adapters() -> Vec<Box<dyn McpAdapter>> {
    vec![
        Box::new(claude::ADAPTER),
        Box::new(codex::ADAPTER),
        Box::new(gemini::ADAPTER),
        Box::new(kiro::ADAPTER),
        Box::new(opencode::ADAPTER),
    ]
}

/// 按 id 取适配器。
pub fn adapter_by_id(id: &str) -> Option<Box<dyn McpAdapter>> {
    all_adapters().into_iter().find(|adapter| adapter.id() == id)
}

/// 已知 Agent 展示名（用于只读 / 未接入的落点记录）。
pub fn agent_label(id: &str) -> String {
    adapter_by_id(id)
        .map(|adapter| adapter.label().to_owned())
        .unwrap_or_else(|| id.to_owned())
}
