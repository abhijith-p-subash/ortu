use crate::db::{ClipboardDB, ClipboardItem};
use tauri::{AppHandle, Manager};

#[tauri::command]
pub fn get_history(app: AppHandle, search: Option<String>) -> Result<Vec<ClipboardItem>, String> {
    let db = app.state::<ClipboardDB>();
    db.get_history(search).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_entry(app: AppHandle, id: i64) -> Result<(), String> {
    let db = app.state::<ClipboardDB>();
    db.delete_item(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn toggle_permanent(app: AppHandle, id: i64) -> Result<(), String> {
    let db = app.state::<ClipboardDB>();
    db.toggle_permanent(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_category(app: AppHandle, id: i64, category: String) -> Result<(), String> {
    let db = app.state::<ClipboardDB>();
    db.set_category(id, category).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_group(app: AppHandle, name: String) -> Result<(), String> {
    let db = app.state::<ClipboardDB>();
    db.create_group(name).map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_group(app: AppHandle, name: String) -> Result<(), String> {
    let db = app.state::<ClipboardDB>();
    db.delete_group(name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn rename_group(app: AppHandle, old_name: String, new_name: String) -> Result<(), String> {
    let db = app.state::<ClipboardDB>();
    db.rename_group(old_name, new_name)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn export_group(app: AppHandle, name: String, path: String) -> Result<(), String> {
    let db = app.state::<ClipboardDB>();
    db.export_group(name, std::path::PathBuf::from(path))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn import_group(app: AppHandle, name: String, path: String) -> Result<(), String> {
    let db = app.state::<ClipboardDB>();
    db.import_group(name, std::path::PathBuf::from(path))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn backup_data(app: AppHandle, path: String) -> Result<(), String> {
    let db = app.state::<ClipboardDB>();
    let json = db.get_all_data_json().map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn restore_data(app: AppHandle, path: String) -> Result<(), String> {
    let db = app.state::<ClipboardDB>();
    let json = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    db.restore_from_json(&json).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_categories(app: AppHandle) -> Result<Vec<String>, String> {
    let db = app.state::<ClipboardDB>();
    db.get_categories().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn paste_item(_app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        // Small delay to ensure the window has hidden and focus returned to previous app
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        let _ = Command::new("osascript")
            .arg("-e")
            .arg("tell application \"System Events\" to keystroke \"v\" using {command down}")
            .spawn();
    }
    Ok(())
}

#[tauri::command]
pub fn manual_cleanup(app: AppHandle) -> Result<(), String> {
    let db = app.state::<ClipboardDB>();
    db.prune_expired().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn close_window(app: AppHandle, label: Option<String>) -> Result<(), String> {
    let target = label.unwrap_or_else(|| "popup".to_string());
    if let Some(window) = app.get_webview_window(&target) {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}
