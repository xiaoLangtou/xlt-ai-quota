use super::super::types::{CatalogSource, McpPackage};

pub const ID: &str = "builtin";

const RAW: &str = include_str!("builtin.json");

/// 随应用打包的精选服务，离线可用。
pub fn packages() -> Vec<McpPackage> {
    serde_json::from_str::<Vec<McpPackage>>(RAW).unwrap_or_default()
}

pub fn source() -> CatalogSource {
    CatalogSource {
        id: ID.to_owned(),
        label: "内置精选".to_owned(),
        kind: "builtin".to_owned(),
        url: None,
        channel_id: None,
        sub_channel_id: None,
        enabled: true,
        builtin: true,
        last_sync_at: None,
        last_error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::types::InstallInfo;

    #[test]
    fn builtin_catalog_parses_and_is_ready() {
        let packages = packages();
        assert!(packages.len() >= 20, "内置精选应至少 20 条");
        assert!(
            packages.iter().all(|package| !package.id.is_empty() && !package.name.is_empty()),
            "每条内置条目都应有 id 与名称"
        );
        assert!(
            packages
                .iter()
                .any(|package| matches!(package.install, InstallInfo::Ready { .. })),
            "应包含可直接安装的条目"
        );
    }
}

