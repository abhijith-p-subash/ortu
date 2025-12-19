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

impl ClipboardDB {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        let app_dir = app_handle.path().app_data_dir().expect("failed to get app data dir");
        std::fs::create_dir_all(&app_dir).expect("failed to create app data dir");
        let db_path = app_dir.join("ortto.db");
        
        let conn = Connection::open(db_path)?;
        
        // Enable WAL mode for performance
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;",
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
        
        // Add index for performance
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_created_at ON history(created_at DESC)",
            [],
        )?;

        Ok(ClipboardDB {
            conn: Mutex::new(conn),
        })
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
             if s.starts_with("category:") {
                 let parts: Vec<&str> = s.splitn(2, ' ').collect();
                 let cat_part = parts[0].replace("category:", "");
                 let search_part = if parts.len() > 1 { parts[1] } else { "" };
                 
                 let cat_pattern = cat_part; // Exact match for category
                 let text_pattern = format!("%{}%", search_part);

                 stmt = conn.prepare(
                    "SELECT id, content_type, raw_content, category, is_permanent, created_at 
                     FROM history 
                     WHERE category = ?1 AND raw_content LIKE ?2 
                     ORDER BY is_permanent DESC, created_at DESC 
                     LIMIT 100"
                )?;
                rows = stmt.query(params![cat_pattern, text_pattern])?;
             } else {
                 let pattern = format!("%{}%", s);
                 stmt = conn.prepare(
                    "SELECT id, content_type, raw_content, category, is_permanent, created_at 
                     FROM history 
                     WHERE raw_content LIKE ?1 OR category LIKE ?1 
                     ORDER BY is_permanent DESC, created_at DESC 
                     LIMIT 100"
                )?;
                rows = stmt.query(params![pattern])?;
             }
        } else {
            stmt = conn.prepare(
                "SELECT id, content_type, raw_content, category, is_permanent, created_at 
                 FROM history 
                 ORDER BY is_permanent DESC, created_at DESC 
                 LIMIT 100"
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
        if content.len() < 5 { return Ok(None); }
        
        // Match on prefix of first word
        let first_word = content.split_whitespace().next().unwrap_or("");
        if first_word.is_empty() { return Ok(None); }

        let mut stmt = conn.prepare(
            "SELECT category FROM history 
             WHERE category IS NOT NULL AND raw_content LIKE ?1 
             LIMIT 1"
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
        conn.execute(
            "DELETE FROM history WHERE is_permanent = 0",
            [],
        )?;
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
}
