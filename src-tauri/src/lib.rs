mod db;
mod clipboard;
mod commands;

use db::ClipboardDB;
use tauri::Manager;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri_plugin_global_shortcut::{Code, Modifiers, ShortcutState, Shortcut};
use std::thread;
use std::time::Duration;

#[cfg(target_os = "macos")]
use cocoa::appkit::{
    NSApplication, NSApp, NSWindow, NSWindowCollectionBehavior,
};
#[cfg(target_os = "macos")]
use cocoa::base::{id, YES};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // ---------------- DB INIT ----------------
            let db = ClipboardDB::new(app.handle())?;
            db.clear_ephemeral_on_start()?;
            app.manage(db);

            // ---------------- TRAY ----------------
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let show_i = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => app.exit(0),
                    "show" => show_window(app),
                    _ => {}
                })
                .build(app)?;

            // macOS: hide dock icon
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // ---------------- WINDOW FOCUS HANDLER ----------------
            if let Some(window) = app.get_webview_window("main") {
                let w = window.clone();
                window.on_window_event(move |e| {
                    if let tauri::WindowEvent::Focused(false) = e {
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

            // ---------------- GLOBAL SHORTCUT (ALT + V) ----------------
            let shortcut = Shortcut::new(Some(Modifiers::ALT), Code::KeyV);
            app.handle().plugin(
                tauri_plugin_global_shortcut::Builder::new()
                    .with_shortcuts(vec![shortcut])?
                    .with_handler(|app, s, e| {
                        if e.state == ShortcutState::Pressed
                            && s.matches(Modifiers::ALT, Code::KeyV)
                        {
                            show_or_toggle(app);
                        }
                    })
                    .build(),
            )?;

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_positioner::init())
        .invoke_handler(tauri::generate_handler![
            commands::get_history,
            commands::delete_entry,
            commands::toggle_permanent,
            commands::set_category,
            commands::get_categories,
            commands::paste_item,
            commands::manual_cleanup,
            commands::close_window
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri app");
}

// ======================================================
// WINDOW HELPERS
// ======================================================

fn show_or_toggle(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            configure_and_show(app, &window);
        }
    }
}

fn show_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        configure_and_show(app, &window);
    }
}

fn configure_and_show(_app: &tauri::AppHandle, window: &tauri::WebviewWindow) {
    #[cfg(target_os = "macos")]
    unsafe {
        let ns_window = window.ns_window().unwrap() as id;

        // Join all Spaces (including fullscreen Spaces)
        let mut behavior = ns_window.collectionBehavior();
        behavior |= NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces;
        behavior |= NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary;
        behavior |= NSWindowCollectionBehavior::NSWindowCollectionBehaviorStationary;
        behavior |= NSWindowCollectionBehavior::NSWindowCollectionBehaviorIgnoresCycle;
        ns_window.setCollectionBehavior_(behavior);

        // Correct floating overlay level (NSStatusWindowLevel = 25)
        // NSFloatingWindowLevel is 3, which is often not enough for full screen apps
        ns_window.setLevel_(25);

        // Activate app (steal focus)
        let app = NSApp();
        app.activateIgnoringOtherApps_(YES);
    }

    let _ = window.show();
    let _ = window.set_focus();

    // Center window
    let _ = tauri_plugin_positioner::WindowExt::move_window(
        window,
        tauri_plugin_positioner::Position::Center,
    );
}
