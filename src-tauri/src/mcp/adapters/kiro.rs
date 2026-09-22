use super::json::{JsonAdapter, JsonShape, JsonSpec};

/// Kiro：`~/.kiro/settings/mcp.json`；项目级 `<项目>/.kiro/settings/mcp.json`。
/// 原生支持 `disabled` 停用字段。
pub const SPEC: JsonSpec = JsonSpec {
    id: "kiro",
    label: "Kiro",
    container: "mcpServers",
    disable_key: Some("disabled"),
    global_rel: &[".kiro", "settings", "mcp.json"],
    project_rel: Some(".kiro/settings/mcp.json"),
    shape: JsonShape::CommandUrl,
};

pub const ADAPTER: JsonAdapter = JsonAdapter(&SPEC);
