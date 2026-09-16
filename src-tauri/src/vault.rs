use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const VAULT_FILE: &str = "xlt-vault.enc.json";

fn vault_path(app: &AppHandle) -> Result<PathBuf, String> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("无法定位本机数据目录: {error}"))?;
    fs::create_dir_all(&directory).map_err(|error| format!("无法创建密钥库目录: {error}"))?;
    Ok(directory.join(VAULT_FILE))
}

#[tauri::command]
pub fn vault_read(app: AppHandle) -> Result<Option<String>, String> {
    let path = vault_path(&app)?;
    match fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("密钥库文件读取失败: {error}")),
    }
}

#[tauri::command]
pub fn vault_write(app: AppHandle, content: String) -> Result<(), String> {
    let path = vault_path(&app)?;
    let temporary_path = path.with_extension("tmp");
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temporary_path)
        .map_err(|error| format!("密钥库文件写入失败: {error}"))?;
    file.write_all(content.as_bytes())
        .map_err(|error| format!("密钥库文件写入失败: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("密钥库文件同步失败: {error}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temporary_path, fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("无法设置密钥库文件权限: {error}"))?;
    }

    fs::rename(&temporary_path, &path)
        .map_err(|error| format!("密钥库文件替换失败: {error}"))
}
