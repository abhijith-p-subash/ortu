# Ortu

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**Ortu** is a minimalist, efficient, and open-source clipboard manager built with **Tauri**, **Rust**, and **SvelteKit**. Designed for speed and distraction-free productivity, Ortu stays out of your way until you need it.

![Main Window Placeholder](https://via.placeholder.com/800x450?text=Ortu+Main+Window)

## 📋 Features

- **Clipboard History**: Automatically tracks text and image copies.
- **Pin Items**: Mark important clips as permanent to prevent them from being pruned.
- **Smart Grouping**: Organize your clips into custom groups (e.g., "Work", "Snippets", "Passwords").
- **Search & Filter**: Find anything instantly with keyword search or filters (e.g., `group:Work`).
- **Direct Paste**: Selected items are pasted directly into your active window.
- **Autostart**: Optionally starts with your OS to ensure clipboard monitoring is always active.
- **Data Portability**: Full support for Backup/Restore (JSON) and Exporting groups or all history (TXT).
- **Lightweight**: Minimal memory footprint thanks to the Rust backend and native SQLite storage.

## ⌨️ Hotkeys

| Platform    | Shortcut             | Action               |
| :---------- | :------------------- | :------------------- |
| **macOS**   | `⌥ + V` (Option + V) | Toggle Stealth Popup |
| **Windows** | `Alt + V`            | Toggle Stealth Popup |
| **Linux**   | `Alt + V`            | Toggle Stealth Popup |

## 🚀 Tech Stack

- **Frontend**: [SvelteKit](https://kit.svelte.dev/) + [Tailwind CSS](https://tailwindcss.com/)
- **Backend**: [Tauri](https://tauri.app/) (Rust)
- **Database**: [SQLite](https://sqlite.org/) (via `rusqlite`)
- **State Management**: Svelte Runes

---

## 🛠 Getting Started

### Prerequisites

To build or develop Ortu, you'll need:

- **Rust**: [Install Rust](https://www.rust-lang.org/tools/install)
- **Node.js**: [Install Node.js](https://nodejs.org/) (LTS recommended)
- **Platform Dependencies**:
  - **macOS**: Xcode Command Line Tools.
  - **Linux**: `libgtk-3-dev`, `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`.
  - **Windows**: [WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) and Build Tools.

### Installation

1. **Clone the repository**:

   ```bash
   git clone https://github.com/abhijithpsubash/ortu.git
   cd ortu
   ```

2. **Install dependencies**:

   ```bash
   npm install
   ```

3. **Run in development mode**:
   ```bash
   npm run tauri dev
   ```

---

## 📦 Building for Production

To create a production-ready installer for your specific OS, run:

```bash
npm run tauri build
```

### Build Artifacts:

- **macOS**: `.app`, `.dmg`
- **Windows**: `.exe`, `.msi`
- **Linux**: `.deb`, `AppImage`

> [!TIP] > **Cross-Platform Builds**: It is highly recommended to use GitHub Actions for cross-platform distribution. See our [Build Guide](.gemini/antigravity/brain/089a0dc8-960b-4343-869c-209564bbb4f3/build_guide.md) for a sample workflow. (Note: Adjust path to build guide if moving to a standard location).

---

## 🔖 Versioning

To update the application version across all configuration files (`package.json`, `tauri.conf.json`, `Cargo.toml`), run:

```bash
npm run version-up <new_version>
```

**Example:**

```bash
npm run version-up 1.0.2
```

---

## 🤝 Contributing

Contributions are welcome! Whether it's a bug report, a feature request, or a pull request, we appreciate your help.

1. Fork the Project
2. Create your Feature Branch (`git checkout -b feature/AmazingFeature`)
3. Commit your Changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the Branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

## 📄 License

Distributed under the MIT License. See `LICENSE` for more information.

---

Made with ❤️ by [Abhijith P Subash](https://github.com/abhijithpsubash)
