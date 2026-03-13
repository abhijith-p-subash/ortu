# macOS Direct Paste Investigation

This document records the investigation, root causes, fixes, and operational guidance for Ortu's macOS "direct paste from popup" behavior.

## Goal

Match Raycast-style behavior on macOS:

1. User is working in another app, for example VS Code.
2. User opens the Ortu popup.
3. User selects a clipboard item.
4. Ortu closes the popup, switches back to the previously active app, and pastes immediately.

## Expected Behavior

When the popup is opened from a foreground app:

1. Ortu must remember the exact app that was frontmost.
2. On selection, Ortu must restore the selected clipboard item.
3. Ortu must reactivate the original target app.
4. Ortu must send `Cmd+V`.
5. Ortu must stay alive in the background after paste.

## Initial Symptoms

The feature behaved differently depending on how the app was run:

- `npm run tauri dev`: direct paste worked.
- built app after DMG install: direct paste failed.
- built app after DMG install and manual trust steps: app later crashed on popup paste.

## Distribution Context

The app is currently distributed on macOS without Apple notarization and without a paid Apple Developer account. The install flow used for testing was:

```bash
xattr -dr com.apple.quarantine "/Applications/Ortu.app"
codesign --force --deep --sign - "/Applications/Ortu.app"
open "/Applications/Ortu.app"
```

After that, Accessibility permission was enabled manually in:

- `System Settings -> Privacy & Security -> Accessibility`

This context matters because macOS treats:

- dev builds
- ad-hoc signed builds
- quarantine-removed builds
- installed `/Applications/Ortu.app`

as different trust and permission scenarios.

## Investigation Timeline

### Stage 1: Wrong target app behavior

Earlier popup paste behavior could return to the wrong app because it relied on a generic app switching pattern instead of restoring the exact app that had focus before the popup opened.

### Stage 2: AppleScript keystroke permission failure

Logs showed:

- `System Events got an error: osascript is not allowed to send keystrokes. (1002)`

This proved the installed app could not rely on:

- `osascript`
- `System Events`
- `keystroke "v" using {command down}`

That path was too fragile for the installed build.

### Stage 3: Installed build still unstable

Even after replacing the AppleScript keystroke path, the installed app still failed during popup paste. At that point the evidence suggested:

- focus handoff timing
- installed app permission state
- or a release-only crash in the event injection path

### Stage 4: Crash report inspection

The decisive step was checking:

- `~/Library/Logs/DiagnosticReports`

Latest crash report:

- `Ortu-2026-03-13-212806.ips`

## Confirmed Crash Root Cause

The crash report showed:

- crash type: `EXC_BREAKPOINT / SIGTRAP`
- faulting thread: `tokio-runtime-worker`
- failing stack included:
  - `enigo::platform::macos_impl::create_string_for_key`
  - `keycode_to_string`
  - `get_layoutdependent_keycode`
  - `send_paste_shortcut`

This proved the installed app was crashing inside `enigo` while trying to resolve:

- `Key::Unicode('v')`

On macOS, `Key::Unicode('v')` triggers keyboard layout lookup through HIToolbox/AppKit. In this case that path trapped on the worker thread and terminated the app.

## Important Conclusion

The installed macOS crash was not caused by:

- Svelte popup UI
- SQLite
- clipboard history DB
- grouping logic
- popup hover preview
- tray behavior

The hard crash came specifically from macOS keyboard event generation through `enigo` using `Unicode('v')`.

## Secondary Root Causes

There were also two important non-crash issues:

### 1. Installed app Accessibility trust is distinct from dev

The direct-paste feature worked in `tauri dev` but not in the installed build because macOS permission state differs per app identity and path.

The installed app needed explicit Accessibility trust.

### 2. Popup flow had multiple close/hide paths

The popup was being hidden by:

- frontend `close_window`
- backend hide during paste flow
- popup `Focused(false)` handler

This created an unnecessary race during focus handoff.

## Fixes Implemented

### 1. Deterministic popup target restore

Before showing the popup, Ortu now stores the exact frontmost app bundle id on macOS.

This allows paste to return to the correct target app instead of relying on a generic app switch.

Relevant file:

- `src-tauri/src/lib.rs`

### 2. AppleScript only for activation, not for keystrokes

AppleScript `System Events` keystroke injection was removed from the paste path.

AppleScript is used only to:

- activate the previously focused app by bundle id

Actual paste keypress is generated separately.

Relevant file:

- `src-tauri/src/commands.rs`

### 3. Accessibility permission check and UX

Added macOS-specific checks so Ortu can detect whether Accessibility permission is granted.

Added commands:

- `get_macos_accessibility_status`
- `open_macos_accessibility_settings`

Added main-window UX:

- a macOS banner when Accessibility is missing
- `Open Settings` button
- `Refresh` button to re-check permission

Relevant files:

- `src-tauri/src/commands.rs`
- `src-tauri/src/lib.rs`
- `src/routes/+page.svelte`

### 4. Popup close race reduction

Removed the redundant frontend popup close before paste so that the backend owns the popup paste lifecycle more cleanly.

Relevant file:

- `src/routes/popup/+page.svelte`

### 5. Added backend logging

Logging was added around the popup paste sequence to make release debugging easier:

- popup paste requested
- popup hidden
- clipboard restored
- target app bundle id
- paste shortcut sent

Relevant file:

- `src-tauri/src/commands.rs`

### 6. Crash fix for `Cmd+V`

This was the critical fix.

Changed macOS paste from:

```rust
Key::Unicode('v')
```

to:

```rust
Key::Other(9)
```

Reason:

- `9` is the fixed macOS virtual key code for `V`
- avoids keyboard layout lookup
- avoids the `enigo` HIToolbox/AppKit crash path

Relevant file:

- `src-tauri/src/commands.rs`

## Current macOS Popup Paste Flow

The current intended flow is:

1. User opens popup while working in another app.
2. Ortu records the frontmost app bundle id.
3. User selects an item from popup.
4. Backend hides popup.
5. Backend restores clipboard payload.
6. Backend reactivates the recorded target app.
7. Backend waits briefly for focus handoff.
8. Backend sends `Cmd+V` using:
   - `Meta` key
   - fixed virtual keycode `9` for `V`
   - `Meta` release

## Why Dev and Installed Builds Behaved Differently

The discrepancy between `tauri dev` and installed `.app` is expected on macOS because these differ in:

- bundle path
- code-signing identity
- quarantine state
- TCC permission association
- runtime trust context

So it is possible for:

- dev build to work
- installed build to fail
- installed build to require separate Accessibility approval

without the underlying feature logic being completely broken.

## Operational Requirement for Installed macOS Builds

For current distribution, users may still need:

```bash
xattr -dr com.apple.quarantine "/Applications/Ortu.app"
codesign --force --deep --sign - "/Applications/Ortu.app"
open "/Applications/Ortu.app"
```

And then:

- enable `Ortu` in `System Settings -> Privacy & Security -> Accessibility`

Without notarization, this remains part of the support burden for public macOS distribution.

## Files Touched During This Investigation

Core backend:

- `src-tauri/src/commands.rs`
- `src-tauri/src/lib.rs`

Popup UI:

- `src/routes/popup/+page.svelte`

Main window UI:

- `src/routes/+page.svelte`

## Validation Performed

The implemented changes were repeatedly validated with:

- `cargo check`
- `npm run check`

Additionally, the macOS crash diagnosis was confirmed using the generated crash report in:

- `~/Library/Logs/DiagnosticReports`

## Current Status

Implemented:

- deterministic target app tracking
- removal of AppleScript keystroke dependency for paste
- macOS Accessibility detection and user guidance
- popup flow cleanup
- crash-safe macOS paste key strategy using fixed virtual keycode

The most important expected outcome is:

- installed macOS builds should no longer crash during popup paste because the `Unicode('v')` layout lookup path has been removed

## Recommended Next Steps

1. Rebuild and retest the installed DMG after the `Key::Other(9)` fix.
2. Update release notes with the macOS direct-paste fix and Accessibility requirement.
3. Add a dedicated macOS troubleshooting entry to the public documentation.
4. When possible, move to proper signed + notarized macOS releases to reduce support friction.

## Suggested Release Note Summary

Recommended short summary for GitHub release:

> Fixed a macOS crash during popup direct paste in installed builds. Ortu now uses a safer macOS key injection path for `Cmd+V`, improves popup-to-target-app handoff, and adds clearer Accessibility guidance for direct paste into other apps.
