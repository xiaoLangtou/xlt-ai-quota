use std::env;
use std::path::PathBuf;

/// MCP 模块使用的本机路径。生效配置始终在各 Agent 自己的文件里，
/// 这里只保存备份与运行期辅助目录。
#[derive(Debug, Clone)]
pub struct McpPaths {
    pub home: PathBuf,
    /// `~/.xlt/mcp`：模块私有目录（暂存区等）。
    pub base_dir: PathBuf,
    /// `~/.xlt/backups/mcp`：写入前备份根目录。
    pub backups_root: PathBuf,
}

impl McpPaths {
    pub fn detect() -> Self {
        let home = dirs::home_dir().unwrap_or_default();
        let xlt_dir = env::var_os("XLT_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".xlt"));
        let base_dir = xlt_dir.join("mcp");
        let backups_root = xlt_dir.join("backups").join("mcp");
        Self {
            home,
            base_dir,
            backups_root,
        }
    }

    /// 每个文件保留的备份份数（默认 10，可被环境变量覆盖）。
    pub fn backup_keep(&self) -> usize {
        env::var("XLT_MCP_BACKUP_KEEP")
            .ok()
            .and_then(|raw| raw.parse::<usize>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(10)
    }
}
