# Changelog

All notable changes to Ortu are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.0.0] - Unreleased

Major release focused on capability, privacy, and performance.

### Added
- **Image & file capture** — clipboard history now records images and file
  selections, with thumbnails and a content-addressed blob store for de-duped
  image storage.
- **FTS5 full-text search** — fast search backed by a SQLite FTS5 index plus a
  fuzzy re-ranker.
- **Paste stack (multi-paste)** — queue multiple clips and paste them in order,
  including a global **copy-selection-to-stack** hotkey and **paste-next** hotkey.
- **Sensitive-data protection** — detected secrets can be masked and **encrypted
  at rest** (AES‑256‑GCM), revealed on demand.
- **Customizable global shortcuts** — rebind the global hotkeys in Settings with
  one-click restore-to-defaults.
- **Configurable history retention** — *On reboot* (default), *Forever*, age
  (7/30/90 days), and a max-items cap. Pinned & grouped items are always kept.
- **Dedicated Settings page** (appearance, privacy, retention, shortcuts),
  replacing the old menu-driven settings modal.
- **Themes** — light, dark, or follow system.
- **Pause/resume capture** — a header status pill toggles clipboard monitoring
  (with a live clip count); the paused state is persisted across restarts.
- **Unified notification (toast) system** across all windows.
- **Snippets with variables** and **"Copy as" transforms**.

### Changed
- **Performance & footprint** — lazy popup-window creation, trimmed
  dependencies, SQLite pragma tuning, and a macOS idle-capture fast path; plus an
  optimized release build profile (LTO, single codegen unit, stripped).
- **Default history behavior** — *On reboot* clears only ungrouped & unpinned
  items; pinned and grouped items persist.
- Aligned Rust crate versions with their npm counterparts (`tauri` 2.11,
  `tauri-plugin-updater` 2.10).
- Expanded documentation (`docs/`), README, and project metadata.

### Fixed
- **Titlebar legibility** — the native titlebar now follows the active theme, so
  the window title stays readable in light mode (previously invisible on macOS
  when the app theme and system appearance disagreed).
- **Windows titlebar color** — the caption bar now matches the app body color
  per theme instead of always rendering a dark bar.
- Removed the redundant in-app logo/title; the native titlebar is now the single
  source of branding, and the header's top spacing is platform-aware.

### Notes
- macOS requires Accessibility permission for direct paste into other apps.
- Unsigned macOS builds trigger Gatekeeper warnings; see the README workaround.

## [1.1.1] - 2025

- Maintenance and stability fixes.

## [1.1.0] - 2025

- Groups and smart auto-grouping; search and filtering improvements.

## [1.0.3] - 2025

- Bug fixes and packaging improvements.

## [1.0.0] - 2025

- Initial public release: text clipboard history, pinning, quick-access popup,
  snippets, backup/restore, local SQLite storage.

[2.0.0]: https://github.com/abhijith-p-subash/ortu/releases
[1.1.1]: https://github.com/abhijith-p-subash/ortu/releases/tag/v1.1.1
[1.1.0]: https://github.com/abhijith-p-subash/ortu/releases/tag/v1.1.0
[1.0.3]: https://github.com/abhijith-p-subash/ortu/releases/tag/v1.0.3
[1.0.0]: https://github.com/abhijith-p-subash/ortu/releases/tag/v1.0.0
