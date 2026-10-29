//! Shell 环境模块测试：解析 / 序列化 / 字节补丁 / 计划管线。
//!
//! 回归样本覆盖（PRD 8.1）：CRLF、无末尾换行、引号与 heredoc、条件定义、
//! 旧格式区块标记、重复标记、含连字符 / 数字开头的别名、单引号转义。
//! 所有测试使用合成文档或临时目录，不触碰真实用户配置。

use crate::shellenv::files::{apply_edits, content_version, read_raw, LineEdit};
use crate::shellenv::parser::*;
use crate::shellenv::plan::{self, Doc, StoredPlan};
use crate::shellenv::serializer::{
    format_alias, format_export, session_command, shell_single_quoted, value_kind_label,
    SessionChange, MANAGED_BEGIN, MANAGED_END,
};
use crate::shellenv::types::*;
use std::fs;
use std::time::Instant;

// ---------------------------------------------------------------------------
// parser：行切分
// ---------------------------------------------------------------------------

#[test]
fn split_lines_preserves_crlf_and_missing_trailing_newline() {
    let raw = "a\r\nb\nc";
    let lines = split_lines(raw);
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0].text(raw), "a");
    assert_eq!(lines[0].terminator(raw), "\r\n");
    assert!(lines[0].has_terminator());
    assert_eq!(lines[1].text(raw), "b");
    assert_eq!(lines[1].terminator(raw), "\n");
    assert_eq!(lines[2].text(raw), "c");
    assert!(!lines[2].has_terminator());
    assert_eq!(detect_line_ending(raw), "\r\n");
    assert_eq!(detect_line_ending("a\nb"), "\n");
}

// ---------------------------------------------------------------------------
// parser：别名
// ---------------------------------------------------------------------------

#[test]
fn alias_parse_variants() {
    let (name, value, _, kind) = parse_alias_line("alias rnpm=\"rimraf package-lock.json\"").unwrap();
    assert_eq!((name.as_str(), value.as_str()), ("rnpm", "rimraf package-lock.json"));
    assert_eq!(kind, ValueKind::Literal); // 双引号但无展开语法

    let (name, value, _, _) = parse_alias_line("alias gcm='git commit -m'").unwrap();
    assert_eq!((name.as_str(), value.as_str()), ("gcm", "git commit -m"));

    let (name, value, _, _) = parse_alias_line("alias y=yarn").unwrap();
    assert_eq!((name.as_str(), value.as_str()), ("y", "yarn"));

    // 含连字符与数字开头的合法别名
    let (name, _, _, _) = parse_alias_line("alias npm-gok='npm config set'").unwrap();
    assert_eq!(name, "npm-gok");
    let (name, _, _, _) = parse_alias_line("alias 1=ls").unwrap();
    assert_eq!(name, "1");

    // 展开语义 → Expression
    let (_, value, _, kind) = parse_alias_line("alias x=\"$PATH/bin\"").unwrap();
    assert_eq!(value, "$PATH/bin");
    assert_eq!(kind, ValueKind::Expression);

    // 注释、非 alias 行、无效名
    assert!(parse_alias_line("# git相关操作简写").is_none());
    assert!(parse_alias_line("export FOO=1").is_none());
    assert!(parse_alias_line("alias -g foo=bar").is_none());
    assert!(parse_alias_line("alias 'a b'=x").is_none());
    assert!(parse_alias_line("export PATH").is_none());
}

#[test]
fn group_comment_ignores_commented_out_alias() {
    let raw = "alias ngc='nest g c'\n#alias ng=\"nest generate\"\nalias ngs='nest g s'\n# git相关\nalias g='git'\n";
    let entries = scan_alias(raw);
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[1].group, None);
    assert_eq!(entries[2].group.as_deref(), Some("git相关"));
}

// ---------------------------------------------------------------------------
// parser：环境变量赋值与值语义
// ---------------------------------------------------------------------------

#[test]
fn assignment_parse_and_value_kinds() {
    let (key, value, _, kind, exported) =
        parse_assignment_line("export HTTPS_PROXY=http://127.0.0.1:8798").unwrap();
    assert_eq!((key.as_str(), value.as_str()), ("HTTPS_PROXY", "http://127.0.0.1:8798"));
    assert_eq!(kind, ValueKind::Literal);
    assert!(exported);

    let (key, value, _, kind, _) = parse_assignment_line("export ZENTAO_PASSWORD='!@#secret'").unwrap();
    assert_eq!((key.as_str(), value.as_str()), ("ZENTAO_PASSWORD", "!@#secret"));
    assert_eq!(kind, ValueKind::Literal);

    // 表达式分类：$、${}、反引号、双引号含展开
    for line in [
        "export PATH=${PATH}:/extra",
        "export PROMPT=\"$(whoami)\"",
        "export HASH=`id -u`",
        "export DOUBLED=\"$HOME/bin\"",
    ] {
        let (key, _, _, kind, _) = parse_assignment_line(line).unwrap();
        assert_eq!(kind, ValueKind::Expression, "{key} 应分类为表达式");
    }

    // 普通赋值（无 export）
    let (key, _, _, _, exported) = parse_assignment_line("FOO=bar").unwrap();
    assert_eq!(key, "FOO");
    assert!(!exported);

    // 无 '=' 的 export、非法名
    assert!(parse_assignment_line("export PATH").is_none());
    assert!(parse_assignment_line("local x=1").is_none());
    assert!(parse_assignment_line("1bad=x").is_none());
}

// ---------------------------------------------------------------------------
// parser：.zshrc 结构扫描
// ---------------------------------------------------------------------------

const ZSHRC_SAMPLE: &str = "\
# 加载 alias
source ~/.alias
export FLUTTER_HOME=/flutter
export PATH=${PATH}:${FLUTTER_HOME}/bin
if [ -t 1 ]; then
  export INSIDE_IF=1
fi
export AFTER=ok
cat <<'EOF'
export IN_HEREDOC=should_skip
EOF
export CONT=a \\
  b
export LAST=done\
";

#[test]
fn zshrc_scan_classifies_contexts() {
    let (entries, block, issues) = scan_zshrc(ZSHRC_SAMPLE);
    assert!(issues.is_empty());
    assert!(block.is_none());

    let find = |key: &str| entries.iter().find(|e| e.key == key).unwrap();
    // 顶层 export：Literal / Expression 正确分类
    assert_eq!(find("FLUTTER_HOME").value_kind, ValueKind::Literal);
    assert_eq!(find("PATH").value_kind, ValueKind::Expression);
    // 条件块内：标记为 Expression（不可表格编辑），但仍列出
    let inside = find("INSIDE_IF");
    assert_eq!(inside.value_kind, ValueKind::Expression);
    assert!(!inside.managed);
    // heredoc 体、续行被跳过
    assert!(entries.iter().all(|e| e.key != "IN_HEREDOC"));
    assert!(entries.iter().all(|e| e.key != "b"));
    assert_eq!(find("LAST").value, "done");
    // 普通赋值 exported=false（样本内无；单独验证）
}

#[test]
fn zshrc_scan_bare_assignment_marked_unexported() {
    let (entries, _, _) = scan_zshrc("FOO=bar\nexport BAZ=qux\n");
    let foo = entries.iter().find(|e| e.key == "FOO").unwrap();
    assert!(!foo.exported);
    let baz = entries.iter().find(|e| e.key == "BAZ").unwrap();
    assert!(baz.exported);
}

#[test]
fn zshrc_scan_managed_block_and_issues() {
    let raw = format!("export KEEP=1\n{MANAGED_BEGIN}\nexport MANAGED_A='a'\n{MANAGED_END}\nexport AFTER=2\n");
    let (entries, block, issues) = scan_zshrc(&raw);
    assert!(issues.is_empty());
    let block = block.unwrap();
    assert_eq!((block.begin_line, block.end_line, block.entry_count), (2, 4, 1));
    let managed: Vec<_> = entries.iter().filter(|e| e.managed).collect();
    assert_eq!(managed.len(), 1);
    assert_eq!(managed[0].id, "env:MANAGED_A");
    assert!(entries.iter().filter(|e| !e.managed).count() >= 2);

    // 重复标记 → issues，条目不再标记 managed
    let bad = format!("{MANAGED_BEGIN}\n{MANAGED_BEGIN}\nexport X=1\n{MANAGED_END}\n");
    let (entries, block, issues) = scan_zshrc(&bad);
    assert!(!issues.is_empty());
    assert!(block.is_none());
    assert!(entries.iter().all(|e| !e.managed));

    // 不配对
    let unmatched = format!("{MANAGED_BEGIN}\nexport X=1\n");
    let (_, _, issues) = scan_zshrc(&unmatched);
    assert!(issues.iter().any(|i| i.contains("结束标记")));

    // 旧格式标记被识别（可规范化）
    let old = "# >>> xlt-managed env >>> (managed by xlt-workbench, do not edit manually)\nexport A=1\n# <<< xlt-managed env <<<\n";
    let (_, block, issues) = scan_zshrc(old);
    assert!(issues.is_empty());
    assert!(block.is_some());
}

#[test]
fn block_normalize_replaces_old_markers_only() {
    let old_begin = "# >>> xlt-managed env >>> (managed by xlt-workbench, do not edit manually)";
    let raw = format!("{old_begin}\nexport A=1\n{MANAGED_END}\n");
    let doc = Doc::from_raw(raw.clone());
    let (_, _, postimage, ops, _) =
        plan::build_change_for_doc(ShellChangeRequest::BlockNormalize, doc).unwrap();
    assert!(postimage.contains(MANAGED_BEGIN));
    assert!(!postimage.contains("(managed by xlt-workbench"));
    assert_eq!(ops.len(), 1);

    // 无旧标记 → 拒绝
    let doc = Doc::from_raw(format!("{MANAGED_BEGIN}\n{MANAGED_END}\n"));
    assert!(plan::build_change_for_doc(ShellChangeRequest::BlockNormalize, doc).is_err());
}

// ---------------------------------------------------------------------------
// serializer
// ---------------------------------------------------------------------------

#[test]
fn single_quote_escaping_roundtrip() {
    for value in ["yarn", "git config user.name '魏鹏程'", "say \"hi\"", "it's", "a\\b"] {
        let quoted = shell_single_quoted(value);
        // zsh 单引号转义规则：'...' 内的 ' 写作 '\''
        assert!(quoted.starts_with('\'') && quoted.ends_with('\''));
        let reopened = quoted.replace("'\\''", "'");
        assert_eq!(reopened, format!("'{value}'"), "roundtrip failed for {value:?}");
    }
}

#[test]
fn formatting_uses_literal_semantics() {
    assert_eq!(format_alias("gcm", "git commit -m"), "alias gcm='git commit -m'");
    assert_eq!(format_export("KEY", "va'l"), "export KEY='va'\\''l'");
}

#[test]
fn session_commands_match_change_kind() {
    assert_eq!(session_command(&SessionChange::AliasAdd), "source ~/.alias");
    assert_eq!(session_command(&SessionChange::AliasEdit), "source ~/.alias");
    assert_eq!(
        session_command(&SessionChange::AliasDelete { name: "gcm" }),
        "unalias gcm 2>/dev/null; source ~/.alias"
    );
    assert_eq!(
        session_command(&SessionChange::EnvSet { assignment: "export K='v'".into() }),
        "export K='v'  # 已开终端直接执行本行即可生效"
    );
    assert_eq!(
        session_command(&SessionChange::EnvDelete { key: "K" }),
        "unset K; source ~/.zshrc"
    );
    assert_eq!(
        session_command(&SessionChange::FullReload),
        "source ~/.alias && source ~/.zshrc"
    );
    assert_eq!(value_kind_label(ValueKind::Literal), "字面量");
}

// ---------------------------------------------------------------------------
// files：字节范围补丁（B04）
// ---------------------------------------------------------------------------

#[test]
fn apply_edits_preserves_unmodified_bytes_with_crlf() {
    let raw = "one\r\ntwo\r\nthree\r\n";
    // 替换中间行：其余行字节原样
    let out = apply_edits(raw, &[LineEdit::Replace { index: 1, text: "TWO".into() }]).unwrap();
    assert_eq!(out, "one\r\nTWO\r\nthree\r\n");

    // 替换末行（有终止符）保持终止符
    let out = apply_edits(raw, &[LineEdit::Replace { index: 2, text: "THREE".into() }]).unwrap();
    assert_eq!(out, "one\r\ntwo\r\nTHREE\r\n");

    // 无末尾换行的文件：替换末行不得擅自补换行
    let raw_no_nl = "a\nb\nc";
    let out =
        apply_edits(raw_no_nl, &[LineEdit::Replace { index: 2, text: "C".into() }]).unwrap();
    assert_eq!(out, "a\nb\nC");
    assert!(!out.ends_with('\n'));
}

#[test]
fn apply_edits_delete_insert_append() {
    let raw = "a\nb\nc\n";
    let out = apply_edits(raw, &[LineEdit::Delete { index: 1 }]).unwrap();
    assert_eq!(out, "a\nc\n");

    let out =
        apply_edits(raw, &[LineEdit::InsertAfter { index: 0, text: "a2".into() }]).unwrap();
    assert_eq!(out, "a\na2\nb\nc\n");

    // 末行无换行时插入：先补终止符
    let raw_no_nl = "a\nb";
    let out =
        apply_edits(raw_no_nl, &[LineEdit::InsertAfter { index: 1, text: "c".into() }]).unwrap();
    assert_eq!(out, "a\nb\nc\n");

    // Append：blank_before 补空行
    let out =
        apply_edits("x\n", &[LineEdit::Append { text: "y".into(), blank_before: true }]).unwrap();
    assert_eq!(out, "x\n\ny\n");
    let out =
        apply_edits("x", &[LineEdit::Append { text: "y".into(), blank_before: false }]).unwrap();
    assert_eq!(out, "x\ny\n");
    // 空文件追加
    let out =
        apply_edits("", &[LineEdit::Append { text: "y".into(), blank_before: true }]).unwrap();
    assert_eq!(out, "y\n");
}

#[test]
fn content_version_is_stable_hex() {
    let v1 = content_version("abc");
    let v2 = content_version("abc");
    assert_eq!(v1, v2);
    assert_eq!(v1.len(), 64);
    assert_ne!(v1, content_version("abd"));
}

// ---------------------------------------------------------------------------
// plan：别名变更（纯核心）
// ---------------------------------------------------------------------------

const ALIAS_SAMPLE: &str = "# npm\nalias ni='npm install'\nalias y='yarn'\nalias npm-gok='npm config set registry=http://nexus.example/repository/'\nalias dyn=\"$PWD/bin\"\n";

fn alias_doc() -> Doc {
    Doc::from_raw(ALIAS_SAMPLE)
}

#[test]
fn alias_add_plan_inserts_under_group() {
    let (kind, _, postimage, ops, session) = plan::build_change_for_doc(
        ShellChangeRequest::AliasAdd {
            name: "pi".into(),
            value: "pnpm install".into(),
            group: Some("npm".into()),
            description: None,
        },
        alias_doc(),
    )
    .unwrap();
    assert_eq!(kind, KIND_ALIAS);
    assert!(postimage.contains("alias pi='pnpm install'\n"));
    // 插入到分组末尾（组内最后一个别名之后，下一个分组注释之前）
    assert!(postimage.find("alias pi=").unwrap() > postimage.find("alias dyn=").unwrap());
    assert_eq!(ops.len(), 1);
    assert_eq!(session, "source ~/.alias");

    // 无分组 → 追加末尾
    let (_, _, postimage, _, _) = plan::build_change_for_doc(
        ShellChangeRequest::AliasAdd { name: "zz".into(), value: "z".into(), group: None, description: None },
        alias_doc(),
    )
    .unwrap();
    assert!(postimage.trim_end().ends_with("alias zz='z'"));
}

#[test]
fn alias_add_creates_missing_group_with_header() {
    let (_, edits, postimage, ops, _) = plan::build_change_for_doc(
        ShellChangeRequest::AliasAdd {
            name: "gcm".into(),
            value: "git commit -m".into(),
            group: Some("git".into()),
            description: Some("提交辅助".into()),
        },
        alias_doc(),
    )
    .unwrap();
    // 两个编辑：组头注释 + 别名行（组头前有空行分隔）
    assert_eq!(edits.len(), 2, "{edits:?}");
    assert!(edits.iter().any(|edit| matches!(edit, LineEdit::Append { text, .. } if text == "# git")));
    assert!(
        postimage.contains("\n# git\nalias gcm='git commit -m' # 提交辅助\n"),
        "应创建组头注释并写入带描述的别名行: {postimage}"
    );
    assert!(ops[0].detail.contains("创建分组「git」"));
}

#[test]
fn alias_description_roundtrip_in_plan() {
    // 新增带描述 → 编辑只改描述 → 描述清空
    let doc = Doc::from_raw("# npm\nalias ni='npm install'\n");
    let (_, _, postimage, _, _) = plan::build_change_for_doc(
        ShellChangeRequest::AliasAdd {
            name: "pi".into(),
            value: "pnpm install".into(),
            group: None,
            description: Some("安装依赖".into()),
        },
        doc.clone(),
    )
    .unwrap();
    assert!(postimage.contains("alias pi='pnpm install' # 安装依赖"), "{postimage}");

    // 扫描回读描述（含单引号值的行，注释切分不受影响）
    let entries = scan_alias(&postimage);
    let pi = entries.iter().find(|entry| entry.name == "pi").unwrap();
    assert_eq!(pi.description.as_deref(), Some("安装依赖"));

    // 编辑：仅改描述（值不变）也应产生变更
    let (_, _, postimage, _, _) = plan::build_change_for_doc(
        ShellChangeRequest::AliasEdit {
            entry_id: "alias:ni".into(),
            value: "npm install".into(),
            description: Some("安装包".into()),
        },
        doc,
    )
    .unwrap();
    assert!(postimage.contains("alias ni='npm install' # 安装包"), "{postimage}");
}

#[test]
fn alias_trailing_comment_parsing() {
    let entries = scan_alias("# 工具\nalias gcm='git commit -m \"x #y\"' # 提交辅助\nalias bare=ls\n");
    assert_eq!(entries.len(), 2);
    let gcm = &entries[0];
    assert_eq!(gcm.name, "gcm");
    assert_eq!(gcm.value, "git commit -m \"x #y\"", "引号内的 # 属于值本身");
    assert_eq!(gcm.description.as_deref(), Some("提交辅助"));
    assert_eq!(gcm.group.as_deref(), Some("工具"));
    assert_eq!(entries[1].description, None);
}

#[test]
fn alias_add_rejects_duplicate_and_expression_value() {
    let err = plan::build_change_for_doc(
        ShellChangeRequest::AliasAdd { name: "ni".into(), value: "npm i".into(), group: None, description: None },
        alias_doc(),
    )
    .unwrap_err();
    assert!(err.contains("已存在"));

    let err = plan::build_change_for_doc(
        ShellChangeRequest::AliasAdd { name: "p".into(), value: "echo $HOME".into(), group: None, description: None },
        alias_doc(),
    )
    .unwrap_err();
    assert!(err.contains("源码视图"));
}

#[test]
fn alias_edit_rejects_expression_entry() {
    let err = plan::build_change_for_doc(
        ShellChangeRequest::AliasEdit { entry_id: "alias:dyn".into(), value: "pwd".into(), description: None },
        alias_doc(),
    )
    .unwrap_err();
    assert!(err.contains("表达式"));

    // Literal 条目可编辑
    let (_, _, postimage, ops, _) = plan::build_change_for_doc(
        ShellChangeRequest::AliasEdit { entry_id: "alias:ni".into(), value: "npm i".into(), description: None },
        alias_doc(),
    )
    .unwrap();
    assert!(postimage.contains("alias ni='npm i'"));
    assert!(ops[0].detail.contains("npm install → npm i"));
}

#[test]
fn alias_rename_is_single_commit() {
    let (_, _, postimage, ops, _) = plan::build_change_for_doc(
        ShellChangeRequest::AliasRename { entry_id: "alias:ni".into(), new_name: "npm-i".into() },
        alias_doc(),
    )
    .unwrap();
    assert!(postimage.contains("alias npm-i='npm install'"));
    assert!(!postimage.contains("alias ni="));
    assert_eq!(ops.len(), 1);
    assert!(ops[0].action.contains("重命名"));
}

#[test]
fn alias_delete_generates_unalias_session_command() {
    let (_, _, postimage, ops, session) = plan::build_change_for_doc(
        ShellChangeRequest::AliasDelete { entry_id: "alias:ni".into() },
        alias_doc(),
    )
    .unwrap();
    assert!(!postimage.contains("alias ni="));
    assert_eq!(ops.len(), 1);
    assert!(session.contains("unalias ni"));
}

// ---------------------------------------------------------------------------
// plan：环境变量变更
// ---------------------------------------------------------------------------

#[test]
fn env_set_requires_block_create_confirmation() {
    let doc = Doc::from_raw("export KEEP=1\n");
    let err = plan::build_change_for_doc(
        ShellChangeRequest::EnvSet {
            key: "HTTP_PROXY".into(),
            value: "http://127.0.0.1:8798".into(),
            confirm_block_create: None,
        },
        doc.clone(),
    )
    .unwrap_err();
    assert_eq!(err, "BLOCK_CREATE_CONFIRM_REQUIRED");

    // 确认后：追加区块（带空行分隔）
    let (_, _, postimage, ops, session) = plan::build_change_for_doc(
        ShellChangeRequest::EnvSet {
            key: "HTTP_PROXY".into(),
            value: "http://127.0.0.1:8798".into(),
            confirm_block_create: Some(true),
        },
        doc,
    )
    .unwrap();
    assert!(postimage.ends_with(&format!("\n{MANAGED_BEGIN}\nexport HTTP_PROXY='http://127.0.0.1:8798'\n{MANAGED_END}\n")));
    assert!(ops.iter().any(|o| o.action.contains("创建托管区块")));
    assert!(session.contains("export HTTP_PROXY="));
}

#[test]
fn env_set_in_existing_block_inserts_and_replaces() {
    let raw = format!("export KEEP=1\n{MANAGED_BEGIN}\nexport A='1'\n{MANAGED_END}\n");
    // 新增条目：插到结束标记前
    let (_, _, postimage, _, _) = plan::build_change_for_doc(
        ShellChangeRequest::EnvSet {
            key: "B".into(),
            value: "2".into(),
            confirm_block_create: None,
        },
        Doc::from_raw(raw.clone()),
    )
    .unwrap();
    assert!(postimage.contains("export A='1'\nexport B='2'\n"));

    // 修改已有条目
    let (_, _, postimage, _, _) = plan::build_change_for_doc(
        ShellChangeRequest::EnvSet {
            key: "A".into(),
            value: "9".into(),
            confirm_block_create: None,
        },
        Doc::from_raw(raw),
    )
    .unwrap();
    assert!(postimage.contains("export A='9'"));
}

#[test]
fn env_set_flags_outside_duplicate() {
    let raw = format!("export DUP=outside\n{MANAGED_BEGIN}\n{MANAGED_END}\n");
    let (_, _, _, ops, _) = plan::build_change_for_doc(
        ShellChangeRequest::EnvSet { key: "DUP".into(), value: "inside".into(), confirm_block_create: None },
        Doc::from_raw(raw),
    )
    .unwrap();
    assert!(ops.iter().any(|o| o.action == "注意" && o.detail.contains("区块外")));
}

#[test]
fn env_set_rejects_when_block_issues() {
    let raw = format!("{MANAGED_BEGIN}\n{MANAGED_BEGIN}\n{MANAGED_END}\n");
    let err = plan::build_change_for_doc(
        ShellChangeRequest::EnvSet { key: "X".into(), value: "1".into(), confirm_block_create: Some(true) },
        Doc::from_raw(raw),
    )
    .unwrap_err();
    assert!(err.contains("结构异常"));
}

#[test]
fn env_delete_only_allows_managed_entries() {
    // 托管条目可删
    let raw = format!("export OUTSIDE=1\n{MANAGED_BEGIN}\nexport INSIDE='2'\n{MANAGED_END}\n");
    let (_, _, postimage, _, session) = plan::build_change_for_doc(
        ShellChangeRequest::EnvDelete { entry_id: "env:INSIDE".into() },
        Doc::from_raw(raw),
    )
    .unwrap();
    assert!(!postimage.contains("export INSIDE="));
    assert!(session.contains("unset INSIDE"));

    // 非托管条目拒绝
    let raw = format!("export OUTSIDE=1\n{MANAGED_BEGIN}\nexport INSIDE='2'\n{MANAGED_END}\n");
    let err = plan::build_change_for_doc(
        ShellChangeRequest::EnvDelete { entry_id: "env:OUTSIDE@0".into() },
        Doc::from_raw(raw),
    )
    .unwrap_err();
    assert!(err.contains("不在托管区块内"));
}

// ---------------------------------------------------------------------------
// plan：源码整写与恢复
// ---------------------------------------------------------------------------

#[test]
fn source_write_preserves_exact_content() {
    // 无末尾换行的内容原样保留，不擅自补换行
    let (_, _, postimage, ops, _) = plan::build_change_for_doc(
        ShellChangeRequest::SourceWrite { file_kind: KIND_ALIAS.into(), content: "alias a='ls'".into() },
        Doc::from_raw("alias a='echo'\n"),
    )
    .unwrap();
    assert_eq!(postimage, "alias a='ls'");
    assert!(ops.iter().any(|o| o.action == "整文件写入"));

    // 内容未变 → 拒绝
    let err = plan::build_change_for_doc(
        ShellChangeRequest::SourceWrite { file_kind: KIND_ALIAS.into(), content: "alias a='echo'\n".into() },
        Doc::from_raw("alias a='echo'\n"),
    )
    .unwrap_err();
    assert!(err.contains("没有变化"));
}

#[test]
fn restore_rejects_invalid_backup_id() {
    let doc = Doc::from_raw("x\n");
    let err = plan::build_change_for_doc(
        ShellChangeRequest::RestoreBackup { backup_id: "../.ssh/id_rsa".into() },
        doc,
    )
    .unwrap_err();
    assert!(err.contains("非法"));
}

// ---------------------------------------------------------------------------
// plan：提交链路（临时目录，含备份保留数量验证）
// ---------------------------------------------------------------------------

fn temp_file(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("xlt-plan-test-{tag}-{}", std::process::id()));
    let _ = fs::create_dir_all(&dir);
    let file = dir.join(".alias");
    fs::write(&file, "alias a='ls'\n").unwrap();
    file
}

fn make_plan(target: std::path::PathBuf, base: &str, postimage: &str) -> StoredPlan {
    StoredPlan {
        plan: ShellChangePlan {
            id: uuid::Uuid::new_v4().to_string(),
            kind: KIND_ALIAS.to_string(),
            base_version: content_version(base),
            ops: vec![PlanOp { action: "测试".into(), detail: "测试".into() }],
            session_command: "source ~/.alias".into(),
            postimage: postimage.to_string(),
            created_at: String::new(),
        },
        target_path: target,
        postimage: postimage.to_string(),
        created: Instant::now(),
    }
}

/// 提交链路综合验证：串行执行以避免共享计划存储的并行竞态。
#[test]
fn commit_pipeline_lifecycle() {
    plan::clear_plans_for_test();

    // --- 单次消费与结果回读 ---
    let target = temp_file("roundtrip");
    let base = fs::read_to_string(&target).unwrap();
    let postimage = "alias a='git'\nalias b='yarn'\n";
    let stored = make_plan(target.clone(), &base, postimage);
    let plan_id = stored.plan.id.clone();
    plan::put_plan_for_test(stored);
    let err = plan::commit("nonexistent", "bad").unwrap_err();
    assert!(err.contains("计划不存在"));
    let result = plan::commit(&plan_id, &content_version(&base)).unwrap();
    assert_eq!(result.version, content_version(postimage));
    assert_eq!(fs::read_to_string(&target).unwrap(), postimage);
    assert_eq!(result.snapshot.aliases.len(), 2);
    // 单次消费：第二次提交失败，磁盘内容保持第一次提交结果
    let err = plan::commit(&plan_id, &content_version(&base)).unwrap_err();
    assert!(err.contains("计划不存在"));
    assert_eq!(fs::read_to_string(&target).unwrap(), postimage);

    // --- 外部修改拒绝 ---
    let target = temp_file("external");
    let base = fs::read_to_string(&target).unwrap();
    let stored = make_plan(target.clone(), &base, "alias a='x'\n");
    let plan_id = stored.plan.id.clone();
    plan::put_plan_for_test(stored);
    fs::write(&target, "alias a='changed-by-external'\n").unwrap();
    let err = plan::commit(&plan_id, &content_version(&base)).unwrap_err();
    assert!(err.contains("外部修改"));
    assert_eq!(fs::read_to_string(&target).unwrap(), "alias a='changed-by-external'\n");

    // --- 备份保留数量：7 次连续提交后保留 5 份 ---
    let target = temp_file("backups");
    let mut current = fs::read_to_string(&target).unwrap();
    for i in 0..7 {
        let postimage = format!("alias a='v{i}'\n");
        let stored = make_plan(target.clone(), &current, &postimage);
        let plan_id = stored.plan.id.clone();
        plan::put_plan_for_test(stored);
        plan::commit(&plan_id, &content_version(&current)).unwrap();
        current = postimage;
    }
    let backups: Vec<_> = fs::read_dir(target.parent().unwrap())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(".bak-"))
        .collect();
    assert_eq!(backups.len(), 5, "应保留最近 5 份备份");
    assert_eq!(fs::read_to_string(&target).unwrap(), "alias a='v6'\n");

    // --- 计划容量上限 16 ---
    let target = temp_file("cap");
    let base = fs::read_to_string(&target).unwrap();
    for i in 0..18 {
        plan::put_plan_for_test(make_plan(target.clone(), &base, &format!("alias a='v{i}'\n")));
    }
    let stored = make_plan(target.clone(), &base, "alias a='final'\n");
    let plan_id = stored.plan.id.clone();
    plan::put_plan_for_test(stored);
    let result = plan::commit(&plan_id, &content_version(&base)).unwrap();
    assert_eq!(result.content, "alias a='final'\n");

    // 清理临时目录
    for tag in ["roundtrip", "external", "backups", "cap"] {
        let dir = std::env::temp_dir().join(format!("xlt-plan-test-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(dir);
    }
    plan::clear_plans_for_test();
}

#[test]
fn read_raw_handles_missing_file() {
    let missing = std::env::temp_dir().join(format!("xlt-missing-{}", uuid::Uuid::new_v4()));
    assert!(read_raw(&missing).unwrap().is_none());
}
