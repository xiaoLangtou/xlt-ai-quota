// Prevents additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clipboard_history;
mod connectors;
mod daily_report;
mod database;
mod mcp;
mod skills;
mod snippets;
mod vault;

use tauri::Manager;

#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

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
        .manage(skills::SkillsState::new())
        .manage(mcp::McpState::new())
        .setup(|app| {
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
            skills::commands::skills_list_roots,
            skills::commands::skills_add_root,
            skills::commands::skills_remove_root,
            skills::commands::skills_update_root_label,
            skills::commands::skills_init_root,
            skills::commands::skills_list_projects,
            skills::commands::skills_add_project,
            skills::commands::skills_remove_project,
            skills::commands::skills_list_project_agent_dirs,
            skills::commands::skills_scan_dir,
            skills::commands::skills_scan,
            skills::commands::skills_list_issues,
            skills::commands::skills_get_detail,
            skills::commands::skills_read_file,
            skills::commands::skills_save,
            skills::commands::skills_create,
            skills::commands::skills_rename,
            skills::commands::skills_delete,
            skills::commands::skills_list_trash,
            skills::commands::skills_restore_trash,
            skills::commands::skills_purge_trash,
            skills::commands::skills_prepare_local_source,
            skills::commands::skills_prepare_upload_source,
            skills::commands::skills_prepare_remote_source,
            skills::commands::skills_remove_staging,
            skills::commands::skills_create_install_plan,
            skills::commands::skills_get_install_plan,
            skills::commands::skills_cancel_install_plan,
            skills::commands::skills_commit_install_plan,
            skills::commands::skills_get_catalog,
            skills::commands::skills_list_sources,
            skills::commands::skills_add_source,
            skills::commands::skills_remove_source,
            skills::commands::skills_sync_source,
            skills::commands::skills_search_skillssh,
            skills::commands::skills_remote_detail,
            skills::commands::skills_remote_file,
            skills::commands::skills_list_installs,
            skills::commands::skills_check_install_update,
            skills::commands::skills_preview_update,
            skills::commands::skills_apply_update,
            skills::commands::skills_rollback_install,
            vault::vault_read,
            vault::vault_write,
            mcp::commands::mcp_scan,
            mcp::commands::mcp_agents,
            mcp::commands::mcp_plan,
            mcp::commands::mcp_apply,
            mcp::commands::mcp_probe,
            mcp::commands::mcp_restore_backup,
            mcp::commands::mcp_set_meta,
            mcp::commands::mcp_parse_paste,
            mcp::commands::mcp_catalog_sources,
            mcp::commands::mcp_catalog_sync,
            mcp::commands::mcp_catalog_list,
            mcp::commands::mcp_catalog_detail,
            mcp::commands::mcp_catalog_readme,
            mcp::commands::mcp_catalog_source_save,
            mcp::commands::mcp_catalog_source_remove,
            mcp::commands::mcp_catalog_favorite,
            quit_app,
            show_dashboard,
            hide_dashboard
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
