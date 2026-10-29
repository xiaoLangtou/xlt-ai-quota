use std::fs;
use std::path::Path;

use super::paths;
use super::types::UsageConfig;

pub fn load() -> UsageConfig {
    let path = paths::config_path();
    let Ok(content) = fs::read_to_string(&path) else {
        return UsageConfig::default();
    };
    serde_json::from_str::<UsageConfig>(&content).unwrap_or_default()
}

pub fn save(config: &UsageConfig) -> Result<(), String> {
    let path = paths::config_path();
    let payload = serde_json::to_string_pretty(config)
        .map_err(|error| format!("序列化用量配置失败: {error}"))?;
    atomic_write(&path, payload.as_bytes())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建目录失败: {error}"))?;
    }
    let temp = path.with_extension("tmp");
    fs::write(&temp, bytes).map_err(|error| format!("写入临时文件失败: {error}"))?;
    fs::rename(&temp, path).map_err(|error| format!("替换文件失败: {error}"))
}
