use rusqlite::{params, Connection, Result};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

pub struct ClipboardDB {
    conn: Mutex<Connection>,
    /// Whether the FTS5 full-text index is available; when false, search falls
    /// back to LIKE scans.
    fts_enabled: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ClipboardItem {
    pub id: i64,
    pub content_type: String,
    pub raw_content: String,
    pub category: Option<String>,
    pub groups: Vec<String>,
    pub is_permanent: bool,
    pub created_at: String,
    pub description: Option<String>,
    pub is_manual: bool,
    // Marked sensitive: content is encrypted at rest and masked in the UI.
    #[serde(default)]
    pub is_sensitive: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct Group {
    pub id: i64,
    pub name: String,
    pub is_system: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct BackupData {
    pub history: Vec<ClipboardItem>,
    pub groups: Vec<Group>,
    pub exported_at: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct Snippet {
    pub id: i64,
    pub name: String,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
}

fn fuzzy_score(query: &str, text: &str) -> i32 {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return 0;
    }
    let t = text.to_lowercase();
    if t.contains(&q) {
        return 1000 - (t.find(&q).unwrap_or(0) as i32);
    }

    let mut score = 0;
    let mut last_idx = 0usize;
    for qc in q.chars() {
        if let Some(pos) = t[last_idx..].find(qc) {
            score += 8;
            last_idx += pos + 1;
        } else {
            return -1;
        }
    }
    score
}

/// Builds an FTS5 MATCH query from free user text. Each alphanumeric token
/// becomes a quoted prefix term so partial words match. Returns None when there
/// are no usable tokens (caller falls back to LIKE).
fn build_fts_query(input: &str) -> Option<String> {
    let tokens: Vec<String> = input
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{}\"*", t.replace('"', "\"\"")))
        .collect();
    if tokens.is_empty() {
        None
    } else {
        Some(tokens.join(" "))
    }
}

impl ClipboardDB {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        let conn = {
            let preferred_path = app_handle.path().app_data_dir().ok().map(|dir| dir.join("ortu.db"));

            if let Some(path) = preferred_path {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                match Connection::open(&path) {
                    Ok(conn) => conn,
                    Err(e) => {
                        eprintln!(
                            "DB: failed to open preferred DB path '{}': {}. Falling back.",
                            path.display(),
                            e
                        );
                        Self::open_fallback_connection()?
                    }
                }
            } else {
                Self::open_fallback_connection()?
            }
        };

        // Enable WAL mode for performance and enforce foreign keys
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;",
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS groups (
                id INTEGER PRIMARY KEY,
                name TEXT UNIQUE NOT NULL,
                is_system BOOLEAN DEFAULT 0
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS history (
                id INTEGER PRIMARY KEY,
                content_type TEXT NOT NULL,
                raw_content TEXT NOT NULL,
                category TEXT,
                is_permanent BOOLEAN DEFAULT 0,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        // Migration: Fix incorrect item_groups foreign key reference if it exists
        let needs_fix = {
            let mut stmt = conn.prepare("PRAGMA foreign_key_list('item_groups')")?;
            let mut rows = stmt.query([])?;
            let mut found = false;
            while let Some(row) = rows.next()? {
                let referenced_table: String = row.get(2)?;
                if referenced_table == "clipboard_items" {
                    found = true;
                    break;
                }
            }
            found
        };

        if needs_fix {
            println!("DB: Fixing incorrect item_groups schema...");
            conn.execute("DROP TABLE item_groups", [])?;
        }

        conn.execute(
            "CREATE TABLE IF NOT EXISTS item_groups (
                item_id INTEGER NOT NULL,
                group_id INTEGER NOT NULL,
                PRIMARY KEY (item_id, group_id),
                FOREIGN KEY(item_id) REFERENCES history(id) ON DELETE CASCADE,
                FOREIGN KEY(group_id) REFERENCES groups(id) ON DELETE CASCADE
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS item_group_confidence (
                item_id INTEGER NOT NULL,
                group_id INTEGER NOT NULL,
                confidence REAL NOT NULL,
                PRIMARY KEY (item_id, group_id),
                FOREIGN KEY(item_id) REFERENCES history(id) ON DELETE CASCADE,
                FOREIGN KEY(group_id) REFERENCES groups(id) ON DELETE CASCADE
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS app_meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS snippets (
                id INTEGER PRIMARY KEY,
                name TEXT UNIQUE NOT NULL,
                body TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        // Content-addressed binary store for clipboard images. A history row of
        // content_type 'image' references a blob by storing its hash in
        // raw_content; identical images are deduplicated by hash.
        conn.execute(
            "CREATE TABLE IF NOT EXISTS blobs (
                hash TEXT PRIMARY KEY,
                mime TEXT NOT NULL,
                data BLOB NOT NULL,
                thumb BLOB,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        // Migrate: add description, is_manual and is_sensitive columns if not present
        let _ = conn.execute("ALTER TABLE history ADD COLUMN description TEXT", []);
        let _ = conn.execute("ALTER TABLE history ADD COLUMN is_manual BOOLEAN DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE history ADD COLUMN is_sensitive BOOLEAN DEFAULT 0", []);

        // Migrate existing categories into groups table
        conn.execute(
            "INSERT OR IGNORE INTO groups (name) 
             SELECT DISTINCT category FROM history WHERE category IS NOT NULL",
            [],
        )?;

        // Migrate existing category column to item_groups
        match conn.execute(
            "INSERT OR IGNORE INTO item_groups (item_id, group_id)
             SELECT h.id, g.id 
             FROM history h
             JOIN groups g ON h.category = g.name
             WHERE h.category IS NOT NULL",
            [],
        ) {
            Ok(_) => {}                                         // Migration successful
            Err(e) => println!("DB: Migration warning: {}", e), // Log warning but verify app continues
        }

        // Add index for performance
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_created_at ON history(created_at DESC)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_item_groups_item_id ON item_groups(item_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_item_groups_group_id ON item_groups(group_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_item_group_confidence_item_id ON item_group_confidence(item_id)",
            [],
        )?;

        // Full-text search index (FTS5), mirroring `history` via triggers. May be
        // absent on some SQLite builds; degrade gracefully to LIKE search.
        let fts_enabled = Self::setup_fts(&conn).unwrap_or_else(|e| {
            eprintln!("DB: FTS5 unavailable, falling back to LIKE search: {}", e);
            false
        });

        Ok(ClipboardDB {
            conn: Mutex::new(conn),
            fts_enabled,
        })
    }

    /// Creates the FTS5 virtual table, sync triggers, and backfills the index
    /// once. Returns Ok(true) when FTS5 is ready.
    fn setup_fts(conn: &Connection) -> Result<bool> {
        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS history_fts USING fts5(
                raw_content,
                description,
                content='history',
                content_rowid='id'
            );

            CREATE TRIGGER IF NOT EXISTS history_fts_ai AFTER INSERT ON history BEGIN
                INSERT INTO history_fts(rowid, raw_content, description)
                VALUES (new.id, new.raw_content, COALESCE(new.description, ''));
            END;

            CREATE TRIGGER IF NOT EXISTS history_fts_ad AFTER DELETE ON history BEGIN
                INSERT INTO history_fts(history_fts, rowid, raw_content, description)
                VALUES ('delete', old.id, old.raw_content, COALESCE(old.description, ''));
            END;

            CREATE TRIGGER IF NOT EXISTS history_fts_au AFTER UPDATE ON history BEGIN
                INSERT INTO history_fts(history_fts, rowid, raw_content, description)
                VALUES ('delete', old.id, old.raw_content, COALESCE(old.description, ''));
                INSERT INTO history_fts(rowid, raw_content, description)
                VALUES (new.id, new.raw_content, COALESCE(new.description, ''));
            END;",
        )?;

        // One-time backfill of existing rows (idempotent rebuild).
        let built: Option<String> = conn
            .query_row(
                "SELECT value FROM app_meta WHERE key = 'fts_built'",
                [],
                |row| row.get(0),
            )
            .ok();
        if built.as_deref() != Some("1") {
            conn.execute_batch("INSERT INTO history_fts(history_fts) VALUES('rebuild');")?;
            conn.execute(
                "INSERT INTO app_meta (key, value) VALUES ('fts_built', '1')
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [],
            )?;
        }

        Ok(true)
    }

    fn open_fallback_connection() -> Result<Connection> {
        let temp_db = std::env::temp_dir().join("ortu").join("ortu.db");
        if let Some(parent) = temp_db.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        match Connection::open(&temp_db) {
            Ok(conn) => Ok(conn),
            Err(e) => {
                eprintln!(
                    "DB: failed to open fallback DB path '{}': {}. Using in-memory DB.",
                    temp_db.display(),
                    e
                );
                Connection::open_in_memory()
            }
        }
    }

    // --- Group CRUD ---

    fn ensure_group_with_type(
        tx: &rusqlite::Transaction<'_>,
        group_name: &str,
        is_system: bool,
    ) -> Result<i64> {
        tx.execute(
            "INSERT OR IGNORE INTO groups (name, is_system) VALUES (?1, ?2)",
            params![group_name, is_system],
        )?;
        tx.query_row(
            "SELECT id FROM groups WHERE name = ?1",
            params![group_name],
            |row| row.get(0),
        )
    }

    fn get_meta_value(tx: &rusqlite::Transaction<'_>, key: &str) -> Result<Option<String>> {
        let value = tx
            .query_row(
                "SELECT value FROM app_meta WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .ok();
        Ok(value)
    }

    fn set_meta_value(tx: &rusqlite::Transaction<'_>, key: &str, value: &str) -> Result<()> {
        tx.execute(
            "INSERT INTO app_meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn clear_ephemeral_on_boot_change(&self, boot_session_id: &str) -> Result<bool> {
        let mut conn = self.conn.lock().map_err(|_| rusqlite::Error::InvalidQuery)?;
        let tx = conn.transaction()?;
        let previous = Self::get_meta_value(&tx, "boot_session_id")?;
        let changed = previous.as_deref() != Some(boot_session_id);

        if changed {
            // Keep pinned items and items explicitly assigned to at least one user group.
            tx.execute(
                "DELETE FROM history
                 WHERE is_permanent = 0
                   AND NOT EXISTS (
                     SELECT 1
                     FROM item_groups ig
                     JOIN groups g ON g.id = ig.group_id
                     WHERE ig.item_id = history.id
                       AND g.is_system = 0
                   )",
                [],
            )?;
            Self::set_meta_value(&tx, "boot_session_id", boot_session_id)?;
        }

        tx.commit()?;
        Ok(changed)
    }

    pub fn create_group(&self, name: String) -> Result<i64> {
        let conn = self.conn.lock().map_err(|_| rusqlite::Error::InvalidQuery)?;
        conn.execute(
            "INSERT INTO groups (name, is_system) VALUES (?1, 0)",
            params![name],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn delete_group(&self, name: String) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| rusqlite::Error::InvalidQuery)?;
        // Set items in this group to NULL or we can delete them.
        // The user request said "merging categories and group feature",
        // usually delete group means either clearing the tag or deleting items.
        // Let's clear the tag for now to be safe.
        conn.execute(
            "UPDATE history SET category = NULL WHERE category = ?1",
            params![name],
        )?;
        conn.execute("DELETE FROM groups WHERE name = ?1", params![name])?;
        Ok(())
    }

    pub fn rename_group(&self, old_name: String, new_name: String) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| rusqlite::Error::InvalidQuery)?;
        conn.execute(
            "UPDATE history SET category = ?1 WHERE category = ?2",
            params![new_name, old_name],
        )?;
        conn.execute(
            "UPDATE groups SET name = ?1 WHERE name = ?2",
            params![new_name, old_name],
        )?;
        Ok(())
    }

    pub fn export_group(&self, name: String, path: std::path::PathBuf) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| rusqlite::Error::InvalidQuery)?;

        let mut stmt = conn.prepare(
            "SELECT h.raw_content, h.description
             FROM history h
             JOIN item_groups ig ON h.id = ig.item_id
             JOIN groups g ON ig.group_id = g.id
             WHERE g.name = ?1
             ORDER BY h.created_at DESC",
        )?;

        let rows: Vec<(String, Option<String>)> = stmt
            .query_map(params![name], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();

        let total = rows.len();
        let exported_at = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let divider = "═".repeat(72);
        let thin = "─".repeat(72);

        let mut out = String::new();
        out.push('\u{FEFF}'); // UTF-8 BOM so text editors detect encoding correctly

        out.push_str(&format!("{divider}\n"));
        out.push_str(&format!("  ORTU GROUP EXPORT  —  {name}\n"));
        out.push_str(&format!("  Exported : {exported_at}\n"));
        out.push_str(&format!("  Items    : {total}\n"));
        out.push_str(&format!("{divider}\n"));

        for (i, (content, description)) in rows.iter().enumerate() {
            if i > 0 {
                out.push_str(&format!("{thin}\n"));
            }

            if let Some(desc) = description {
                if !desc.trim().is_empty() {
                    out.push_str(&format!("Description: {desc}\n"));
                }
            }

            out.push('\n');
            for line in content.lines() {
                out.push_str(&format!("  {line}\n"));
            }
            out.push('\n');
        }

        out.push_str(&format!("{divider}\n"));
        out.push_str(&format!("  End of export  —  {name}  ({total} items)\n"));
        out.push_str(&format!("{divider}\n"));

        std::fs::write(path, out)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        Ok(())
    }

    pub fn export_all_txt(&self, path: std::path::PathBuf) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| rusqlite::Error::InvalidQuery)?;

        let mut stmt = conn.prepare(
            "SELECT h.raw_content, h.description
             FROM history h
             ORDER BY h.created_at DESC",
        )?;

        let rows: Vec<(String, Option<String>)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();

        let total = rows.len();
        let exported_at = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let divider = "═".repeat(72);
        let thin = "─".repeat(72);

        let mut out = String::new();
        out.push('\u{FEFF}'); // UTF-8 BOM so text editors detect encoding correctly

        out.push_str(&format!("{divider}\n"));
        out.push_str("  ORTU CLIPBOARD EXPORT\n");
        out.push_str(&format!("  Exported : {exported_at}\n"));
        out.push_str(&format!("  Total    : {total} item(s)\n"));
        out.push_str(&format!("{divider}\n"));

        for (i, (content, description)) in rows.iter().enumerate() {
            if i > 0 {
                out.push_str(&format!("{thin}\n"));
            }

            if let Some(desc) = description {
                if !desc.trim().is_empty() {
                    out.push_str(&format!("Description: {desc}\n"));
                }
            }

            out.push('\n');
            for line in content.lines() {
                out.push_str(&format!("  {line}\n"));
            }
            out.push('\n');
        }

        out.push_str(&format!("{divider}\n"));
        out.push_str(&format!("  End of export — {total} item(s)\n"));
        out.push_str(&format!("{divider}\n"));

        std::fs::write(path, out)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        Ok(())
    }

    pub fn import_group(&self, name: String, path: std::path::PathBuf) -> Result<()> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        let items: Vec<&str> = content.split("\n---\n").collect();

        // Ensure group exists
        let _ = self.create_group(name.clone());

        for item in items {
            if !item.trim().is_empty() {
                let _ = self.insert_item(item.to_string(), Some(name.clone()));
            }
        }
        Ok(())
    }

    pub fn insert_item(&self, content: String, category: Option<String>) -> Result<i64> {
        let mut groups: Vec<(String, f32)> = Vec::new();
        if let Some(cat) = category.clone() {
            groups.push((cat, 1.0));
        }
        self.insert_item_with_groups("text", content, category, groups, false, false)
    }

    pub fn insert_auto_grouped_content(
        &self,
        content_type: &str,
        content: String,
        groups: Vec<(String, f32)>,
        is_sensitive: bool,
    ) -> Result<i64> {
        let primary = groups.first().map(|(name, _)| name.clone());
        self.insert_item_with_groups(content_type, content, primary, groups, true, is_sensitive)
    }

    fn insert_item_with_groups(
        &self,
        content_type: &str,
        content: String,
        primary_category: Option<String>,
        groups: Vec<(String, f32)>,
        system_groups: bool,
        is_sensitive: bool,
    ) -> Result<i64> {
        let mut conn = self.conn.lock().map_err(|_| rusqlite::Error::InvalidQuery)?;
        let tx = conn.transaction()?;

        let existing_item_id: Option<i64> = tx
            .query_row(
                "SELECT id FROM history WHERE raw_content = ?1 ORDER BY created_at DESC LIMIT 1",
                params![content],
                |row| row.get(0),
            )
            .ok();

        let item_id = if let Some(id) = existing_item_id {
            tx.execute(
                "UPDATE history
                 SET created_at = CURRENT_TIMESTAMP,
                     category = COALESCE(?1, category),
                     content_type = ?2,
                     is_sensitive = ?3
                 WHERE id = ?4",
                params![primary_category, content_type, is_sensitive, id],
            )?;
            id
        } else {
            tx.execute(
                "INSERT INTO history (content_type, raw_content, category, is_manual, is_sensitive) VALUES (?1, ?2, ?3, 0, ?4)",
                params![content_type, content, primary_category, is_sensitive],
            )?;
            tx.last_insert_rowid()
        };

        for (group_name, confidence) in groups {
            let trimmed = group_name.trim();
            if trimmed.is_empty() {
                continue;
            }
            let group_id = Self::ensure_group_with_type(&tx, trimmed, system_groups)?;
            tx.execute(
                "INSERT OR IGNORE INTO item_groups (item_id, group_id) VALUES (?1, ?2)",
                params![item_id, group_id],
            )?;
            tx.execute(
                "INSERT INTO item_group_confidence (item_id, group_id, confidence)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(item_id, group_id) DO UPDATE SET confidence = excluded.confidence",
                params![item_id, group_id, confidence.max(0.0)],
            )?;
        }

        tx.commit()?;
        Ok(item_id)
    }

    pub fn get_history(&self, search: Option<String>) -> Result<Vec<ClipboardItem>> {
        let conn = self.conn.lock().map_err(|_| rusqlite::Error::InvalidQuery)?;
        let mut stmt;
        let mut rows;
        let mut fuzzy_query: Option<String> = None;

        if let Some(s) = search {
            if s.starts_with("group:") || s.starts_with("category:") {
                let parts: Vec<&str> = s.splitn(2, ' ').collect();
                let group_name = parts[0]
                    .trim_start_matches("group:")
                    .trim_start_matches("category:")
                    .to_string();
                let search_term = if parts.len() > 1 { parts[1] } else { "" };
                let search_pattern = format!("%{}%", search_term);

                if group_name.eq_ignore_ascii_case("text") {
                    stmt = conn.prepare(
                        "SELECT h.id, h.content_type, h.raw_content, h.category, h.is_permanent, h.created_at, h.description, COALESCE(h.is_manual, 0), COALESCE(h.is_sensitive, 0)
                         FROM history h
                         WHERE h.content_type = 'text' AND h.raw_content LIKE ?1
                         ORDER BY h.is_permanent DESC, h.created_at DESC
                         LIMIT 100",
                    )?;
                    rows = stmt.query(params![search_pattern])?;
                } else if group_name.eq_ignore_ascii_case("images") {
                    stmt = conn.prepare(
                        "SELECT h.id, h.content_type, h.raw_content, h.category, h.is_permanent, h.created_at, h.description, COALESCE(h.is_manual, 0), COALESCE(h.is_sensitive, 0)
                         FROM history h
                         WHERE h.content_type = 'image' AND h.raw_content LIKE ?1
                         ORDER BY h.is_permanent DESC, h.created_at DESC
                         LIMIT 100",
                    )?;
                    rows = stmt.query(params![search_pattern])?;
                } else if group_name.eq_ignore_ascii_case("url")
                    || group_name.eq_ignore_ascii_case("urls")
                {
                    stmt = conn.prepare(
                        "SELECT DISTINCT h.id, h.content_type, h.raw_content, h.category, h.is_permanent, h.created_at, h.description, COALESCE(h.is_manual, 0), COALESCE(h.is_sensitive, 0)
                         FROM history h
                         LEFT JOIN item_groups ig ON h.id = ig.item_id
                         LEFT JOIN groups g ON ig.group_id = g.id
                         WHERE (
                            g.name = 'URL'
                            OR h.raw_content LIKE 'http://%'
                            OR h.raw_content LIKE 'https://%'
                            OR h.raw_content LIKE 'ftp://%'
                         ) AND h.raw_content LIKE ?1
                         ORDER BY h.is_permanent DESC, h.created_at DESC
                         LIMIT 100",
                    )?;
                    rows = stmt.query(params![search_pattern])?;
                } else {
                    stmt = conn.prepare(
                        "SELECT DISTINCT h.id, h.content_type, h.raw_content, h.category, h.is_permanent, h.created_at, h.description, COALESCE(h.is_manual, 0), COALESCE(h.is_sensitive, 0)
                         FROM history h
                         JOIN item_groups ig ON h.id = ig.item_id
                         JOIN groups g ON ig.group_id = g.id
                         WHERE g.name = ?1 AND (h.raw_content LIKE ?2 OR h.description LIKE ?2)
                         ORDER BY h.is_permanent DESC, h.created_at DESC
                         LIMIT 100",
                    )?;
                    rows = stmt.query(params![group_name, search_pattern])?;
                }
            } else {
                fuzzy_query = Some(s.clone());
                let pattern = format!("%{}%", s);
                // Fast path: FTS5 retrieves content/description candidates; we
                // still match category/group names for parity, then the Rust
                // fuzzy reranker orders the candidate set.
                match (self.fts_enabled, build_fts_query(&s)) {
                    (true, Some(fts)) => {
                        stmt = conn.prepare(
                            "SELECT id, content_type, raw_content, category, is_permanent, created_at, description, COALESCE(is_manual, 0), COALESCE(is_sensitive, 0)
                             FROM history
                             WHERE id IN (SELECT rowid FROM history_fts WHERE history_fts MATCH ?1)
                                OR category LIKE ?2
                                OR EXISTS (
                                    SELECT 1
                                    FROM item_groups ig
                                    JOIN groups g ON ig.group_id = g.id
                                    WHERE ig.item_id = history.id AND g.name LIKE ?2
                                )
                             ORDER BY is_permanent DESC, created_at DESC
                             LIMIT 500",
                        )?;
                        rows = stmt.query(params![fts, pattern])?;
                    }
                    _ => {
                        // Fallback: LIKE scan (FTS unavailable or no usable tokens).
                        stmt = conn.prepare(
                            "SELECT id, content_type, raw_content, category, is_permanent, created_at, description, COALESCE(is_manual, 0), COALESCE(is_sensitive, 0)
                             FROM history
                             WHERE raw_content LIKE ?1
                                OR description LIKE ?1
                                OR category LIKE ?1
                                OR EXISTS (
                                    SELECT 1
                                    FROM item_groups ig
                                    JOIN groups g ON ig.group_id = g.id
                                    WHERE ig.item_id = history.id AND g.name LIKE ?1
                                )
                             ORDER BY is_permanent DESC, created_at DESC
                             LIMIT 500",
                        )?;
                        rows = stmt.query(params![pattern])?;
                    }
                }
            }
        } else {
            stmt = conn.prepare(
                "SELECT id, content_type, raw_content, category, is_permanent, created_at, description, COALESCE(is_manual, 0), COALESCE(is_sensitive, 0)
                 FROM history
                 ORDER BY is_permanent DESC, created_at DESC
                 LIMIT 100",
            )?;
            rows = stmt.query([])?;
        }

        let mut items = Vec::new();
        let mut item_ids = Vec::new();

        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            item_ids.push(id);
            let is_sensitive: bool = row.get(8)?;
            // Sensitive items are never sent to the UI in clear; the content
            // stays encrypted at rest and is only revealed on explicit request.
            let raw_content: String = if is_sensitive {
                String::new()
            } else {
                row.get(2)?
            };
            items.push(ClipboardItem {
                id,
                content_type: row.get(1)?,
                raw_content,
                category: row.get(3)?,
                groups: Vec::new(),
                is_permanent: row.get(4)?,
                created_at: row.get(5)?,
                description: row.get(6)?,
                is_manual: row.get(7)?,
                is_sensitive,
            });
        }

        // Fetch groups for these items
        if !item_ids.is_empty() {
            // SQLite has a limit on the number of variables (usually 999).
            // For larger sets, we can use a temporary table or chunk the requests.
            // Since this is for a UI list (limited to 100), we are safe for now,
            // but for backup/restore we need to be careful.
            // Let's implement chunking as a safety measure.
            let mut groups_map: HashMap<i64, Vec<String>> = HashMap::new();

            for chunk in item_ids.chunks(900) {
                let placeholders: Vec<String> = chunk.iter().map(|_| "?".to_string()).collect();
                let sql = format!(
                    "SELECT ig.item_id, g.name 
                     FROM item_groups ig
                     JOIN groups g ON ig.group_id = g.id
                     WHERE ig.item_id IN ({})",
                    placeholders.join(",")
                );

                let mut stmt = conn.prepare(&sql)?;
                let params = rusqlite::params_from_iter(chunk.iter());
                let mut group_rows = stmt.query(params)?;

                while let Some(row) = group_rows.next()? {
                    let item_id: i64 = row.get(0)?;
                    let group_name: String = row.get(1)?;
                    groups_map.entry(item_id).or_default().push(group_name);
                }
            }

            for item in &mut items {
                if let Some(g_list) = groups_map.get(&item.id) {
                    item.groups = g_list.clone();
                }
            }
        }

        if let Some(query) = fuzzy_query {
            let mut ranked: Vec<(i32, ClipboardItem)> = items
                .into_iter()
                .filter_map(|item| {
                    let group_text = item.groups.join(" ");
                    let combined = format!(
                        "{} {} {}",
                        item.raw_content,
                        item.category.clone().unwrap_or_default(),
                        group_text
                    );
                    let score = fuzzy_score(&query, &combined);
                    if score >= 0 {
                        Some((score, item))
                    } else {
                        None
                    }
                })
                .collect();
            ranked.sort_by(|a, b| b.0.cmp(&a.0));
            items = ranked.into_iter().take(100).map(|(_, item)| item).collect();
        }

        Ok(items)
    }

    pub fn add_to_group(&self, item_id: i64, group_name: String) -> Result<()> {
        let mut conn = self.conn.lock().map_err(|_| rusqlite::Error::InvalidQuery)?;
        let tx = conn.transaction()?;

        // Check if item exists to avoid generic FK error
        let item_exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM history WHERE id = ?1)",
            params![item_id],
            |row| row.get(0),
        )?;

        if !item_exists {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }

        // Ensure group exists
        let group_id = Self::ensure_group_with_type(&tx, &group_name, false)?;

        tx.execute(
            "INSERT OR IGNORE INTO item_groups (item_id, group_id) VALUES (?1, ?2)",
            params![item_id, group_id],
        )?;

        tx.commit()?;
        Ok(())
    }

    pub fn remove_from_group(&self, item_id: i64, group_name: String) -> Result<()> {
        let mut conn = self.conn.lock().map_err(|_| rusqlite::Error::InvalidQuery)?;
        let tx = conn.transaction()?;

        let group_id_res: Result<i64> = tx.query_row(
            "SELECT id FROM groups WHERE name = ?1",
            params![group_name],
            |row| row.get(0),
        );

        if let Ok(group_id) = group_id_res {
            tx.execute(
                "DELETE FROM item_groups WHERE item_id = ?1 AND group_id = ?2",
                params![item_id, group_id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn set_category(&self, id: i64, category: String) -> Result<()> {
        // Updated to use new Group commands for compatibility
        // But for "set_category", usually implies "move to ONLY this group" or "add tag"?
        // Given the request said "Removing from a group does not delete the item",
        // likely we are moving towards tags.
        // For backwards compatibility with the UI calls "set_category", let's treat it as ADD to group for now,
        // OR distinct logic. The user request "Groups are user-defined collections... Items can belong to Zero, One, Multiple".
        // The UI currently has "Drag to group", which usually implies "Add".
        // Let's implement set_category as: Clear old groups? No, that's destructive.
        // Let's implement it as Add To Group for now.
        // BUT, the existing UI might expect it to assume it's the *only* category if we are filtering.
        // Actually, let's keep `category` column updated for now as a "primary" category or just for backward compat
        // until we fully migrate the UI.

        let conn = self.conn.lock().map_err(|_| rusqlite::Error::InvalidQuery)?;

        // Update legacy column
        conn.execute(
            "UPDATE history SET category = ?1 WHERE id = ?2",
            params![category, id],
        )?;

        // Update new relation
        drop(conn); // Unlock to call other method
        self.add_to_group(id, category)
    }

    pub fn find_similar_category(&self, content: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().map_err(|_| rusqlite::Error::InvalidQuery)?;
        if content.len() < 5 {
            return Ok(None);
        }

        let first_word = content.split_whitespace().next().unwrap_or("");
        if first_word.is_empty() {
            return Ok(None);
        }

        let pattern = format!("{}%", first_word);

        // Prefer user-defined groups associated with similar items (most common wins)
        let mut stmt = conn.prepare(
            "SELECT g.name, COUNT(*) AS cnt
             FROM history h
             JOIN item_groups ig ON h.id = ig.item_id
             JOIN groups g ON ig.group_id = g.id
             WHERE h.raw_content LIKE ?1 AND g.is_system = 0
             GROUP BY g.name
             ORDER BY cnt DESC
             LIMIT 1",
        )?;
        let mut rows = stmt.query(params![pattern])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(row.get(0)?));
        }

        // Fallback: legacy category column
        let mut stmt2 = conn.prepare(
            "SELECT category FROM history
             WHERE category IS NOT NULL AND raw_content LIKE ?1
             LIMIT 1",
        )?;
        let mut rows2 = stmt2.query(params![pattern])?;
        if let Some(row) = rows2.next()? {
            return Ok(Some(row.get(0)?));
        }

        Ok(None)
    }

    pub fn update_item(
        &self,
        id: i64,
        content: String,
        description: Option<String>,
    ) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| rusqlite::Error::InvalidQuery)?;
        conn.execute(
            "UPDATE history SET raw_content = ?1, description = ?2 WHERE id = ?3",
            params![content, description, id],
        )?;
        Ok(())
    }

    pub fn delete_item(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| rusqlite::Error::InvalidQuery)?;
        conn.execute("DELETE FROM history WHERE id = ?1", params![id])?;
        let _ = Self::prune_orphan_blobs(&conn);
        Ok(())
    }

    pub fn get_item_payload(&self, id: i64) -> Result<(String, String)> {
        let conn = self.conn.lock().map_err(|_| rusqlite::Error::InvalidQuery)?;
        conn.query_row(
            "SELECT content_type, raw_content FROM history WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
    }

    /// Replaces an item's stored content and sets its sensitive flag. Used when
    /// marking (store ciphertext) or unmarking (store plaintext) an item.
    pub fn set_raw_and_sensitive(&self, id: i64, content: &str, is_sensitive: bool) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| rusqlite::Error::InvalidQuery)?;
        conn.execute(
            "UPDATE history SET raw_content = ?1, is_sensitive = ?2 WHERE id = ?3",
            params![content, is_sensitive, id],
        )?;
        Ok(())
    }

    /// Fetches items by id (for the paste stack), masking sensitive content and
    /// returning them in the same order as `ids`.
    pub fn get_items_by_ids(&self, ids: &[i64]) -> Result<Vec<ClipboardItem>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.conn.lock().map_err(|_| rusqlite::Error::InvalidQuery)?;
        let placeholders: Vec<String> = ids.iter().map(|_| "?".to_string()).collect();
        let sql = format!(
            "SELECT id, content_type, raw_content, category, is_permanent, created_at, description, COALESCE(is_manual, 0), COALESCE(is_sensitive, 0)
             FROM history WHERE id IN ({})",
            placeholders.join(",")
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(ids.iter()), |row| {
            let is_sensitive: bool = row.get(8)?;
            let raw_content: String = if is_sensitive { String::new() } else { row.get(2)? };
            Ok(ClipboardItem {
                id: row.get(0)?,
                content_type: row.get(1)?,
                raw_content,
                category: row.get(3)?,
                groups: Vec::new(),
                is_permanent: row.get(4)?,
                created_at: row.get(5)?,
                description: row.get(6)?,
                is_manual: row.get(7)?,
                is_sensitive,
            })
        })?;
        let mut by_id: HashMap<i64, ClipboardItem> = HashMap::new();
        for r in rows {
            let item = r?;
            by_id.insert(item.id, item);
        }
        Ok(ids.iter().filter_map(|id| by_id.remove(id)).collect())
    }

    // --- App settings (key/value in app_meta) ---

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().map_err(|_| rusqlite::Error::InvalidQuery)?;
        Ok(conn
            .query_row(
                "SELECT value FROM app_meta WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .ok())
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| rusqlite::Error::InvalidQuery)?;
        conn.execute(
            "INSERT INTO app_meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    // --- Image blob store ---

    /// Stores image bytes (+ optional thumbnail) keyed by content hash. No-op if
    /// the hash already exists (content-addressed dedup).
    pub fn insert_blob(&self, hash: &str, mime: &str, data: &[u8], thumb: Option<&[u8]>) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| rusqlite::Error::InvalidQuery)?;
        conn.execute(
            "INSERT OR IGNORE INTO blobs (hash, mime, data, thumb) VALUES (?1, ?2, ?3, ?4)",
            params![hash, mime, data, thumb],
        )?;
        Ok(())
    }

    /// Full binary payload for a blob hash.
    pub fn get_blob(&self, hash: &str) -> Result<Vec<u8>> {
        let conn = self.conn.lock().map_err(|_| rusqlite::Error::InvalidQuery)?;
        conn.query_row("SELECT data FROM blobs WHERE hash = ?1", params![hash], |row| {
            row.get(0)
        })
    }

    /// Thumbnail for a blob hash, falling back to the full data.
    pub fn get_blob_thumb(&self, hash: &str) -> Result<Vec<u8>> {
        let conn = self.conn.lock().map_err(|_| rusqlite::Error::InvalidQuery)?;
        conn.query_row(
            "SELECT COALESCE(thumb, data) FROM blobs WHERE hash = ?1",
            params![hash],
            |row| row.get(0),
        )
    }

    /// Removes blobs no longer referenced by any image history row.
    fn prune_orphan_blobs(conn: &Connection) -> Result<()> {
        conn.execute(
            "DELETE FROM blobs WHERE hash NOT IN (
                 SELECT raw_content FROM history WHERE content_type = 'image'
             )",
            [],
        )?;
        Ok(())
    }

    pub fn toggle_permanent(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| rusqlite::Error::InvalidQuery)?;
        conn.execute(
            "UPDATE history SET is_permanent = NOT is_permanent WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn prune_expired(&self) -> Result<()> {
        // Configurable retention (settings in app_meta). Pinned items are always
        // kept and never count toward the max-items limit.
        let conn = self.conn.lock().map_err(|_| rusqlite::Error::InvalidQuery)?;
        let get_int = |key: &str| -> Option<i64> {
            conn.query_row(
                "SELECT value FROM app_meta WHERE key = ?1",
                params![key],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
        };

        // Retention only ever removes "ephemeral" items: NOT pinned and NOT in
        // any user-defined (non-system) group. Pinned and grouped items are
        // kept forever.
        let not_curated = "is_permanent = 0
             AND NOT EXISTS (
                 SELECT 1 FROM item_groups ig
                 JOIN groups g ON g.id = ig.group_id
                 WHERE ig.item_id = history.id AND g.is_system = 0
             )";

        // Age-based: delete ephemeral items older than N days (0 = keep forever).
        if let Some(days) = get_int("retention_days") {
            if days > 0 {
                conn.execute(
                    &format!(
                        "DELETE FROM history
                         WHERE {not_curated}
                           AND created_at < datetime('now', '-{days} days')"
                    ),
                    [],
                )?;
            }
        }

        // Count-based: keep only the newest N ephemeral items (0 = unlimited).
        if let Some(max) = get_int("retention_max_items") {
            if max > 0 {
                conn.execute(
                    &format!(
                        "DELETE FROM history
                         WHERE {not_curated}
                           AND id NOT IN (
                             SELECT id FROM history h2
                             WHERE h2.is_permanent = 0
                               AND NOT EXISTS (
                                 SELECT 1 FROM item_groups ig
                                 JOIN groups g ON g.id = ig.group_id
                                 WHERE ig.item_id = h2.id AND g.is_system = 0
                               )
                             ORDER BY h2.created_at DESC
                             LIMIT ?1
                           )"
                    ),
                    params![max],
                )?;
            }
        }

        let _ = Self::prune_orphan_blobs(&conn);
        Ok(())
    }

    pub fn get_categories(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().map_err(|_| rusqlite::Error::InvalidQuery)?;
        let mut stmt =
            conn.prepare("SELECT name FROM groups WHERE is_system = 0 ORDER BY name ASC")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        let mut categories = Vec::new();
        for cat in rows {
            categories.push(cat?);
        }
        Ok(categories)
    }

    pub fn list_snippets(&self) -> Result<Vec<Snippet>> {
        let conn = self.conn.lock().map_err(|_| rusqlite::Error::InvalidQuery)?;
        let mut stmt = conn.prepare(
            "SELECT id, name, body, created_at, updated_at
             FROM snippets
             ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Snippet {
                id: row.get(0)?,
                name: row.get(1)?,
                body: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn upsert_snippet(&self, name: String, body: String) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| rusqlite::Error::InvalidQuery)?;
        conn.execute(
            "INSERT INTO snippets (name, body, created_at, updated_at)
             VALUES (?1, ?2, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
             ON CONFLICT(name) DO UPDATE SET
               body = excluded.body,
               updated_at = CURRENT_TIMESTAMP",
            params![name, body],
        )?;
        Ok(())
    }

    pub fn delete_snippet(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| rusqlite::Error::InvalidQuery)?;
        conn.execute("DELETE FROM snippets WHERE id = ?1", params![id])?;
        Ok(())
    }

    // --- Backup & Restore ---

    pub fn get_all_data_json(&self, selected_groups: Option<Vec<String>>) -> Result<String> {
        let conn = self.conn.lock().map_err(|_| rusqlite::Error::InvalidQuery)?;

        // 1. Determine which items to fetch
        let sql = if let Some(ref groups) = selected_groups {
            if groups.is_empty() {
                "SELECT DISTINCT h.id, h.content_type, h.raw_content, h.category, h.is_permanent, h.created_at, h.description, COALESCE(h.is_manual, 0), COALESCE(h.is_sensitive, 0)
                  FROM history h"
            } else {
                "SELECT DISTINCT h.id, h.content_type, h.raw_content, h.category, h.is_permanent, h.created_at, h.description, COALESCE(h.is_manual, 0), COALESCE(h.is_sensitive, 0)
                  FROM history h
                  JOIN item_groups ig ON h.id = ig.item_id
                  JOIN groups g ON ig.group_id = g.id
                  WHERE g.name IN "
            }
        } else {
            "SELECT id, content_type, raw_content, category, is_permanent, created_at, description, COALESCE(is_manual, 0), COALESCE(is_sensitive, 0) FROM history"
        };

        let mut final_sql = sql.to_string();
        let mut params_vec: Vec<String> = Vec::new();

        if let Some(ref groups) = selected_groups {
            if !groups.is_empty() {
                let placeholders: Vec<String> = groups.iter().map(|_| "?".to_string()).collect();
                if final_sql.ends_with("IN ") {
                    final_sql = format!("{} ({})", final_sql, placeholders.join(","));
                    params_vec = groups.clone();
                }
            }
        }

        let mut stmt = conn.prepare(&final_sql)?;
        let params = rusqlite::params_from_iter(params_vec.iter());

        let history_iter = stmt.query_map(params, |row| {
            // Backup keeps sensitive content as-is (encrypted), so a restore on
            // the same machine (same key) round-trips it.
            Ok(ClipboardItem {
                id: row.get(0)?,
                content_type: row.get(1)?,
                raw_content: row.get(2)?,
                category: row.get(3)?,
                groups: Vec::new(),
                is_permanent: row.get(4)?,
                created_at: row.get(5)?,
                description: row.get(6)?,
                is_manual: row.get(7)?,
                is_sensitive: row.get(8)?,
            })
        })?;
        let mut history: Vec<ClipboardItem> = history_iter.collect::<Result<_, _>>()?;

        // 2. Populate groups for these items
        if !history.is_empty() {
            let item_ids: Vec<String> = history.iter().map(|i| i.id.to_string()).collect();
            let placeholders: Vec<String> = item_ids.iter().map(|_| "?".to_string()).collect();
            let sql_groups = format!(
                "SELECT ig.item_id, g.name 
                 FROM item_groups ig 
                 JOIN groups g ON ig.group_id = g.id
                 WHERE ig.item_id IN ({})",
                placeholders.join(",")
            );

            let mut stmt_g = conn.prepare(&sql_groups)?;
            let params_g = rusqlite::params_from_iter(item_ids.iter());
            let g_rows = stmt_g.query_map(params_g, |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?;

            let mut groups_map: HashMap<i64, Vec<String>> = HashMap::new();
            for r in g_rows {
                let (item_id, group_name) = r?;
                groups_map.entry(item_id).or_default().push(group_name);
            }

            for item in &mut history {
                if let Some(gs) = groups_map.get(&item.id) {
                    item.groups = gs.clone();
                }
            }
        }

        // 3. Get relevant groups
        let group_sql = if let Some(ref groups) = selected_groups {
            if !groups.is_empty() {
                let placeholders: Vec<String> = groups.iter().map(|_| "?".to_string()).collect();
                format!(
                    "SELECT id, name, is_system FROM groups WHERE name IN ({})",
                    placeholders.join(",")
                )
            } else {
                "SELECT id, name, is_system FROM groups".to_string()
            }
        } else {
            "SELECT id, name, is_system FROM groups".to_string()
        };

        let mut stmt_grp = conn.prepare(&group_sql)?;
        let grp_params_vec: Vec<String> = if let Some(ref groups) = selected_groups {
            groups.clone()
        } else {
            Vec::new()
        };
        let grp_params = rusqlite::params_from_iter(grp_params_vec.iter());

        let groups_iter = stmt_grp.query_map(grp_params, |row| {
            Ok(Group {
                id: row.get(0)?,
                name: row.get(1)?,
                is_system: row.get(2)?,
            })
        })?;
        let groups: Vec<Group> = groups_iter.collect::<Result<_, _>>()?;

        let backup = BackupData {
            history,
            groups,
            exported_at: chrono::Local::now().to_rfc3339(),
        };

        serde_json::to_string_pretty(&backup)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
    }

    pub fn restore_from_json(&self, json_content: &str, mode: &str) -> Result<()> {
        let backup: BackupData = serde_json::from_str(json_content)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        let mut conn = self.conn.lock().map_err(|_| rusqlite::Error::InvalidQuery)?;
        let tx = conn.transaction()?;

        if mode == "replace" {
            // Clear existing data
            tx.execute("DELETE FROM history", [])?;
            tx.execute("DELETE FROM groups", [])?;
        }

        // Restore groups
        {
            // use INSERT OR IGNORE to handle duplicates in merge mode
            let mut stmt =
                tx.prepare("INSERT OR IGNORE INTO groups (name, is_system) VALUES (?1, ?2)")?;

            for group in backup.groups {
                stmt.execute(params![group.name, group.is_system])?;
            }
        }

        // Restore history
        {
            let mut insert_stmt = tx.prepare(
                "INSERT INTO history (content_type, raw_content, category, is_permanent, created_at, description, is_manual, is_sensitive)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
            )?;

            // For checking existence in Merge mode
            let mut check_stmt = tx.prepare("SELECT id FROM history WHERE raw_content = ?1")?;

            let mut group_stmt = tx.prepare(
                "INSERT OR IGNORE INTO item_groups (item_id, group_id) 
                 SELECT ?1, id FROM groups WHERE name = ?2",
            )?;

            for item in backup.history {
                let mut item_id = -1;

                if mode == "merge" {
                    // Check if exists
                    let exists: Result<i64> =
                        check_stmt.query_row(params![item.raw_content], |row| row.get(0));
                    if let Ok(existing_id) = exists {
                        item_id = existing_id;
                    }
                }

                if item_id == -1 {
                    insert_stmt.execute(params![
                        item.content_type,
                        item.raw_content,
                        item.category,
                        item.is_permanent,
                        item.created_at,
                        item.description,
                        item.is_manual,
                        item.is_sensitive
                    ])?;
                    item_id = tx.last_insert_rowid();
                }

                // Restore/Merge item groups
                for g_name in item.groups {
                    group_stmt.execute(params![item_id, g_name])?;
                }
            }
        }

        tx.commit()?;
        Ok(())
    }

    pub fn add_manual_item(
        &self,
        content: String,
        description: Option<String>,
        group_name: Option<String>,
    ) -> Result<i64> {
        let mut conn = self.conn.lock().map_err(|_| rusqlite::Error::InvalidQuery)?;
        let tx = conn.transaction()?;

        tx.execute(
            "INSERT INTO history (content_type, raw_content, description, is_manual, is_permanent)
             VALUES ('text', ?1, ?2, 1, 1)",
            params![content, description],
        )?;
        let item_id = tx.last_insert_rowid();

        if let Some(group) = group_name {
            let trimmed = group.trim();
            if !trimmed.is_empty() {
                let group_id = Self::ensure_group_with_type(&tx, trimmed, false)?;
                tx.execute(
                    "INSERT OR IGNORE INTO item_groups (item_id, group_id) VALUES (?1, ?2)",
                    params![item_id, group_id],
                )?;
            }
        }

        tx.commit()?;
        Ok(item_id)
    }
}

