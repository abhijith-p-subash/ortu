#![allow(unexpected_cfgs)]
mod clipboard;
mod commands;
mod db;

use db::ClipboardDB;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use sysinfo::System;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::Manager;
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutState};

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }))
        // ---------------- CRITICAL FIX: Safe Global Shortcut Init ----------------
        // Initialize the plugin without shortcuts to prevent startup panics.
        // We register shortcuts safely in the setup hook.
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, s, e| {
                    if e.state == ShortcutState::Pressed && s.matches(Modifiers::ALT, Code::KeyV) {
                        toggle_popup(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            // ---------------- SAFE GLOBAL SHORTCUT REGISTRATION ----------------
            {
                use tauri_plugin_global_shortcut::GlobalShortcutExt;
                let shortcut = Shortcut::new(Some(Modifiers::ALT), Code::KeyV);
                if let Err(e) = app.global_shortcut().register(shortcut) {
                    log::error!("Failed to register global shortcut: {}", e);
                    eprintln!("Failed to register global shortcut: {}", e);
                }
            }

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

            if !is_hidden {
                show_main_window(app.handle());
            }

            // ---------------- DB INIT ----------------
            let db = ClipboardDB::new(app.handle())?;
            let boot_session_id = current_boot_session_id();
            let _ = db.clear_ephemeral_on_boot_change(&boot_session_id)?;
            app.manage(db);
            app.manage(PopupPasteTarget(Mutex::new(None)));

            // ---------------- TRAY ----------------
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let show_i = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            let tray_enabled = if let Some(icon) = app.default_window_icon().cloned() {
                TrayIconBuilder::new()
                    .icon(icon)
                    .menu(&menu)
                    .on_menu_event(|app, event| match event.id.as_ref() {
                        "quit" => app.exit(0),
                        "show" => show_main_window(app),
                        _ => {}
                    })
                    .build(app)?;
                true
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
            }

            if tray_enabled {
                if let Some(window) = app.get_webview_window("main") {
                    let w = window.clone();
                    window.on_window_event(move |e| {
                        if let tauri::WindowEvent::CloseRequested { api, .. } = e {
                            // Prevent the window from closing and hide instead.
                            // This requires a tray icon path to bring the app back.
                            api.prevent_close();
                            let _ = w.hide();
                        }
                    });
                }
            }

            // ---------------- POPUP WINDOW SETUP ----------------
            if let Some(window) = app.get_webview_window("popup") {
                let w = window.clone();
                #[cfg(target_os = "macos")]
                setup_mac_popup(&w);

                window.on_window_event(move |e| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = e {
                        // Also prevent popup from closing the app
                        api.prevent_close();
                        let _ = w.hide();
                    } else if let tauri::WindowEvent::Focused(false) = e {
                        let _ = w.hide();
                    }
                });
            }

            // ---------------- CLIPBOARD LISTENER ----------------
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
            commands::list_snippets,
            commands::save_snippet,
            commands::delete_snippet,
            commands::render_snippet,
            commands::transform_content
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri app");
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
            RawWindowHandle::Win32(h) => HWND(h.hwnd.get() as isize),
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

fn toggle_popup(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("popup") {
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
