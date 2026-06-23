# Roadmap

A living, non-binding list of where Ortu is headed. Have an idea? Open a
[discussion or issue](https://github.com/abhijith-p-subash/ortu/issues).

## Shipped (v2.0)

- Multi-format capture (text, images, files)
- Pause/resume capture from the header (persisted across restarts)
- Auto-grouping classifier with confidence scoring
- FTS5 full-text search + fuzzy re-ranking
- Paste stack (multi-paste) with global copy-to-stack and paste-next hotkeys
- Sensitive-data masking with field-level AES‑256‑GCM encryption
- Configurable history retention (On reboot / Forever / age / count)
- Customizable, rebindable global shortcuts with restore-to-defaults
- Light / dark / system themes with a native-matching titlebar
- Unified notification system
- Dedicated Settings page
- Performance & footprint work (lazy popup window, slimmer dependencies,
  SQLite pragma tuning, macOS idle-capture fast path)

## Candidate next

> Priorities are not committed; community input welcome.

- **Customizable in-app shortcuts** — extend rebinding beyond the global hotkeys.
- **Per-app paste rules** — auto-transform on paste depending on the target app.
- **Richer image/file previews** and quick actions.
- **Optional encrypted, self-hosted sync** (strictly opt-in, end-to-end).
- **Import/export improvements** — encrypted backups, more formats.
- **Accessibility** pass (screen-reader labels, focus order, high-contrast).
- **Internationalization (i18n)**.
- **Packaging** — Homebrew cask, winget, Flatpak/Snap.
- **Test coverage** — unit tests for the classifier and DB layer; e2e smoke tests.

## Known limitations / cleanups

- In-app navigation shortcuts are not yet rebindable (only the global ones are).
- Linux clipboard change detection is poll-based (no native event path yet).
- No automated test suite in CI yet (currently type/compile checks only).

See [CHANGELOG.md](../CHANGELOG.md) for released changes.
