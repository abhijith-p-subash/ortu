use rusqlite::{params, Connection, Result};
use std::collections::HashMap;
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
    pub category: Option<String>, // Deprecated, kept for compat or primary display
    pub groups: Vec<String>,      // New: Many-to-Many groups
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
        let conn = self.conn.lock().unwrap();
        // Fetch items associated with this group name via item_groups
        let mut stmt = conn.prepare(
            "SELECT h.raw_content 
             FROM history h
             JOIN item_groups ig ON h.id = ig.item_id
             JOIN groups g ON ig.group_id = g.id
             WHERE g.name = ?1
             ORDER BY h.created_at DESC",
        )?;
        let rows = stmt.query_map(params![name], |row| row.get::<_, String>(0))?;
        let mut content = Vec::new();
        for r in rows {
            content.push(r?);
        }

        let output = content.join("\n---\n");
        std::fs::write(path, output)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        Ok(())
    }

    pub fn export_all_txt(&self, path: std::path::PathBuf) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT raw_content FROM history ORDER BY created_at DESC")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut content = Vec::new();
        for r in rows {
            content.push(r?);
        }

        let output = content.join("\n---\n");
        std::fs::write(path, output)
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
                // Filter items by category/group name in item_groups
                let parts: Vec<&str> = s.splitn(2, ' ').collect();
                let cat_name = parts[0].replace("category:", "");
                let search_term = if parts.len() > 1 { parts[1] } else { "" };
                let search_pattern = format!("%{}%", search_term);

                stmt = conn.prepare(
                    "SELECT DISTINCT h.id, h.content_type, h.raw_content, h.category, h.is_permanent, h.created_at 
                     FROM history h
                     JOIN item_groups ig ON h.id = ig.item_id
                     JOIN groups g ON ig.group_id = g.id
                     WHERE g.name = ?1 AND h.raw_content LIKE ?2
                     ORDER BY h.is_permanent DESC, h.created_at DESC 
                     LIMIT 100",
                )?;
                rows = stmt.query(params![cat_name, search_pattern])?;
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
        let mut item_ids = Vec::new();

        while let Some(row) = rows.next()? {
            let id: i64 = row.get(0)?;
            item_ids.push(id);
            items.push(ClipboardItem {
                id,
                content_type: row.get(1)?,
                raw_content: row.get(2)?,
                category: row.get(3)?,
                groups: Vec::new(), // Will populate below
                is_permanent: row.get(4)?,
                created_at: row.get(5)?,
            });
        }

        // Fetch groups for these items
        if !item_ids.is_empty() {
            // Create a placeholder string like "?, ?, ?"
            let placeholders: Vec<String> = item_ids.iter().map(|_| "?".to_string()).collect();
            let sql = format!(
                "SELECT ig.item_id, g.name 
                 FROM item_groups ig
                 JOIN groups g ON ig.group_id = g.id
                 WHERE ig.item_id IN ({})",
                placeholders.join(",")
            );

            let mut stmt = conn.prepare(&sql)?;
            // Convert ids to reference types rusqlite expects
            let params = rusqlite::params_from_iter(item_ids.iter());

            let mut group_rows = stmt.query(params)?;

            let mut groups_map: HashMap<i64, Vec<String>> = HashMap::new();

            while let Some(row) = group_rows.next()? {
                let item_id: i64 = row.get(0)?;
                let group_name: String = row.get(1)?;
                groups_map.entry(item_id).or_default().push(group_name);
            }

            for item in &mut items {
                if let Some(g_list) = groups_map.get(&item.id) {
                    item.groups = g_list.clone();
                }
            }
        }

        Ok(items)
    }

    pub fn add_to_group(&self, item_id: i64, group_name: String) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
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
        tx.execute(
            "INSERT OR IGNORE INTO groups (name) VALUES (?1)",
            params![group_name],
        )?;
        let group_id: i64 = tx.query_row(
            "SELECT id FROM groups WHERE name = ?1",
            params![group_name],
            |row| row.get(0),
        )?;

        tx.execute(
            "INSERT OR IGNORE INTO item_groups (item_id, group_id) VALUES (?1, ?2)",
            params![item_id, group_id],
        )?;

        tx.commit()?;
        Ok(())
    }

    pub fn remove_from_group(&self, item_id: i64, group_name: String) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
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

        let conn = self.conn.lock().unwrap();

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
        let mut stmt = conn.prepare("SELECT name FROM groups ORDER BY name ASC")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        let mut categories = Vec::new();
        for cat in rows {
            categories.push(cat?);
        }
        Ok(categories)
    }

    // --- Backup & Restore ---

    pub fn get_all_data_json(&self, selected_groups: Option<Vec<String>>) -> Result<String> {
        let conn = self.conn.lock().unwrap();

        // 1. Determine which items to fetch
        let sql = if let Some(ref groups) = selected_groups {
            if groups.is_empty() {
                // Empty list means all? Or none? Assuming "All" if Option is None, but if Some([]), maybe nothing?
                // Let's assume UI passes None for "All".
                "SELECT DISTINCT h.id, h.content_type, h.raw_content, h.category, h.is_permanent, h.created_at 
                  FROM history h"
            } else {
                // Filter by groups
                "SELECT DISTINCT h.id, h.content_type, h.raw_content, h.category, h.is_permanent, h.created_at 
                  FROM history h 
                  JOIN item_groups ig ON h.id = ig.item_id 
                  JOIN groups g ON ig.group_id = g.id 
                  WHERE g.name IN "
            }
        } else {
            "SELECT id, content_type, raw_content, category, is_permanent, created_at FROM history"
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
            Ok(ClipboardItem {
                id: row.get(0)?,
                content_type: row.get(1)?,
                raw_content: row.get(2)?,
                category: row.get(3)?,
                groups: Vec::new(),
                is_permanent: row.get(4)?,
                created_at: row.get(5)?,
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

        let mut conn = self.conn.lock().unwrap();
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
                "INSERT INTO history (content_type, raw_content, category, is_permanent, created_at) 
                 VALUES (?1, ?2, ?3, ?4, ?5)"
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
                    // New item
                    // Note: We ignore item.id from backup to let SQLite autoincrement prevent conflicts in merge
                    insert_stmt.execute(params![
                        item.content_type,
                        item.raw_content,
                        item.category,
                        item.is_permanent,
                        item.created_at
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
}
