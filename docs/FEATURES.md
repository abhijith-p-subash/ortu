# Features

A complete tour of what Ortu can do.

## Capture

- **Text, images, and files.** Each history entry has a `content_type` of
  `text`, `image`, or `files`.
- **Background listener.** A lightweight watcher records new clipboard content.
  On macOS it uses the pasteboard change counter so it stays idle (low CPU)
  until something actually changes.
- **De-duplication.** Re-copying the same content updates the existing entry's
  timestamp instead of creating duplicates.
- **Large-content guard.** Extremely large payloads are skipped to keep the DB
  small and the UI responsive.

## Organize

- **Auto-grouping.** A rule-based classifier inspects each clip and assigns it to
  one or more groups (e.g. `URL`, `Code`, `JSON`, `Shell`, `Email`, `Secret`,
  `Path`, …) with a confidence score.
- **User groups.** Create your own groups and assign items to them. An item can
  belong to multiple groups.
- **Pinning.** Pin items you want to keep regardless of retention.
- **System vs user groups.** Auto-generated groups are "system" groups; the ones
  you create are "user" groups. User-grouped items are always retained.

## Find

- **Full-text search (FTS5).** Searches are backed by a SQLite FTS5 index and
  then re-ranked with a fuzzy matcher for relevance.
- **Filters.** Use `group:<name>` to scope to a group, plus type filters for
  text / images / URLs.
- **Keyboard-first.** Arrow keys to move, Enter to copy, number keys for instant
  copy by position.

## Paste stack (multi-paste)

Queue several clips and paste them out **in order**, across any apps:

1. Add items to the stack (button, in-app shortcut, or the global
   "copy selection to stack" hotkey).
2. Switch to your target app.
3. Press the paste-stack hotkey repeatedly — each press pastes the next item.

See [SHORTCUTS.md](SHORTCUTS.md) for the exact keys.

## Snippets & transforms

- **Snippets.** Save reusable text with variables: `{{date}}`, `{{time}}`,
  `{{datetime}}`, `{{clipboard}}`, and custom date formats like
  `{{date:%Y-%m-%d}}`.
- **Transforms / "Copy as".** Transform a clip on the way to the clipboard:
  trim, UPPERCASE, lowercase, slugify, pretty/minify JSON, Base64
  encode/decode, URL encode/decode.

## Privacy & sensitive data

- **Secret detection.** The classifier flags likely secrets (API keys, tokens,
  SSH keys, JWTs, env vars, etc.).
- **Auto-mask.** When enabled, detected secrets are masked in the UI and
  **encrypted at rest** with AES‑256‑GCM. Reveal them on demand.
- **Local-only.** Nothing is uploaded; there is no telemetry.

Details: [PRIVACY_AND_SECURITY.md](PRIVACY_AND_SECURITY.md).

## History retention

Choose how long history is kept (Settings → History):

| Mode | Behavior |
|------|----------|
| **On reboot** (default) | Clears ungrouped & unpinned items every time the computer reboots |
| **Forever** | Never auto-clears |
| **7 / 30 / 90 days** | Removes ungrouped & unpinned items older than N days |
| **Max items** | Caps the number of ungrouped & unpinned items kept |

Pinned items and items in a user group are **always** kept. See
[CONFIGURATION.md](CONFIGURATION.md#history-retention).

## Windows & UI

- **Main window** — full history browser with groups, search, snippets, and
  backup/restore.
- **Quick popup** — a small always-on-top picker summoned by a global hotkey,
  lazily created on first use for a lighter footprint.
- **Settings page** — appearance, privacy, retention, and shortcut customization.

## Backup, restore & export

- **JSON backup/restore** (merge or replace).
- **Plain-text export** of a single group or the whole history.
- **Group import** from a text file.

## System integration

- **Global hotkeys** — open popup, copy-to-stack, paste-next; all rebindable.
- **Auto-start on login.**
- **Tray icon** with show/quit.
- **Auto-update** via the Tauri updater plugin (GitHub Releases).
- **Themes** — light / dark / system, with a native-matching titlebar.
