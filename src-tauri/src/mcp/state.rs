use std::collections::HashMap;
use std::sync::Mutex;

use super::paths::McpPaths;
use super::plan::{PlanRegistry, StoredPlan};

/// MCP 模块运行态：路径配置与待确认的变更计划。
pub struct McpState {
    pub paths: McpPaths,
    pub plans: PlanRegistry,
}

impl McpState {
    pub fn new() -> Self {
        Self {
            paths: McpPaths::detect(),
            plans: Mutex::new(HashMap::<String, StoredPlan>::new()),
        }
    }
}

impl Default for McpState {
    fn default() -> Self {
        Self::new()
    }
}
