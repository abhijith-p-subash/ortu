# Windows Crash Postmortem (Ortu)

## Context

- Symptom on Windows installer builds (`.nsis` / `.msi`): app opens briefly, then closes/crashes.
- macOS `.dmg` builds continued to work.
- Primary failure pattern in Event Viewer:
  - `Exception code: 0xc0000409`
  - Faulting module: `Ortu.exe`
- Repro pattern varied by build stage:
  - Sometimes failed before app `setup` ran.
  - Sometimes reached setup, then failed in native callback path.

---

## Root Causes Identified

## 1) Logger plugin startup crash (early phase)

- Error reproduced in Rust startup:
  - `PluginInitialization("log", "Cannot create a file when that file already exists. (os error 183)")`
- Impact:
  - Could crash on startup after first successful run because log file already existed.
- Fix:
  - Removed/avoided crash-prone log plugin initialization path from startup flow.

## 2) Hidden window startup behavior made app look dead

- Main window configured hidden during some startup flows.
- If startup path partially failed, process could be alive while no visible window appeared.
- Fix:
  - Defaulted normal launch path to show main window.
  - Kept explicit hidden launch behavior only for explicit hidden args.

## 3) Tray/menu initialization instability on Windows

- Multiple crash signatures implicated tray/menu native path.
- Observation matched user report:
  - Sometimes tray appeared and app worked.
  - Often app crashed while tray/menu setup was happening.
- Fixes:
  - Made tray creation non-fatal where possible.
  - Reworked startup to reduce early fragile paths.
  - Later reintroduced tray with guarded initialization once core stability improved.

## 4) WebView2 native startup instability (major recurring cause)

- Panic logs repeatedly showed:
  - `panic in a function that cannot unwind`
  - Frames around `CreateWebViewEnvironmentWithOptionsInternal` / native callback path.
- `startup.log` showed many failed launches stopping at `run: builder init` (pre-setup).
- Fixes:
  - Set `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--disable-gpu`.
  - Forced writable WebView2 user data location under `%TEMP%`.
  - Changed to per-process WebView2 user data folder to avoid profile lock contention on rapid relaunches.
  - Removed static secondary popup webview from startup config and created popup lazily at runtime.

## 5) Missing plugin state regressions (mid-debug regression)

- During isolation, some plugins were temporarily removed, causing frontend command/state mismatch and secondary crashes.
- Fix:
  - Restored required plugins (`opener`, `positioner`, `dialog`, `os`) after isolating root causes.

## 6) DB startup robustness issues

- Startup path had `expect` and many `lock().unwrap()` points.
- Any upstream panic/poison scenario could trigger hard failure.
- Fixes:
  - Replaced panic-prone startup path with error propagation/fallback.
  - Added DB open fallback chain:
    - app data DB path
    - `%TEMP%` DB path
    - in-memory DB as last resort
  - Reduced hard-panic lock paths by returning errors where possible.

---

## Final Architecture/Behavior After Fixes

- Windows startup:
  - Configures WebView2 environment at process start.
  - Uses per-process WebView2 profile folder.
  - Avoids static popup webview creation in config.
  - Creates popup window lazily (`ensure_popup_window`) when needed.
- Restored product features:
  - Tray icon
  - Close main window to tray behavior
  - Global shortcut `Alt+V` popup behavior
  - Autostart enable logic
- macOS behavior:
  - Preserved via non-Windows popup creation/setup path.
  - macOS-specific popup/native code remains intact.

---

## Key Files Touched

- `src-tauri/src/lib.rs`
  - Startup hardening
  - Panic/startup tracing
  - WebView2 env configuration
  - Tray + global shortcut + autostart integration logic
  - Dynamic popup creation path
- `src-tauri/src/db.rs`
  - DB init fallback logic
  - lock/error hardening
- `src-tauri/tauri.conf.json`
  - Window configuration changes (removed static popup definition for stable Windows boot)
  - CSP and version updates during debugging
- `src-tauri/Cargo.toml`
  - `tauri` feature toggles (`tray-icon` adjusted during investigation and restored)
  - version bumps
- `package.json`
  - version bumps
- `%TEMP%\\ortu\\startup.log` and `%TEMP%\\ortu\\panic.log`
  - runtime diagnostics used for root-cause isolation

---

## Debugging Signals That Were Most Useful

- Event Viewer `Application Error` entries:
  - consistent `0xc0000409` with stable fault offsets between builds
- `startup.log` phase markers:
  - whether crash occurred pre-setup or post-setup
- `panic.log` native frames:
  - WebView2 callback/unwind path
- Comparing installed exe version/timestamp:
  - confirmed whether tested build was actually the latest installer output

---

## Hotspots To Monitor

- `src-tauri/src/lib.rs` startup sequence
  - tray/menu init
  - global shortcut registration
  - autostart enable path
  - popup creation timing
- WebView2 env:
  - `WEBVIEW2_USER_DATA_FOLDER` and folder permissions
  - GPU path on machines with unstable graphics stack
- DB path permissions:
  - app data directory permission failures on some Windows systems
- Any reintroduction of panic-prone APIs:
  - `expect`, `unwrap`, or fatal plugin init in startup path

---

## Recommended Ongoing Safeguards

1. Keep panic/startup logs enabled for installer builds.
2. Add a minimal startup smoke test checklist for Windows packaging:
   - launch app
   - tray appears
   - close-to-tray works
   - `Alt+V` works
   - reboot + autostart works
3. Keep popup lazy-created on Windows unless WebView2 dual-webview startup is proven stable across machines.
4. Avoid moving optional native integrations into pre-setup critical path without guards.
5. Preserve version bumps during crash investigations to prevent stale-installer confusion.

---

## Outcome

- Core crash loop was broken and app became launch-stable.
- Feature regressions introduced during stabilization (tray/background/shortcut/autostart) were then reintroduced with safer startup handling.
- macOS path was kept compatible by preserving non-Windows popup/native behavior.

