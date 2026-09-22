use super::json::{JsonAdapter, JsonShape, JsonSpec};

/// Gemini CLI：`~/.gemini/settings.json`；项目级 `<项目>/.gemini/settings.json`。
/// 远程传输使用 `httpUrl`，读写时原样保留该键名。
pub const SPEC: JsonSpec = JsonSpec {
    id: "gemini",
    label: "Gemini CLI",
    container: "mcpServers",
    disable_key: None,
    global_rel: &[".gemini", "settings.json"],
    project_rel: Some(".gemini/settings.json"),
    shape: JsonShape::CommandUrl,
};

pub const ADAPTER: JsonAdapter = JsonAdapter(&SPEC);
