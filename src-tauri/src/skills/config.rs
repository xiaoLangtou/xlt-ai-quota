use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use super::error::SkillsError;
use super::paths::{normalize, resolve_path};

/// 已知 Agent 目录段 → Agent 展示名。
const AGENT_SEGMENTS: &[(&str, &str)] = &[
    (".qoder", "Qoder"),
    (".claude", "Claude"),
    (".codex", "Codex"),
    (".cursor", "Cursor"),
    (".gemini", "Gemini"),
    (".windsurf", "Windsurf"),
    (".trae", "Trae"),
    (".augment", "Augment"),
    (".agents", "Agents"),
    (".cline", "Cline"),
    (".continue", "Continue"),
    (".kiro", "Kiro"),
    (".codeium", "Codeium"),
    (".aider", "Aider"),
    (".roo", "Roo"),
    ("opencode", "OpenCode"),
    (".codebuddy", "CodeBuddy"),
    (".tongyi", "通义灵码"),
    (".marscode", "MarsCode"),
];

/// 已知 Agent 的 skill 目录相对路径（存在才纳入）。
const KNOWN_AGENT_SKILL_SUBDIRS: &[&str] = &[
    ".qoder/skills",
    ".claude/skills",
    ".augment/skills",
    ".agents/skills",
    ".codex/skills",
    ".cursor/skills",
    ".gemini/skills",
    ".windsurf/skills",
    ".trae/skills",
    ".cline/skills",
    ".continue/skills",
    ".kiro/skills",
    ".codeium/skills",
    ".aider/skills",
    ".roo/skills",
    ".config/opencode/skills",
    ".tongyi/skills",
    ".marscode/skills",
    ".codebuddy/skills",
];

/// 项目根目录下探测 skill 根目录的候选相对路径（含普通 `skills`）。
const PROJECT_SKILL_SUBDIRS: &[&str] = &[
    ".qoder/skills",
    ".claude/skills",
    ".augment/skills",
    ".agents/skills",
    ".codex/skills",
    ".cursor/skills",
    ".gemini/skills",
    ".windsurf/skills",
    ".trae/skills",
    ".cline/skills",
    ".continue/skills",
    ".kiro/skills",
    ".codeium/skills",
    ".aider/skills",
    ".roo/skills",
    ".config/opencode/skills",
    ".tongyi/skills",
    ".marscode/skills",
    ".codebuddy/skills",
    "skills",
];

/// 目录扫描时跳过的目录名。
const SCAN_SKIP_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "dist",
    "build",
    ".next",
    ".nuxt",
    ".cache",
    ".turbo",
    "target",
    "vendor",
    ".venv",
    "__pycache__",
    ".idea",
    ".vscode-test",
];

/// 目录扫描最大深度。
const MAX_SCAN_DEPTH: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillScope {
    Global,
    Project,
}

/// 一个 skill 根目录（如 ~/.qoder/skills）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillRoot {
    pub id: String,
    pub label: String,
    pub path: String,
    pub writable: bool,
    pub is_default: bool,
    pub exists: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_count: Option<u64>,
    pub scope: SkillScope,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_label: Option<String>,
}

/// 一个已登记的项目（项目级 skill 的来源目录）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillProject {
    pub id: String,
    pub label: String,
    pub path: String,
    pub exists: bool,
    pub root_count: usize,
    pub skill_count: usize,
}

/// 项目下已知 Agent 的候选 skill 目录（含尚未创建的）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAgentDir {
    pub root_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    pub path: String,
    pub exists: bool,
    pub writable: bool,
    pub skill_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirScanAgent {
    pub agent: String,
    pub skills: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirScanProject {
    pub path: String,
    pub label: String,
    pub registered: bool,
    pub agents: Vec<DirScanAgent>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirScanResult {
    pub root: String,
    pub skill_dir_count: usize,
    pub projects: Vec<DirScanProject>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StoredProject {
    pub path: String,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StoredConfig {
    #[serde(default)]
    pub extra_roots: Vec<String>,
    #[serde(default)]
    pub label_overrides: HashMap<String, String>,
    #[serde(default)]
    pub projects: Vec<StoredProject>,
}

/// 本机路径与配置目录。集中封装，便于测试注入临时目录。
#[derive(Debug, Clone)]
pub struct SkillsPaths {
    pub home: PathBuf,
    pub config_dir: PathBuf,
    pub staging_dir: PathBuf,
    pub default_roots: Vec<PathBuf>,
    /// 是否启用「自动发现 ~ 下已知 Agent skill 目录」。设置 LSM_DEFAULT_ROOTS 时为 false。
    pub discover_default_roots: bool,
}

impl SkillsPaths {
    pub fn detect() -> Self {
        let home = dirs::home_dir().unwrap_or_default();
        let config_dir = env::var_os("LSM_CONFIG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local-skills-manager"));
        let staging_dir = env::var_os("LSM_STAGING_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                env::temp_dir()
                    .join("local-skills-manager")
                    .join("staging")
            });
        let (default_roots, discover_default_roots) = match env::var("LSM_DEFAULT_ROOTS") {
            Ok(value) => (
                value
                    .split(':')
                    .filter(|item| !item.is_empty())
                    .map(PathBuf::from)
                    .collect(),
                false,
            ),
            Err(_) => (
                vec![home.join(".qoder").join("skills"), home.join(".claude").join("skills")],
                true,
            ),
        };
        Self {
            home,
            config_dir,
            staging_dir,
            default_roots,
            discover_default_roots,
        }
    }

    /// 配置目录（供回收站/安装记录/数据源等模块复用同一隔离约定）。
    pub fn config_file(&self) -> PathBuf {
        self.config_dir.join("config.json")
    }
}

/// 基于绝对路径生成稳定 root id（sha1 前 10 位，与原实现一致）。
pub fn root_id(abs: &Path) -> String {
    let resolved = normalize(abs).to_string_lossy().to_string();
    let digest = Sha1::digest(resolved.as_bytes());
    format!("{digest:x}").chars().take(10).collect()
}

/// 基于项目绝对路径生成稳定 project id。
pub fn project_id(abs: &Path) -> String {
    let resolved = normalize(abs).to_string_lossy().to_string();
    let digest = Sha1::digest(format!("project:{resolved}").as_bytes());
    format!("{digest:x}").chars().take(10).collect()
}

fn read_stored(paths: &SkillsPaths) -> StoredConfig {
    fs::read_to_string(paths.config_file())
        .ok()
        .and_then(|raw| serde_json::from_str::<StoredConfig>(&raw).ok())
        .unwrap_or_default()
}

fn write_stored(paths: &SkillsPaths, config: &StoredConfig) -> Result<(), SkillsError> {
    fs::create_dir_all(&paths.config_dir)?;
    let text = serde_json::to_string_pretty(config)
        .map_err(|error| SkillsError::internal(error.to_string()))?;
    fs::write(paths.config_file(), text)?;
    Ok(())
}

fn path_string(path: &Path) -> String {
    normalize(path).to_string_lossy().to_string()
}

/// 目录是否可写：不存在时逐级向上找最近的已存在祖先测试写权限。
fn is_writable(path: &Path) -> bool {
    let mut current = path.to_path_buf();
    loop {
        if current.exists() {
            return match fs::metadata(&current) {
                Ok(metadata) => !metadata.permissions().readonly(),
                Err(_) => false,
            };
        }
        match current.parent() {
            Some(parent) if parent != current => current = parent.to_path_buf(),
            _ => return false,
        }
    }
}

/// 轻量统计目录下的 skill（子目录/软链接）数量。
fn count_skills(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') {
                return false;
            }
            entry
                .file_type()
                .map(|kind| kind.is_dir() || kind.is_symlink())
                .unwrap_or(false)
        })
        .count() as u64
}

fn agent_segment_name(segment: &str) -> Option<&'static str> {
    for (candidate, agent) in AGENT_SEGMENTS {
        if *candidate == segment {
            return Some(*agent);
        }
    }
    None
}

/// 根据路径识别所属 Agent；未命中已知模式时取路径倒数第二段推断。
pub fn detect_agent(abs: &Path) -> Option<String> {
    let parts: Vec<String> = abs
        .components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().to_string()),
            _ => None,
        })
        .collect();
    for part in &parts {
        if let Some(agent) = agent_segment_name(part) {
            return Some(agent.to_owned());
        }
    }
    if parts.len() >= 2 {
        let segment = &parts[parts.len() - 2];
        if segment != "skills" {
            return Some(segment.clone());
        }
    }
    None
}

fn display_label(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) => {
            let suffix = rest.to_string_lossy().replace('\\', "/");
            if suffix.is_empty() {
                "~".to_owned()
            } else {
                format!("~/{suffix}")
            }
        }
        Err(_) => normalize(path).to_string_lossy().to_string(),
    }
}

#[derive(Default)]
struct RootOptions {
    scope: Option<SkillScope>,
    project_id: Option<String>,
    project_label: Option<String>,
    label_override: Option<String>,
}

fn to_root(paths: &SkillsPaths, abs: &Path, is_default: bool, options: RootOptions) -> SkillRoot {
    let abs = normalize(abs);
    SkillRoot {
        id: root_id(&abs),
        label: options
            .label_override
            .filter(|label| !label.trim().is_empty())
            .unwrap_or_else(|| display_label(&abs, &paths.home)),
        path: path_string(&abs),
        writable: is_writable(&abs),
        is_default,
        exists: abs.exists(),
        agent: detect_agent(&abs),
        skill_count: Some(count_skills(&abs)),
        scope: options.scope.unwrap_or(SkillScope::Global),
        project_id: options.project_id,
        project_label: options.project_label,
    }
}

/// 探测全局（~）下所有已知 Agent 的 skill 目录绝对路径（存在才纳入）。
fn discover_global_root_paths(paths: &SkillsPaths) -> Vec<PathBuf> {
    if !paths.discover_default_roots {
        return Vec::new();
    }
    KNOWN_AGENT_SKILL_SUBDIRS
        .iter()
        .map(|sub| {
            sub.split('/')
                .fold(paths.home.clone(), |acc, part| acc.join(part))
        })
        .filter(|abs| abs.exists())
        .collect()
}

/// 探测某个项目目录下存在的 skill 根目录绝对路径。
fn probe_project_root_paths(project: &Path) -> Vec<PathBuf> {
    PROJECT_SKILL_SUBDIRS
        .iter()
        .map(|sub| {
            sub.split('/')
                .fold(project.to_path_buf(), |acc, part| acc.join(part))
        })
        .filter(|abs| abs.exists())
        .collect()
}

fn project_to_dto(project: &StoredProject, home: &Path) -> SkillProject {
    let project_abs = resolve_path(&project.path);
    let root_paths = probe_project_root_paths(&project_abs);
    let label = project
        .label
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| {
            project_abs
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| display_label(&project_abs, home))
        });
    SkillProject {
        id: project_id(&project_abs),
        label,
        path: path_string(&project_abs),
        exists: project_abs.exists(),
        root_count: root_paths.len(),
        skill_count: root_paths
            .iter()
            .map(|path| count_skills(path) as usize)
            .sum(),
    }
}

/// 汇总所有项目级 skill 根目录。
fn list_project_roots(paths: &SkillsPaths, stored: &StoredConfig) -> Vec<SkillRoot> {
    let mut roots = Vec::new();
    for project in &stored.projects {
        let project_abs = resolve_path(&project.path);
        let pid = project_id(&project_abs);
        let project_label = project
            .label
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| {
                project_abs
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_default()
            });
        for abs in probe_project_root_paths(&project_abs) {
            roots.push(to_root(
                paths,
                &abs,
                false,
                RootOptions {
                    scope: Some(SkillScope::Project),
                    project_id: Some(pid.clone()),
                    project_label: Some(project_label.clone()),
                    label_override: stored.label_overrides.get(&root_id(&abs)).cloned(),
                },
            ));
        }
    }
    roots
}

/// 返回所有已登记的根目录（全局默认 + 全局额外 + 项目级，去重）。
pub fn list_roots(paths: &SkillsPaths) -> Vec<SkillRoot> {
    let stored = read_stored(paths);
    let mut seen = HashSet::new();
    let mut roots = Vec::new();
    let mut push = |root: SkillRoot, roots: &mut Vec<SkillRoot>| {
        if seen.insert(root.path.clone()) {
            roots.push(root);
        }
    };

    for path in &paths.default_roots {
        let abs = resolve_path(&path.to_string_lossy());
        let label_override = stored.label_overrides.get(&root_id(&abs)).cloned();
        push(
            to_root(
                paths,
                &abs,
                true,
                RootOptions {
                    scope: Some(SkillScope::Global),
                    label_override,
                    ..Default::default()
                },
            ),
            &mut roots,
        );
    }
    for abs in discover_global_root_paths(paths) {
        let label_override = stored.label_overrides.get(&root_id(&abs)).cloned();
        push(
            to_root(
                paths,
                &abs,
                true,
                RootOptions {
                    scope: Some(SkillScope::Global),
                    label_override,
                    ..Default::default()
                },
            ),
            &mut roots,
        );
    }
    for raw in &stored.extra_roots {
        let abs = resolve_path(raw);
        let label_override = stored.label_overrides.get(&root_id(&abs)).cloned();
        push(
            to_root(
                paths,
                &abs,
                false,
                RootOptions {
                    scope: Some(SkillScope::Global),
                    label_override,
                    ..Default::default()
                },
            ),
            &mut roots,
        );
    }
    for root in list_project_roots(paths, &stored) {
        push(root, &mut roots);
    }
    roots
}

/// 按 id 查找根目录。
pub fn get_root(paths: &SkillsPaths, id: &str) -> Option<SkillRoot> {
    list_roots(paths).into_iter().find(|root| root.id == id)
}

/// 返回所有已登记的项目及其统计。
pub fn list_projects(paths: &SkillsPaths) -> Vec<SkillProject> {
    read_stored(paths)
        .projects
        .iter()
        .map(|project| project_to_dto(project, &paths.home))
        .collect()
}

/// 新增一个根目录。
pub fn add_root(paths: &SkillsPaths, raw_path: &str) -> Result<SkillRoot, SkillsError> {
    let abs = resolve_path(raw_path);
    let mut stored = read_stored(paths);
    let already_default = paths
        .default_roots
        .iter()
        .any(|item| resolve_path(&item.to_string_lossy()) == abs);
    let already_extra = stored
        .extra_roots
        .iter()
        .any(|item| resolve_path(item) == abs);
    if !already_default && !already_extra {
        stored.extra_roots.push(path_string(&abs));
        write_stored(paths, &stored)?;
    }
    let label_override = stored.label_overrides.get(&root_id(&abs)).cloned();
    Ok(to_root(
        paths,
        &abs,
        false,
        RootOptions {
            scope: Some(SkillScope::Global),
            label_override,
            ..Default::default()
        },
    ))
}

/// 移除一个额外根目录（仅移除登记，不删除实际文件）。
pub fn remove_root(paths: &SkillsPaths, id: &str) -> bool {
    let mut stored = read_stored(paths);
    let before = stored.extra_roots.len();
    stored
        .extra_roots
        .retain(|item| root_id(&resolve_path(item)) != id);
    if stored.extra_roots.len() == before {
        return false;
    }
    write_stored(paths, &stored).is_ok()
}

/// 修改根目录展示名。
pub fn update_root_label(
    paths: &SkillsPaths,
    id: &str,
    label: &str,
) -> Result<SkillRoot, SkillsError> {
    let trimmed = label.trim();
    if trimmed.is_empty() {
        return Err(SkillsError::bad_request("展示名不能为空"));
    }
    let root = get_root(paths, id).ok_or_else(|| SkillsError::not_found("根目录不存在"))?;
    let mut stored = read_stored(paths);
    stored
        .label_overrides
        .insert(id.to_owned(), trimmed.to_owned());
    write_stored(paths, &stored)?;
    Ok(SkillRoot {
        label: trimmed.to_owned(),
        ..root
    })
}

/// 目录初始化：目录不存在且可写时创建。
pub fn init_root_dir(paths: &SkillsPaths, id: &str) -> Result<SkillRoot, SkillsError> {
    let root = get_root(paths, id).ok_or_else(|| SkillsError::not_found("根目录不存在"))?;
    if root.exists {
        return Ok(root);
    }
    if !root.writable {
        return Err(SkillsError::forbidden(format!(
            "父目录不可写，无法创建：{}",
            root.path
        )));
    }
    fs::create_dir_all(&root.path)
        .map_err(|error| SkillsError::internal(format!("创建目录失败：{error}")))?;
    get_root(paths, id).ok_or_else(|| SkillsError::not_found("根目录不存在"))
}

/// 新增一个项目。
pub fn add_project(
    paths: &SkillsPaths,
    raw_path: &str,
    label: Option<String>,
) -> Result<SkillProject, SkillsError> {
    let abs = resolve_path(raw_path);
    let mut stored = read_stored(paths);
    let exists = stored
        .projects
        .iter()
        .any(|project| resolve_path(&project.path) == abs);
    if !exists {
        stored.projects.push(StoredProject {
            path: path_string(&abs),
            label: label
                .as_ref()
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned),
        });
        write_stored(paths, &stored)?;
    }
    let project = stored
        .projects
        .iter()
        .find(|project| resolve_path(&project.path) == abs)
        .cloned()
        .unwrap_or(StoredProject {
            path: path_string(&abs),
            label,
        });
    Ok(project_to_dto(&project, &paths.home))
}

/// 移除一个项目（仅移除登记，不删除实际文件）。
pub fn remove_project(paths: &SkillsPaths, id: &str) -> bool {
    let mut stored = read_stored(paths);
    let before = stored.projects.len();
    stored
        .projects
        .retain(|project| project_id(&resolve_path(&project.path)) != id);
    if stored.projects.len() == before {
        return false;
    }
    write_stored(paths, &stored).is_ok()
}

/// 列出项目下已知 Agent 的候选 skill 目录（含尚未创建的）。
pub fn list_project_agent_dirs(
    paths: &SkillsPaths,
    pid: &str,
) -> Result<Vec<ProjectAgentDir>, SkillsError> {
    let stored = read_stored(paths);
    let project = stored
        .projects
        .iter()
        .find(|project| project_id(&resolve_path(&project.path)) == pid)
        .ok_or_else(|| SkillsError::not_found("项目不存在"))?;
    let project_abs = resolve_path(&project.path);

    let mut dirs: Vec<ProjectAgentDir> = PROJECT_SKILL_SUBDIRS
        .iter()
        .filter_map(|sub| {
            let abs = sub
                .split('/')
                .fold(project_abs.clone(), |acc, part| acc.join(part));
            let exists = abs.exists();
            // 普通 skills 目录没有 agent 归属，只在已存在时展示。
            if !exists && *sub == "skills" {
                return None;
            }
            Some(ProjectAgentDir {
                root_id: root_id(&abs),
                agent: detect_agent(&abs),
                path: path_string(&abs),
                exists,
                writable: is_writable(&abs),
                skill_count: if exists {
                    count_skills(&abs) as usize
                } else {
                    0
                },
            })
        })
        .collect();
    dirs.sort_by(|a, b| {
        b.exists
            .cmp(&a.exists)
            .then_with(|| a.agent.clone().unwrap_or_default().cmp(&b.agent.clone().unwrap_or_default()))
    });
    Ok(dirs)
}

/// 解析安装目标根目录：已登记根目录，或已登记项目下尚未创建的已知 agent 目录。
pub fn find_target_root(paths: &SkillsPaths, id: &str) -> Option<SkillRoot> {
    if let Some(root) = get_root(paths, id) {
        return Some(root);
    }
    let stored = read_stored(paths);
    for project in &stored.projects {
        let project_abs = resolve_path(&project.path);
        for sub in PROJECT_SKILL_SUBDIRS {
            let abs = sub
                .split('/')
                .fold(project_abs.clone(), |acc, part| acc.join(part));
            if root_id(&abs) == id {
                let project_label = project
                    .label
                    .as_ref()
                    .map(|value| value.trim())
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| {
                        project_abs
                            .file_name()
                            .map(|name| name.to_string_lossy().to_string())
                            .unwrap_or_default()
                    });
                return Some(to_root(
                    paths,
                    &abs,
                    false,
                    RootOptions {
                        scope: Some(SkillScope::Project),
                        project_id: Some(project_id(&project_abs)),
                        project_label: Some(project_label),
                        ..Default::default()
                    },
                ));
            }
        }
    }
    None
}

/// 递归收集含 SKILL.md 的目录路径段（skill 目录视为叶子，不再深入）。
fn walk_skill_dirs(dir: &Path, segments: &mut Vec<String>, depth: usize, found: &mut Vec<Vec<String>>) {
    if depth > MAX_SCAN_DEPTH {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let entries: Vec<fs::DirEntry> = entries.flatten().collect();
    if entries
        .iter()
        .any(|entry| entry.file_name() == "SKILL.md" && entry.path().is_file())
    {
        found.push(segments.clone());
        return;
    }
    for entry in entries {
        let file_type = match entry.file_type() {
            Ok(kind) => kind,
            Err(_) => continue,
        };
        let name = entry.file_name().to_string_lossy().to_string();
        if file_type.is_dir() && !SCAN_SKIP_DIRS.contains(&name.as_str()) {
            segments.push(name);
            walk_skill_dirs(&entry.path(), segments, depth + 1, found);
            segments.pop();
        }
    }
}

/// 扫描一个目录，按「<项目>/<agent 目录>/skills/<skill>」归组出含 agent 的项目。
pub fn scan_dir_for_projects(
    paths: &SkillsPaths,
    raw_path: &str,
) -> Result<DirScanResult, SkillsError> {
    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
        return Err(SkillsError::bad_request("缺少 path"));
    }
    let root = resolve_path(trimmed);
    if !root.exists() || !root.is_dir() {
        return Err(SkillsError::bad_request("路径不存在或不是目录"));
    }
    let mut found = Vec::new();
    walk_skill_dirs(&root, &mut Vec::new(), 0, &mut found);

    let registered: HashSet<String> = read_stored(paths)
        .projects
        .iter()
        .map(|project| path_string(&resolve_path(&project.path)))
        .collect();

    // 项目绝对路径 → agent 名 → skill 名列表
    let mut by_project: HashMap<String, HashMap<String, Vec<String>>> = HashMap::new();
    for segments in &found {
        for index in 0..segments.len().saturating_sub(1) {
            let Some(agent) = agent_segment_name(&segments[index]) else {
                continue;
            };
            if segments[index + 1] != "skills" {
                continue;
            }
            let project_path = segments[..index]
                .iter()
                .fold(root.clone(), |acc, part| acc.join(part));
            let project_key = path_string(&project_path);
            let skill_name = segments.last().cloned().unwrap_or_default();
            by_project
                .entry(project_key)
                .or_default()
                .entry(agent.to_owned())
                .or_default()
                .push(skill_name);
            break;
        }
    }

    let mut projects: Vec<DirScanProject> = by_project
        .into_iter()
        .map(|(path, agents)| {
            let mut agents: Vec<DirScanAgent> = agents
                .into_iter()
                .map(|(agent, mut skills)| {
                    skills.sort();
                    DirScanAgent { agent, skills }
                })
                .collect();
            agents.sort_by(|a, b| a.agent.cmp(&b.agent));
            let total = agents.iter().map(|agent| agent.skills.len()).sum();
            DirScanProject {
                label: Path::new(&path)
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_default(),
                registered: registered.contains(&path),
                path,
                agents,
                total,
            }
        })
        .collect();
    projects.sort_by(|a, b| b.total.cmp(&a.total));

    Ok(DirScanResult {
        root: path_string(&root),
        skill_dir_count: found.len(),
        projects,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_paths(root: &Path) -> SkillsPaths {
        SkillsPaths {
            home: root.join("home"),
            config_dir: root.join("config"),
            staging_dir: root.join("staging"),
            default_roots: vec![root.join("home/.qoder/skills")],
            discover_default_roots: false,
        }
    }

    #[test]
    fn detects_agent_from_segment() {
        assert_eq!(
            detect_agent(Path::new("/Users/me/.claude/skills")),
            Some("Claude".to_owned())
        );
        assert_eq!(
            detect_agent(Path::new("/Users/me/.config/opencode/skills")),
            Some("OpenCode".to_owned())
        );
    }

    #[test]
    fn root_id_is_stable_and_short() {
        let id = root_id(Path::new("/Users/me/.qoder/skills"));
        assert_eq!(id.len(), 10);
        assert_eq!(id, root_id(Path::new("/Users/me/.qoder/skills")));
    }

    #[test]
    fn adds_and_lists_roots() {
        let dir = tempfile_dir();
        let paths = temp_paths(&dir);
        let root = add_root(&paths, &dir.join("extra/skills").to_string_lossy()).unwrap();
        assert!(!root.is_default);
        let listed = list_roots(&paths);
        assert!(listed.iter().any(|item| item.id == root.id));
        assert!(remove_root(&paths, &root.id));
        assert!(!remove_root(&paths, &root.id));
    }

    fn tempfile_dir() -> PathBuf {
        let dir = env::temp_dir().join(format!(
            "xlt-skills-test-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
