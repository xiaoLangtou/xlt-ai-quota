use crate::database;
use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tauri::AppHandle;
use uuid::Uuid;

const TAG_SEPARATOR: char = '\u{001f}';

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snippet {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub content: String,
    pub language: Option<String>,
    pub description: String,
    pub tags: Vec<String>,
    pub pinned: bool,
    pub use_count: i64,
    pub last_used_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnippetDraft {
    pub id: Option<String>,
    pub title: String,
    pub kind: String,
    pub content: String,
    pub language: Option<String>,
    pub description: String,
    pub tags: Vec<String>,
    pub pinned: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnippetQuery {
    pub query: Option<String>,
    pub kind: Option<String>,
    pub tag: Option<String>,
    pub sort: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagCount {
    pub name: String,
    pub count: i64,
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn is_kind(value: &str) -> bool {
    matches!(value, "code" | "command" | "prompt" | "text" | "link")
}

fn normalized_tag(value: &str) -> String {
    value.trim().to_lowercase()
}

fn clean_tags(values: &[String]) -> Vec<String> {
    let mut normalized = HashSet::new();
    values
        .iter()
        .map(|tag| tag.trim())
        .filter(|tag| !tag.is_empty())
        .filter(|tag| normalized.insert(normalized_tag(tag)))
        .map(ToOwned::to_owned)
        .collect()
}

fn clean_draft(draft: SnippetDraft) -> Result<SnippetDraft, String> {
    let title = draft.title.trim().to_owned();
    let content = draft.content;
    let kind = draft.kind.trim().to_owned();
    if title.is_empty() {
        return Err("请输入片段标题".to_owned());
    }
    if title.chars().count() > 120 {
        return Err("片段标题不能超过 120 个字符".to_owned());
    }
    if content.trim().is_empty() {
        return Err("请输入片段内容".to_owned());
    }
    if !is_kind(&kind) {
        return Err("片段类型无效".to_owned());
    }
    Ok(SnippetDraft {
        id: draft.id.filter(|id| !id.trim().is_empty()),
        title,
        kind,
        content,
        language: draft
            .language
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty()),
        description: draft.description.trim().to_owned(),
        tags: clean_tags(&draft.tags),
        pinned: draft.pinned,
    })
}

fn tags_for_snippet(
    transaction: &Transaction<'_>,
    snippet_id: &str,
) -> Result<Vec<String>, String> {
    let mut statement = transaction
        .prepare(
            "SELECT tags.name
             FROM tags
             INNER JOIN snippet_tags ON snippet_tags.tag_id = tags.id
             WHERE snippet_tags.snippet_id = ?1
             ORDER BY tags.name COLLATE NOCASE",
        )
        .map_err(|error| format!("无法读取片段标签: {error}"))?;
    let rows = statement
        .query_map(params![snippet_id], |row| row.get::<_, String>(0))
        .map_err(|error| format!("无法读取片段标签: {error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("无法读取片段标签: {error}"))
}

fn read_snippet(connection: &Connection, id: &str) -> Result<Snippet, String> {
    let mut statement = connection
        .prepare(
            "SELECT s.id, s.title, s.kind, s.content, s.language, s.description, s.pinned,
                    s.use_count, s.last_used_at, s.created_at, s.updated_at,
                    COALESCE(GROUP_CONCAT(tags.name, char(31)), '')
             FROM snippets s
             LEFT JOIN snippet_tags ON snippet_tags.snippet_id = s.id
             LEFT JOIN tags ON tags.id = snippet_tags.tag_id
             WHERE s.id = ?1
             GROUP BY s.id",
        )
        .map_err(|error| format!("无法读取片段: {error}"))?;
    statement
        .query_row(params![id], row_to_snippet)
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => "片段不存在或已删除".to_owned(),
            _ => format!("无法读取片段: {error}"),
        })
}

fn row_to_snippet(row: &rusqlite::Row<'_>) -> rusqlite::Result<Snippet> {
    let tags = row.get::<_, String>(11)?;
    Ok(Snippet {
        id: row.get(0)?,
        title: row.get(1)?,
        kind: row.get(2)?,
        content: row.get(3)?,
        language: row.get(4)?,
        description: row.get(5)?,
        pinned: row.get::<_, i64>(6)? != 0,
        use_count: row.get(7)?,
        last_used_at: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        tags: if tags.is_empty() {
            Vec::new()
        } else {
            tags.split(TAG_SEPARATOR).map(ToOwned::to_owned).collect()
        },
    })
}

fn upsert_tags(
    transaction: &Transaction<'_>,
    snippet_id: &str,
    tags: &[String],
) -> Result<(), String> {
    transaction
        .execute(
            "DELETE FROM snippet_tags WHERE snippet_id = ?1",
            params![snippet_id],
        )
        .map_err(|error| format!("无法更新片段标签: {error}"))?;
    for name in tags {
        let normalized_name = normalized_tag(name);
        transaction
            .execute(
                "INSERT INTO tags (id, name, normalized_name) VALUES (?1, ?2, ?3)
                 ON CONFLICT(normalized_name) DO NOTHING",
                params![Uuid::new_v4().to_string(), name, normalized_name],
            )
            .map_err(|error| format!("无法保存片段标签: {error}"))?;
        let tag_id = transaction
            .query_row(
                "SELECT id FROM tags WHERE normalized_name = ?1",
                params![normalized_name],
                |row| row.get::<_, String>(0),
            )
            .map_err(|error| format!("无法读取片段标签: {error}"))?;
        transaction
            .execute(
                "INSERT INTO snippet_tags (snippet_id, tag_id) VALUES (?1, ?2)",
                params![snippet_id, tag_id],
            )
            .map_err(|error| format!("无法关联片段标签: {error}"))?;
    }
    transaction
        .execute(
            "DELETE FROM tags WHERE id NOT IN (SELECT DISTINCT tag_id FROM snippet_tags)",
            [],
        )
        .map_err(|error| format!("无法清理片段标签: {error}"))?;
    Ok(())
}

fn update_search_index(transaction: &Transaction<'_>, snippet_id: &str) -> Result<(), String> {
    transaction
        .execute(
            "DELETE FROM snippets_fts WHERE id = ?1",
            params![snippet_id],
        )
        .map_err(|error| format!("无法更新片段搜索索引: {error}"))?;
    let (title, content, description, language): (String, String, String, Option<String>) =
        transaction
            .query_row(
                "SELECT title, content, description, language FROM snippets WHERE id = ?1",
                params![snippet_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .map_err(|error| format!("无法更新片段搜索索引: {error}"))?;
    let tags = tags_for_snippet(transaction, snippet_id)?.join(" ");
    transaction
        .execute(
            "INSERT INTO snippets_fts (id, title, content, description, language, tags)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![snippet_id, title, content, description, language, tags],
        )
        .map_err(|error| format!("无法更新片段搜索索引: {error}"))?;
    Ok(())
}

fn fts_query(value: &str) -> Option<String> {
    let tokens = value
        .split_whitespace()
        .filter(|token| !token.is_empty())
        .map(|token| format!("\"{}\"", token.replace('"', "\"\"")))
        .collect::<Vec<_>>();
    (!tokens.is_empty()).then(|| tokens.join(" AND "))
}

#[tauri::command]
pub fn snippet_list(app: AppHandle, filter: SnippetQuery) -> Result<Vec<Snippet>, String> {
    let connection = database::open(&app)?;
    let query = filter.query.as_deref().and_then(fts_query);
    let kind = filter.kind.filter(|value| is_kind(value));
    let tag = filter
        .tag
        .map(|value| normalized_tag(&value))
        .filter(|value| !value.is_empty());
    let order_by = match filter.sort.as_str() {
        "updated" => "s.pinned DESC, s.updated_at DESC",
        "created" => "s.pinned DESC, s.created_at DESC",
        "title" => "s.pinned DESC, s.title COLLATE NOCASE ASC",
        "usage" => "s.pinned DESC, s.use_count DESC, s.last_used_at DESC, s.updated_at DESC",
        _ => "s.pinned DESC, CASE WHEN s.last_used_at IS NULL THEN 1 ELSE 0 END ASC, s.last_used_at DESC, s.updated_at DESC",
    };
    let sql = format!(
        "SELECT s.id, s.title, s.kind, s.content, s.language, s.description, s.pinned,
                s.use_count, s.last_used_at, s.created_at, s.updated_at,
                COALESCE(GROUP_CONCAT(tags.name, char(31)), '')
         FROM snippets s
         LEFT JOIN snippet_tags ON snippet_tags.snippet_id = s.id
         LEFT JOIN tags ON tags.id = snippet_tags.tag_id
         WHERE (?1 IS NULL OR s.id IN (SELECT id FROM snippets_fts WHERE snippets_fts MATCH ?1))
           AND (?2 IS NULL OR s.kind = ?2)
           AND (?3 IS NULL OR EXISTS (
             SELECT 1 FROM snippet_tags selected_tags
             INNER JOIN tags selected_tag ON selected_tag.id = selected_tags.tag_id
             WHERE selected_tags.snippet_id = s.id AND selected_tag.normalized_name = ?3
           ))
         GROUP BY s.id
         ORDER BY {order_by}",
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| format!("无法搜索片段: {error}"))?;
    let rows = statement
        .query_map(params![query, kind, tag], row_to_snippet)
        .map_err(|error| format!("无法搜索片段: {error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("无法搜索片段: {error}"))
}

#[tauri::command]
pub fn snippet_list_tags(app: AppHandle) -> Result<Vec<TagCount>, String> {
    let connection = database::open(&app)?;
    let mut statement = connection
        .prepare(
            "SELECT tags.name, COUNT(snippet_tags.snippet_id)
             FROM tags
             INNER JOIN snippet_tags ON snippet_tags.tag_id = tags.id
             GROUP BY tags.id
             ORDER BY tags.name COLLATE NOCASE",
        )
        .map_err(|error| format!("无法读取片段标签: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok(TagCount {
                name: row.get(0)?,
                count: row.get(1)?,
            })
        })
        .map_err(|error| format!("无法读取片段标签: {error}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("无法读取片段标签: {error}"))
}

#[tauri::command]
pub fn snippet_save(app: AppHandle, draft: SnippetDraft) -> Result<Snippet, String> {
    let draft = clean_draft(draft)?;
    let mut connection = database::open(&app)?;
    let transaction = connection
        .transaction()
        .map_err(|error| format!("无法保存片段: {error}"))?;
    let id = draft
        .id
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let existing = transaction
        .query_row(
            "SELECT 1 FROM snippets WHERE id = ?1",
            params![&id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|error| format!("无法保存片段: {error}"))?
        .is_some();
    let timestamp = now();
    if existing {
        transaction
            .execute(
                "UPDATE snippets
                 SET title = ?2, kind = ?3, content = ?4, language = ?5, description = ?6,
                     pinned = ?7, updated_at = ?8
                 WHERE id = ?1",
                params![
                    &id,
                    &draft.title,
                    &draft.kind,
                    &draft.content,
                    &draft.language,
                    &draft.description,
                    draft.pinned,
                    &timestamp
                ],
            )
            .map_err(|error| format!("无法保存片段: {error}"))?;
    } else {
        transaction
            .execute(
                "INSERT INTO snippets (id, title, kind, content, language, description, pinned, use_count, last_used_at, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, NULL, ?8, ?8)",
                params![&id, &draft.title, &draft.kind, &draft.content, &draft.language, &draft.description, draft.pinned, &timestamp],
            )
            .map_err(|error| format!("无法保存片段: {error}"))?;
    }
    upsert_tags(&transaction, &id, &draft.tags)?;
    update_search_index(&transaction, &id)?;
    transaction
        .commit()
        .map_err(|error| format!("无法保存片段: {error}"))?;
    read_snippet(&connection, &id)
}

#[tauri::command]
pub fn snippet_delete(app: AppHandle, id: String) -> Result<(), String> {
    let mut connection = database::open(&app)?;
    let transaction = connection
        .transaction()
        .map_err(|error| format!("无法删除片段: {error}"))?;
    let affected = transaction
        .execute("DELETE FROM snippets WHERE id = ?1", params![&id])
        .map_err(|error| format!("无法删除片段: {error}"))?;
    if affected == 0 {
        return Err("片段不存在或已删除".to_owned());
    }
    transaction
        .execute("DELETE FROM snippets_fts WHERE id = ?1", params![&id])
        .map_err(|error| format!("无法删除片段搜索索引: {error}"))?;
    transaction
        .execute(
            "DELETE FROM tags WHERE id NOT IN (SELECT DISTINCT tag_id FROM snippet_tags)",
            [],
        )
        .map_err(|error| format!("无法清理片段标签: {error}"))?;
    transaction
        .commit()
        .map_err(|error| format!("无法删除片段: {error}"))
}

#[tauri::command]
pub fn snippet_touch(app: AppHandle, id: String) -> Result<Snippet, String> {
    let connection = database::open(&app)?;
    let timestamp = now();
    let affected = connection
        .execute(
            "UPDATE snippets SET use_count = use_count + 1, last_used_at = ?2 WHERE id = ?1",
            params![&id, &timestamp],
        )
        .map_err(|error| format!("无法更新片段使用记录: {error}"))?;
    if affected == 0 {
        return Err("片段不存在或已删除".to_owned());
    }
    read_snippet(&connection, &id)
}

#[tauri::command]
pub fn snippet_copy(app: AppHandle, id: String) -> Result<Snippet, String> {
    let connection = database::open(&app)?;
    let snippet = read_snippet(&connection, &id)?;
    crate::clipboard_history::write_plain_text(&snippet.content)?;
    drop(connection);
    snippet_touch(app, id)
}

#[cfg(test)]
mod tests {
    use super::{clean_draft, fts_query, SnippetDraft};

    #[test]
    fn keeps_code_whitespace_and_deduplicates_tags() {
        let cleaned = clean_draft(SnippetDraft {
            id: None,
            title: "  类型检查  ".to_owned(),
            kind: "code".to_owned(),
            content: "  const value = 1;\n".to_owned(),
            language: Some(" typescript ".to_owned()),
            description: "  用于提交前检查  ".to_owned(),
            tags: vec![
                "Git".to_owned(),
                "git".to_owned(),
                " debug ".to_owned(),
                "".to_owned(),
            ],
            pinned: true,
        })
        .expect("清理片段草稿");
        assert_eq!(cleaned.title, "类型检查");
        assert_eq!(cleaned.content, "  const value = 1;\n");
        assert_eq!(cleaned.language.as_deref(), Some("typescript"));
        assert_eq!(cleaned.tags, vec!["Git", "debug"]);
    }

    #[test]
    fn turns_multiple_search_terms_into_an_fts_and_query() {
        assert_eq!(
            fts_query("pnpm typecheck"),
            Some("\"pnpm\" AND \"typecheck\"".to_owned())
        );
        assert_eq!(fts_query("   "), None);
    }
}
