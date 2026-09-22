use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use super::adapters::{adapter_by_id, McpAdapter};
use super::error::McpError;
use super::paths::McpPaths;
use super::types::{
    McpPlan, PlanKind, PlanRequest, ServerOp, StageAction, TargetPlan, WriteTarget,
};

/// 已生成、等待确认的计划及其写前状态。
#[derive(Debug, Clone)]
pub struct StoredPlan {
    pub request: PlanRequest,
    /// 生成计划时的目标文件 mtime（毫秒）；写前比对，变化即中止。
    pub mtimes: HashMap<String, i64>,
    pub stage_actions: Vec<StageAction>,
}

/// 内存中的待确认计划表。
pub type PlanRegistry = std::sync::Mutex<HashMap<String, StoredPlan>>;

pub fn file_mtime(path: &Path) -> i64 {
    fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(-1)
}

/// 解析写入目标对应的配置文件。
pub fn resolve_target(
    adapter: &dyn McpAdapter,
    paths: &McpPaths,
    target: &WriteTarget,
) -> Result<PathBuf, McpError> {
    if target.scope == "project" {
        let project = target
            .project_path
            .as_ref()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| McpError::bad_request("项目范围缺少项目路径"))?;
        adapter
            .project_path(Path::new(project))
            .ok_or_else(|| McpError::bad_request(format!("{} 不支持项目级配置", adapter.label())))
    } else {
        adapter
            .global_path(paths)
            .ok_or_else(|| McpError::bad_request(format!("{} 没有全局配置", adapter.label())))
    }
}

/// 生成最小行级 diff（公共前缀 / 后缀裁剪后的 +/- 块）。
pub fn make_diff(before: &str, after: &str) -> String {
    if before == after {
        return String::new();
    }
    let old: Vec<&str> = before.lines().collect();
    let new: Vec<&str> = after.lines().collect();
    let mut start = 0;
    while start < old.len() && start < new.len() && old[start] == new[start] {
        start += 1;
    }
    let mut end_old = old.len();
    let mut end_new = new.len();
    while end_old > start && end_new > start && old[end_old - 1] == new[end_new - 1] {
        end_old -= 1;
        end_new -= 1;
    }
    let mut out = String::new();
    for line in &old[start..end_old] {
        out.push_str("- ");
        out.push_str(line);
        out.push('\n');
    }
    for line in &new[start..end_new] {
        out.push_str("+ ");
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn plan_summary(kind: PlanKind, name: &str, target_count: usize) -> String {
    let verb = match kind {
        PlanKind::Upsert => "写入",
        PlanKind::Remove => "删除",
        PlanKind::Enable => "启用",
        PlanKind::Disable => "停用",
    };
    format!("{verb}服务 {name}（{target_count} 个目标）")
}

/// 为某个目标生成操作序列（build 与 apply 共用，保证一致）。
pub fn ops_for(
    adapter: &dyn McpAdapter,
    request: &PlanRequest,
    target: &WriteTarget,
) -> Result<(Vec<ServerOp>, Vec<StageAction>), McpError> {
    let project_path = target.project_path.clone().unwrap_or_default();
    let mut ops: Vec<ServerOp> = Vec::new();
    let mut stage_actions: Vec<StageAction> = Vec::new();
    match request.kind {
        PlanKind::Upsert => {
            let server = request
                .server
                .clone()
                .ok_or_else(|| McpError::bad_request("缺少服务定义"))?;
            ops.push(ServerOp::Upsert { server });
        }
        PlanKind::Remove => ops.push(ServerOp::Remove {
            name: request.name.clone(),
        }),
        PlanKind::Enable => {
            if adapter.supports_disable() {
                ops.push(ServerOp::SetEnabled {
                    name: request.name.clone(),
                    enabled: true,
                });
            } else {
                let server = request
                    .server
                    .clone()
                    .ok_or_else(|| McpError::bad_request("恢复服务需要原始定义"))?;
                ops.push(ServerOp::Upsert {
                    server: server.clone(),
                });
                stage_actions.push(StageAction {
                    agent: target.agent.clone(),
                    scope: target.scope.clone(),
                    project_path: project_path.clone(),
                    server,
                    action: "unstage".to_owned(),
                });
            }
        }
        PlanKind::Disable => {
            if adapter.supports_disable() {
                ops.push(ServerOp::SetEnabled {
                    name: request.name.clone(),
                    enabled: false,
                });
            } else {
                let server = request
                    .server
                    .clone()
                    .ok_or_else(|| McpError::bad_request("停用服务需要原始定义"))?;
                ops.push(ServerOp::Remove {
                    name: request.name.clone(),
                });
                stage_actions.push(StageAction {
                    agent: target.agent.clone(),
                    scope: target.scope.clone(),
                    project_path: project_path.clone(),
                    server,
                    action: "stage".to_owned(),
                });
            }
        }
    }
    Ok((ops, stage_actions))
}

/// 生成变更计划：每个目标文件一个 diff，附 mtime 快照。不落盘。
pub fn build_plan(
    paths: &McpPaths,
    request: &PlanRequest,
) -> Result<(McpPlan, StoredPlan), McpError> {
    if request.targets.is_empty() {
        return Err(McpError::bad_request("至少选择一个安装目标"));
    }
    if request.name.trim().is_empty() {
        return Err(McpError::bad_request("服务名不能为空"));
    }

    let mut targets: Vec<TargetPlan> = Vec::new();
    let mut mtimes: HashMap<String, i64> = HashMap::new();
    let mut stage_actions: Vec<StageAction> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    for target in &request.targets {
        let adapter = adapter_by_id(&target.agent)
            .ok_or_else(|| McpError::bad_request(format!("未知 Agent：{}", target.agent)))?;
        let path = resolve_target(adapter.as_ref(), paths, target)?;
        let existed = path.is_file();
        let raw = if existed {
            fs::read_to_string(&path)?
        } else {
            String::new()
        };

        let (ops, target_stage) = ops_for(adapter.as_ref(), request, target)?;
        stage_actions.extend(target_stage);

        if let Some(server) = &request.server {
            if server.url.is_some() && !adapter.supports_remote() {
                warnings.push(format!(
                    "{} 不支持远程 MCP，连接地址不会写入",
                    adapter.label()
                ));
            }
        }

        let after = if ops.is_empty() {
            raw.clone()
        } else {
            adapter.apply(&raw, &ops)?
        };

        mtimes.insert(path.display().to_string(), file_mtime(&path));
        let diff = make_diff(&raw, &after);
        targets.push(TargetPlan {
            agent: adapter.id().to_owned(),
            agent_label: adapter.label().to_owned(),
            scope: target.scope.clone(),
            project_path: target.project_path.clone(),
            config_file: path.display().to_string(),
            exists: existed,
            before: raw,
            after,
            diff,
            warnings: Vec::new(),
        });
    }

    warnings.sort();
    warnings.dedup();

    let id = uuid::Uuid::new_v4().to_string();
    let plan = McpPlan {
        id: id.clone(),
        summary: plan_summary(request.kind, &request.name, targets.len()),
        targets,
        warnings: warnings.clone(),
    };
    let stored = StoredPlan {
        request: request.clone(),
        mtimes,
        stage_actions,
    };
    Ok((plan, stored))
}
