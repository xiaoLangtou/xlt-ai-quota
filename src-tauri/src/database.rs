use rusqlite::{Connection, OptionalExtension};
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

            CREATE TABLE IF NOT EXISTS mcp_meta (
              key TEXT PRIMARY KEY NOT NULL,
              note TEXT NOT NULL DEFAULT '',
              tags TEXT NOT NULL DEFAULT '[]',
              source_id TEXT,
              updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS mcp_staged (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              agent TEXT NOT NULL,
              scope TEXT NOT NULL,
              project_path TEXT NOT NULL DEFAULT '',
              name TEXT NOT NULL,
              payload TEXT NOT NULL,
              created_at TEXT NOT NULL,
              UNIQUE(agent, scope, project_path, name)
            );

            CREATE TABLE IF NOT EXISTS mcp_catalog (
              source TEXT NOT NULL,
              id TEXT NOT NULL,
              kind TEXT NOT NULL DEFAULT 'server',
              payload TEXT NOT NULL,
              fetched_at TEXT NOT NULL,
              PRIMARY KEY (source, id)
            );

            CREATE TABLE IF NOT EXISTS mcp_catalog_source (
              id TEXT PRIMARY KEY NOT NULL,
              payload TEXT NOT NULL,
              fetched_at TEXT NOT NULL,
              error TEXT
            );

            CREATE TABLE IF NOT EXISTS mcp_favorite (
              package_id TEXT PRIMARY KEY NOT NULL,
              created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS todo_task (
              id TEXT PRIMARY KEY NOT NULL,
              title TEXT NOT NULL,
              description TEXT,
              status TEXT NOT NULL CHECK(status IN ('open', 'doing', 'done')),
              priority TEXT NOT NULL CHECK(priority IN ('high', 'medium', 'low')),
              category TEXT NOT NULL CHECK(category IN ('work', 'planning', 'personal')),
              planned_date TEXT,
              planned_time TEXT,
              estimate_minutes INTEGER,
              source_type TEXT NOT NULL CHECK(source_type IN ('manual', 'document', 'zentao')),
              source_name TEXT,
              created_at INTEGER NOT NULL,
              updated_at INTEGER NOT NULL,
              completed_at INTEGER,
              external_id TEXT,
              external_type TEXT,
              external_url TEXT,
              external_due TEXT,
              synced_at INTEGER
            );

            CREATE TABLE IF NOT EXISTS todo_acceptance_criterion (
              id TEXT PRIMARY KEY NOT NULL,
              task_id TEXT NOT NULL REFERENCES todo_task(id) ON DELETE CASCADE,
              content TEXT NOT NULL,
              sort_order INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS todo_dependency (
              task_id TEXT NOT NULL REFERENCES todo_task(id) ON DELETE CASCADE,
              depends_on_task_id TEXT NOT NULL REFERENCES todo_task(id) ON DELETE CASCADE,
              PRIMARY KEY (task_id, depends_on_task_id),
              CHECK (task_id <> depends_on_task_id)
            );

            CREATE TABLE IF NOT EXISTS calendar_event (
              id TEXT PRIMARY KEY NOT NULL,
              task_id TEXT UNIQUE REFERENCES todo_task(id) ON DELETE CASCADE,
              title TEXT NOT NULL,
              category TEXT NOT NULL,
              start_at TEXT NOT NULL,
              end_at TEXT,
              all_day INTEGER NOT NULL DEFAULT 0 CHECK(all_day IN (0, 1)),
              created_at INTEGER NOT NULL,
              updated_at INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_todo_planned ON todo_task(planned_date, planned_time);
            CREATE INDEX IF NOT EXISTS idx_todo_status ON todo_task(status);
            CREATE INDEX IF NOT EXISTS idx_acceptance_task ON todo_acceptance_criterion(task_id, sort_order);
            CREATE INDEX IF NOT EXISTS idx_event_start ON calendar_event(start_at);
            ",
        )
        .map_err(|error| format!("无法初始化本机数据库: {error}"))?;
    migrate_todo_status(connection)?;
    migrate_todo_zentao(connection)?;
    // 保留禅道原始状态与指派关系，不能用本地三态推断远端处理结果。
    let columns = connection.prepare("PRAGMA table_info(todo_task)")
        .map_err(|e| e.to_string())?
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    for (name, definition) in [
        ("external_status", "TEXT"),
        ("external_updated_date", "TEXT"),
        ("external_assigned_to", "TEXT"),
        ("external_transferred", "INTEGER NOT NULL DEFAULT 0"),
    ] {
        if !columns.iter().any(|column| column == name) {
            connection.execute(&format!("ALTER TABLE todo_task ADD COLUMN {name} {definition}"), [])
                .map_err(|e| format!("无法迁移禅道状态字段: {e}"))?;
        }
    }
    // external_id 索引在表迁移完成后统一创建，避免老表缺列时初始化失败。
    connection
        .execute("CREATE INDEX IF NOT EXISTS idx_todo_external ON todo_task(external_id)", [])
        .map_err(|error| format!("无法初始化本机数据库: {error}"))?;
    Ok(())
}

/// 待办状态从 open/done 扩展为 open/doing/done 时，重建带旧 CHECK 约束的表并保留数据。
fn migrate_todo_status(connection: &Connection) -> Result<(), String> {
    let existing: Option<String> = connection
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'todo_task'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("无法读取待办表定义: {error}"))?;
    let Some(sql) = existing else {
        return Ok(());
    };
    if sql.contains("'doing'") || sql.contains("\"doing\"") {
        return Ok(());
    }
    connection
        .execute_batch(
            "
            PRAGMA foreign_keys = OFF;
            BEGIN;
            CREATE TABLE todo_task_migrated (
              id TEXT PRIMARY KEY NOT NULL,
              title TEXT NOT NULL,
              description TEXT,
              status TEXT NOT NULL CHECK(status IN ('open', 'doing', 'done')),
              priority TEXT NOT NULL CHECK(priority IN ('high', 'medium', 'low')),
              category TEXT NOT NULL CHECK(category IN ('work', 'planning', 'personal')),
              planned_date TEXT,
              planned_time TEXT,
              estimate_minutes INTEGER,
              source_type TEXT NOT NULL CHECK(source_type IN ('manual', 'document')),
              source_name TEXT,
              created_at INTEGER NOT NULL,
              updated_at INTEGER NOT NULL,
              completed_at INTEGER
            );
            INSERT INTO todo_task_migrated (
              id, title, description, status, priority, category, planned_date, planned_time,
              estimate_minutes, source_type, source_name, created_at, updated_at, completed_at
            )
            SELECT id, title, description, status, priority, category, planned_date, planned_time,
              estimate_minutes, source_type, source_name, created_at, updated_at, completed_at
            FROM todo_task;
            DROP TABLE todo_task;
            ALTER TABLE todo_task_migrated RENAME TO todo_task;
            CREATE INDEX IF NOT EXISTS idx_todo_planned ON todo_task(planned_date, planned_time);
            CREATE INDEX IF NOT EXISTS idx_todo_status ON todo_task(status);
            COMMIT;
            PRAGMA foreign_keys = ON;
            ",
        )
        .map_err(|error| format!("无法迁移待办状态字段: {error}"))?;
    Ok(())
}

/// 接入禅道同步：todo_task 增加 zentao 来源与外部条目字段（幂等键、跳转链接等），
/// 重建带旧 CHECK 约束的表并保留既有数据。
fn migrate_todo_zentao(connection: &Connection) -> Result<(), String> {
    let existing: Option<String> = connection
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'todo_task'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("无法读取待办表定义: {error}"))?;
    let Some(sql) = existing else {
        return Ok(());
    };
    if sql.contains("external_id") {
        return Ok(());
    }
    connection
        .execute_batch(
            "
            PRAGMA foreign_keys = OFF;
            BEGIN;
            CREATE TABLE todo_task_migrated (
              id TEXT PRIMARY KEY NOT NULL,
              title TEXT NOT NULL,
              description TEXT,
              status TEXT NOT NULL CHECK(status IN ('open', 'doing', 'done')),
              priority TEXT NOT NULL CHECK(priority IN ('high', 'medium', 'low')),
              category TEXT NOT NULL CHECK(category IN ('work', 'planning', 'personal')),
              planned_date TEXT,
              planned_time TEXT,
              estimate_minutes INTEGER,
              source_type TEXT NOT NULL CHECK(source_type IN ('manual', 'document', 'zentao')),
              source_name TEXT,
              created_at INTEGER NOT NULL,
              updated_at INTEGER NOT NULL,
              completed_at INTEGER,
              external_id TEXT,
              external_type TEXT,
              external_url TEXT,
              external_due TEXT,
              synced_at INTEGER
            );
            INSERT INTO todo_task_migrated (
              id, title, description, status, priority, category, planned_date, planned_time,
              estimate_minutes, source_type, source_name, created_at, updated_at, completed_at
            )
            SELECT id, title, description, status, priority, category, planned_date, planned_time,
              estimate_minutes, source_type, source_name, created_at, updated_at, completed_at
            FROM todo_task;
            DROP TABLE todo_task;
            ALTER TABLE todo_task_migrated RENAME TO todo_task;
            CREATE INDEX IF NOT EXISTS idx_todo_planned ON todo_task(planned_date, planned_time);
            CREATE INDEX IF NOT EXISTS idx_todo_status ON todo_task(status);
            CREATE INDEX IF NOT EXISTS idx_todo_external ON todo_task(external_id);
            COMMIT;
            PRAGMA foreign_keys = ON;
            ",
        )
        .map_err(|error| format!("无法迁移待办外部来源字段: {error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::initialize;
    use rusqlite::{params, Connection};

    #[test]
    fn migrates_todo_status_to_include_doing() {
        let connection = Connection::open_in_memory().expect("创建内存数据库");
        connection
            .execute_batch(
                "CREATE TABLE todo_task (
                  id TEXT PRIMARY KEY NOT NULL,
                  title TEXT NOT NULL,
                  description TEXT,
                  status TEXT NOT NULL CHECK(status IN ('open', 'done')),
                  priority TEXT NOT NULL CHECK(priority IN ('high', 'medium', 'low')),
                  category TEXT NOT NULL CHECK(category IN ('work', 'planning', 'personal')),
                  planned_date TEXT,
                  planned_time TEXT,
                  estimate_minutes INTEGER,
                  source_type TEXT NOT NULL CHECK(source_type IN ('manual', 'document')),
                  source_name TEXT,
                  created_at INTEGER NOT NULL,
                  updated_at INTEGER NOT NULL,
                  completed_at INTEGER
                );
                INSERT INTO todo_task (id, title, status, priority, category, source_type, created_at, updated_at, completed_at)
                VALUES ('t1', '旧任务', 'done', 'medium', 'work', 'manual', 1, 1, 2);",
            )
            .expect("创建旧表");
        initialize(&connection).expect("初始化并迁移");
        connection
            .execute("UPDATE todo_task SET status = 'doing' WHERE id = 't1'", [])
            .expect("迁移后允许 doing 状态");
        let count = connection
            .query_row("SELECT COUNT(*) FROM todo_task", [], |row| row.get::<_, i64>(0))
            .expect("读取待办数量");
        assert_eq!(count, 1, "迁移必须保留原有数据");
    }

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
