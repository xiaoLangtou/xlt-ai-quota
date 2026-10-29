//! R1 只读来源模型：来源文件登记、经确认的字面量 `source` 引用跟踪、
//! 已知工具归属标记匹配、跨文件环境定义归因。
//!
//! 边界（PRD 7.2）：
//! - 默认只读已注册的两个文件；`.zshenv` / `.zprofile` / `/etc/paths` / `/etc/paths.d`
//!   由用户主动勾选扩展后才读取，全部只读、不做任何修改；
//! - 仅跟踪「经静态证明的简单字面量 source 引用」：路径为字面量（允许 ~ 与 $HOME
//!   前缀），条件表达式 / 命令替换 / 变量拼接一律不跟踪；
//! - 预算：深度 3、来源文件 20 个、单文件 1 MiB、总量 4 MiB；循环停止并显示路径；
//! - 工具名称来自已知标记匹配（行内签名或 `# >>> name >>>` 区块），未知显示未知，
//!   不推测所有者；
//! - 诊断措辞保守：说明「当前扫描文件」的事实，不声称所有终端的结果。

use crate::shellenv::parser::{scan_definitions, split_lines, top_level_statement_lines};
use crate::shellenv::types::ValueKind;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// 来源跟踪预算。
pub const MAX_DEPTH: usize = 3;
pub const MAX_FILES: usize = 20;
pub const MAX_TOTAL_BYTES: usize = 4 * 1024 * 1024;

// ---------------------------------------------------------------------------
// DTO
// ---------------------------------------------------------------------------

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceFileMeta {
    /// 稳定文件身份：`f1`、`f2`…
    pub id: String,
    pub path: String,
    /// 注册角色：alias / zshrc / zshenv / zprofile / system-paths / system-paths-d / sourced
    pub role: String,
    pub exists: bool,
    pub size: u64,
    /// 是否纳入本轮扫描（登记 ≠ 读取）
    pub included: bool,
    /// 纳入原因：registered / opt-in / sourced-ref
    pub via: String,
    /// 经 source 引入时：引入者文件与行（`zshrc:6`）
    pub sourced_from: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvOrigin {
    pub file_id: String,
    pub path: String,
    pub line: usize,
    pub key: String,
    pub value: String,
    pub value_kind: ValueKind,
    pub exported: bool,
    /// 仅 ~/.zshrc 托管区块内的条目为 true
    pub managed: bool,
    /// 上下文：顶层定义 / 条件或函数体内（动态）
    pub context: String,
    /// 工具归属：kiro / sdkman / mise / nvm / starship / homebrew…，未知为 unknown
    pub owner: String,
    /// 归属依据：signature（行内签名）/ marker（区块标记）/ unknown
    pub owner_evidence: String,
    pub raw: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceDiagnostic {
    /// source-loop / budget-depth / budget-files / budget-size / unreadable / marker-block
    pub code: String,
    pub message: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceMap {
    pub files: Vec<SourceFileMeta>,
    pub envs: Vec<EnvOrigin>,
    pub diagnostics: Vec<SourceDiagnostic>,
}

// ---------------------------------------------------------------------------
// 已知工具标记（保守匹配；未知一律 unknown）
// ---------------------------------------------------------------------------

/// 行内签名：出现在同一行即归属该工具。
const OWNER_SIGNATURES: &[(&str, &str)] = &[
    ("NVM_DIR", "nvm"),
    ("nvm.sh", "nvm"),
    ("sdkman-init.sh", "sdkman"),
    ("mise activate", "mise"),
    ("mise self-activate", "mise"),
    ("starship init", "starship"),
    ("brew shellenv", "homebrew"),
    ("kiro-cli", "kiro"),
    ("KIRO_", "kiro"),
    ("zoxide init", "zoxide"),
];

/// `# >>> <name> >>>` 区块标记中的名称（小写）。
fn block_marker_name(line: &str) -> Option<(String, bool)> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix("# >>>")?;
    let name: String = rest
        .trim()
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if name.is_empty() {
        return None;
    }
    Some((name.to_ascii_lowercase(), true))
}

fn line_signature_owner(line: &str) -> Option<&'static str> {
    OWNER_SIGNATURES
        .iter()
        .find(|(token, _)| line.contains(token))
        .map(|(_, owner)| *owner)
}

/// 归属判定：先看行内签名（certain），再看所属区块标记；都不命中则 unknown。
pub fn detect_owner(raw: &str, index: usize, line: &str) -> (String, String) {
    if let Some(owner) = line_signature_owner(line) {
        return (owner.to_string(), "signature".to_string());
    }
    // 所属区块：向上找最近的 `# >>> name >>>`，且之间未出现对应的 `# <<<`
    let lines = split_lines(raw);
    for back in (0..index).rev() {
        let text = lines[back].text(raw);
        let trimmed = text.trim();
        if trimmed.starts_with("# <<<") {
            break;
        }
        if let Some((name, _)) = block_marker_name(text) {
            return (name, "marker".to_string());
        }
    }
    ("unknown".to_string(), "unknown".to_string())
}

// ---------------------------------------------------------------------------
// 字面量 source 引用
// ---------------------------------------------------------------------------

/// 识别顶层、简单字面量的 source 行：`source <literal>` / `. <literal>`，
/// 尾部仅允许注释。路径允许 `~/` 与 `$HOME/` 前缀（由后端用已知主目录解析，
/// 不使用 eval）；其余变量 / 命令替换 / 拼接一律不跟踪。
pub fn literal_source_target(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let rest = match trimmed.strip_prefix("source ") {
        Some(rest) => rest,
        None => trimmed.strip_prefix(". ")?,
    };
    // 去掉尾部注释
    let token = rest.split('#').next()?.trim();
    if token.is_empty() || token.contains(char::is_whitespace) {
        return None;
    }
    // 字面量约束：单引号包裹原样取值；否则不得含 $（$HOME 前缀除外）与展开语法
    let expanded: String = if let Some(inner) = token.strip_prefix('\'') {
        inner.strip_suffix('\'')?.to_string()
    } else if token.starts_with('$') {
        let expanded = token.strip_prefix("$HOME")?;
        if expanded.contains('$') || expanded.contains('`') {
            return None;
        }
        format!("~{expanded}")
    } else {
        let bare = if token.len() >= 2 && token.starts_with('"') && token.ends_with('"') {
            &token[1..token.len() - 1]
        } else {
            token
        };
        if let Some(rest) = bare.strip_prefix("$HOME/") {
            if rest.contains('$') || rest.contains('`') || rest.contains('(') {
                return None;
            }
            format!("~/{rest}")
        } else if bare == "$HOME" {
            "~".to_string()
        } else {
            if bare.contains('$') || bare.contains('`') || bare.contains('(') {
                return None;
            }
            bare.to_string()
        }
    };
    // 归一：$HOME 前缀统一按 ~ 交给后端解析
    let expanded = if let Some(rest) = expanded.strip_prefix("$HOME") {
        format!("~{rest}")
    } else {
        expanded
    };
    Some(expanded)
}

fn resolve_path(token: &str, home: &Path) -> PathBuf {
    // $HOME 前缀统一按 ~ 处理
    let normalized = if let Some(rest) = token.strip_prefix("$HOME") {
        format!("~{rest}")
    } else {
        token.to_string()
    };
    if let Some(rest) = normalized.strip_prefix("~/") {
        home.join(rest)
    } else if normalized == "~" {
        home.to_path_buf()
    } else {
        PathBuf::from(normalized)
    }
}

/// 解析 `~` / `$HOME` 前缀的路径文本（仅这两类前缀；不使用 eval）。
pub fn resolve_path_text(token: &str, home: &Path) -> String {
    resolve_path(token, home).display().to_string()
}

// ---------------------------------------------------------------------------
// 扫描集合（供来源地图与 PATH 报告共用）
// ---------------------------------------------------------------------------

pub struct ScannedFile {
    pub meta: SourceFileMeta,
    pub raw: String,
    /// 是否为逐行目录清单（/etc/paths 与 /etc/paths.d/*）
    pub is_path_list: bool,
}

pub struct ScannedSet {
    /// 按近似 zsh 加载顺序排列：zshenv → system-paths → zprofile → zshrc → sourced
    pub files: Vec<ScannedFile>,
    pub diagnostics: Vec<SourceDiagnostic>,
}

fn read_limited(path: &Path) -> Result<Option<String>, String> {
    let meta = fs::metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if meta.len() > crate::shellenv::types::MAX_FILE_BYTES as u64 {
        return Err(format!("{} 超过单文件预算", path.display()));
    }
    let raw = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    String::from_utf8(raw)
        .map(Some)
        .map_err(|_| format!("{} 不是有效的 UTF-8 文本", path.display()))
}

fn file_meta(id: &str, path: &Path, role: &str, via: &str, sourced_from: Option<String>) -> SourceFileMeta {
    let (exists, size) = fs::metadata(path)
        .map(|meta| (true, meta.len()))
        .unwrap_or((false, 0));
    SourceFileMeta {
        id: id.to_string(),
        path: path.display().to_string(),
        role: role.to_string(),
        exists,
        size,
        included: false,
        via: via.to_string(),
        sourced_from,
    }
}

pub struct SourceOptions {
    /// 用户主动勾选：扩展读取 ~/.zshenv / ~/.zprofile / /etc/paths / /etc/paths.d
    pub include_extended: bool,
}

/// 构建来源扫描集合：登记 → 按需读取 → 字面量 source 引用跟踪（带预算与循环检测）。
pub fn build_scanned_set(options: &SourceOptions) -> Result<ScannedSet, String> {
    build_scanned_set_in(&crate::shellenv::files::home_dir()?, options)
}

/// 可注入主目录的扫描构建（测试隔离用）。
pub fn build_scanned_set_in(home: &Path, options: &SourceOptions) -> Result<ScannedSet, String> {
    let mut diagnostics = Vec::new();
    let mut files: Vec<ScannedFile> = Vec::new();
    let mut next_id = 1usize;
    // path → 已读取（循环检测）
    let mut loaded: HashMap<PathBuf, String> = HashMap::new();
    let mut total_bytes = 0usize;

    let mut push_file = |meta: SourceFileMeta, include: bool, is_path_list: bool| {
        let mut meta = meta;
        meta.included = include;
        files.push(ScannedFile { meta, raw: String::new(), is_path_list });
    };

    // 登记：已注册的两个文件
    let registered: Vec<(PathBuf, &str)> = vec![
        (home.join(".zshenv"), "zshenv"),
        (home.join(".alias"), "alias"),
        (home.join(".zshrc"), "zshrc"),
        (home.join(".zprofile"), "zprofile"),
    ];
    let mut pending_source_roots: Vec<String> = Vec::new();
    for (path, role) in &registered {
        let include = matches!(*role, "alias" | "zshrc") || options.include_extended;
        let meta = file_meta(&format!("f{next_id}"), path, role, "registered", None);
        next_id += 1;
        if include {
            pending_source_roots.push(path.display().to_string());
        }
        push_file(meta, include, false);
    }

    // 登记：系统路径清单（用户勾选后只读）
    if options.include_extended {
        let etc_paths = PathBuf::from("/etc/paths");
        push_file(file_meta(&format!("f{next_id}"), &etc_paths, "system-paths", "opt-in", None), true, true);
        next_id += 1;
        if let Ok(entries) = fs::read_dir("/etc/paths.d") {
            let mut names: Vec<PathBuf> = entries
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .filter(|path| path.is_file())
                .take(MAX_FILES)
                .collect();
            names.sort();
            for path in names {
                push_file(file_meta(&format!("f{next_id}"), &path, "system-paths-d", "opt-in", None), true, true);
                next_id += 1;
            }
        }
    }

    // 读取：登记文件（按登记顺序；id 与 meta 对应）
    for scanned in files.iter_mut() {
        if !scanned.meta.included {
            continue;
        }
        let path = PathBuf::from(&scanned.meta.path);
        match read_limited(&path) {
            Ok(Some(raw)) => {
                total_bytes += raw.len();
                loaded.insert(path.clone(), scanned.meta.id.clone());
                scanned.raw = raw;
            }
            Ok(None) => {}
            Err(error) => diagnostics.push(SourceDiagnostic {
                code: "unreadable".to_string(),
                message: format!("无法读取 {}: {error}", path.display()),
            }),
        }
    }

    // 字面量 source 引用跟踪：递归内联展开，深度 ≤ 3，文件总数 ≤ 20，总量 ≤ 4 MiB。
    // 循环检测基于当前展开链路：已处理过的文件（如 .zshrc 合法引用 ~/.alias）静默跳过。
    if options.include_extended {
        let mut processed: std::collections::HashSet<PathBuf> =
            loaded.keys().cloned().collect();
        let roots: Vec<(PathBuf, String)> = pending_source_roots
            .iter()
            .filter_map(|display| {
                files
                    .iter()
                    .find(|scanned| scanned.meta.path == *display)
                    .map(|scanned| (PathBuf::from(display), scanned.meta.id.clone()))
            })
            .collect();
        let mut context = FollowContext {
            files: &mut files,
            loaded: &mut loaded,
            processed: &mut processed,
            diagnostics: &mut diagnostics,
            home: &home,
            next_id: &mut next_id,
            total_bytes: &mut total_bytes,
        };
        let mut chain: Vec<PathBuf> = Vec::new();
        for (path, id) in roots {
            context.follow(&path, &id, 1, &mut chain);
        }
    }

    Ok(ScannedSet { files, diagnostics })
}

/// source 引用跟踪的共享上下文。
struct FollowContext<'a> {
    files: &'a mut Vec<ScannedFile>,
    loaded: &'a mut HashMap<PathBuf, String>,
    processed: &'a mut std::collections::HashSet<PathBuf>,
    diagnostics: &'a mut Vec<SourceDiagnostic>,
    home: &'a Path,
    next_id: &'a mut usize,
    total_bytes: &'a mut usize,
}

impl FollowContext<'_> {
    fn follow(&mut self, parent_path: &Path, parent_id: &str, depth: usize, chain: &mut Vec<PathBuf>) {
        if depth > MAX_DEPTH {
            self.diagnostics.push(SourceDiagnostic {
                code: "budget-depth".to_string(),
                message: format!("source 引用深度超过 {MAX_DEPTH}，停止跟踪：{}", parent_path.display()),
            });
            return;
        }
        let Some(parent_raw) = self
            .files
            .iter()
            .find(|scanned| scanned.meta.id == parent_id)
            .map(|scanned| scanned.raw.clone())
        else {
            return;
        };
        let parent_display = parent_path.display().to_string();
        chain.push(parent_path.to_path_buf());
        for index in top_level_statement_lines(&parent_raw) {
            let line = split_lines(&parent_raw)[index].text(&parent_raw).to_string();
            let Some(token) = literal_source_target(&line) else {
                continue;
            };
            let target = resolve_path(&token, self.home);
            // 循环：目标已在当前展开链路中
            if chain.contains(&target) {
                self.diagnostics.push(SourceDiagnostic {
                    code: "source-loop".to_string(),
                    message: format!(
                        "检测到循环 source 引用，已停止：{parent_display}:{} → {}（链路中已包含）",
                        index + 1,
                        target.display()
                    ),
                });
                continue;
            }
            // 已登记处理过的文件（如 .zshrc 引用已注册的 ~/.alias）：不重复登记
            if self.processed.contains(&target) {
                continue;
            }
            if self.files.len() >= MAX_FILES {
                self.diagnostics.push(SourceDiagnostic {
                    code: "budget-files".to_string(),
                    message: format!("来源文件数达到上限 {MAX_FILES}，停止跟踪更多 source 引用"),
                });
                break;
            }
            self.processed.insert(target.clone());
            let sourced_from = Some(format!("{parent_display}:{}", index + 1));
            let raw = match read_limited(&target) {
                Ok(Some(raw)) => raw,
                Ok(None) => {
                    // 引用目标不存在：登记但不读取
                    let meta = file_meta(&format!("f{}", self.next_id), &target, "sourced", "sourced-ref", sourced_from);
                    *self.next_id += 1;
                    self.files.push(ScannedFile { meta, raw: String::new(), is_path_list: false });
                    continue;
                }
                Err(error) => {
                    self.diagnostics.push(SourceDiagnostic {
                        code: "unreadable".to_string(),
                        message: format!("无法读取 {}: {error}", target.display()),
                    });
                    continue;
                }
            };
            *self.total_bytes += raw.len();
            if *self.total_bytes > MAX_TOTAL_BYTES {
                self.diagnostics.push(SourceDiagnostic {
                    code: "budget-size".to_string(),
                    message: format!("扫描总量超过 {MAX_TOTAL_BYTES} 字节预算，停止跟踪更多 source 引用"),
                });
                break;
            }
            let mut meta = file_meta(&format!("f{}", self.next_id), &target, "sourced", "sourced-ref", sourced_from);
            *self.next_id += 1;
            meta.included = true;
            let id = meta.id.clone();
            self.loaded.insert(target.clone(), id.clone());
            self.files.push(ScannedFile { meta, raw, is_path_list: false });
            self.follow(&target, &id, depth + 1, chain);
        }
        chain.pop();
    }
}

/// 跨文件环境定义归因（来源地图）。
pub fn build_env_origins(set: &ScannedSet) -> Vec<EnvOrigin> {
    let mut origins = Vec::new();
    for scanned in &set.files {
        if !scanned.meta.included || scanned.is_path_list {
            continue;
        }
        let zshrc = scanned.meta.role == "zshrc";
        let (entries, _, _) = if zshrc {
            crate::shellenv::parser::scan_zshrc(&scanned.raw)
        } else {
            // 非托管文件：仅归因，无区块概念
            let definitions = scan_definitions(&scanned.raw);
            let lines = split_lines(&scanned.raw);
            let entries = definitions
                .into_iter()
                .map(|def| crate::shellenv::types::EnvEntry {
                    id: String::new(),
                    key: def.key,
                    value: def.value,
                    value_kind: def.value_kind,
                    exported: def.exported,
                    managed: false,
                    line: def.index + 1,
                    raw: lines[def.index].text(&scanned.raw).to_string(),
                })
                .collect();
            (entries, None, Vec::new())
        };
        for entry in entries {
            let index = entry.line - 1;
            let (owner, evidence) = detect_owner(&scanned.raw, index, &entry.raw);
            origins.push(EnvOrigin {
                file_id: scanned.meta.id.clone(),
                path: scanned.meta.path.clone(),
                line: entry.line,
                key: entry.key,
                value: entry.value,
                value_kind: entry.value_kind,
                exported: entry.exported,
                managed: entry.managed,
                context: if entry.value_kind == ValueKind::Expression && !entry.managed {
                    "dynamic".to_string()
                } else {
                    "static".to_string()
                },
                owner,
                owner_evidence: evidence,
                raw: entry.raw,
            });
        }
    }
    origins
}
