use rusqlite::{params, Connection, Result};
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

pub struct ClipboardDB {
    conn: Mutex<Connection>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ClipboardItem {
    pub id: i64,
    pub content_type: String,
    pub raw_content: String,
    pub category: Option<String>,
    pub is_permanent: bool,
    pub created_at: String,
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

impl ClipboardDB {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        let app_dir = app_handle
            .path()
            .app_data_dir()
            .expect("failed to get app data dir");
        std::fs::create_dir_all(&app_dir).expect("failed to create app data dir");
        let db_path = app_dir.join("ortu.db");

        let conn = Connection::open(db_path)?;

        // Enable WAL mode for performance
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;",
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

        // Migrate existing categories into groups table
        conn.execute(
            "INSERT OR IGNORE INTO groups (name) 
             SELECT DISTINCT category FROM history WHERE category IS NOT NULL",
            [],
        )?;

        // Add index for performance
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_created_at ON history(created_at DESC)",
            [],
        )?;

        Ok(ClipboardDB {
            conn: Mutex::new(conn),
        })
    }

    // --- Group CRUD ---

    pub fn create_group(&self, name: String) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute("INSERT INTO groups (name) VALUES (?1)", params![name])?;
        Ok(conn.last_insert_rowid())
    }

    pub fn delete_group(&self, name: String) -> Result<()> {
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
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
        let items = self.get_history(Some(format!("category:{}", name)))?;
        let content = items
            .into_iter()
            .map(|i| i.raw_content)
            .collect::<Vec<_>>()
            .join("\n---\n"); // Using a separator for clarity

        std::fs::write(path, content)
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
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO history (content_type, raw_content, category) VALUES (?1, ?2, ?3)",
            params!["text", content, category],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_history(&self, search: Option<String>) -> Result<Vec<ClipboardItem>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt;
        let mut rows;

        if let Some(s) = search {
            if s.starts_with("group:") {
                let parts: Vec<&str> = s.splitn(2, ' ').collect();
                let group_name = parts[0].replace("group:", "");
                let search_term = if parts.len() > 1 { parts[1] } else { "" };
                let search_pattern = format!("%{}%", search_term);

                let where_clause = match group_name.as_str() {
                    "Dev" => "category IN ('Docker', 'Kubernetes', 'IaC', 'Cloud CLI', 'Shell / OS', 'CI / Build')",
                    "Code" => "category IN ('Version Control', 'Package Management', 'Runtime / Build', 'Database')",
                    "URL" => "category = 'URL'",
                    "Images" => "content_type = 'image'",
                    "Text" => "content_type = 'text'",
                    _ => "1=0" // Unknown group returns nothing
                };

                let sql = format!(
                    "SELECT id, content_type, raw_content, category, is_permanent, created_at 
                     FROM history 
                     WHERE ({}) AND raw_content LIKE ?1
                     ORDER BY is_permanent DESC, created_at DESC 
                     LIMIT 100",
                    where_clause
                );

                stmt = conn.prepare(&sql)?;
                rows = stmt.query(params![search_pattern])?;
            } else if s.starts_with("category:") {
                // Format is "category:GroupName Optional Search"
                // Fallback to simple split for now
                let parts: Vec<&str> = s.splitn(2, ' ').collect();
                let cat_part = parts[0].replace("category:", "");
                let search_part_val = if parts.len() > 1 { parts[1] } else { "" };

                let cat_pattern = cat_part;
                let text_pattern = format!("%{}%", search_part_val);

                stmt = conn.prepare(
                    "SELECT id, content_type, raw_content, category, is_permanent, created_at 
                     FROM history 
                     WHERE category = ?1 AND raw_content LIKE ?2 
                     ORDER BY is_permanent DESC, created_at DESC 
                     LIMIT 100",
                )?;
                rows = stmt.query(params![cat_pattern, text_pattern])?;
            } else {
                let pattern = format!("%{}%", s);
                stmt = conn.prepare(
                    "SELECT id, content_type, raw_content, category, is_permanent, created_at 
                     FROM history 
                     WHERE raw_content LIKE ?1 OR category LIKE ?1 
                     ORDER BY is_permanent DESC, created_at DESC 
                     LIMIT 100",
                )?;
                rows = stmt.query(params![pattern])?;
            }
        } else {
            stmt = conn.prepare(
                "SELECT id, content_type, raw_content, category, is_permanent, created_at 
                 FROM history 
                 ORDER BY is_permanent DESC, created_at DESC 
                 LIMIT 100",
            )?;
            rows = stmt.query([])?;
        }

        let mut items = Vec::new();
        while let Some(row) = rows.next()? {
            items.push(ClipboardItem {
                id: row.get(0)?,
                content_type: row.get(1)?,
                raw_content: row.get(2)?,
                category: row.get(3)?,
                is_permanent: row.get(4)?,
                created_at: row.get(5)?,
            });
        }
        Ok(items)
    }

    pub fn set_category(&self, id: i64, category: String) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE history SET category = ?1 WHERE id = ?2",
            params![category, id],
        )?;
        Ok(())
    }

    pub fn find_similar_category(&self, content: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        // Simple logic: find items that share the first 10-15 characters if it's a command
        if content.len() < 5 {
            return Ok(None);
        }

        // Match on prefix of first word
        let first_word = content.split_whitespace().next().unwrap_or("");
        if first_word.is_empty() {
            return Ok(None);
        }

        let mut stmt = conn.prepare(
            "SELECT category FROM history 
             WHERE category IS NOT NULL AND raw_content LIKE ?1 
             LIMIT 1",
        )?;
        let pattern = format!("{}%", first_word);
        let mut rows = stmt.query(params![pattern])?;
        if let Some(row) = rows.next()? {
            return Ok(Some(row.get(0)?));
        }
        Ok(None)
    }

    pub fn delete_item(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM history WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn toggle_permanent(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE history SET is_permanent = NOT is_permanent WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn prune_expired(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM history WHERE is_permanent = 0 AND created_at < datetime('now', '-24 hours')",
            [],
        )?;
        Ok(())
    }

    pub fn clear_ephemeral_on_start(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        // "Until computer shutdown" - cleared when app starts
        conn.execute("DELETE FROM history WHERE is_permanent = 0", [])?;
        Ok(())
    }

    pub fn get_categories(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT DISTINCT category FROM history WHERE category IS NOT NULL ORDER BY category ASC")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        let mut categories = Vec::new();
        for cat in rows {
            categories.push(cat?);
        }
        Ok(categories)
    }

    // --- Backup & Restore ---

    pub fn get_all_data_json(&self) -> Result<String> {
        let conn = self.conn.lock().unwrap();

        // Get all history
        let mut stmt = conn.prepare(
            "SELECT id, content_type, raw_content, category, is_permanent, created_at FROM history",
        )?;
        let history_iter = stmt.query_map([], |row| {
            Ok(ClipboardItem {
                id: row.get(0)?,
                content_type: row.get(1)?,
                raw_content: row.get(2)?,
                category: row.get(3)?,
                is_permanent: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        let history: Vec<ClipboardItem> = history_iter.collect::<Result<_, _>>()?;

        // Get all groups
        let mut stmt = conn.prepare("SELECT id, name, is_system FROM groups")?;
        let groups_iter = stmt.query_map([], |row| {
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

    pub fn restore_from_json(&self, json_content: &str) -> Result<()> {
        let backup: BackupData = serde_json::from_str(json_content)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        // Clear existing data
        tx.execute("DELETE FROM history", [])?;
        tx.execute("DELETE FROM groups", [])?;

        // Restore groups
        {
            let mut stmt =
                tx.prepare("INSERT INTO groups (id, name, is_system) VALUES (?1, ?2, ?3)")?;
            for group in backup.groups {
                stmt.execute(params![group.id, group.name, group.is_system])?;
            }
        }

        // Restore history
        {
            let mut stmt = tx.prepare(
                "INSERT INTO history (id, content_type, raw_content, category, is_permanent, created_at) 
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
            )?;
            for item in backup.history {
                stmt.execute(params![
                    item.id,
                    item.content_type,
                    item.raw_content,
                    item.category,
                    item.is_permanent,
                    item.created_at
                ])?;
            }
        }

        tx.commit()?;
        Ok(())
    }
}
