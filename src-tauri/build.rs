fn main() {
    // 前后端共用编译期开关，停用时不编译模块、状态及 IPC 命令。
    println!("cargo:rerun-if-changed=../src/config/features.json");
    println!("cargo:rustc-check-cfg=cfg(skills_mcp)");
    let features: serde_json::Value =
        serde_json::from_str(include_str!("../src/config/features.json"))
            .expect("功能开关配置必须是有效 JSON");
    let enabled = features["skillsMcp"]
        .as_bool()
        .expect("skillsMcp 必须显式配置为布尔值");
    if enabled {
        println!("cargo:rustc-cfg=skills_mcp");
    }
    tauri_build::build()
}
