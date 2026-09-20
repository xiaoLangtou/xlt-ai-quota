// Prevents additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clipboard_history;
mod connectors;
mod daily_report;
mod database;
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
            vault::vault_read,
            vault::vault_write,
            quit_app,
            show_dashboard,
            hide_dashboard
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
