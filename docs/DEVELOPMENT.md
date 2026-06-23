# Development

How to set up Ortu locally and work on it.

## Prerequisites

- **Rust** (stable) — <https://www.rust-lang.org/tools/install>
- **Node.js** (LTS) — <https://nodejs.org/>
- Platform build dependencies:

| OS | Dependencies |
|----|--------------|
| macOS | Xcode Command Line Tools (`xcode-select --install`) |
| Windows | WebView2 Runtime + Microsoft C++ Build Tools (MSVC) |
| Linux | `libgtk-3-dev`, `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `patchelf` |

## Getting started

```bash
git clone https://github.com/abhijith-p-subash/ortu.git
cd ortu
npm install
npm run tauri dev      # launches the app with hot-reload frontend
```

The first run compiles the Rust core, which can take a few minutes; subsequent
runs are fast.

## Common commands

| Command | What it does |
|---------|--------------|
| `npm run tauri dev` | Run the full desktop app in dev mode |
| `npm run dev` | Run the frontend only (Vite) — limited without the Tauri backend |
| `npm run check` | Type-check the Svelte/TS frontend |
| `npm run tauri build` | Build production installers |
| `npm run version-up <x.y.z>` | Bump version across package.json / tauri.conf.json / Cargo.toml |
| `cargo check` (in `src-tauri/`) | Type-check the Rust backend |
| `cargo fmt` / `cargo clippy` (in `src-tauri/`) | Format / lint Rust |

## Project structure

```
ortu/
├── src/                      # SvelteKit frontend
│   ├── routes/               # Windows: main (/), popup, settings
│   ├── lib/                  # Shared TS modules & components
│   ├── app.css               # Tailwind entry + theme tokens
│   └── app.html
├── src-tauri/                # Rust backend (Tauri)
│   ├── src/
│   │   ├── lib.rs            # App setup (windows, tray, shortcuts, updater)
│   │   ├── commands.rs       # IPC command handlers
│   │   ├── clipboard.rs      # Capture listener + classifier
│   │   ├── db.rs             # SQLite schema/queries/FTS/retention
│   │   ├── crypto.rs         # Sensitive-item encryption
│   │   └── main.rs           # Binary entry
│   ├── tauri.conf.json       # App/bundle/updater config
│   └── Cargo.toml
├── docs/                     # Documentation (this folder)
├── static/                   # Static assets (logo, etc.)
└── update-version.mjs        # Version bump script
```

## Conventions

- **Match the surrounding style.** Keep comment density, naming, and idioms
  consistent with nearby code.
- **Frontend:** Svelte 5 runes (`$state`, `$derived`, `$effect`), Tailwind
  utility classes, theme tokens (`bg-surface`, `text-fg`, `border-overlay/…`)
  rather than hard-coded colors so light/dark both work.
- **Shortcuts:** add/change keys via `src/lib/shortcuts.ts` (single source of
  truth) so the UI, Help dialog, and hints stay in sync.
- **Notifications:** use the shared `showToast()` from `src/lib/toast.ts` and the
  `<Toaster />` component — don't roll bespoke toasts.
- **Backend:** keep DB migrations idempotent (`CREATE TABLE IF NOT EXISTS`,
  guarded `ALTER TABLE … ADD COLUMN`). New settings go through `app_meta`.

## Before opening a PR

Run both checks and make sure they're clean:

```bash
npm run check
cd src-tauri && cargo check
```

Then see [../CONTRIBUTING.md](../CONTRIBUTING.md) for the PR workflow.

## Debugging

- **Startup/panic traces:** the app writes diagnostics to a temp log directory
  (`<temp>/ortu/startup.log` and `panic.log`) — handy for launch issues.
- **Frontend devtools:** available in dev builds via the WebView's inspector.
- **Database:** open `ortu.db` (see paths in
  [CONFIGURATION.md](CONFIGURATION.md#storage-locations)) with any SQLite browser.
  Prefer read-only while the app is running.
