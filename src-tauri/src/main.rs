// Prevents additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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
#[cfg(skills_mcp)]
#[cfg(not(feature = "diagnostic"))]
mod skills;
#[cfg(not(feature = "diagnostic"))]
mod snippets;
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
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        let _ = clipboard_history::toggle_quick_panel(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            #[cfg(skills_mcp)]
            {
                app.manage(skills::SkillsState::new());
                app.manage(mcp::McpState::new());
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
