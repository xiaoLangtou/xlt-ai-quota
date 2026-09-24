use tauri::utils::config::{Config, WebviewUrl};

/// 必须在创建窗口前校验，避免只开 Cargo feature 却沿用正式版数据与窗口配置。
fn validate_config(config: &Config) -> Result<(), &'static str> {
    if config.identifier != "com.xlt.workbench.diagnostic" {
        return Err("诊断版必须使用独立的应用标识");
    }
    if config.app.windows.len() != 1 {
        return Err("诊断版只能创建一个普通窗口");
    }
    let window = &config.app.windows[0];
    if window.label != "main"
        || window.url != WebviewUrl::App("diagnostic.html".into())
        || !window.visible
        || !window.decorations
        || window.transparent
        || window.always_on_top
        || window.window_effects.is_some()
        || config.app.macos_private_api
    {
        return Err("诊断版窗口配置不满足隔离要求");
    }
    Ok(())
}

#[tauri::command]
fn quit_diagnostic(app: tauri::AppHandle) {
    app.exit(0);
}

pub fn run() {
    let context = tauri::generate_context!();
    validate_config(context.config()).expect("请通过 pnpm tauri:build:diagnostic 构建诊断包");

    // 不注册业务插件或状态，不扫描数据，不切换 Accessory，不拦截关闭事件。
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![quit_diagnostic])
        .run(context)
        .expect("诊断窗口启动失败");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        serde_json::from_str(include_str!("../tauri.diagnostic.conf.json")).unwrap()
    }

    #[test]
    fn accepts_isolated_diagnostic_config() {
        assert!(validate_config(&config()).is_ok());
    }

    #[test]
    fn rejects_original_identifier_before_creating_windows() {
        let mut config = config();
        config.identifier = "com.xlt.workbench".into();
        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn rejects_hidden_panels_and_business_entry() {
        let mut config = config();
        config.app.windows.push(config.app.windows[0].clone());
        assert!(validate_config(&config).is_err());
        config.app.windows.pop();
        config.app.windows[0].url = WebviewUrl::App("index.html".into());
        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn rejects_overlay_and_hidden_window() {
        let mut config = config();
        config.app.windows[0].always_on_top = true;
        assert!(validate_config(&config).is_err());
        config.app.windows[0].always_on_top = false;
        config.app.windows[0].transparent = true;
        assert!(validate_config(&config).is_err());
        config.app.windows[0].transparent = false;
        config.app.windows[0].visible = false;
        assert!(validate_config(&config).is_err());
    }
}
