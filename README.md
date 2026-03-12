# Ortu

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Ortu is a local-first clipboard manager built with Tauri, Rust, and SvelteKit.
It is designed for fast recall, keyboard-driven access, and organized clipboard history.

## Features

- Clipboard history capture (current runtime pipeline is text-first).
- Pin important items to keep them persistent.
- User groups and smart auto-grouping support.
- Search and filtering (`group:<name>` and free-text search).
- Popup quick access (`Alt+V` on Windows/Linux, `Option+V` on macOS).
- Snippets and text transforms.
- Backup and restore (JSON), plus export options.
- Local SQLite storage (no cloud dependency by default).

## Tech Stack

- Frontend: SvelteKit + Tailwind CSS
- Backend: Rust (Tauri v2)
- Storage: SQLite via `rusqlite`

## Supported Platforms

- macOS
- Windows
- Linux

## Prerequisites

- Rust toolchain: <https://www.rust-lang.org/tools/install>
- Node.js (LTS recommended): <https://nodejs.org/>
- Platform dependencies:
- macOS: Xcode Command Line Tools
- Linux: `libgtk-3-dev`, `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`
- Windows: WebView2 Runtime and MSVC build tools

## Quick Start

```bash
git clone https://github.com/abhijithpsubash/ortu.git
cd ortu
npm install
npm run tauri dev
```

## Build

```bash
npm run tauri build
```

Typical artifacts:
- macOS: `.app`, `.dmg`
- Windows: `.exe`, `.msi`
- Linux: `.deb`, `AppImage`

## Hotkeys

- macOS: `Option + V`
- Windows: `Alt + V`
- Linux: `Alt + V`

## macOS Gatekeeper Workaround (No Apple Developer Account)

If macOS shows:
`"Ortu" is damaged and can’t be opened. You should move it to the Bin.`

run:

```bash
xattr -dr com.apple.quarantine "/Applications/Ortu.app"
codesign --force --deep --sign - "/Applications/Ortu.app"
open "/Applications/Ortu.app"
```

Note:
- Without Apple notarization, Gatekeeper warnings are expected for shared binaries.
- macOS does not allow fully automatic script execution during drag-and-drop install from DMG.

## Versioning

Update app version in all required config files:

```bash
npm run version-up <new_version>
```

Example:

```bash
npm run version-up 1.0.4
```

## Project Structure

- `src/`: SvelteKit frontend
- `src-tauri/src/`: Rust backend logic
- `src-tauri/tauri.conf.json`: Tauri app/bundle config

## Contributing

Contributions are welcome.

1. Fork the repository
2. Create a branch (`git checkout -b feature/your-change`)
3. Commit changes (`git commit -m "your message"`)
4. Push branch (`git push origin feature/your-change`)
5. Open a Pull Request

## Security and Privacy

- Clipboard data is stored locally.
- Review code and release process before distributing binaries.
- For production public distribution on macOS, signing + notarization is strongly recommended.

## License

MIT License. See [LICENSE](./LICENSE).
