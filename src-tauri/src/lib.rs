#![allow(unexpected_cfgs)]
mod clipboard;
mod commands;
mod db;

use db::ClipboardDB;
use std::thread;
use std::time::Duration;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::Manager;
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutState};

#[cfg(target_os = "macos")]
use cocoa::appkit::NSApp;
#[cfg(target_os = "macos")]
use cocoa::base::{id, nil, YES};
#[cfg(target_os = "macos")]
use objc::{msg_send, sel, sel_impl};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }))
        // ---------------- CRITICAL FIX: Move Global Shortcut to Builder Chain (Safe Init) ----------------
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_shortcuts(vec![Shortcut::new(Some(Modifiers::ALT), Code::KeyV)])
                .expect("Failed to register global shortcut")
                .with_handler(|app, s, e| {
                    if e.state == ShortcutState::Pressed && s.matches(Modifiers::ALT, Code::KeyV) {
                        toggle_popup(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
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
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }

            // ---------------- DB INIT ----------------
            let db = ClipboardDB::new(app.handle())?;
            db.clear_ephemeral_on_start()?;
            app.manage(db);

            // ---------------- TRAY ----------------
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let show_i = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            // ---------------- CRITICAL FIX: Safe Tray Icon Unwrap ----------------
            let icon = app
                .default_window_icon()
                .cloned()
                .unwrap_or_else(|| tauri::image::Image::new(&[], 0, 0));

            TrayIconBuilder::new()
                .icon(icon)
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => app.exit(0),
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    _ => {}
                })
                .build(app)?;

            // macOS: Run as background accessory app (no dock icon, stays in menu bar)
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // ---------------- MAIN WINDOW SETUP ----------------
            // Prevent the app from quitting when the main window is closed
            if let Some(window) = app.get_webview_window("main") {
                let w = window.clone();
                window.on_window_event(move |e| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = e {
                        // Prevent the window from closing and hide instead
                        api.prevent_close();
                        let _ = w.hide();
                    }
                });
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
                    println!("Autostart not enabled. Enabling...");
                    match autostart_manager.enable() {
                        Ok(_) => println!("Autostart enabled successfully."),
                        Err(e) => eprintln!("Failed to enable autostart: {}", e),
                    }
                }
            } else {
                eprintln!("Failed to check autostart status");
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
            commands::manual_cleanup,
            commands::close_window,
            commands::backup_data,
            commands::restore_data,
            commands::add_to_group,
            commands::remove_from_group,
            commands::export_all_txt
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri app");
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
