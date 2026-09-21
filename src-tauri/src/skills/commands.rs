use serde::Serialize;
use tauri::State;

use super::config::{
    self, DirScanResult, ProjectAgentDir, SkillProject, SkillRoot,
};
use super::error::SkillsError;
use super::install::catalog::{self, CatalogSkill};
use super::install::planner::{
    self, CommitPlanRequest, CommitPlanResponse, CreateInstallPlanRequest, InstallPlan,
};
use super::install::sources::{self, PrepareSourceResponse};
use super::install::sources_registry::{self, AddSourceRequest, SkillSource};
use super::install::staging;
use super::install::updates::{self, ApplyUpdateRequest, InstallUpdateInfo, UpdateDiffPreview};
use super::install::records::InstallRecord;
use super::install::transaction::TargetInstallResult;
use super::remote_detail::{self, RemoteFileContent, RemoteSkillDetail};
use super::scan::{self, ScanFailure, SkillSummary};
use super::skill_io::{self, CreateSkillRequest, DeleteResult, SaveSkillRequest, SkillDetail};
use super::skillssh::{self, SkillsShSearchItem};
use super::state::SkillsState;
use super::trash::{self, TrashItem};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Removed {
    pub removed: bool,
}

#[tauri::command]
pub fn skills_list_roots(state: State<'_, SkillsState>) -> Vec<SkillRoot> {
    config::list_roots(&state.paths)
}

#[tauri::command]
pub fn skills_add_root(state: State<'_, SkillsState>, path: String) -> Result<SkillRoot, SkillsError> {
    config::add_root(&state.paths, &path)
}

#[tauri::command]
pub fn skills_remove_root(state: State<'_, SkillsState>, id: String) -> Result<Removed, SkillsError> {
    if config::remove_root(&state.paths, &id) {
        Ok(Removed { removed: true })
    } else {
        Err(SkillsError::bad_request("默认根不可移除或不存在"))
    }
}

#[tauri::command]
pub fn skills_update_root_label(
    state: State<'_, SkillsState>,
    id: String,
    label: String,
) -> Result<SkillRoot, SkillsError> {
    config::update_root_label(&state.paths, &id, &label)
}

#[tauri::command]
pub fn skills_init_root(state: State<'_, SkillsState>, id: String) -> Result<SkillRoot, SkillsError> {
    config::init_root_dir(&state.paths, &id)
}

#[tauri::command]
pub fn skills_list_projects(state: State<'_, SkillsState>) -> Vec<SkillProject> {
    config::list_projects(&state.paths)
}

#[tauri::command]
pub fn skills_add_project(
    state: State<'_, SkillsState>,
    path: String,
    label: Option<String>,
) -> Result<SkillProject, SkillsError> {
    config::add_project(&state.paths, &path, label)
}

#[tauri::command]
pub fn skills_remove_project(
    state: State<'_, SkillsState>,
    id: String,
) -> Result<Removed, SkillsError> {
    if config::remove_project(&state.paths, &id) {
        Ok(Removed { removed: true })
    } else {
        Err(SkillsError::not_found("项目不存在"))
    }
}

#[tauri::command]
pub fn skills_list_project_agent_dirs(
    state: State<'_, SkillsState>,
    id: String,
) -> Result<Vec<ProjectAgentDir>, SkillsError> {
    config::list_project_agent_dirs(&state.paths, &id)
}

#[tauri::command]
pub fn skills_scan_dir(
    state: State<'_, SkillsState>,
    path: String,
) -> Result<DirScanResult, SkillsError> {
    config::scan_dir_for_projects(&state.paths, &path)
}

// ---------- 技能扫描（只读） ----------

/// 扫描所有根目录；顺带刷新解析失败缓存（供问题中心）。
#[tauri::command]
pub fn skills_scan(state: State<'_, SkillsState>) -> Vec<SkillSummary> {
    let (skills, failures) = scan::scan_all_skills(&state.paths);
    if let Ok(mut cache) = state.scan_failures.lock() {
        *cache = failures;
    }
    skills
}

/// 返回最近一次扫描的解析失败项（先触发一次扫描以刷新缓存）。
#[tauri::command]
pub fn skills_list_issues(state: State<'_, SkillsState>) -> Vec<ScanFailure> {
    let (_, failures) = scan::scan_all_skills(&state.paths);
    if let Ok(mut cache) = state.scan_failures.lock() {
        *cache = failures.clone();
    }
    failures
}

#[tauri::command]
pub fn skills_get_detail(
    state: State<'_, SkillsState>,
    root_id: String,
    name: String,
) -> Result<SkillDetail, SkillsError> {
    skill_io::get_skill_detail(&state.paths, &root_id, &name)
}

#[tauri::command]
pub fn skills_read_file(
    state: State<'_, SkillsState>,
    root_id: String,
    name: String,
    path: String,
) -> Result<FileContent, SkillsError> {
    let content = skill_io::read_skill_file(&state.paths, &root_id, &name, &path)?;
    Ok(FileContent { path, content })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileContent {
    pub path: String,
    pub content: String,
}

// ---------- 编辑 ----------

#[tauri::command]
pub fn skills_save(
    state: State<'_, SkillsState>,
    root_id: String,
    name: String,
    request: SaveSkillRequest,
) -> Result<SkillDetail, SkillsError> {
    skill_io::save_skill(&state.paths, &root_id, &name, &request)
}

#[tauri::command]
pub fn skills_create(
    state: State<'_, SkillsState>,
    request: CreateSkillRequest,
) -> Result<SkillDetail, SkillsError> {
    skill_io::create_skill(&state.paths, &request)
}

#[tauri::command]
pub fn skills_rename(
    state: State<'_, SkillsState>,
    root_id: String,
    name: String,
    new_name: String,
) -> Result<SkillDetail, SkillsError> {
    skill_io::rename_skill(&state.paths, &root_id, &name, &new_name)
}

#[tauri::command]
pub fn skills_delete(
    state: State<'_, SkillsState>,
    root_id: String,
    name: String,
) -> Result<DeleteResult, SkillsError> {
    skill_io::delete_skill(&state.paths, &root_id, &name)
}

// ---------- 回收站 ----------

#[tauri::command]
pub fn skills_list_trash(state: State<'_, SkillsState>) -> Vec<TrashItem> {
    trash::list_trash(&state.paths)
}

#[tauri::command]
pub fn skills_restore_trash(
    state: State<'_, SkillsState>,
    id: String,
) -> Result<SkillDetail, SkillsError> {
    let (root_id, name) = trash::restore_trash(&state.paths, &id)?;
    skill_io::get_skill_detail(&state.paths, &root_id, &name)
}

#[tauri::command]
pub fn skills_purge_trash(
    state: State<'_, SkillsState>,
    id: String,
) -> Result<Purged, SkillsError> {
    if trash::purge_trash(&state.paths, &id) {
        Ok(Purged { purged: true })
    } else {
        Err(SkillsError::not_found("回收站条目不存在"))
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Purged {
    pub purged: bool,
}

// ---------- 安装流水线：来源准备 → 计划 → 提交 ----------

#[tauri::command]
pub fn skills_prepare_local_source(
    state: State<'_, SkillsState>,
    source_path: String,
) -> Result<PrepareSourceResponse, SkillsError> {
    sources::prepare_local_source(&state.paths, &state.staging, &source_path)
}

#[tauri::command]
pub fn skills_prepare_upload_source(
    state: State<'_, SkillsState>,
    archive_path: String,
    filename: String,
) -> Result<PrepareSourceResponse, SkillsError> {
    sources::prepare_upload_source(&state.paths, &state.staging, &archive_path, &filename)
}

#[tauri::command]
pub async fn skills_prepare_remote_source(
    state: State<'_, SkillsState>,
    source: String,
    reference: String,
) -> Result<PrepareSourceResponse, SkillsError> {
    sources::prepare_remote_source(&state.paths, &state.staging, &source, &reference).await
}

#[tauri::command]
pub fn skills_remove_staging(
    state: State<'_, SkillsState>,
    id: String,
) -> Removed {
    Removed {
        removed: staging::remove_staging(&state.paths, &state.staging, &id),
    }
}

#[tauri::command]
pub fn skills_create_install_plan(
    state: State<'_, SkillsState>,
    request: CreateInstallPlanRequest,
) -> Result<InstallPlan, SkillsError> {
    planner::create_install_plan(&state.paths, &state.staging, &state.plans, &request)
}

#[tauri::command]
pub fn skills_get_install_plan(
    state: State<'_, SkillsState>,
    id: String,
) -> Result<InstallPlan, SkillsError> {
    planner::get_install_plan(&state.plans, &id)
}

#[tauri::command]
pub fn skills_cancel_install_plan(
    state: State<'_, SkillsState>,
    id: String,
) -> Cancelled {
    Cancelled {
        cancelled: planner::cancel_install_plan(&state.plans, &id),
    }
}

#[tauri::command]
pub fn skills_commit_install_plan(
    state: State<'_, SkillsState>,
    id: String,
    request: CommitPlanRequest,
) -> Result<CommitPlanResponse, SkillsError> {
    planner::commit_install_plan(&state.paths, &state.staging, &state.plans, &id, &request)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cancelled {
    pub cancelled: bool,
}

// ---------- 目录与数据源 ----------

#[tauri::command]
pub async fn skills_get_catalog(
    state: State<'_, SkillsState>,
) -> Result<Vec<CatalogSkill>, SkillsError> {
    Ok(catalog::get_catalog(&state.paths, &state.stars_cache).await)
}

#[tauri::command]
pub fn skills_list_sources(state: State<'_, SkillsState>) -> Vec<SkillSource> {
    sources_registry::list_sources(&state.paths)
}

#[tauri::command]
pub fn skills_add_source(
    state: State<'_, SkillsState>,
    request: AddSourceRequest,
) -> Result<SkillSource, SkillsError> {
    sources_registry::add_source(&state.paths, &request)
}

#[tauri::command]
pub fn skills_remove_source(
    state: State<'_, SkillsState>,
    id: String,
) -> Result<Removed, SkillsError> {
    if sources_registry::remove_source(&state.paths, &id)? {
        Ok(Removed { removed: true })
    } else {
        Err(SkillsError::not_found("数据源不存在"))
    }
}

#[tauri::command]
pub async fn skills_sync_source(
    state: State<'_, SkillsState>,
    id: String,
) -> Result<SkillSource, SkillsError> {
    sources_registry::sync_source(&state.paths, &id).await
}

// ---------- skills.sh 搜索 / 远程详情 ----------

#[tauri::command]
pub async fn skills_search_skillssh(
    query: String,
    limit: Option<u32>,
) -> Result<Vec<SkillsShSearchItem>, SkillsError> {
    skillssh::search_skills_sh(&query, limit).await
}

#[tauri::command]
pub async fn skills_remote_detail(
    source: String,
    reference: String,
) -> Result<RemoteSkillDetail, SkillsError> {
    remote_detail::get_remote_skill_detail(&source, &reference).await
}

#[tauri::command]
pub async fn skills_remote_file(
    source: String,
    reference: String,
    path: String,
) -> Result<RemoteFileContent, SkillsError> {
    remote_detail::get_remote_skill_file(&source, &reference, &path).await
}

// ---------- 安装记录 / 更新检查 / 回滚 ----------

#[tauri::command]
pub fn skills_list_installs(state: State<'_, SkillsState>) -> Vec<InstallRecord> {
    super::install::records::list_install_records(&state.paths)
}

#[tauri::command]
pub async fn skills_check_install_update(
    state: State<'_, SkillsState>,
    id: String,
) -> Result<InstallUpdateInfo, SkillsError> {
    updates::check_install_update(&state.paths, &id).await
}

#[tauri::command]
pub async fn skills_preview_update(
    state: State<'_, SkillsState>,
    id: String,
) -> Result<UpdateDiffPreview, SkillsError> {
    updates::preview_update_diff(&state.paths, &state.staging, &id).await
}

#[tauri::command]
pub fn skills_apply_update(
    state: State<'_, SkillsState>,
    id: String,
    request: ApplyUpdateRequest,
) -> Result<TargetInstallResult, SkillsError> {
    updates::apply_install_update(&state.paths, &state.staging, &id, &request)
}

#[tauri::command]
pub fn skills_rollback_install(
    state: State<'_, SkillsState>,
    id: String,
    backup_id: String,
) -> Result<InstallRecord, SkillsError> {
    updates::rollback_install(&state.paths, &id, &backup_id)
}
