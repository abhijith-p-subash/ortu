use crate::db::{ClipboardDB, ClipboardItem, Snippet};
#[cfg(target_os = "macos")]
use crate::PopupPasteTarget;
use base64::Engine as _;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

fn validate_path(path_str: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(path_str);
    if !path.is_absolute() {
        return Err("Path must be absolute".to_string());
    }

    // Basic protection against sensitive system paths
    let p_str = path.to_string_lossy();
    let dangerous_prefixes = [
        "/etc",
        "/var",
        "/bin",
        "/sbin",
        "/lib",
        "/usr/bin",
        "/usr/sbin",
        "C:\\Windows",
        "C:\\Program Files",
        "C:\\Users\\Public",
    ];

    for prefix in dangerous_prefixes {
        if p_str.starts_with(prefix) {
            return Err(
                "Access to system directories is restricted for security reasons".to_string(),
            );
        }
    }

    Ok(path)
}

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
pub fn add_to_group(app: AppHandle, item_id: i64, group_name: String) -> Result<(), String> {
    let db = app.state::<ClipboardDB>();
    db.add_to_group(item_id, group_name)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_from_group(app: AppHandle, item_id: i64, group_name: String) -> Result<(), String> {
    let db = app.state::<ClipboardDB>();
    db.remove_from_group(item_id, group_name)
        .map_err(|e| e.to_string())
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
    let validated_path = validate_path(&path)?;
    let db = app.state::<ClipboardDB>();
    db.export_group(name, validated_path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn import_group(app: AppHandle, name: String, path: String) -> Result<(), String> {
    let validated_path = validate_path(&path)?;
    let db = app.state::<ClipboardDB>();
    db.import_group(name, validated_path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn backup_data(
    app: AppHandle,
    path: String,
    groups: Option<Vec<String>>,
) -> Result<(), String> {
    let validated_path = validate_path(&path)?;
    let db = app.state::<ClipboardDB>();
    let json = db.get_all_data_json(groups).map_err(|e| e.to_string())?;
    std::fs::write(validated_path, json).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn restore_data(app: AppHandle, path: String, mode: String) -> Result<(), String> {
    let validated_path = validate_path(&path)?;
    let db = app.state::<ClipboardDB>();
    let json = std::fs::read_to_string(validated_path).map_err(|e| e.to_string())?;
    db.restore_from_json(&json, &mode)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_categories(app: AppHandle) -> Result<Vec<String>, String> {
    let db = app.state::<ClipboardDB>();
    db.get_categories().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_snippets(app: AppHandle) -> Result<Vec<Snippet>, String> {
    let db = app.state::<ClipboardDB>();
    db.list_snippets().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_snippet(app: AppHandle, name: String, body: String) -> Result<(), String> {
    let db = app.state::<ClipboardDB>();
    db.upsert_snippet(name, body).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_snippet(app: AppHandle, id: i64) -> Result<(), String> {
    let db = app.state::<ClipboardDB>();
    db.delete_snippet(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn render_snippet(body: String, clipboard: Option<String>) -> Result<String, String> {
    let now = chrono::Local::now();
    let mut rendered = body
        .replace("{{date}}", &now.format("%Y-%m-%d").to_string())
        .replace("{{time}}", &now.format("%H:%M:%S").to_string())
        .replace("{{datetime}}", &now.format("%Y-%m-%d %H:%M:%S").to_string());
    rendered = rendered.replace("{{clipboard}}", &clipboard.unwrap_or_default());
    Ok(rendered)
}

#[tauri::command]
pub fn transform_content(content: String, transform: String) -> Result<String, String> {
    let value = match transform.as_str() {
        "trim" => content.trim().to_string(),
        "uppercase" => content.to_uppercase(),
        "lowercase" => content.to_lowercase(),
        "slugify" => content
            .trim()
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-"),
        "json_pretty" => serde_json::to_string_pretty(
            &serde_json::from_str::<serde_json::Value>(&content).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?,
        "json_minify" => serde_json::to_string(
            &serde_json::from_str::<serde_json::Value>(&content).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?,
        "base64_encode" => base64::engine::general_purpose::STANDARD.encode(content),
        "base64_decode" => String::from_utf8(
            base64::engine::general_purpose::STANDARD
                .decode(content)
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?,
        "url_encode" => urlencoding::encode(&content).to_string(),
        "url_decode" => urlencoding::decode(&content)
            .map_err(|e| e.to_string())?
            .to_string(),
        _ => return Err("Unknown transform".to_string()),
    };
    Ok(value)
}

#[tauri::command]
pub async fn paste_item(_app: AppHandle) -> Result<(), String> {
    tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
    send_paste_shortcut()
}

fn send_paste_shortcut() -> Result<(), String> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};

    #[cfg(target_os = "macos")]
    ensure_macos_accessibility_permission()?;

    log::info!("Sending paste shortcut");

    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;

    #[cfg(target_os = "macos")]
    {
        let _ = enigo.key(Key::Meta, Direction::Press);
        // Use the fixed macOS virtual keycode for "V" to avoid layout lookup
        // on a Tokio worker thread, which can trap in release builds.
        let _ = enigo.key(Key::Other(9), Direction::Click);
        let _ = enigo.key(Key::Meta, Direction::Release);
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = enigo.key(Key::Control, Direction::Press);
        let _ = enigo.key(Key::Unicode('v'), Direction::Click);
        let _ = enigo.key(Key::Control, Direction::Release);
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn escape_applescript_string(v: &str) -> String {
    v.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(target_os = "macos")]
fn ensure_macos_accessibility_permission() -> Result<(), String> {
    let trusted = unsafe { AXIsProcessTrusted() };
    if trusted {
        return Ok(());
    }

    let _ = open_macos_accessibility_settings();

    Err(
        "Ortu needs macOS Accessibility permission to paste into other apps. Enable Ortu in System Settings -> Privacy & Security -> Accessibility, then try again.".to_string(),
    )
}

#[cfg(target_os = "macos")]
fn activate_popup_target_macos(target_bundle_id: Option<&str>) -> Result<(), String> {
    use std::process::Command;

    log::info!(
        "Activating popup paste target on macOS: {:?}",
        target_bundle_id
    );

    let script = if let Some(bundle) = target_bundle_id {
        let escaped = escape_applescript_string(bundle);
        format!(
            r#"
tell application id "{escaped}" to activate
delay 0.10
tell application id "{escaped}" to activate
"#
        )
    } else {
        return Ok(());
    };

    let status = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .status()
        .map_err(|e| e.to_string())?;

    if !status.success() {
        return Err(format!(
            "Failed to activate previous app before paste (exit: {})",
            status
        ));
    }

    Ok(())
}

#[tauri::command]
pub fn get_macos_accessibility_status() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        return Ok(unsafe { AXIsProcessTrusted() });
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(true)
    }
}

#[tauri::command]
pub fn open_macos_accessibility_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .status()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(())
    }
}

#[tauri::command]
pub fn copy_item_to_clipboard(app: AppHandle, id: i64) -> Result<(), String> {
    use arboard::Clipboard;

    let db = app.state::<ClipboardDB>();
    let (_content_type, raw_content) = db.get_item_payload(id).map_err(|e| e.to_string())?;

    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.set_text(raw_content).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn copy_item_and_paste(app: AppHandle, id: i64) -> Result<(), String> {
    copy_item_to_clipboard(app, id)?;
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    send_paste_shortcut()
}

#[tauri::command]
pub async fn copy_item_and_paste_from_popup(app: AppHandle, id: i64) -> Result<(), String> {
    log::info!("Popup paste requested for item {}", id);

    if let Some(window) = app.get_webview_window("popup") {
        log::info!("Hiding popup window before paste");
        let _ = window.hide();
    }

    copy_item_to_clipboard(app.clone(), id)?;
    log::info!("Clipboard payload restored for item {}", id);

    #[cfg(target_os = "macos")]
    {
        let target_bundle = app
            .try_state::<PopupPasteTarget>()
            .and_then(|s| s.0.lock().ok().and_then(|g| g.clone()));
        log::info!("Stored popup target bundle: {:?}", target_bundle);
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        activate_popup_target_macos(target_bundle.as_deref())?;
        tokio::time::sleep(tokio::time::Duration::from_millis(450)).await;
        return send_paste_shortcut();
    }

    #[cfg(not(target_os = "macos"))]
    {
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
        return send_paste_shortcut();
    }
}

#[tauri::command]
pub async fn export_all_txt(app: AppHandle, path: String) -> Result<(), String> {
    let validated_path = validate_path(&path)?;
    let db = app.state::<ClipboardDB>();
    db.export_all_txt(validated_path).map_err(|e| e.to_string())
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
