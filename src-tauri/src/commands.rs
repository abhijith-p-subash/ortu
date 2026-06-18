use crate::db::{ClipboardDB, ClipboardItem, Snippet};
#[cfg(target_os = "macos")]
use crate::PopupPasteTarget;
use crate::PasteStack;
use base64::Engine as _;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};

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
pub fn render_snippet(
    body: String,
    clipboard: Option<String>,
    inputs: Option<std::collections::HashMap<String, String>>,
) -> Result<String, String> {
    let now = chrono::Local::now();
    let mut s = body;

    // {{input:Label}} — values collected from the user before rendering.
    if let Some(map) = inputs {
        for (label, value) in map {
            s = s.replace(&format!("{{{{input:{}}}}}", label), &value);
        }
    }

    // {{date:FORMAT}} — chrono strftime, e.g. {{date:%A, %d %b %Y}}.
    if let Ok(re) = regex::Regex::new(r"\{\{date:([^}]+)\}\}") {
        s = re
            .replace_all(&s, |caps: &regex::Captures| now.format(&caps[1]).to_string())
            .to_string();
    }

    // Simple built-in variables.
    s = s
        .replace("{{date}}", &now.format("%Y-%m-%d").to_string())
        .replace("{{time}}", &now.format("%H:%M:%S").to_string())
        .replace("{{datetime}}", &now.format("%Y-%m-%d %H:%M:%S").to_string())
        .replace("{{clipboard}}", &clipboard.unwrap_or_default());

    // {{uuid}} — a fresh v4 UUID for each occurrence.
    while s.contains("{{uuid}}") {
        s = s.replacen("{{uuid}}", &gen_uuid_v4(), 1);
    }

    // {{cursor}} is a paste-time marker; not meaningful for copy, so strip it.
    s = s.replace("{{cursor}}", "");

    Ok(s)
}

/// Generates a random v4 UUID string.
fn gen_uuid_v4() -> String {
    let mut b = [0u8; 16];
    let _ = getrandom::getrandom(&mut b);
    b[6] = (b[6] & 0x0f) | 0x40; // version 4
    b[8] = (b[8] & 0x3f) | 0x80; // variant
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    )
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
    let (content_type, raw_content) = db.get_item_payload(id).map_err(|e| e.to_string())?;

    match content_type.as_str() {
        "image" => {
            // raw_content is the blob hash; decode the stored PNG back to RGBA.
            let png = db.get_blob(&raw_content).map_err(|e| e.to_string())?;
            let rgba = image::load_from_memory(&png)
                .map_err(|e| e.to_string())?
                .to_rgba8();
            let (width, height) = rgba.dimensions();
            let data = arboard::ImageData {
                width: width as usize,
                height: height as usize,
                bytes: std::borrow::Cow::Owned(rgba.into_raw()),
            };
            let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
            clipboard.set_image(data).map_err(|e| e.to_string())?;
        }
        "files" => {
            // raw_content is a JSON array of file paths; restore them as file
            // URLs so paste targets receive the actual files.
            let paths: Vec<String> =
                serde_json::from_str(&raw_content).map_err(|e| e.to_string())?;
            if !crate::write_clipboard_file_paths(&paths) {
                // Fallback (non-macOS or failure): copy the paths as text.
                let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
                clipboard.set_text(paths.join("\n")).map_err(|e| e.to_string())?;
            }
        }
        _ => {
            // Sensitive items are stored encrypted; decrypt before copying.
            let text = if crate::crypto::is_encrypted(&raw_content) {
                let key = crate::crypto::get_or_create_key(&app)?;
                crate::crypto::decrypt(&key, &raw_content)?
            } else {
                raw_content
            };
            let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
            clipboard.set_text(text).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

/// Puts arbitrary text on the system clipboard (used by the snippet "use" flow).
#[tauri::command]
pub fn set_clipboard_text(text: String) -> Result<(), String> {
    use arboard::Clipboard;
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.set_text(text).map_err(|e| e.to_string())
}

/// Returns the plaintext for stored content, decrypting it if it's a sensitive
/// (encrypted) item.
fn resolve_plaintext(app: &AppHandle, raw: &str) -> Result<String, String> {
    if crate::crypto::is_encrypted(raw) {
        let key = crate::crypto::get_or_create_key(app)?;
        crate::crypto::decrypt(&key, raw)
    } else {
        Ok(raw.to_string())
    }
}

/// Copies a text item to the clipboard after applying a transform
/// (uppercase, json_pretty, base64_encode, …). Powers "Copy as / paste as".
#[tauri::command]
pub fn copy_as(app: AppHandle, id: i64, transform: String) -> Result<(), String> {
    use arboard::Clipboard;
    let db = app.state::<ClipboardDB>();
    let (content_type, raw) = db.get_item_payload(id).map_err(|e| e.to_string())?;
    if content_type != "text" {
        return Err("Transforms apply to text items only".to_string());
    }
    let plain = resolve_plaintext(&app, &raw)?;
    let out = transform_content(plain, transform)?;
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.set_text(out).map_err(|e| e.to_string())?;
    Ok(())
}

/// Marks an item sensitive (encrypts its content + masks it) or clears the flag
/// (decrypts and restores plaintext). Only meaningful for text items.
#[tauri::command]
pub fn set_item_sensitive(app: AppHandle, id: i64, sensitive: bool) -> Result<(), String> {
    let db = app.state::<ClipboardDB>();
    let (_content_type, raw_content) = db.get_item_payload(id).map_err(|e| e.to_string())?;
    let key = crate::crypto::get_or_create_key(&app)?;

    if sensitive {
        if crate::crypto::is_encrypted(&raw_content) {
            return Ok(()); // already encrypted
        }
        let enc = crate::crypto::encrypt(&key, &raw_content)?;
        db.set_raw_and_sensitive(id, &enc, true).map_err(|e| e.to_string())?;
    } else {
        let plain = if crate::crypto::is_encrypted(&raw_content) {
            crate::crypto::decrypt(&key, &raw_content)?
        } else {
            raw_content
        };
        db.set_raw_and_sensitive(id, &plain, false).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Returns the decrypted plaintext of a sensitive item, for an explicit reveal.
#[tauri::command]
pub fn reveal_item(app: AppHandle, id: i64) -> Result<String, String> {
    let db = app.state::<ClipboardDB>();
    let (_content_type, raw_content) = db.get_item_payload(id).map_err(|e| e.to_string())?;
    if crate::crypto::is_encrypted(&raw_content) {
        let key = crate::crypto::get_or_create_key(&app)?;
        crate::crypto::decrypt(&key, &raw_content)
    } else {
        Ok(raw_content)
    }
}

#[tauri::command]
pub fn get_setting(app: AppHandle, key: String) -> Result<Option<String>, String> {
    let db = app.state::<ClipboardDB>();
    db.get_setting(&key).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_setting(app: AppHandle, key: String, value: String) -> Result<(), String> {
    let db = app.state::<ClipboardDB>();
    db.set_setting(&key, &value).map_err(|e| e.to_string())
}

// ── Paste stack (multi-paste queue) ─────────────────────────────────────────

/// Appends an item to the paste stack (no duplicates).
#[tauri::command]
pub fn stack_add(app: AppHandle, id: i64) -> Result<(), String> {
    {
        let stack = app.state::<PasteStack>();
        let mut q = stack.0.lock().map_err(|_| "lock".to_string())?;
        if !q.contains(&id) {
            q.push(id);
        }
    }
    let _ = app.emit("stack-updated", ());
    Ok(())
}

/// Removes a specific item from the paste stack.
#[tauri::command]
pub fn stack_remove(app: AppHandle, id: i64) -> Result<(), String> {
    {
        let stack = app.state::<PasteStack>();
        let mut q = stack.0.lock().map_err(|_| "lock".to_string())?;
        q.retain(|x| *x != id);
    }
    let _ = app.emit("stack-updated", ());
    Ok(())
}

/// Empties the paste stack.
#[tauri::command]
pub fn stack_clear(app: AppHandle) -> Result<(), String> {
    {
        let stack = app.state::<PasteStack>();
        let mut q = stack.0.lock().map_err(|_| "lock".to_string())?;
        q.clear();
    }
    let _ = app.emit("stack-updated", ());
    Ok(())
}

/// Returns the queued items (masked where sensitive), in paste order.
#[tauri::command]
pub fn stack_list(app: AppHandle) -> Result<Vec<ClipboardItem>, String> {
    let ids = {
        let stack = app.state::<PasteStack>();
        let q = stack.0.lock().map_err(|_| "lock".to_string())?;
        q.clone()
    };
    let db = app.state::<ClipboardDB>();
    db.get_items_by_ids(&ids).map_err(|e| e.to_string())
}

/// Pops the front of the paste stack, copies it to the clipboard, and pastes it
/// into the current frontmost app. Returns false when the stack is empty.
#[tauri::command]
pub async fn paste_next_from_stack(app: AppHandle) -> Result<bool, String> {
    let next = {
        let stack = app.state::<PasteStack>();
        let mut q = stack.0.lock().map_err(|_| "lock".to_string())?;
        if q.is_empty() {
            None
        } else {
            Some(q.remove(0))
        }
    };

    let Some(id) = next else {
        return Ok(false);
    };

    copy_item_to_clipboard(app.clone(), id)?;
    let _ = app.emit("stack-updated", ());
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    send_paste_shortcut()?;
    Ok(true)
}

/// Returns a base64 PNG data URL for an image item's thumbnail, for UI display.
#[tauri::command]
pub fn get_image_thumbnail(app: AppHandle, id: i64) -> Result<String, String> {
    let db = app.state::<ClipboardDB>();
    let (content_type, raw_content) = db.get_item_payload(id).map_err(|e| e.to_string())?;
    if content_type != "image" {
        return Err("not an image item".to_string());
    }
    let thumb = db.get_blob_thumb(&raw_content).map_err(|e| e.to_string())?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&thumb);
    Ok(format!("data:image/png;base64,{}", b64))
}

/// Generates a thumbnail for an image file on disk (used to preview image files
/// captured from the clipboard). Errors for non-image files so the UI can fall
/// back to a generic file icon.
#[tauri::command]
pub fn get_file_thumbnail(path: String) -> Result<String, String> {
    let lower = path.to_lowercase();
    let is_image = [".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp"]
        .iter()
        .any(|ext| lower.ends_with(ext));
    if !is_image {
        return Err("not a supported image file".to_string());
    }
    let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
    if meta.len() > 40 * 1024 * 1024 {
        return Err("file too large".to_string());
    }
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let img = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
    let mut thumb = Vec::new();
    img.thumbnail(240, 240)
        .write_to(&mut std::io::Cursor::new(&mut thumb), image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&thumb);
    Ok(format!("data:image/png;base64,{}", b64))
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
pub fn add_manual_item(
    app: AppHandle,
    content: String,
    description: Option<String>,
    group_name: Option<String>,
) -> Result<i64, String> {
    let db = app.state::<ClipboardDB>();
    db.add_manual_item(content, description, group_name)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_item(
    app: AppHandle,
    id: i64,
    content: String,
    description: Option<String>,
) -> Result<(), String> {
    if content.trim().is_empty() {
        return Err("Content cannot be empty".to_string());
    }
    let db = app.state::<ClipboardDB>();
    db.update_item(id, content, description)
        .map_err(|e| e.to_string())
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

#[cfg(test)]
mod snippet_tests {
    use super::*;
    #[test]
    fn renders_smart_variables() {
        let mut inputs = std::collections::HashMap::new();
        inputs.insert("Name".to_string(), "Alice".to_string());
        let body = "Hi {{input:Name}} | cb={{clipboard}} | yr={{date:%Y}} | id={{uuid}} | cur={{cursor}}END".to_string();
        let out = render_snippet(body, Some("CLIP".to_string()), Some(inputs)).unwrap();
        assert!(out.contains("Hi Alice "), "input: {out}");
        assert!(out.contains("cb=CLIP "), "clipboard: {out}");
        assert!(out.contains(&format!("yr={} ", chrono::Local::now().format("%Y"))), "date:fmt: {out}");
        assert!(out.contains("cur=END"), "cursor stripped: {out}");
        assert!(!out.contains("{{"), "all placeholders replaced: {out}");
        // uuid: 36 chars with dashes at the right spots
        let uuid = out.split("id=").nth(1).unwrap().split(' ').next().unwrap();
        assert_eq!(uuid.len(), 36, "uuid len: {uuid}");
        assert_eq!(uuid.as_bytes()[14], b'4', "uuid v4: {uuid}");
    }
}
