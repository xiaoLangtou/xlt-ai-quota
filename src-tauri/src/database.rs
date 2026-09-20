use rusqlite::Connection;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const DATABASE_FILE: &str = "xlt-workbench.sqlite3";

pub fn database_path(app: &AppHandle) -> Result<PathBuf, String> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("无法定位本机数据目录: {error}"))?;
    fs::create_dir_all(&directory).map_err(|error| format!("无法创建本机数据目录: {error}"))?;
    Ok(directory.join(DATABASE_FILE))
}

pub fn open(app: &AppHandle) -> Result<Connection, String> {
    let path = database_path(app)?;
    let connection =
        Connection::open(path).map_err(|error| format!("无法打开本机数据库: {error}"))?;
    initialize(&connection)?;
    Ok(connection)
}

pub fn initialize(connection: &Connection) -> Result<(), String> {
    connection
        .execute_batch(
            "
            PRAGMA foreign_keys = ON;
            PRAGMA journal_mode = WAL;

            CREATE TABLE IF NOT EXISTS snippets (
              id TEXT PRIMARY KEY NOT NULL,
              title TEXT NOT NULL,
              kind TEXT NOT NULL CHECK(kind IN ('code', 'command', 'prompt', 'text', 'link')),
              content TEXT NOT NULL,
              language TEXT,
              description TEXT NOT NULL DEFAULT '',
              pinned INTEGER NOT NULL DEFAULT 0 CHECK(pinned IN (0, 1)),
              use_count INTEGER NOT NULL DEFAULT 0,
              last_used_at TEXT,
              created_at TEXT NOT NULL,
              updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS tags (
              id TEXT PRIMARY KEY NOT NULL,
              name TEXT NOT NULL,
              normalized_name TEXT NOT NULL UNIQUE
            );

            CREATE TABLE IF NOT EXISTS snippet_tags (
              snippet_id TEXT NOT NULL REFERENCES snippets(id) ON DELETE CASCADE,
              tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
              PRIMARY KEY (snippet_id, tag_id)
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS snippets_fts USING fts5(
              id UNINDEXED,
              title,
              content,
              description,
              language,
              tags
            );

            CREATE INDEX IF NOT EXISTS idx_snippets_recent ON snippets(pinned DESC, last_used_at DESC, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_snippets_kind ON snippets(kind);
            CREATE INDEX IF NOT EXISTS idx_snippet_tags_tag ON snippet_tags(tag_id, snippet_id);

            CREATE TABLE IF NOT EXISTS clipboard_items (
              id TEXT PRIMARY KEY NOT NULL,
              kind TEXT NOT NULL CHECK(kind IN ('text', 'image', 'files')),
              content TEXT NOT NULL,
              hash TEXT NOT NULL UNIQUE,
              source_app TEXT,
              pinned INTEGER NOT NULL DEFAULT 0 CHECK(pinned IN (0, 1)),
              copy_count INTEGER NOT NULL DEFAULT 1,
              size_bytes INTEGER NOT NULL DEFAULT 0,
              image_width INTEGER,
              image_height INTEGER,
              created_at TEXT NOT NULL,
              updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS clipboard_config (
              key TEXT PRIMARY KEY NOT NULL,
              value TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_clipboard_recent
              ON clipboard_items(pinned DESC, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_clipboard_kind
              ON clipboard_items(kind, updated_at DESC);

            INSERT OR IGNORE INTO clipboard_config (key, value) VALUES
              ('enabled', 'true'),
              ('max_items', '100'),
              ('ttl_days', '3'),
              ('shortcut', 'CommandOrControl+Shift+V'),
              ('launch_at_login', 'false'),
              ('excluded_apps', '[\"1Password\",\"Bitwarden\",\"KeePass\",\"KeePassXC\"]');
            ",
        )
        .map_err(|error| format!("无法初始化本机数据库: {error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::initialize;
    use rusqlite::{params, Connection};

    #[test]
    fn initializes_snippet_tables_and_fts_index() {
        let connection = Connection::open_in_memory().expect("创建内存数据库");
        initialize(&connection).expect("初始化数据库");
        connection
      .execute(
        "INSERT INTO snippets (id, title, kind, content, language, description, pinned, use_count, last_used_at, created_at, updated_at)
         VALUES ('snippet-1', 'Typecheck', 'command', 'pnpm typecheck', 'bash', '', 0, 0, NULL, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
      )
      .expect("写入片段");
        connection
            .execute(
                "INSERT INTO snippets_fts (id, title, content, description, language, tags)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    "snippet-1",
                    "Typecheck",
                    "pnpm typecheck",
                    "",
                    "bash",
                    "quality"
                ],
            )
            .expect("写入搜索索引");
        let id = connection
            .query_row(
                "SELECT id FROM snippets_fts WHERE snippets_fts MATCH ?1",
                params!["\"typecheck\""],
                |row| row.get::<_, String>(0),
            )
            .expect("读取 FTS 搜索结果");
        assert_eq!(id, "snippet-1");

        let config_count = connection
            .query_row("SELECT COUNT(*) FROM clipboard_config", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("读取剪贴板配置");
        assert_eq!(config_count, 6);
    }
}
