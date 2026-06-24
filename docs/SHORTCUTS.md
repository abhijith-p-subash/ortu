# Keyboard Shortcuts

Ortu is keyboard-first. Shortcut labels adapt to your OS (⌘/⌥/⇧ on macOS,
Ctrl/Alt/Shift elsewhere). The labels and defaults live in one place:
`src/lib/shortcuts.ts`.

## Global shortcuts (work anywhere)

These are registered system-wide and are **rebindable** in
**Settings → Global Shortcuts** (with a one-click *Restore defaults*).

| Action | Default (macOS) | Default (Win/Linux) | `app_meta` key |
|--------|-----------------|---------------------|----------------|
| Open quick popup | `⌥V` | `Alt+V` | `shortcut_open_popup` |
| Copy selection to stack (any app) | `⌘⇧C` | `Ctrl+Shift+C` | `shortcut_copy_stack` |
| Paste next item from stack | `⌥⇧V` | `Alt+Shift+V` | `shortcut_paste_stack` |

**Copy-to-stack** sends the OS copy command to the focused app, then enqueues the
result. **Paste-next-from-stack** pastes the next queued item into the focused
app. Together they let you collect several things and paste them in sequence.

### Rebinding

1. Open **Settings → Global Shortcuts**.
2. Click a shortcut and press the new combination (must include a modifier).
3. If the combo is already taken by the OS or another app, the change is
   rejected and the previous binding is restored.

Bindings are validated as Tauri accelerator strings (e.g.
`CommandOrControl+Shift+C`) and stored in `app_meta`.

## In-app shortcuts (main window)

| Action | macOS | Win/Linux |
|--------|-------|-----------|
| Move selection | `↑` / `↓` | `↑` / `↓` |
| Copy selected item | `Enter` / click | `Enter` / click |
| Quick copy by position | `⌘1`–`⌘9` | `Ctrl+1`–`Ctrl+9` |
| Delete item | `Del` / `⌘⌫` | `Del` / `Ctrl+Backspace` |
| Pin / unpin | `⌘P` | `Ctrl+P` |
| Add to group | `⌘C` | `Ctrl+C` |
| Add selected item to stack | `⌘S` | `Ctrl+S` |
| Manage groups | `⌘G` | `Ctrl+G` |
| Save (in editors) | `⌘↵` | `Ctrl+Enter` |
| Close / dismiss | `Esc` | `Esc` |

## Quick popup

| Action | Key |
|--------|-----|
| Paste highlighted item | `Enter` |
| Hide popup | `Esc` |
| Quick copy by position | `⌘`/`Ctrl` + `1`–`9` |

## Notes

- The in-app shortcuts above are currently fixed (not rebindable). Only the three
  **global** hotkeys are customizable today.
- The Help dialog (in-app) always reflects your current global bindings.
