//! Keeps the OS "launch at login" entry pointing at the build that is running.
//!
//! `tauri-plugin-autostart`'s `is_enabled()` only answers whether *an* entry
//! exists — on every platform `auto-launch` ignores where that entry points
//! (Linux and macOS just stat the file, Windows only checks the registry value
//! is present). So an entry written by a `tauri dev` run, by an older install,
//! or by an AppImage that has since been moved or upgraded survives forever,
//! and at login the desktop launches whatever stale path it names.
//!
//! When that stale path is a dev binary the failure is loud: dev builds have the
//! Vite dev-server URL compiled in, nothing is listening on it that early after
//! login, and the window comes up showing "Could not connect to localhost:
//! Connection refused" (ortu#20, upstream tauri-apps/plugins-workspace#2063).
//!
//! Two guards close that off: debug builds never register themselves, and every
//! release launch re-points a stale entry at the current executable.

use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;

use crate::startup_trace;

/// Passed by the startup entry so a login launch goes straight to the tray
/// instead of raising the main window. Read back out during `setup`.
pub const HIDDEN_FLAG: &str = "--hidden";

/// Whether the OS startup entry launched this process.
pub fn launched_hidden() -> bool {
    std::env::args().any(|arg| arg == HIDDEN_FLAG)
}

/// Creates the startup entry when it is missing and rewrites it when it no
/// longer points at this build. No-op on debug builds, which must never claim
/// the user's login — see the module docs.
pub fn sync(app: &AppHandle) {
    if cfg!(debug_assertions) {
        startup_trace("autostart: debug build, startup entry left untouched");
        return;
    }

    let manager = app.autolaunch();
    let registered = match manager.is_enabled() {
        Ok(registered) => registered,
        Err(e) => {
            log::error!("Failed to check autostart status: {e}");
            startup_trace(&format!("autostart: status unreadable: {e}"));
            return;
        }
    };

    let stale = registered && entry_is_stale(app);
    if registered && !stale {
        return;
    }

    // `enable` truncates/overwrites the existing entry on all three backends,
    // so a stale one needs no separate removal.
    match manager.enable() {
        Ok(()) => startup_trace(if stale {
            "autostart: startup entry re-pointed at this build"
        } else {
            "autostart: startup entry created"
        }),
        Err(e) => {
            log::error!("Failed to enable autostart: {e}");
            startup_trace(&format!("autostart: enable failed: {e}"));
        }
    }
}

/// True only when the entry is readable *and* demonstrably wrong: it names a
/// different binary, or predates [`HIDDEN_FLAG`]. An entry we cannot read or
/// cannot compare against is left alone, so no layout we did not anticipate can
/// put the app into a rewrite-every-launch loop.
fn entry_is_stale(app: &AppHandle) -> bool {
    let Some(entry) = read_entry(app) else {
        return false;
    };
    let targets = target_paths(app);
    if targets.is_empty() {
        return false;
    }
    !targets.iter().any(|target| entry.contains(target)) || !entry.contains(HIDDEN_FLAG)
}

/// The path(s) `tauri-plugin-autostart` could have written for this process: the
/// AppImage when running from one, otherwise the executable. Both the raw and
/// the canonicalized executable path count as a match, because the plugin writes
/// the raw path on Linux and the canonical one on macOS — comparing against only
/// one of them would make a symlinked install look permanently stale.
#[cfg_attr(not(target_os = "linux"), allow(unused_variables))]
fn target_paths(app: &AppHandle) -> Vec<String> {
    #[cfg(target_os = "linux")]
    if let Some(appimage) = app
        .env()
        .appimage
        .and_then(|p| p.to_str().map(str::to_string))
    {
        return vec![appimage];
    }

    let Ok(exe) = std::env::current_exe() else {
        return Vec::new();
    };

    let mut paths = Vec::new();
    if let Some(path) = exe.to_str() {
        paths.push(path.to_string());
    }
    if let Some(path) = exe
        .canonicalize()
        .ok()
        .and_then(|p| p.to_str().map(str::to_string))
    {
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
    paths
}

/// Reads the startup entry `tauri-plugin-autostart` maintains, as raw text.
#[cfg(target_os = "linux")]
fn read_entry(app: &AppHandle) -> Option<String> {
    let path = app
        .path()
        .home_dir()
        .ok()?
        .join(".config")
        .join("autostart")
        .join(format!("{}.desktop", app.package_info().name));
    std::fs::read_to_string(path).ok()
}

#[cfg(target_os = "macos")]
fn read_entry(app: &AppHandle) -> Option<String> {
    let path = app
        .path()
        .home_dir()
        .ok()?
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{}.plist", app.package_info().name));
    std::fs::read_to_string(path).ok()
}

#[cfg(target_os = "windows")]
fn read_entry(app: &AppHandle) -> Option<String> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};
    use winreg::RegKey;

    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run", KEY_READ)
        .ok()?
        .get_value::<String, _>(app.package_info().name.as_str())
        .ok()
}
