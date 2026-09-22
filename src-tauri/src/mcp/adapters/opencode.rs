use super::json::{JsonAdapter, JsonShape, JsonSpec};

/// OpenCode：`~/.config/opencode/opencode.json` 的 `mcp`；项目级 `<项目>/opencode.json`。
/// 结构差异较大：`type: local/remote`、局部 `command` 为数组、环境字段为 `environment`，含 `enabled`。
pub const SPEC: JsonSpec = JsonSpec {
    id: "opencode",
    label: "OpenCode",
    container: "mcp",
    disable_key: None,
    global_rel: &[".config", "opencode", "opencode.json"],
    project_rel: Some("opencode.json"),
    shape: JsonShape::OpenCode,
};

pub const ADAPTER: JsonAdapter = JsonAdapter(&SPEC);
