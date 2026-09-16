// Prevents additional console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod connectors;
mod daily_report;
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
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            show_dashboard(app.handle().clone()).map_err(std::io::Error::other)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            connectors::connector_get,
            daily_report::daily_report_validate_project,
            daily_report::daily_report_collect_commits,
            daily_report::daily_report_generate,
            daily_report::daily_report_import_gitreports,
            vault::vault_read,
            vault::vault_write,
            quit_app,
            show_dashboard,
            hide_dashboard
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
