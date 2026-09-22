use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use tauri::AppHandle;
use url::Url;

use super::adapters::{all_adapters, McpAdapter};
use super::error::McpError;
use super::metadata::{self, McpMeta};
use super::paths::McpPaths;
use super::types::{
    InstanceStatus, McpInstance, McpScanIssue, McpScanResult, McpServer, McpService, McpTransport,
};

/// 扫描开关：一次性聚合所有 Agent 的 MCP 配置。
pub struct ScanOptions<'a> {
    pub projects: &'a [String],
}

fn command_exists(command: &str) -> bool {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.contains('/') || Path::new(trimmed).is_absolute() {
        return Path::new(trimmed).is_file();
    }
    match env::var_os("PATH") {
        Some(value) => env::split_paths(&value).any(|dir| dir.join(trimmed).is_file()),
        None => false,
    }
}

fn is_valid_url(raw: &str) -> bool {
    match Url::parse(raw) {
        Ok(url) => matches!(url.scheme(), "http" | "https"),
        Err(_) => false,
    }
}

fn entry_status(
    adapter: &dyn McpAdapter,
    server: &McpServer,
    enabled: bool,
    staged: bool,
) -> InstanceStatus {
    if !adapter.supports_disable() && staged {
        return InstanceStatus::Disabled;
    }
    if !enabled {
        return InstanceStatus::Disabled;
    }
    if server.transport == McpTransport::Stdio {
        match &server.command {
            Some(command) if command_exists(command) => InstanceStatus::Ok,
            Some(_) => InstanceStatus::CommandMissing,
            None => InstanceStatus::ConfigError,
        }
    } else {
        match &server.url {
            Some(url) if is_valid_url(url) => InstanceStatus::Ok,
            _ => InstanceStatus::ConfigError,
        }
    }
}

fn recompute(service: &mut McpService) {
    let mut agents: Vec<String> = Vec::new();
    let mut scopes: Vec<String> = Vec::new();
    let mut enabled = false;
    let mut severity = InstanceStatus::Ok;
    for instance in &service.instances {
        if !agents.contains(&instance.agent) {
            agents.push(instance.agent.clone());
        }
        if !scopes.contains(&instance.scope) {
            scopes.push(instance.scope.clone());
        }
        enabled = enabled || instance.enabled;
        if instance.status.severity() > severity.severity() {
            severity = instance.status;
        }
    }
    service.agents = agents;
    service.scopes = scopes;
    service.enabled = enabled;
    service.status = severity.as_str().to_owned();
}

fn add_service(
    services: &mut Vec<McpService>,
    index: &mut HashMap<String, usize>,
    server: McpServer,
    instance: McpInstance,
) {
    let key = server.fingerprint();
    if let Some(&position) = index.get(&key) {
        services[position].instances.push(instance);
        recompute(&mut services[position]);
        return;
    }
    index.insert(key.clone(), services.len());
    let mut service = McpService {
        key,
        name: server.name.clone(),
        transport: server.transport.as_str().to_owned(),
        command: server.command.clone(),
        args: server.args.clone(),
        env: server.env.clone(),
        url: server.url.clone(),
        headers: server.headers.clone(),
        instances: vec![instance],
        agents: Vec::new(),
        scopes: Vec::new(),
        enabled: true,
        status: InstanceStatus::Ok.as_str().to_owned(),
        note: String::new(),
        tags: Vec::new(),
        source_id: None,
    };
    recompute(&mut service);
    services.push(service);
}

fn attach_meta(services: &mut [McpService], meta: &HashMap<String, McpMeta>) {
    for service in services.iter_mut() {
        if let Some(entry) = meta.get(&service.key) {
            service.note = entry.note.clone();
            service.tags = entry.tags.clone();
            service.source_id = entry.source_id.clone();
        }
    }
}

/// 扫描全部 Agent 的 MCP 配置并合并。全局默认扫描，项目按传入路径扫描。
pub fn scan(app: &AppHandle, paths: &McpPaths, options: &ScanOptions<'_>) -> Result<McpScanResult, McpError> {
    let adapters = all_adapters();
    let staged = metadata::list_staged(app)?;
    let meta = metadata::load_meta(app)?;

    let staged_keys: HashSet<(String, String, String, String)> = staged
        .iter()
        .map(|entry| {
            (
                entry.agent.clone(),
                entry.scope.clone(),
                entry.project_path.clone(),
                entry.server.name.clone(),
            )
        })
        .collect();

    let mut services: Vec<McpService> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut issues: Vec<McpScanIssue> = Vec::new();
    let mut scanned_files: Vec<String> = Vec::new();
    let mut seen: HashSet<(String, String, String, String)> = HashSet::new();

    for adapter in &adapters {
        if let Some(path) = adapter.global_path(paths) {
            scan_file(
                adapter.as_ref(),
                &path,
                "global",
                "",
                None,
                &staged_keys,
                &mut services,
                &mut index,
                &mut issues,
                &mut scanned_files,
                &mut seen,
            );
        }
        for project in options.projects {
            let project_path = Path::new(project);
            if let Some(path) = adapter.project_path(project_path) {
                let scope = "project";
                scan_file(
                    adapter.as_ref(),
                    &path,
                    scope,
                    project,
                    Some(project_path),
                    &staged_keys,
                    &mut services,
                    &mut index,
                    &mut issues,
                    &mut scanned_files,
                    &mut seen,
                );
            }
        }
    }

    // 暂存区里、配置中已不存在的服务：以「已停用」出现，保证停用后可恢复。
    for entry in &staged {
        let key = (
            entry.agent.clone(),
            entry.scope.clone(),
            entry.project_path.clone(),
            entry.server.name.clone(),
        );
        if seen.contains(&key) {
            continue;
        }
        let Some(adapter) = all_adapters().into_iter().find(|item| item.id() == entry.agent) else {
            continue;
        };
        let config_file = if entry.scope == "project" && !entry.project_path.is_empty() {
            adapter
                .project_path(Path::new(&entry.project_path))
                .map(|path| path.display().to_string())
                .unwrap_or_default()
        } else {
            adapter
                .global_path(paths)
                .map(|path| path.display().to_string())
                .unwrap_or_default()
        };
        let project_path = if entry.project_path.is_empty() {
            None
        } else {
            Some(entry.project_path.clone())
        };
        add_service(
            &mut services,
            &mut index,
            entry.server.clone(),
            McpInstance {
                agent: entry.agent.clone(),
                agent_label: adapter.label().to_owned(),
                scope: entry.scope.clone(),
                project_path,
                config_file,
                enabled: false,
                status: InstanceStatus::Disabled,
                error: None,
            },
        );
    }

    attach_meta(&mut services, &meta);
    services.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(McpScanResult {
        services,
        issues,
        scanned_files,
    })
}

#[allow(clippy::too_many_arguments)]
fn scan_file(
    adapter: &dyn McpAdapter,
    path: &PathBuf,
    scope: &str,
    project: &str,
    project_path: Option<&Path>,
    staged_keys: &HashSet<(String, String, String, String)>,
    services: &mut Vec<McpService>,
    index: &mut HashMap<String, usize>,
    issues: &mut Vec<McpScanIssue>,
    scanned_files: &mut Vec<String>,
    seen: &mut HashSet<(String, String, String, String)>,
) {
    if !path.is_file() {
        return;
    }
    scanned_files.push(path.display().to_string());
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) => {
            issues.push(McpScanIssue {
                agent: adapter.id().to_owned(),
                agent_label: adapter.label().to_owned(),
                config_file: path.display().to_string(),
                message: format!("读取失败：{error}"),
            });
            return;
        }
    };
    match adapter.parse(&raw) {
        Ok(entries) => {
            for entry in entries {
                let name = entry.server.name.clone();
                let key = (
                    adapter.id().to_owned(),
                    scope.to_owned(),
                    project.to_owned(),
                    name,
                );
                seen.insert(key.clone());
                let staged = staged_keys.contains(&key);
                let status = entry_status(adapter, &entry.server, entry.enabled, staged);
                add_service(
                    services,
                    index,
                    entry.server,
                    McpInstance {
                        agent: adapter.id().to_owned(),
                        agent_label: adapter.label().to_owned(),
                        scope: scope.to_owned(),
                        project_path: project_path.map(|path| path.display().to_string()),
                        config_file: path.display().to_string(),
                        enabled: status != InstanceStatus::Disabled,
                        status,
                        error: None,
                    },
                );
            }
        }
        Err(error) => {
            issues.push(McpScanIssue {
                agent: adapter.id().to_owned(),
                agent_label: adapter.label().to_owned(),
                config_file: path.display().to_string(),
                message: error.message,
            });
        }
    }
}
