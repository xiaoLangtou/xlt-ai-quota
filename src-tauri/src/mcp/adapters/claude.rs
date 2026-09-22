use super::json::{JsonAdapter, JsonShape, JsonSpec};

/// Claude Code：`~/.claude.json` 的 `mcpServers`；项目级 `<项目>/.mcp.json`。
/// 该文件含大量运行时状态，写入策略上由流水线按「仅改 mcpServers 容器」处理。
pub const SPEC: JsonSpec = JsonSpec {
    id: "claude",
    label: "Claude Code",
    container: "mcpServers",
    disable_key: None,
    global_rel: &[".claude.json"],
    project_rel: Some(".mcp.json"),
    shape: JsonShape::CommandUrl,
};

pub const ADAPTER: JsonAdapter = JsonAdapter(&SPEC);
