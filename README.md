<div align="center">

<img src="static/logo.png" alt="Ortu" width="96" height="96" />

# Ortu

**A fast, local-first, privacy-focused clipboard manager.**

Built with [Tauri](https://tauri.app) (Rust) + [SvelteKit](https://kit.svelte.dev) + SQLite.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Platforms](https://img.shields.io/badge/platforms-macOS%20%7C%20Windows%20%7C%20Linux-blue.svg)](#supported-platforms)
[![Built with Tauri](https://img.shields.io/badge/built%20with-Tauri%20v2-24C8DB.svg)](https://tauri.app)
[![Release](https://img.shields.io/github/v/release/abhijith-p-subash/ortu?sort=semver)](https://github.com/abhijith-p-subash/ortu/releases)
[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-support-FFDD00.svg?logo=buymeacoffee&logoColor=black)](https://buymeacoffee.com/abhijithpsubash)

[Website](https://ortu.abhijithpsubash.com/) · [Features](#features) · [Install](#install) · [Build from source](#build-from-source) · [Documentation](docs/README.md) · [Contributing](CONTRIBUTING.md)

</div>

---

## What is Ortu?

Ortu remembers what you copy so you can paste it again later — instantly, from the keyboard, and **without sending anything to the cloud**. Everything lives in a local SQLite database on your machine.

It captures **text, images, and files**, auto-organizes clips into groups, masks detected secrets, and ships a power-user **paste stack** for pasting several items in sequence across apps.

> Local-first by design: no account, no telemetry, no cloud sync. Your clipboard stays on your device.

## Features

- 📋 **Multi-format capture** — text, images, and file selections.
- ⏸️ **Pause capture** — toggle clipboard monitoring on/off from the header; the choice persists across restarts.
- 🧠 **Smart auto-grouping** — a rule-based classifier sorts clips (URLs, code, JSON, shell, secrets, and more) with confidence scores.
- 🗂️ **Groups & pinning** — organize items into user groups; pin the ones you always need. An item can belong to multiple groups.
- 🔎 **Fast search** — SQLite **FTS5** full-text index with a fuzzy re-ranker, plus `group:<name>` and type filters.
- 🥞 **Paste stack** — queue multiple clips and paste them one-by-one, in order, into any app.
- 🔐 **Sensitive-data protection** — detected secrets/keys/tokens can be masked and **encrypted at rest** (AES‑256‑GCM), revealed only on demand.
- ✂️ **Snippets & transforms** — reusable snippets with variables, and "Copy as" transforms (JSON pretty/minify, Base64, URL encode, case, slugify…).
- ⚡ **Quick-access popup** — summon a lightweight picker over any app with a global hotkey.
- ⌨️ **Customizable global shortcuts** — rebind the global hotkeys in Settings, with one-click restore-to-defaults.
- 🎨 **Themes** — light, dark, or follow system.
- 🧹 **Flexible history retention** — clear on reboot (default), keep forever, or age/count limits. Pinned & grouped items are always kept.
- 💾 **Backup & restore** — JSON backup/restore and plain-text export.
- 🔄 **Auto-update** — signed updates delivered via GitHub Releases.
- 🪶 **Lightweight & native** — no bundled browser; a single small binary per platform.

See the full breakdown in **[docs/FEATURES.md](docs/FEATURES.md)**.

## Screenshots

**Main window** — full clipboard history with search, type filters (URLs, text, files) and groups.

![Main window with clipboard history](docs/assets/1.png)

**Quick popup** — summon a lightweight picker over any app with a global hotkey.

![Quick-access popup over the desktop](docs/assets/2.png)

**Paste Stack** — queue items from any app and paste them back in order.

![Paste Stack panel](docs/assets/4.png)

**Settings** — themes, secret auto-masking, history retention and rebindable global shortcuts.

![Settings window](docs/assets/3.png)

## Supported Platforms

| Platform | Status | Notes |
|----------|--------|-------|
| macOS    | ✅ | Needs Accessibility permission for direct paste |
| Windows  | ✅ | Requires WebView2 runtime |
| Linux    | ✅ | X11/Wayland via WebKitGTK |

## Install

Download the latest installer for your OS from the **[Releases page](https://github.com/abhijith-p-subash/ortu/releases/latest)**.

| Platform | Artifacts |
|----------|-----------|
| macOS    | `.dmg`, `.app` |
| Windows  | `.msi`, `.exe` (NSIS) |
| Linux    | `.AppImage`, `.deb` |

### macOS Gatekeeper note (unsigned builds)

If macOS reports the app is "damaged", clear the quarantine attribute:

```bash
xattr -dr com.apple.quarantine "/Applications/Ortu.app"
codesign --force --deep --sign - "/Applications/Ortu.app"
open "/Applications/Ortu.app"
```

This is expected for binaries that aren't notarized with an Apple Developer account. See [docs/PRIVACY_AND_SECURITY.md](docs/PRIVACY_AND_SECURITY.md#distribution--signing).

## Keyboard shortcuts (defaults)

| Action | macOS | Windows / Linux |
|--------|-------|-----------------|
| Open quick popup | `⌥V` | `Alt+V` |
| Copy selection to stack (any app) | `⌘⇧C` | `Ctrl+Shift+C` |
| Paste next item from stack | `⌥⇧V` | `Alt+Shift+V` |
| Quick copy by position | `⌘1–9` | `Ctrl+1–9` |
| Pin / unpin | `⌘P` | `Ctrl+P` |
| Add to group | `⌘C` | `Ctrl+C` |
| Add selected item to stack | `⌘S` | `Ctrl+S` |

Global shortcuts are **rebindable** in Settings. Full reference: **[docs/SHORTCUTS.md](docs/SHORTCUTS.md)**.

## Build from source

### Prerequisites

- [Rust toolchain](https://www.rust-lang.org/tools/install) (stable)
- [Node.js](https://nodejs.org/) (LTS)
- Platform dependencies:
  - **macOS:** Xcode Command Line Tools
  - **Windows:** WebView2 Runtime + MSVC build tools
  - **Linux:** `libgtk-3-dev`, `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `patchelf`

### Run in development

```bash
git clone https://github.com/abhijith-p-subash/ortu.git
cd ortu
npm install
npm run tauri dev
```

### Production build

```bash
npm run tauri build
```

More detail in **[docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)** and **[docs/BUILD_AND_RELEASE.md](docs/BUILD_AND_RELEASE.md)**.

## Documentation

| Guide | Description |
|-------|-------------|
| [Overview](docs/README.md) | Documentation index |
| [Features](docs/FEATURES.md) | Everything Ortu can do |
| [Architecture](docs/ARCHITECTURE.md) | How the app is built |
| [Configuration](docs/CONFIGURATION.md) | Settings, retention, storage |
| [Shortcuts](docs/SHORTCUTS.md) | All keyboard shortcuts |
| [Development](docs/DEVELOPMENT.md) | Local dev & project layout |
| [Build & Release](docs/BUILD_AND_RELEASE.md) | Building & publishing |
| [Privacy & Security](docs/PRIVACY_AND_SECURITY.md) | Data, encryption, signing |
| [FAQ](docs/FAQ.md) | Common questions |
| [Roadmap](docs/ROADMAP.md) | What's planned |

## Tech stack

- **Frontend:** SvelteKit (SPA) + Tailwind CSS
- **Backend:** Rust + Tauri v2
- **Storage:** SQLite (WAL) via `rusqlite`, with a content-addressed blob store for images

## Versioning

Update the version across `package.json`, `tauri.conf.json`, and `Cargo.toml`:

```bash
npm run version-up <new_version>   # e.g. npm run version-up 2.0.1
```

Ortu follows [Semantic Versioning](https://semver.org). Notable changes are recorded in [CHANGELOG.md](CHANGELOG.md).

## Contributing

Contributions are welcome! Please read **[CONTRIBUTING.md](CONTRIBUTING.md)** and our **[Code of Conduct](CODE_OF_CONDUCT.md)** before opening a PR. Found a security issue? See **[SECURITY.md](SECURITY.md)**.

## Links

- 🌐 Website: <https://ortu.abhijithpsubash.com/>
- 📦 Releases: <https://github.com/abhijith-p-subash/ortu/releases>
- 🐙 Source: <https://github.com/abhijith-p-subash/ortu>

## Support

Ortu is free and open source. If it saves you time, you can support development:

<a href="https://buymeacoffee.com/abhijithpsubash" target="_blank">
  <img src="https://img.shields.io/badge/Buy%20Me%20a%20Coffee-abhijithpsubash-FFDD00?logo=buymeacoffee&logoColor=black" alt="Buy Me a Coffee" />
</a>

## License

[MIT](LICENSE) © Abhijith P Subash
