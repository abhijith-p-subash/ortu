# Configuration

Most behavior is configured from the in-app **Settings** page (gear icon in the
header). Settings are stored locally in the `app_meta` key/value table inside the
SQLite database — there are no external config files to edit by hand.

## Settings reference

| Setting (UI) | `app_meta` key | Values | Default |
|--------------|----------------|--------|---------|
| Theme | _(stored client-side)_ | `system` \| `light` \| `dark` | `dark` |
| Auto-mask detected secrets | `auto_mask_secrets` | `0` \| `1` | `0` (off) |
| Keep history for | `retention_days` | `reboot` \| `0` \| `7` \| `30` \| `90` | `reboot` |
| Max items | `retention_max_items` | `0` (unlimited) \| `500` \| `1000` \| `5000` | `0` |
| Global shortcuts | `shortcut_<action>` | Tauri accelerator string | see [SHORTCUTS.md](SHORTCUTS.md) |

> Keys not listed here (e.g. `boot_session_id`, `fts_built`) are internal
> bookkeeping and should not be edited.

## History retention

The **Keep history for** setting controls how ephemeral history is cleaned up.
In every mode, **pinned items and items in a user group are always kept** — only
*ungrouped and unpinned* items are ever removed.

| Mode | What happens |
|------|--------------|
| **On reboot** (default) | On each OS reboot, all ungrouped & unpinned items are cleared (all content types). A normal app restart without a reboot clears nothing. |
| **Forever** | Nothing is auto-removed. |
| **7 / 30 / 90 days** | Ungrouped & unpinned items older than N days are removed (checked periodically and on settings change). |

The **Max items** cap independently limits how many ungrouped & unpinned items
are retained, keeping the newest.

How "on reboot" is detected: the app records the OS boot time. If it differs from
the stored value at launch, a reboot happened and the wipe runs once.

## Privacy / sensitive data

- **Auto-mask detected secrets** (`auto_mask_secrets`): when on, items the
  classifier flags as secrets are masked in the UI and **encrypted at rest**.
  You can reveal them on demand. See
  [PRIVACY_AND_SECURITY.md](PRIVACY_AND_SECURITY.md).

## Global shortcuts

The three global hotkeys are user-rebindable from Settings → Global Shortcuts,
with a one-click **Restore defaults**. They're persisted as Tauri accelerator
strings under `shortcut_open_popup`, `shortcut_copy_stack`, and
`shortcut_paste_stack`. Full list & defaults: [SHORTCUTS.md](SHORTCUTS.md).

## Storage locations

Ortu stores everything under the OS app-data directory for identifier
`com.ortu.clipboard`:

| OS | Path |
|----|------|
| macOS | `~/Library/Application Support/com.ortu.clipboard/` |
| Windows | `%APPDATA%\com.ortu.clipboard\` |
| Linux | `~/.local/share/com.ortu.clipboard/` (XDG data dir) |

Inside that directory:

| File | Purpose |
|------|---------|
| `ortu.db` (`-wal`, `-shm`) | SQLite database (history, groups, settings, blobs, FTS) |
| `.sensitive_key` | 256-bit key for sensitive-item encryption (file mode `0600`) |

> ⚠️ Deleting `.sensitive_key` makes previously encrypted (masked) items
> unreadable; everything else is unaffected. Include it when migrating between
> machines if you want masked items to remain readable.

## Backup & restore

- **Backup** writes a JSON file containing history + groups.
- **Restore** supports **merge** (keep existing, add new) or **replace**
  (clear, then import).
- **Export** writes plain text for a single group or the whole history.

Backups and text exports are **not encrypted** — treat exported files as
sensitive if your history contains secrets.

## Autostart & updates

- **Autostart on login** is enabled by the app on first run.
- **Auto-update** checks GitHub Releases via the Tauri updater plugin and applies
  signed updates. The update endpoint and public key live in `tauri.conf.json`.
