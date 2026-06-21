#![allow(unexpected_cfgs)]
mod clipboard;
mod commands;
mod crypto;
mod db;

use db::ClipboardDB;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use sysinfo::System;
use tauri::Manager;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::{Shortcut, ShortcutState};

#[cfg(target_os = "windows")]
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::HWND;
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR};

#[cfg(target_os = "macos")]
use cocoa::appkit::NSApp;
#[cfg(target_os = "macos")]
use cocoa::base::{id, nil, YES};
#[cfg(target_os = "macos")]
use objc::{class, msg_send, sel, sel_impl};
#[cfg(target_os = "macos")]
use std::ffi::CStr;
#[cfg(target_os = "macos")]
use std::os::raw::c_char;

pub struct PopupPasteTarget(pub Mutex<Option<String>>);

/// Ordered queue of history item ids for the paste stack (multi-paste).
pub struct PasteStack(pub Mutex<Vec<i64>>);

/// Maps currently-registered global shortcuts to their action id so the global
/// handler can dispatch. Rebuilt whenever shortcuts change.
pub struct ShortcutMap(pub Mutex<HashMap<Shortcut, String>>);

/// The user-rebindable global shortcut actions.
pub const SHORTCUT_ACTIONS: [&str; 3] = ["open_popup", "copy_stack", "paste_stack"];

/// Default accelerator for an action (Tauri accelerator syntax;
/// "CommandOrControl" → Cmd on macOS, Ctrl elsewhere).
pub fn default_accelerator(action: &str) -> &'static str {
    match action {
        "open_popup" => "Alt+V",
        "copy_stack" => "CommandOrControl+Shift+C",
        "paste_stack" => "Alt+Shift+V",
        _ => "",
    }
}

/// Resolves the accelerator for an action: user setting, else default.
pub fn accelerator_for(app: &AppHandle, action: &str) -> String {
    app.try_state::<ClipboardDB>()
        .and_then(|db| {
            db.get_setting(&format!("shortcut_{action}"))
                .ok()
                .flatten()
        })
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| default_accelerator(action).to_string())
}

/// (Re)registers all global shortcuts from current settings and rebuilds the
/// action lookup map. Returns the list of actions that failed to register
/// (e.g. invalid or already taken by another app).
pub fn register_global_shortcuts(app: &AppHandle) -> Vec<String> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    let gs = app.global_shortcut();
    let _ = gs.unregister_all();

    let mut map: HashMap<Shortcut, String> = HashMap::new();
    let mut failed: Vec<String> = Vec::new();

    for action in SHORTCUT_ACTIONS {
        let acc = accelerator_for(app, action);
        match acc.parse::<Shortcut>() {
            Ok(sc) => match gs.register(sc) {
                Ok(_) => {
                    map.insert(sc, action.to_string());
                }
                Err(e) => {
                    log::error!("Failed to register {action} ({acc}): {e}");
                    failed.push(action.to_string());
                }
            },
            Err(e) => {
                log::error!("Invalid accelerator for {action} ({acc}): {e}");
                failed.push(action.to_string());
            }
        }
    }

    if let Some(state) = app.try_state::<ShortcutMap>() {
        if let Ok(mut guard) = state.0.lock() {
            *guard = map;
        }
    }
    failed
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    install_panic_hook();
    configure_windows_webview2();
    startup_trace("run: builder init");

    let mut builder = tauri::Builder::default();

    #[cfg(not(target_os = "windows"))]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }));
    }

    #[cfg(desktop)]
    {
        builder = builder
            .plugin(tauri_plugin_updater::Builder::new().build())
            .plugin(tauri_plugin_process::init());
    }

    builder = builder
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, s, e| {
                    if e.state != ShortcutState::Pressed {
                        return;
                    }
                    // Look up which action this pressed shortcut maps to.
                    let action = app
                        .try_state::<ShortcutMap>()
                        .and_then(|m| m.0.lock().ok().and_then(|g| g.get(s).cloned()));

                    match action.as_deref() {
                        Some("open_popup") => toggle_popup(app),
                        Some("paste_stack") => {
                            // Paste the next item from the paste stack into the
                            // current frontmost app.
                            let app = app.clone();
                            tauri::async_runtime::spawn(async move {
                                let _ = commands::paste_next_from_stack(app).await;
                            });
                        }
                        Some("copy_stack") => {
                            // Copy the current selection straight into the paste stack.
                            let app = app.clone();
                            tauri::async_runtime::spawn(async move {
                                let _ = commands::copy_selection_to_stack(app).await;
                            });
                        }
                        _ => {}
                    }
                })
                .build(),
        );

    builder
        .setup(|app| {
            startup_trace("setup: entered");
            // Global shortcuts are registered after the DB + ShortcutMap state
            // are managed (they are read from user settings). See below.

            // ---------------- ARGUMENT CHECK (AUTOSTART VS MANUAL) ----------------
            let args: Vec<String> = std::env::args().collect();
            let mut is_hidden = false;

            // Defensive parsing of arguments
            for arg in &args {
                if arg == "--hidden" {
                    is_hidden = true;
                    break;
                }
            }

            if is_hidden {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            } else {
                show_main_window(app.handle());
            }

            // ---------------- DB INIT ----------------
            startup_trace("setup: db init start");
            let db = match ClipboardDB::new(app.handle()) {
                Ok(db) => {
                    startup_trace("setup: db init ok");
                    db
                }
                Err(e) => {
                    startup_trace(&format!("setup: db init error: {}", e));
                    return Err(e.into());
                }
            };
            let boot_session_id = current_boot_session_id();
            match db.clear_ephemeral_on_boot_change(&boot_session_id) {
                Ok(_) => startup_trace("setup: boot cleanup ok"),
                Err(e) => {
                    startup_trace(&format!("setup: boot cleanup error: {}", e));
                    return Err(e.into());
                }
            }
            // Apply retention limits at startup (also runs hourly).
            let _ = db.prune_expired();
            app.manage(db);
            app.manage(PopupPasteTarget(Mutex::new(None)));
            app.manage(PasteStack(Mutex::new(Vec::new())));
            app.manage(ShortcutMap(Mutex::new(HashMap::new())));

            // ---------------- GLOBAL SHORTCUT REGISTRATION ----------------
            let failed = register_global_shortcuts(app.handle());
            if failed.is_empty() {
                startup_trace("setup: global shortcuts registered");
            } else {
                startup_trace(&format!("setup: shortcuts failed to register: {:?}", failed));
            }

            // ---------------- TRAY ----------------
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let show_i = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            let tray_enabled = if let Some(icon) = app.default_window_icon().cloned() {
                match TrayIconBuilder::new()
                    .icon(icon)
                    .menu(&menu)
                    .on_menu_event(|app: &tauri::AppHandle, event| match event.id.as_ref() {
                        "quit" => app.exit(0),
                        "show" => show_main_window(app),
                        _ => {}
                    })
                    .build(app)
                {
                    Ok(_) => true,
                    Err(e) => {
                        log::error!("Tray icon initialization failed: {}", e);
                        false
                    }
                }
            } else {
                log::warn!("Default window icon missing; skipping tray icon initialization.");
                false
            };

            // macOS: Run as background accessory app (no dock icon, stays in menu bar)
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // ---------------- MAIN WINDOW SETUP ----------------
            // Prevent the app from quitting when the main window is closed
            if let Some(window) = app.get_webview_window("main") {
                apply_main_titlebar_color(&window);
                if tray_enabled {
                    let w = window.clone();
                    window.on_window_event(move |e| {
                        if let tauri::WindowEvent::CloseRequested { api, .. } = e {
                            api.prevent_close();
                            let _ = w.hide();
                        }
                    });
                }
            }

            // ---------------- POPUP WINDOW SETUP ----------------
            let _ = ensure_popup_window(app.handle());

            // ---------------- CLIPBOARD LISTENER ----------------
            startup_trace("setup: start clipboard listener");
            clipboard::start_listener(app.handle().clone());

            // ---------------- CLEANUP TASK ----------------
            let handle = app.handle().clone();
            thread::spawn(move || loop {
                thread::sleep(Duration::from_secs(3600));
                if let Some(db) = handle.try_state::<ClipboardDB>() {
                    let _ = db.prune_expired();
                }
            });

            // Global shortcut plugin moved to builder chain.

            // ---------------- AUTOSTART ----------------
            // Only enable autostart if logic requires it, avoid aggressive re-enabling which might corrupt registry
            {
                let autostart_manager = app.autolaunch();
                if let Ok(enabled) = autostart_manager.is_enabled() {
                    if !enabled {
                        log::info!("Autostart not enabled. Enabling...");
                        match autostart_manager.enable() {
                            Ok(_) => log::info!("Autostart enabled successfully."),
                            Err(e) => log::error!("Failed to enable autostart: {}", e),
                        }
                    }
                } else {
                    log::error!("Failed to check autostart status");
                }
            }

            startup_trace("setup: completed");
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_os::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_history,
            commands::delete_entry,
            commands::toggle_permanent,
            commands::set_category,
            commands::get_categories,
            commands::create_group,
            commands::delete_group,
            commands::rename_group,
            commands::export_group,
            commands::import_group,
            commands::paste_item,
            commands::copy_item_to_clipboard,
            commands::get_image_thumbnail,
            commands::get_file_thumbnail,
            commands::set_item_sensitive,
            commands::reveal_item,
            commands::get_setting,
            commands::set_setting,
            commands::get_shortcuts,
            commands::get_default_shortcuts,
            commands::set_shortcut,
            commands::reset_shortcuts,
            commands::stack_add,
            commands::copy_selection_to_stack,
            commands::stack_remove,
            commands::stack_clear,
            commands::stack_list,
            commands::paste_next_from_stack,
            commands::copy_as,
            commands::set_clipboard_text,
            commands::copy_item_and_paste,
            commands::copy_item_and_paste_from_popup,
            commands::get_macos_accessibility_status,
            commands::open_macos_accessibility_settings,
            commands::manual_cleanup,
            commands::close_window,
            commands::backup_data,
            commands::restore_data,
            commands::add_to_group,
            commands::remove_from_group,
            commands::export_all_txt,
            commands::add_manual_item,
            commands::update_item,
            commands::list_snippets,
            commands::save_snippet,
            commands::delete_snippet,
            commands::render_snippet,
            commands::transform_content
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri app");
}

fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let mut message = String::new();
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let tid = format!("{:?}", std::thread::current().id());
        message.push_str(&format!("\n=== panic captured [{}] thread={} ===\n", now, tid));

        if let Some(location) = info.location() {
            message.push_str(&format!(
                "location: {}:{}\n",
                location.file(),
                location.line()
            ));
        }

        if let Some(payload) = info.payload().downcast_ref::<&str>() {
            message.push_str(&format!("payload: {}\n", payload));
        } else if let Some(payload) = info.payload().downcast_ref::<String>() {
            message.push_str(&format!("payload: {}\n", payload));
        } else {
            message.push_str("payload: <non-string>\n");
        }

        let backtrace = std::backtrace::Backtrace::force_capture();
        message.push_str(&format!("backtrace:\n{:?}\n", backtrace));

        let dir = std::env::temp_dir().join("ortu");
        let path = dir.join("panic.log");
        let _ = fs::create_dir_all(&dir);
        if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(&path) {
            let _ = file.write_all(message.as_bytes());
        }
    }));
}

#[cfg(target_os = "windows")]
fn configure_windows_webview2() {
    // Force a writable user-data folder and disable GPU acceleration to avoid
    // intermittent native WebView2 startup crashes on some Windows systems.
    // Use a per-process profile dir to avoid lock/contention crashes on rapid relaunches.
    let pid = std::process::id();
    let data_dir = std::env::temp_dir()
        .join("ortu")
        .join("webview2-data")
        .join(format!("pid-{}", pid));
    let _ = fs::create_dir_all(&data_dir);
    std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", data_dir);
    std::env::set_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", "--disable-gpu");
    startup_trace(&format!("run: windows webview2 configured (pid={})", pid));
}

#[cfg(not(target_os = "windows"))]
fn configure_windows_webview2() {}

fn startup_trace(message: &str) {
    let dir = std::env::temp_dir().join("ortu");
    let path = dir.join("startup.log");
    let _ = fs::create_dir_all(&dir);
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let _ = writeln!(file, "[{}] {}", now, message);
    }
}

fn current_boot_session_id() -> String {
    // Stable across app restarts, changes on OS reboot.
    format!("{}", System::boot_time())
}

fn apply_main_titlebar_color(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "windows")]
    {
        let Ok(handle) = window.window_handle() else {
            return;
        };
        let hwnd = match handle.as_raw() {
            RawWindowHandle::Win32(h) => HWND(h.hwnd.get() as *mut std::ffi::c_void),
            _ => return,
        };

        // COLORREF format: 0x00BBGGRR (for #171A1D).
        let caption_color: u32 = 0x001D1A17;
        let text_color: u32 = 0x00E8E8E8;

        unsafe {
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_CAPTION_COLOR,
                &caption_color as *const _ as _,
                std::mem::size_of::<u32>() as u32,
            );
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_TEXT_COLOR,
                &text_color as *const _ as _,
                std::mem::size_of::<u32>() as u32,
            );
        }
    }

    #[cfg(target_os = "macos")]
    {
        let w = window.clone();
        let _ = window.run_on_main_thread(move || {
            if let Ok(handle) = w.ns_window() {
                let ns_window = handle as id;
                unsafe {
                    let color: id = msg_send![
                        class!(NSColor),
                        colorWithSRGBRed: 23.0f64 / 255.0f64
                        green: 26.0f64 / 255.0f64
                        blue: 29.0f64 / 255.0f64
                        alpha: 1.0f64
                    ];
                    let _: () = msg_send![ns_window, setTitlebarAppearsTransparent: YES];
                    let _: () = msg_send![ns_window, setBackgroundColor: color];
                }
            }
        });
    }
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        apply_main_titlebar_color(&window);
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn ensure_popup_window(app: &tauri::AppHandle) -> Option<tauri::WebviewWindow> {
    if let Some(window) = app.get_webview_window("popup") {
        return Some(window);
    }

    let mut builder = WebviewWindowBuilder::new(app, "popup", WebviewUrl::App("/popup".into()))
        .title("Ortu Quick Access")
        .inner_size(550.0, 450.0)
        .resizable(false)
        .decorations(false)
        .visible(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(true);

    #[cfg(not(target_os = "windows"))]
    {
        builder = builder.transparent(true);
    }

    #[cfg(target_os = "windows")]
    {
        builder = builder.transparent(false);
    }

    let window = match builder.build() {
        Ok(window) => window,
        Err(e) => {
            startup_trace(&format!("popup: create failed: {}", e));
            return None;
        }
    };

    let w = window.clone();
    #[cfg(target_os = "macos")]
    setup_mac_popup(&w);

    window.on_window_event(move |e| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = e {
            api.prevent_close();
            let _ = w.hide();
        } else if let tauri::WindowEvent::Focused(false) = e {
            let _ = w.hide();
        }
    });

    Some(window)
}

fn toggle_popup(app: &tauri::AppHandle) {
    if let Some(window) = ensure_popup_window(app) {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            show_popup(app);
        }
    }
}

#[cfg(target_os = "macos")]
fn setup_mac_popup(window: &tauri::WebviewWindow) {
    let w = window.clone();
    let _ = window.run_on_main_thread(move || {
        if let Ok(handle) = w.ns_window() {
            let ns_window = handle as id;
            unsafe {
                let style_mask: i32 = 0 | 8 | 128 | 128;
                let _: () = msg_send![ns_window, setStyleMask: style_mask];
                let _: () = msg_send![ns_window, setTitleVisibility: 1];
                let _: () = msg_send![ns_window, setTitlebarAppearsTransparent: YES];
                let behavior_flags: i64 = 1 | 64 | 256 | 1024;
                let _: () = msg_send![ns_window, setCollectionBehavior: behavior_flags];
                let _: () = msg_send![ns_window, setLevel: 2000];
                let _: () = msg_send![ns_window, setCanHide: false];
            }
        }
    });
}

fn show_popup(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("popup") {
        let w = window.clone();

        #[cfg(target_os = "macos")]
        {
            if let Some(target) = get_frontmost_app_bundle_id_macos() {
                if let Some(state) = app.try_state::<PopupPasteTarget>() {
                    if let Ok(mut guard) = state.0.lock() {
                        *guard = Some(target);
                    }
                }
            }

            let _ = app.run_on_main_thread(move || {
                unsafe {
                    let ns_app = NSApp();
                    let _: () = msg_send![ns_app, activateIgnoringOtherApps: YES];
                    if let Ok(handle) = w.ns_window() {
                        let ns_window = handle as id;
                        let _: () = msg_send![ns_window, setLevel: 2000];
                        let _: () = msg_send![ns_window, makeKeyAndOrderFront: nil];
                    }
                }
                let _ = w.show();
                let _ = w.set_focus();
            });
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = window.show();
            let _ = window.set_focus();
            let _ = window.set_always_on_top(true);
        }

        let _ = tauri_plugin_positioner::WindowExt::move_window(
            &window,
            tauri_plugin_positioner::Position::Center,
        );
    }
}

#[cfg(target_os = "macos")]
fn get_frontmost_app_bundle_id_macos() -> Option<String> {
    unsafe {
        let ws: id = msg_send![class!(NSWorkspace), sharedWorkspace];
        if ws == nil {
            return None;
        }
        let frontmost: id = msg_send![ws, frontmostApplication];
        if frontmost == nil {
            return None;
        }
        let bundle_id: id = msg_send![frontmost, bundleIdentifier];
        if bundle_id == nil {
            return None;
        }
        let ptr: *const c_char = msg_send![bundle_id, UTF8String];
        if ptr.is_null() {
            return None;
        }
        Some(CStr::from_ptr(ptr).to_string_lossy().into_owned())
    }
}

/// Reads file paths currently on the clipboard (macOS). Returns None when the
/// clipboard holds no file selection.
#[cfg(target_os = "macos")]
pub(crate) fn read_clipboard_file_paths() -> Option<Vec<String>> {
    unsafe {
        let pool: id = msg_send![class!(NSAutoreleasePool), new];
        let result = (|| {
            let pb: id = msg_send![class!(NSPasteboard), generalPasteboard];
            if pb == nil {
                return None;
            }
            let cflag = std::ffi::CString::new("NSFilenamesPboardType").ok()?;
            let ftype: id = msg_send![class!(NSString), stringWithUTF8String: cflag.as_ptr()];
            let plist: id = msg_send![pb, propertyListForType: ftype];
            if plist == nil {
                return None;
            }
            let count: usize = msg_send![plist, count];
            if count == 0 {
                return None;
            }
            let mut paths = Vec::with_capacity(count);
            for i in 0..count {
                let s: id = msg_send![plist, objectAtIndex: i];
                if s == nil {
                    continue;
                }
                let ptr: *const c_char = msg_send![s, UTF8String];
                if ptr.is_null() {
                    continue;
                }
                paths.push(CStr::from_ptr(ptr).to_string_lossy().into_owned());
            }
            if paths.is_empty() {
                None
            } else {
                Some(paths)
            }
        })();
        let _: () = msg_send![pool, drain];
        result
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn read_clipboard_file_paths() -> Option<Vec<String>> {
    None
}

/// Writes the given file paths to the clipboard as file URLs (macOS), so a
/// subsequent paste in Finder or other apps pastes the actual files.
#[cfg(target_os = "macos")]
pub(crate) fn write_clipboard_file_paths(paths: &[String]) -> bool {
    unsafe {
        let pool: id = msg_send![class!(NSAutoreleasePool), new];
        let ok = (|| {
            let pb: id = msg_send![class!(NSPasteboard), generalPasteboard];
            if pb == nil {
                return false;
            }
            let _: () = msg_send![pb, clearContents];
            let array: id = msg_send![class!(NSMutableArray), array];
            for p in paths {
                let cstr = match std::ffi::CString::new(p.as_str()) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let ns_path: id = msg_send![class!(NSString), stringWithUTF8String: cstr.as_ptr()];
                let url: id = msg_send![class!(NSURL), fileURLWithPath: ns_path];
                if url != nil {
                    let _: () = msg_send![array, addObject: url];
                }
            }
            let count: usize = msg_send![array, count];
            if count == 0 {
                return false;
            }
            let wrote: bool = msg_send![pb, writeObjects: array];
            wrote
        })();
        let _: () = msg_send![pool, drain];
        ok
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn write_clipboard_file_paths(_paths: &[String]) -> bool {
    false
}
