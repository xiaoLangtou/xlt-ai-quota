use std::fs;
use std::path::PathBuf;

/// 用量数据根目录。默认 `~/.xlt-workbench`，可用 `XLT_USAGE_DIR` 覆盖（测试/诊断用）。
pub fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("XLT_USAGE_DIR") {
        return PathBuf::from(dir);
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".xlt-workbench")
}

pub fn queue_dir() -> PathBuf {
    data_dir().join("queue")
}

pub fn cursors_path() -> PathBuf {
    data_dir().join("cursors.json")
}

pub fn manifest_path() -> PathBuf {
    data_dir().join("manifest.json")
}

pub fn config_path() -> PathBuf {
    data_dir().join("config.json")
}

pub fn quotas_path() -> PathBuf {
    data_dir().join("quotas.json")
}

pub fn pricing_overlay_path() -> PathBuf {
    data_dir().join("pricing-overlay.json")
}

pub fn logs_dir() -> PathBuf {
    data_dir().join("logs")
}

pub fn ensure_dirs() -> Result<(), String> {
    fs::create_dir_all(queue_dir()).map_err(|error| format!("创建数据目录失败: {error}"))?;
    fs::create_dir_all(logs_dir()).map_err(|error| format!("创建日志目录失败: {error}"))?;
    Ok(())
}
