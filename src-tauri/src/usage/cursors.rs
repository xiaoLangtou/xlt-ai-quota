use std::fs;
use std::path::Path;

use serde_json::Value;

use super::paths;
use super::types::Cursors;

pub fn load() -> Cursors {
    let path = paths::cursors_path();
    let Ok(content) = fs::read_to_string(&path) else {
        return Cursors::default();
    };
    serde_json::from_str::<Cursors>(&content).unwrap_or_default()
}

pub fn save(cursors: &Cursors) -> Result<(), String> {
    let path = paths::cursors_path();
    let payload = serde_json::to_string_pretty(cursors)
        .map_err(|error| format!("序列化 cursors 失败: {error}"))?;
    atomic_write(&path, payload.as_bytes())
}

pub fn get_source(source: &str) -> Option<Value> {
    load().sources.get(source).cloned()
}

pub fn set_source(source: &str, value: Value) -> Result<(), String> {
    let mut cursors = load();
    cursors.sources.insert(source.to_owned(), value);
    save(&cursors)
}

pub fn remove_source(source: &str) -> Result<(), String> {
    let mut cursors = load();
    cursors.sources.remove(source);
    save(&cursors)
}

pub fn clear() -> Result<(), String> {
    save(&Cursors::default())
}

pub fn sources() -> Vec<String> {
    load().sources.keys().cloned().collect()
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建目录失败: {error}"))?;
    }
    let temp = path.with_extension("tmp");
    fs::write(&temp, bytes).map_err(|error| format!("写入临时文件失败: {error}"))?;
    fs::rename(&temp, path).map_err(|error| format!("替换文件失败: {error}"))
}
