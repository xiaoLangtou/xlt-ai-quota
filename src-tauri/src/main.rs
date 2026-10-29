// Prevents additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(not(feature = "diagnostic"))]
mod ai_provider;
#[cfg(not(feature = "diagnostic"))]
mod clipboard_history;
#[cfg(not(feature = "diagnostic"))]
mod connectors;
#[cfg(not(feature = "diagnostic"))]
mod daily_report;
#[cfg(not(feature = "diagnostic"))]
mod database;
#[cfg(feature = "diagnostic")]
mod diagnostic;
#[cfg(skills_mcp)]
#[cfg(not(feature = "diagnostic"))]
mod mcp;
#[cfg(not(feature = "diagnostic"))]
mod productivity;
#[cfg(skills_mcp)]
#[cfg(not(feature = "diagnostic"))]
mod skills;
#[cfg(not(feature = "diagnostic"))]
mod shellenv;
#[cfg(not(feature = "diagnostic"))]
mod snippets;
#[cfg(not(feature = "diagnostic"))]
mod translate;
#[cfg(not(feature = "diagnostic"))]
mod usage;
#[cfg(not(feature = "diagnostic"))]
mod vault;

#[cfg(not(feature = "diagnostic"))]
use tauri::Manager;

#[cfg(not(feature = "diagnostic"))]
#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

#[cfg(not(feature = "diagnostic"))]
#[tauri::command]
fn show_dashboard(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Regular)
        .map_err(|error| error.to_string())?;

    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "未找到主窗口".to_string())?;
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())
}

#[cfg(not(feature = "diagnostic"))]
#[tauri::command]
fn hide_dashboard(app: tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "未找到主窗口".to_string())?;
    window.hide().map_err(|error| error.to_string())?;

    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory)
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(feature = "diagnostic")]
fn main() {
    diagnostic::run();
}

#[cfg(not(feature = "diagnostic"))]
#[tauri::command]
fn open_external(url: String) -> Result<(), String> {
    let url = url.trim();
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("仅允许打开 http/https 链接".to_owned());
    }
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "linux")]
    let result = std::process::Command::new("xdg-open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("cmd")
        .args(["/c", "start", "", url])
        .spawn();
    result
        .map(|_| ())
        .map_err(|error| format!("无法打开链接: {error}"))
}

#[cfg(not(feature = "diagnostic"))]
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        let owns = translate::owns_shortcut(app, shortcut);
                        // 快捷键排查：确认按键是否真的到达应用，以及被分流到哪个面板。
                        eprintln!("[global-shortcut] {shortcut:?} translate={owns}");
                        // 剪贴板面板与翻译面板共用一套全局注册，按按键归属分流。
                        if owns {
                            if let Err(error) = translate::toggle_panel(app) {
                                eprintln!("翻译面板显示失败: {error}");
                            }
                        } else if let Err(error) = clipboard_history::toggle_quick_panel(app) {
                            eprintln!("剪贴板面板显示失败: {error}");
                        }
                    }
                })
                .build(),
        )
        .setup(|app| {
            // shellenv 命令无条件注册，其 State 不能放进 skills_mcp 门控块
            app.manage(shellenv::commands::ShellEnvLock::default());
            app.manage(translate::ShortcutState::default());
            app.manage(translate::execution::TranslationRequests::default());
            #[cfg(skills_mcp)]
            {
                app.manage(skills::SkillsState::new());
                app.manage(mcp::McpState::new());
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
                    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                    loop {
                        interval.tick().await;
                        let skills_state = handle.state::<skills::SkillsState>();
                        if let Ok(mut plans) = skills_state.plans.lock() {
                            skills::install::planner::prune_expired_plans(&mut plans);
                        }
                        let mcp_state = handle.state::<mcp::McpState>();
                        if let Ok(mut plans) = mcp_state.plans.lock() {
                            mcp::plan::prune_expired_plans(&mut plans);
                        };
                    }
                });
            }
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let hidden_start = std::env::args().any(|argument| argument == "--hidden");
            if !hidden_start {
                show_dashboard(app.handle().clone()).map_err(std::io::Error::other)?;
            }
            let shortcut = clipboard_history::configured_shortcut(app.handle())
                .map_err(std::io::Error::other)?;
            clipboard_history::register_shortcut(app.handle(), &shortcut)
                .map_err(std::io::Error::other)?;
            // 剪贴板面板注册时会先 unregister_all，翻译面板必须排在其后注册。
            let translate_shortcut = translate::load_config(app.handle()).shortcut;
            if let Err(error) = translate::register_shortcut(app.handle(), &translate_shortcut) {
                eprintln!("翻译面板快捷键注册失败: {error}");
            }
            clipboard_history::initialize_launch_at_login(app.handle())
                .map_err(std::io::Error::other)?;
            clipboard_history::start_listener(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            clipboard_history::clipboard_list,
            clipboard_history::clipboard_status,
            clipboard_history::clipboard_request_accessibility,
            clipboard_history::clipboard_set_pinned,
            clipboard_history::clipboard_copy,
            clipboard_history::clipboard_copy_plain,
            clipboard_history::clipboard_paste,
            clipboard_history::clipboard_copy_merged,
            clipboard_history::clipboard_paste_merged,
            clipboard_history::clipboard_write_text,
            clipboard_history::clipboard_sequence_start,
            clipboard_history::clipboard_sequence_status,
            clipboard_history::clipboard_sequence_next,
            clipboard_history::clipboard_sequence_paste_next,
            clipboard_history::clipboard_sequence_cancel,
            clipboard_history::clipboard_delete,
            clipboard_history::clipboard_clear,
            clipboard_history::clipboard_update_settings,
            clipboard_history::clipboard_save_as_snippet,
            clipboard_history::clipboard_image_data_url,
            clipboard_history::clipboard_image_thumbnail_data_url,
            clipboard_history::clipboard_file_preview,
            clipboard_history::clipboard_open_special,
            clipboard_history::clipboard_show_panel,
            connectors::connector_get,
            connectors::connector_login,
            usage::usage_init,
            usage::usage_collect,
            usage::usage_collector_sources,
            usage::usage_load_buckets,
            usage::usage_append_buckets,
            usage::usage_replace_source_range,
            usage::usage_save_quotas,
            usage::usage_list_quotas,
            usage::usage_clear_quotas,
            usage::usage_config,
            usage::usage_set_config,
            usage::usage_cache_stats,
            usage::usage_cleanup,
            usage::usage_clear_cache,
            usage::usage_cursors_get,
            usage::usage_cursors_set,
            usage::usage_cursors_clear,
            usage::usage_pricing_lookup,
            usage::usage_pricing_table,
            usage::usage_pricing_status,
            usage::usage_refresh_pricing,
            usage::usage_import_legacy,
            daily_report::daily_report_validate_project,
            daily_report::daily_report_collect_commits,
            daily_report::daily_report_generate,
            daily_report::daily_report_import_gitreports,
            snippets::snippet_list,
            snippets::snippet_list_tags,
            snippets::snippet_save,
            snippets::snippet_delete,
            snippets::snippet_touch,
            snippets::snippet_copy,
            shellenv::commands::shell_snapshot,
            shellenv::commands::shell_source_map,
            shellenv::commands::shell_path_report,
            shellenv::commands::shell_change_preview,
            shellenv::commands::shell_change_commit,
            shellenv::commands::shell_change_cancel,
            shellenv::commands::shell_config_backup_list,
            shellenv::commands::shell_config_validate,
            shellenv::commands::shell_file_open,
            #[cfg(skills_mcp)]
            skills::commands::skills_list_roots,
            #[cfg(skills_mcp)]
            skills::commands::skills_add_root,
            #[cfg(skills_mcp)]
            skills::commands::skills_remove_root,
            #[cfg(skills_mcp)]
            skills::commands::skills_update_root_label,
            #[cfg(skills_mcp)]
            skills::commands::skills_init_root,
            #[cfg(skills_mcp)]
            skills::commands::skills_list_projects,
            #[cfg(skills_mcp)]
            skills::commands::skills_add_project,
            #[cfg(skills_mcp)]
            skills::commands::skills_remove_project,
            #[cfg(skills_mcp)]
            skills::commands::skills_list_project_agent_dirs,
            #[cfg(skills_mcp)]
            skills::commands::skills_scan_dir,
            #[cfg(skills_mcp)]
            skills::commands::skills_scan,
            #[cfg(skills_mcp)]
            skills::commands::skills_list_issues,
            #[cfg(skills_mcp)]
            skills::commands::skills_get_detail,
            #[cfg(skills_mcp)]
            skills::commands::skills_read_file,
            #[cfg(skills_mcp)]
            skills::commands::skills_save,
            #[cfg(skills_mcp)]
            skills::commands::skills_create,
            #[cfg(skills_mcp)]
            skills::commands::skills_rename,
            #[cfg(skills_mcp)]
            skills::commands::skills_delete,
            #[cfg(skills_mcp)]
            skills::commands::skills_list_trash,
            #[cfg(skills_mcp)]
            skills::commands::skills_restore_trash,
            #[cfg(skills_mcp)]
            skills::commands::skills_purge_trash,
            #[cfg(skills_mcp)]
            skills::commands::skills_prepare_local_source,
            #[cfg(skills_mcp)]
            skills::commands::skills_prepare_upload_source,
            #[cfg(skills_mcp)]
            skills::commands::skills_prepare_remote_source,
            #[cfg(skills_mcp)]
            skills::commands::skills_remove_staging,
            #[cfg(skills_mcp)]
            skills::commands::skills_create_install_plan,
            #[cfg(skills_mcp)]
            skills::commands::skills_get_install_plan,
            #[cfg(skills_mcp)]
            skills::commands::skills_cancel_install_plan,
            #[cfg(skills_mcp)]
            skills::commands::skills_commit_install_plan,
            #[cfg(skills_mcp)]
            skills::commands::skills_get_catalog,
            #[cfg(skills_mcp)]
            skills::commands::skills_list_sources,
            #[cfg(skills_mcp)]
            skills::commands::skills_add_source,
            #[cfg(skills_mcp)]
            skills::commands::skills_remove_source,
            #[cfg(skills_mcp)]
            skills::commands::skills_sync_source,
            #[cfg(skills_mcp)]
            skills::commands::skills_search_skillssh,
            #[cfg(skills_mcp)]
            skills::commands::skills_remote_detail,
            #[cfg(skills_mcp)]
            skills::commands::skills_remote_file,
            #[cfg(skills_mcp)]
            skills::commands::skills_list_installs,
            #[cfg(skills_mcp)]
            skills::commands::skills_check_install_update,
            #[cfg(skills_mcp)]
            skills::commands::skills_preview_update,
            #[cfg(skills_mcp)]
            skills::commands::skills_apply_update,
            #[cfg(skills_mcp)]
            skills::commands::skills_rollback_install,
            vault::vault_read,
            vault::vault_write,
            translate::translate_config_get,
            translate::translate_config_save,
            translate::translate_set_engine,
            translate::execution::translate_run,
            translate::execution::translate_retry,
            translate::execution::translate_cancel,
            translate::translate_test,
            translate::translate_show_panel,
            translate::translate_hide_panel,
            translate::translate_shortcut_status,
            translate::translate_set_shortcut,
            productivity::commands::calendar_list_range,
            productivity::commands::calendar_save_event,
            productivity::commands::calendar_move_event,
            productivity::commands::calendar_delete_event,
            productivity::commands::todo_list,
            productivity::commands::todo_get,
            productivity::commands::todo_save,
            productivity::commands::todo_set_status,
            productivity::commands::todo_delete,
            productivity::commands::document_read,
            productivity::commands::document_decomposition_generate,
            productivity::commands::document_decomposition_commit,
            productivity::commands::zentao_status,
            productivity::commands::zentao_login,
            productivity::commands::zentao_logout,
            productivity::commands::zentao_sync,
            productivity::commands::zentao_task_detail,
            productivity::commands::zentao_action_context,
            productivity::commands::zentao_action,
            productivity::commands::zentao_comment,
            productivity::commands::zentao_summary_generate,
            productivity::commands::zentao_batch_resolve,
            productivity::commands::zentao_download_file,
            productivity::commands::zentao_read_file,
            productivity::commands::zentao_read_archive,
            productivity::commands::zentao_read_archive_entry,
            #[cfg(skills_mcp)]
            mcp::commands::mcp_scan,
            #[cfg(skills_mcp)]
            mcp::commands::mcp_agents,
            #[cfg(skills_mcp)]
            mcp::commands::mcp_plan,
            #[cfg(skills_mcp)]
            mcp::commands::mcp_apply,
            #[cfg(skills_mcp)]
            mcp::commands::mcp_probe,
            #[cfg(skills_mcp)]
            mcp::commands::mcp_restore_backup,
            #[cfg(skills_mcp)]
            mcp::commands::mcp_set_meta,
            #[cfg(skills_mcp)]
            mcp::commands::mcp_parse_paste,
            #[cfg(skills_mcp)]
            mcp::commands::mcp_catalog_sources,
            #[cfg(skills_mcp)]
            mcp::commands::mcp_catalog_sync,
            #[cfg(skills_mcp)]
            mcp::commands::mcp_catalog_list,
            #[cfg(skills_mcp)]
            mcp::commands::mcp_catalog_detail,
            #[cfg(skills_mcp)]
            mcp::commands::mcp_catalog_readme,
            #[cfg(skills_mcp)]
            mcp::commands::mcp_catalog_source_save,
            #[cfg(skills_mcp)]
            mcp::commands::mcp_catalog_source_remove,
            #[cfg(skills_mcp)]
            mcp::commands::mcp_catalog_favorite,
            quit_app,
            open_external,
            show_dashboard,
            hide_dashboard
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod feature_tests {
    #[test]
    fn native_gate_matches_shared_config() {
        let features: serde_json::Value =
            serde_json::from_str(include_str!("../../src/config/features.json")).unwrap();
        assert_eq!(Some(cfg!(skills_mcp)), features["skillsMcp"].as_bool());
    }
}
