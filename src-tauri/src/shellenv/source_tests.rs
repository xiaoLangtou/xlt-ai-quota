//! R1 只读来源地图与 PATH 模型测试（PRD 7.2 / 8.1）。
//!
//! 全部使用临时目录与合成文件；不读取用户真实配置（`build_scanned_set_in`
//! 支持注入主目录；PATH 合并测试直接构造 `ScannedSet`）。

use crate::shellenv::pathmodel::build_path_report;
use crate::shellenv::source::{
    build_scanned_set_in, literal_source_target, resolve_path_text, SourceFileMeta, SourceOptions,
    ScannedFile, ScannedSet,
};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

fn temp_home() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("xlt-source-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).expect("创建临时主目录");
    dir
}

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("创建父目录");
    }
    fs::write(path, content).expect("写入测试文件");
}

// ---------------------------------------------------------------------------
// 字面量 source 识别
// ---------------------------------------------------------------------------

#[test]
fn literal_source_accepts_simple_paths_only() {
    assert_eq!(
        literal_source_target("source ~/.profile").as_deref(),
        Some("~/.profile")
    );
    assert_eq!(
        literal_source_target("source $HOME/.cargo/env").as_deref(),
        Some("~/.cargo/env")
    );
    assert_eq!(
        literal_source_target("source '$HOME/.cargo/env'").as_deref(),
        Some("~/.cargo/env")
    );
    assert_eq!(
        literal_source_target(". /tmp/xlt-libs/tools.sh").as_deref(),
        Some("/tmp/xlt-libs/tools.sh")
    );
    assert_eq!(
        literal_source_target("source \"$HOME/.cargo/env\"  # rust").as_deref(),
        Some("~/.cargo/env")
    );
    // 命令替换 / 多变量拼接 / 多余参数一律不跟踪
    assert_eq!(literal_source_target("source $(brew --prefix)/etc/brew Rx"), None);
    assert_eq!(literal_source_target("source $NVM_DIR/nvm.sh"), None);
    assert_eq!(literal_source_target("[ -f ~/.x ] && source ~/.x"), None);
    assert_eq!(literal_source_target("source ~/a ~/b"), None);
}

#[test]
fn resolve_path_text_expands_home_only() {
    let home = Path::new("/Users/demo");
    assert_eq!(resolve_path_text("~/bin", home), "/Users/demo/bin");
    assert_eq!(resolve_path_text("$HOME/.cargo/env", home), "/Users/demo/.cargo/env");
    assert_eq!(resolve_path_text("/opt/tools", home), "/opt/tools");
}

// ---------------------------------------------------------------------------
// 扫描集合：source 引用跟踪、循环检测、预算
// ---------------------------------------------------------------------------

#[test]
fn scanned_set_follows_literal_sources_and_reports_loop() {
    let home = temp_home();
    let lib = home.join("libs/tools.sh");
    write_file(&lib, "export LIB_VAR=1\n");
    write_file(
        &home.join(".zshrc"),
        &format!("export A=1\nsource {}\nsource {}\n", lib.display(), home.join(".zshrc").display()),
    );

    let set = build_scanned_set_in(&home, &SourceOptions { include_extended: true }).expect("扫描");

    // lib.sh 经 source 引入并读取
    let sourced = set.files.iter().find(|scanned| scanned.meta.role == "sourced").expect("sourced 文件应登记");
    assert!(sourced.meta.included);
    assert_eq!(sourced.raw, "export LIB_VAR=1\n");
    assert!(sourced.meta.sourced_from.as_deref().unwrap().ends_with(".zshrc:2"));

    // 自引用 .zshrc 属于链路循环
    assert!(
        set.diagnostics.iter().any(|diagnostic| diagnostic.code == "source-loop"),
        "应报告循环引用: {:?}",
        set.diagnostics
    );

    // 归因：A 来自 .zshrc，LIB_VAR 来自 sourced 文件
    let origins = crate::shellenv::source::build_env_origins(&set);
    let lib_origin = origins.iter().find(|origin| origin.key == "LIB_VAR").expect("LIB_VAR 归因");
    assert_eq!(lib_origin.file_id, sourced.meta.id);
    assert!(origins.iter().any(|origin| origin.key == "A" && origin.path.ends_with(".zshrc")));

    let _ = fs::remove_dir_all(&home);
}

#[test]
fn scanned_set_without_extended_reads_only_registered() {
    let home = temp_home();
    write_file(&home.join(".zshrc"), "export A=1\n");
    write_file(&home.join("other.sh"), "export B=2\n");

    let set = build_scanned_set_in(&home, &SourceOptions { include_extended: false }).expect("扫描");
    // 默认只读已注册的两个文件；zshenv / zprofile 仅登记
    let included: Vec<&str> = set
        .files
        .iter()
        .filter(|scanned| scanned.meta.included)
        .map(|scanned| scanned.meta.role.as_str())
        .collect();
    assert_eq!(included.len(), 2, "默认只读 alias + zshrc: {included:?}");
    assert!(included.contains(&"zshrc") && included.contains(&"alias"));
    assert!(set.files.iter().any(|scanned| scanned.meta.role == "zshenv" && !scanned.meta.included));

    let _ = fs::remove_dir_all(&home);
}

// ---------------------------------------------------------------------------
// PATH 模型：分类、合并、诊断
// ---------------------------------------------------------------------------

fn scan_file(id: &str, path: &str, role: &str, raw: &str) -> ScannedFile {
    ScannedFile {
        meta: SourceFileMeta {
            id: id.to_string(),
            path: path.to_string(),
            role: role.to_string(),
            exists: true,
            size: raw.len() as u64,
            included: true,
            via: "test".to_string(),
            sourced_from: None,
        },
        raw: raw.to_string(),
        is_path_list: false,
    }
}

fn synthetic_set(files: Vec<ScannedFile>) -> ScannedSet {
    ScannedSet { files, diagnostics: Vec::new() }
}

#[test]
fn path_segments_are_classified() {
    let home = temp_home();
    let set = synthetic_set(vec![scan_file(
        "f1",
        "/tmp/x/.zshrc",
        "zshrc",
        "export PATH=\"$HOME/bin:/opt/tools:$PATH\"\n",
    )]);
    let report = build_path_report(&set, &home);
    assert_eq!(report.contributions.len(), 1);
    let contribution = &report.contributions[0];
    assert_eq!(contribution.mode, "prepend");
    assert_eq!(contribution.context, "static");
    let kinds: Vec<&str> = contribution.segments.iter().map(|segment| segment.kind.as_str()).collect();
    assert_eq!(kinds, ["home-relative", "literal", "reference"]);
    assert_eq!(
        contribution.segments[0].resolved.as_deref(),
        Some(home.join("bin").to_str().unwrap())
    );
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn path_array_and_dynamic_context() {
    let home = temp_home();
    let set = synthetic_set(vec![scan_file(
        "f1",
        "/tmp/x/.zshrc",
        "zshrc",
        "path=(~/bin $path)\nif [ -d $HOME/x ]; then\n  export PATH=$HOME/x:$PATH\nfi\n",
    )]);
    let report = build_path_report(&set, &home);
    assert_eq!(report.contributions.len(), 2);
    // 数组：reference 前的元素为 prepend
    let array = &report.contributions[0];
    assert_eq!(array.mode, "prepend");
    assert_eq!(array.segments[0].kind, "home-relative");
    // 条件块内：dynamic，不并入 merged
    let dynamic = &report.contributions[1];
    assert_eq!(dynamic.context, "dynamic");
    assert!(report.merged.iter().all(|entry| entry.text != "$HOME/x"));
    assert!(report.diagnostics.iter().any(|diagnostic| diagnostic.code == "dynamic-note"));
    assert_eq!(report.summary.dynamic_count, 1);
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn path_merge_respects_load_order() {
    let home = temp_home();
    let set = synthetic_set(vec![
        scan_file("f1", "/tmp/x/.zshenv", "zshenv", "export PATH=/a-first:$PATH\n"),
        scan_file("f2", "/tmp/x/.zshrc", "zshrc", "export PATH=/b-second:$PATH\n"),
        scan_file("f3", "/tmp/x/tools.sh", "sourced", "export PATH=/c-sourced:$PATH\n"),
        // .zshrc source tools.sh：内联展开，c 应位于 b 与原引用点之间
        scan_file("f4", "/tmp/x/.zprofile", "zprofile", "export PATH=/d-zprofile:$PATH\n"),
    ]);
    // 给 f2 加 source 行
    let mut files = set.files;
    files[1].raw = format!("source {}\n{}", "/tmp/x/tools.sh", files[1].raw);
    let set = ScannedSet { files, diagnostics: Vec::new() };

    let report = build_path_report(&set, &home);
    let texts: Vec<&str> = report.merged.iter().map(|entry| entry.text.as_str()).collect();
    // zsh 语义：后执行的语句 prepend 在前 → 最左是 .zshrc（最后执行），
    // 内联 tools.sh 在 .zshrc 的 PATH 语句之前；最先执行的 .zshenv 在最右。
    assert_eq!(texts, ["/b-second", "/c-sourced", "/d-zprofile", "/a-first"]);
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn path_diagnostics_duplicates_missing_relative_empty() {
    let home = temp_home();
    let set = synthetic_set(vec![
        scan_file(
            "f1",
            "/tmp/x/.zshenv",
            "zshenv",
            "export PATH=/opt/dup:$PATH\nexport PATH=bin:$PATH\nexport PATH=:$PATH\n",
        ),
        scan_file("f2", "/tmp/x/.zshrc", "zshrc", "export PATH=/opt/dup:/opt/definitely-missing-xyz:$PATH\n"),
    ]);
    let report = build_path_report(&set, &home);
    let codes: Vec<&str> = report.diagnostics.iter().map(|diagnostic| diagnostic.code.as_str()).collect();
    assert!(codes.contains(&"duplicate"), "{codes:?}");
    assert!(codes.contains(&"relative"));
    assert!(codes.contains(&"empty-entry"));
    assert!(codes.contains(&"missing"));
    // 重复证据含两个来源位置
    let duplicate = report
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "duplicate")
        .expect("重复诊断");
    assert_eq!(duplicate.sources.len(), 2);
    assert_eq!(report.summary.duplicate_groups, 1);
    let _ = fs::remove_dir_all(&home);
}

#[test]
fn path_replace_mode_resets_merged_list() {
    let home = temp_home();
    let set = synthetic_set(vec![scan_file(
        "f1",
        "/tmp/x/.zshrc",
        "zshrc",
        "export PATH=/a:$PATH\nexport PATH=/only\n",
    )]);
    let report = build_path_report(&set, &home);
    let texts: Vec<&str> = report.merged.iter().map(|entry| entry.text.as_str()).collect();
    assert_eq!(texts, ["/only"], "无 $PATH 引用的赋值是整体替换: {texts:?}");
    let _ = fs::remove_dir_all(&home);
}
