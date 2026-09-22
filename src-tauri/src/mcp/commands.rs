use serde::Serialize;
use tauri::{AppHandle, State};

use super::adapters::all_adapters;
use super::catalog;
use super::error::McpError;
use super::metadata;
use super::parse;
use super::plan::{self, StoredPlan};
use super::probe;
use super::scan::{self, ScanOptions};
use super::state::McpState;
use super::types::{
    ApplyPlanRequest, ApplyResult, CatalogFilter, CatalogSource, McpAgentInfo, McpPackage, McpPlan,
    McpScanResult, McpServer, ParsedServer, PlanRequest, ProbeResult, ReadmeResult,
    SourceSaveRequest,
};
use super::write;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Removed {
    pub removed: bool,
}

// ---------- MCP 服务 ----------

/// 已接入的 Agent 适配器能力。
#[tauri::command]
pub fn mcp_agents(state: State<'_, McpState>) -> Vec<McpAgentInfo> {
    all_adapters()
        .into_iter()
        .map(|adapter| {
            let global_path = adapter.global_path(&state.paths);
            McpAgentInfo {
                id: adapter.id().to_owned(),
                label: adapter.label().to_owned(),
                supports_disable: adapter.supports_disable(),
                supports_project: adapter.project_path(std::path::Path::new("/")).is_some(),
                supports_remote: adapter.supports_remote(),
                available: global_path
                    .as_ref()
                    .map(|path| path.exists())
                    .unwrap_or(false),
                global_path: global_path.map(|path| path.display().to_string()),
            }
        })
        .collect()
}

/// 扫描各 Agent 的 MCP 配置并合并展示。
#[tauri::command]
pub fn mcp_scan(
    app: AppHandle,
    state: State<'_, McpState>,
    projects: Option<Vec<String>>,
) -> Result<McpScanResult, McpError> {
    let projects = projects.unwrap_or_default();
    scan::scan(&app, &state.paths, &ScanOptions { projects: &projects })
}

/// 生成变更计划（不落盘）。
#[tauri::command]
pub fn mcp_plan(state: State<'_, McpState>, request: PlanRequest) -> Result<McpPlan, McpError> {
    let (plan, stored) = plan::build_plan(&state.paths, &request)?;
    let mut registry = state
        .plans
        .lock()
        .map_err(|_| McpError::internal("计划表锁定失败"))?;
    registry.insert(
        plan.id.clone(),
        StoredPlan {
            request: stored.request,
            mtimes: stored.mtimes,
            stage_actions: stored.stage_actions,
        },
    );
    Ok(plan)
}

/// 提交计划：备份 → 原子写入 → 校验 → 失败回滚。
#[tauri::command]
pub fn mcp_apply(
    app: AppHandle,
    state: State<'_, McpState>,
    id: String,
    request: ApplyPlanRequest,
) -> Result<ApplyResult, McpError> {
    write::apply(&app, &state.paths, &state.plans, &id, &request)
}

/// 运行层检测：启动进程或连接 URL，列出工具。
#[tauri::command]
pub fn mcp_probe(server: McpServer) -> ProbeResult {
    probe::probe(&server)
}

/// 从备份目录恢复原文件。
#[tauri::command]
pub fn mcp_restore_backup(
    state: State<'_, McpState>,
    backup_dir: String,
) -> Result<Removed, McpError> {
    write::restore_backup(&state.paths, &backup_dir)?;
    Ok(Removed { removed: true })
}

/// 保存备注 / 标签 / 来源。
#[tauri::command]
pub fn mcp_set_meta(
    app: AppHandle,
    key: String,
    note: String,
    tags: Vec<String>,
) -> Result<Removed, McpError> {
    metadata::save_meta(&app, &key, &note, &tags, None)?;
    Ok(Removed { removed: true })
}

/// 解析添加抽屉粘贴的 JSON / 命令行（永不执行）。
#[tauri::command]
pub fn mcp_parse_paste(text: String) -> Vec<ParsedServer> {
    parse::parse_text(&text)
}

// ---------- MCP 库 ----------

#[tauri::command]
pub fn mcp_catalog_sources(app: AppHandle) -> Result<Vec<CatalogSource>, McpError> {
    catalog::all_sources(&app)
}

#[tauri::command]
pub async fn mcp_catalog_sync(
    app: AppHandle,
    source_id: Option<String>,
) -> Result<Vec<CatalogSource>, McpError> {
    let client = catalog::http_client()?;
    catalog::sync_all(&app, &client, source_id.as_deref()).await
}

#[tauri::command]
pub fn mcp_catalog_list(
    app: AppHandle,
    state: State<'_, McpState>,
    filter: CatalogFilter,
    projects: Option<Vec<String>>,
) -> Result<Vec<McpPackage>, McpError> {
    let projects = projects.unwrap_or_default();
    let scan = scan::scan(&app, &state.paths, &ScanOptions { projects: &projects })?;
    catalog::list_packages(&app, &filter, &scan.services)
}

#[tauri::command]
pub fn mcp_catalog_detail(app: AppHandle, id: String) -> Result<Option<McpPackage>, McpError> {
    metadata::get_package(&app, &id)
}

#[tauri::command]
pub async fn mcp_catalog_readme(app: AppHandle, id: String) -> Result<ReadmeResult, McpError> {
    let Some(package) = metadata::get_package(&app, &id)? else {
        return Err(McpError::not_found("库条目不存在"));
    };
    let client = catalog::http_client()?;
    Ok(catalog::readme(&client, &package).await)
}

#[tauri::command]
pub fn mcp_catalog_source_save(
    app: AppHandle,
    request: SourceSaveRequest,
) -> Result<CatalogSource, McpError> {
    catalog::save_source(&app, &request)
}

#[tauri::command]
pub fn mcp_catalog_source_remove(app: AppHandle, id: String) -> Result<Removed, McpError> {
    if catalog::remove_source(&app, &id)? {
        Ok(Removed { removed: true })
    } else {
        Err(McpError::not_found("数据源不存在"))
    }
}

#[tauri::command]
pub fn mcp_catalog_favorite(app: AppHandle, id: String, value: bool) -> Result<Removed, McpError> {
    metadata::set_favorite(&app, &id, value)?;
    Ok(Removed { removed: !value })
}
