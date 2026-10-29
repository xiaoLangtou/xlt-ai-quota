//! 会话级验证（PRD 8.1「会话」检查项）：在隔离的 `zsh -f`（不加载任何 rc）子进程
//! 中加载临时配置文件，验证序列化结果的运行时语义。
//!
//! - 字面量语义：`$`、反引号、单引号、括号在单引号序列化后不被展开、不改变含义；
//! - alias 命令文本语义：`alias x` 回读文本一致，调用时机行为正确；
//! - 删除清理：删除条目后的后像 + `unalias` 能准确清理，不影响其他条目。
//!
//! 全部使用临时目录与合成文件，不触碰用户真实配置；zsh 缺失时跳过。

use crate::shellenv::files::{apply_edits, LineEdit};
use crate::shellenv::serializer::{format_alias, format_export, shell_single_quoted};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use uuid::Uuid;

fn zsh_available() -> bool {
    Command::new("zsh")
        .arg("-f")
        .arg("-c")
        .arg("true")
        .output()
        .is_ok()
}

fn temp_config(content: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("xlt-session-{}.zsh", Uuid::new_v4()));
    fs::write(&path, content).expect("写入临时配置");
    path
}

/// `zsh -f -c <script>`：-f 跳过全局与用户 rc，保证隔离。
fn run_zsh(script: &str) -> (bool, String) {
    let output = Command::new("zsh")
        .arg("-f")
        .arg("-c")
        .arg(script)
        .output()
        .expect("执行 zsh 失败");
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    if text.is_empty() {
        text = String::from_utf8_lossy(&output.stderr).to_string();
    }
    (output.status.success(), text)
}

fn source_script(path: &PathBuf, body: &str) -> String {
    format!("source {}; {}", path.display(), body)
}

#[test]
fn session_env_literal_is_not_expanded() {
    if !zsh_available() {
        eprintln!("跳过：zsh 不可用");
        return;
    }
    // 含 $、反引号、命令替换语感的值：字面量序列化后必须原样回读
    let tricky = "$HOME/sub && echo pwned; `id -u`";
    let line = format_export("XLT_TEST_K", tricky);
    assert_eq!(line, "export XLT_TEST_K='$HOME/sub && echo pwned; `id -u`'");
    let file = temp_config(&format!("{line}\n"));
    // 先 unset 再 source，避免继承宿主环境里可能的同名变量
    let (ok, out) = run_zsh(&format!(
        "unset XLT_TEST_K; {}",
        source_script(&file, "print -r -- \"$XLT_TEST_K\"")
    ));
    let _ = fs::remove_file(&file);
    assert!(ok, "zsh 执行失败: {out}");
    assert_eq!(out.trim(), tricky, "值必须按字面量回读，不得展开");
}

#[test]
fn session_env_value_with_single_quote_roundtrips() {
    if !zsh_available() {
        eprintln!("跳过：zsh 不可用");
        return;
    }
    let value = "it's a \"test\"";
    let file = temp_config(&format!("{}\n", format_export("XLT_TEST_Q", value)));
    let (ok, out) = run_zsh(&format!(
        "unset XLT_TEST_Q; {}",
        source_script(&file, "print -r -- \"$XLT_TEST_Q\"")
    ));
    let _ = fs::remove_file(&file);
    assert!(ok, "含单引号值的序列化 zsh 解析失败: {out}");
    assert_eq!(out.trim(), value);
}

#[test]
fn session_alias_command_text_roundtrips() {
    if !zsh_available() {
        eprintln!("跳过：zsh 不可用");
        return;
    }
    // 命令文本语义：alias 回读文本一致；$ 在命令文本中保留（调用时才展开）
    let file = temp_config(&format!("{}\n", format_alias("xlt_gcm", "git commit -m $FLAG")));
    let (ok, out) = run_zsh(&source_script(&file, "alias xlt_gcm"));
    let _ = fs::remove_file(&file);
    assert!(ok, "alias 回读失败: {out}");
    assert!(
        out.contains("xlt_gcm='git commit -m $FLAG'"),
        "alias 文本应原样保留: {out}"
    );
}

#[test]
fn session_alias_value_with_quotes_roundtrips() {
    if !zsh_available() {
        eprintln!("跳过：zsh 不可用");
        return;
    }
    let value = "say 'hi' && echo done";
    let file = temp_config(&format!("{}\n", format_alias("xlt_say", value)));
    let (ok, out) = run_zsh(&source_script(&file, "whence -v xlt_say"));
    let _ = fs::remove_file(&file);
    assert!(ok, "含引号 alias 解析失败: {out}");
    assert!(out.contains(value), "alias 命令文本应原样回读: {out}");
}

#[test]
fn session_delete_cleanup_via_unalias_and_postimage() {
    if !zsh_available() {
        eprintln!("跳过：zsh 不可用");
        return;
    }
    // 原文件两条别名；删除 gcm 的后像只剩 say
    let original = format!(
        "{}\n{}\n",
        format_alias("xlt_gcm", "git commit -m"),
        format_alias("xlt_say", "echo hi")
    );
    let postimage = apply_edits(
        &original,
        &[LineEdit::Delete { index: 0 }],
    )
    .unwrap();
    assert!(!postimage.contains("xlt_gcm"));
    assert!(postimage.contains("xlt_say"));

    let original_file = temp_config(&original);
    let postimage_file = temp_config(&postimage);
    // 已开终端的清理路径：unalias 掉被删除的定义，再 source 后像
    let (ok, out) = run_zsh(&format!(
        "{}; unalias xlt_gcm; {}; {}",
        source_script(&original_file, "true"),
        source_script(&postimage_file, "true"),
        "if alias xlt_gcm 2>/dev/null; then echo STILL_THERE; else echo CLEANED; fi; alias xlt_say >/dev/null && echo KEEP_OK"
    ));
    let _ = fs::remove_file(&original_file);
    let _ = fs::remove_file(&postimage_file);
    assert!(ok, "清理会话执行失败: {out}");
    assert!(out.contains("CLEANED"), "删除后 unalias + source 必须清理旧定义: {out}");
    assert!(!out.contains("STILL_THERE"));
    assert!(out.contains("KEEP_OK"), "删除不得影响其他别名: {out}");
}

#[test]
fn session_env_delete_cleanup_via_unset() {
    if !zsh_available() {
        eprintln!("跳过：zsh 不可用");
        return;
    }
    let original = format!(
        "{}\n{}\n",
        format_export("XLT_TEST_DEL", "old"),
        format_export("XLT_TEST_KEEP", "keep")
    );
    let postimage = apply_edits(&original, &[LineEdit::Delete { index: 0 }]).unwrap();
    let original_file = temp_config(&original);
    let postimage_file = temp_config(&postimage);
    let (ok, out) = run_zsh(&format!(
        "{}; unset XLT_TEST_DEL; {}; {}",
        source_script(&original_file, "true"),
        source_script(&postimage_file, "true"),
        "print -r -- \"${XLT_TEST_DEL-REMOVED}|${XLT_TEST_KEEP}\""
    ));
    let _ = fs::remove_file(&original_file);
    let _ = fs::remove_file(&postimage_file);
    assert!(ok, "清理会话执行失败: {out}");
    assert_eq!(out.trim(), "REMOVED|keep");
}

#[test]
fn session_generated_block_sources_cleanly() {
    if !zsh_available() {
        eprintln!("跳过：zsh 不可用");
        return;
    }
    // 首次创建托管区块的完整后像：标记 + 条目 + 结束标记，source 后变量可用
    use crate::shellenv::serializer::{MANAGED_BEGIN, MANAGED_END};
    let content = format!(
        "export BEFORE=1\n{MANAGED_BEGIN}\n{}\n{MANAGED_END}\n",
        format_export("XLT_TEST_BLOCK", "from block")
    );
    let file = temp_config(&content);
    let (ok, out) = run_zsh(&format!(
        "unset XLT_TEST_BLOCK; {}",
        source_script(&file, "print -r -- \"$XLT_TEST_BLOCK\"")
    ));
    let _ = fs::remove_file(&file);
    assert!(ok, "托管区块后像 source 失败: {out}");
    assert_eq!(out.trim(), "from block");
}
