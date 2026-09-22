use std::collections::{HashMap, HashSet};

use rusqlite::params;
use tauri::AppHandle;

use crate::database;

use super::error::McpError;
use super::types::{CatalogSource, McpPackage, McpServer};

/// 本地元数据（备注 / 标签 / 来源）。
#[derive(Debug, Clone, Default)]
pub struct McpMeta {
    pub note: String,
    pub tags: Vec<String>,
    pub source_id: Option<String>,
}

/// 暂存区条目（不支持原生停用的 Agent 的服务）。
#[derive(Debug, Clone)]
pub struct StagedEntry {
    pub id: i64,
    pub agent: String,
    pub scope: String,
    pub project_path: String,
    pub server: McpServer,
    pub created_at: String,
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn db_error(error: rusqlite::Error) -> McpError {
    McpError::internal(format!("本机数据库操作失败：{error}"))
}

pub fn load_meta(app: &AppHandle) -> Result<HashMap<String, McpMeta>, McpError> {
    let connection = database::open(app).map_err(McpError::internal)?;
    let mut statement = connection
        .prepare("SELECT key, note, tags, source_id FROM mcp_meta")
        .map_err(db_error)?;
    let rows = statement
        .query_map([], |row| {
            let key: String = row.get(0)?;
            let note: String = row.get(1)?;
            let tags: String = row.get(2)?;
            let source_id: Option<String> = row.get(3)?;
            Ok((key, note, tags, source_id))
        })
        .map_err(db_error)?;
    let mut out = HashMap::new();
    for row in rows {
        let (key, note, tags, source_id) = row.map_err(db_error)?;
        out.insert(
            key,
            McpMeta {
                note,
                tags: serde_json::from_str(&tags).unwrap_or_default(),
                source_id,
            },
        );
    }
    Ok(out)
}

pub fn save_meta(
    app: &AppHandle,
    key: &str,
    note: &str,
    tags: &[String],
    source_id: Option<&str>,
) -> Result<(), McpError> {
    let connection = database::open(app).map_err(McpError::internal)?;
    let tags_json = serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_owned());
    connection
        .execute(
            "INSERT INTO mcp_meta (key, note, tags, source_id, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(key) DO UPDATE SET
               note = excluded.note,
               tags = excluded.tags,
               source_id = COALESCE(excluded.source_id, mcp_meta.source_id),
               updated_at = excluded.updated_at",
            params![key, note, tags_json, source_id, now()],
        )
        .map_err(db_error)?;
    Ok(())
}

// ---------- 暂存区 ----------

pub fn list_staged(app: &AppHandle) -> Result<Vec<StagedEntry>, McpError> {
    let connection = database::open(app).map_err(McpError::internal)?;
    let mut statement = connection
        .prepare("SELECT id, agent, scope, project_path, name, payload, created_at FROM mcp_staged")
        .map_err(db_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        })
        .map_err(db_error)?;
    let mut out = Vec::new();
    for row in rows {
        let (id, agent, scope, project_path, name, payload, created_at) = row.map_err(db_error)?;
        let mut server: McpServer = match serde_json::from_str(&payload) {
            Ok(server) => server,
            Err(_) => continue,
        };
        if server.name.is_empty() {
            server.name = name;
        }
        out.push(StagedEntry {
            id,
            agent,
            scope,
            project_path,
            server,
            created_at,
        });
    }
    Ok(out)
}

pub fn stage(
    app: &AppHandle,
    agent: &str,
    scope: &str,
    project_path: &str,
    server: &McpServer,
) -> Result<(), McpError> {
    let connection = database::open(app).map_err(McpError::internal)?;
    let payload = serde_json::to_string(server).map_err(McpError::from)?;
    connection
        .execute(
            "INSERT INTO mcp_staged (agent, scope, project_path, name, payload, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(agent, scope, project_path, name) DO UPDATE SET payload = excluded.payload",
            params![agent, scope, project_path, server.name, payload, now()],
        )
        .map_err(db_error)?;
    Ok(())
}

pub fn unstage(
    app: &AppHandle,
    agent: &str,
    scope: &str,
    project_path: &str,
    name: &str,
) -> Result<Option<McpServer>, McpError> {
    let connection = database::open(app).map_err(McpError::internal)?;
    let payload: Option<String> = connection
        .query_row(
            "SELECT payload FROM mcp_staged WHERE agent = ?1 AND scope = ?2 AND project_path = ?3 AND name = ?4",
            params![agent, scope, project_path, name],
            |row| row.get(0),
        )
        .ok();
    connection
        .execute(
            "DELETE FROM mcp_staged WHERE agent = ?1 AND scope = ?2 AND project_path = ?3 AND name = ?4",
            params![agent, scope, project_path, name],
        )
        .map_err(db_error)?;
    Ok(payload.and_then(|raw| serde_json::from_str(&raw).ok()))
}

// ---------- 库缓存 ----------

pub fn replace_catalog(
    app: &AppHandle,
    source: &str,
    packages: &[McpPackage],
) -> Result<(), McpError> {
    let mut connection = database::open(app).map_err(McpError::internal)?;
    let transaction = connection.transaction().map_err(db_error)?;
    transaction
        .execute("DELETE FROM mcp_catalog WHERE source = ?1", params![source])
        .map_err(db_error)?;
    let stamp = now();
    for package in packages {
        let payload = serde_json::to_string(package).map_err(McpError::from)?;
        transaction
            .execute(
                "INSERT OR REPLACE INTO mcp_catalog (source, id, kind, payload, fetched_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![source, package.id, package.kind, payload, stamp],
            )
            .map_err(db_error)?;
    }
    transaction.commit().map_err(db_error)?;
    Ok(())
}

pub fn catalog_by_source(app: &AppHandle, source: &str) -> Result<Vec<McpPackage>, McpError> {
    let connection = database::open(app).map_err(McpError::internal)?;
    let mut statement = connection
        .prepare("SELECT payload FROM mcp_catalog WHERE source = ?1")
        .map_err(db_error)?;
    let rows = statement
        .query_map(params![source], |row| row.get::<_, String>(0))
        .map_err(db_error)?;
    let mut out = Vec::new();
    for row in rows {
        if let Ok(package) = serde_json::from_str::<McpPackage>(&row.map_err(db_error)?) {
            out.push(package);
        }
    }
    Ok(out)
}

pub fn get_package(app: &AppHandle, id: &str) -> Result<Option<McpPackage>, McpError> {
    let connection = database::open(app).map_err(McpError::internal)?;
    let payload: Option<String> = connection
        .query_row(
            "SELECT payload FROM mcp_catalog WHERE id = ?1 LIMIT 1",
            params![id],
            |row| row.get(0),
        )
        .ok();
    Ok(payload.and_then(|raw| serde_json::from_str(&raw).ok()))
}

pub fn favorite_ids(app: &AppHandle) -> Result<HashSet<String>, McpError> {
    let connection = database::open(app).map_err(McpError::internal)?;
    let mut statement = connection
        .prepare("SELECT package_id FROM mcp_favorite")
        .map_err(db_error)?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(db_error)?;
    let mut out = HashSet::new();
    for row in rows {
        out.insert(row.map_err(db_error)?);
    }
    Ok(out)
}

pub fn set_favorite(app: &AppHandle, id: &str, value: bool) -> Result<(), McpError> {
    let connection = database::open(app).map_err(McpError::internal)?;
    if value {
        connection
            .execute(
                "INSERT OR IGNORE INTO mcp_favorite (package_id, created_at) VALUES (?1, ?2)",
                params![id, now()],
            )
            .map_err(db_error)?;
    } else {
        connection
            .execute(
                "DELETE FROM mcp_favorite WHERE package_id = ?1",
                params![id],
            )
            .map_err(db_error)?;
    }
    Ok(())
}

pub fn load_source_states(app: &AppHandle) -> Result<HashMap<String, (CatalogSource, Option<String>)>, McpError> {
    let connection = database::open(app).map_err(McpError::internal)?;
    let mut statement = connection
        .prepare("SELECT payload, error FROM mcp_catalog_source")
        .map_err(db_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })
        .map_err(db_error)?;
    let mut out = HashMap::new();
    for row in rows {
        let (payload, error) = row.map_err(db_error)?;
        if let Ok(source) = serde_json::from_str::<CatalogSource>(&payload) {
            out.insert(source.id.clone(), (source, error));
        }
    }
    Ok(out)
}

pub fn save_source_state(
    app: &AppHandle,
    source: &CatalogSource,
    error: Option<&str>,
) -> Result<(), McpError> {
    let connection = database::open(app).map_err(McpError::internal)?;
    let payload = serde_json::to_string(source).map_err(McpError::from)?;
    connection
        .execute(
            "INSERT OR REPLACE INTO mcp_catalog_source (id, payload, fetched_at, error)
             VALUES (?1, ?2, ?3, ?4)",
            params![source.id, payload, now(), error],
        )
        .map_err(db_error)?;
    Ok(())
}

pub fn save_custom_source(app: &AppHandle, source: &CatalogSource) -> Result<(), McpError> {
    save_source_state(app, source, None)
}

/// 删除一个自定义源及其缓存条目。
pub fn remove_source(app: &AppHandle, id: &str) -> Result<bool, McpError> {
    let mut connection = database::open(app).map_err(McpError::internal)?;
    let transaction = connection.transaction().map_err(db_error)?;
    let removed = transaction
        .execute("DELETE FROM mcp_catalog_source WHERE id = ?1", params![id])
        .map_err(db_error)?
        > 0;
    transaction
        .execute("DELETE FROM mcp_catalog WHERE source = ?1", params![id])
        .map_err(db_error)?;
    transaction.commit().map_err(db_error)?;
    Ok(removed)
}
