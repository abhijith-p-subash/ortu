#!/usr/bin/env bash
set -euo pipefail

if [[ "${OSTYPE:-}" != linux* ]]; then
  echo "This installer is intended for Linux." >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

if [[ -f "${HOME}/.cargo/env" ]]; then
  # shellcheck disable=SC1090
  source "${HOME}/.cargo/env"
fi

cd "${REPO_ROOT}"

pnpm exec tauri build --bundles appimage

APPIMAGE_PATH="$(find "${REPO_ROOT}/src-tauri/target/release/bundle/appimage" -maxdepth 1 -type f -name '*.AppImage' | sort | tail -n 1)"

if [[ -z "${APPIMAGE_PATH}" ]]; then
  echo "AppImage build succeeded but no AppImage artifact was found." >&2
  exit 1
fi

INSTALL_DIR="${HOME}/Applications/Ortu"
INSTALL_APPIMAGE="${INSTALL_DIR}/Ortu.AppImage"
DESKTOP_DIR="${HOME}/.local/share/applications"
AUTOSTART_DIR="${HOME}/.config/autostart"
ICON_DIR="${HOME}/.local/share/icons/hicolor/128x128/apps"
ICON_PATH="${ICON_DIR}/ortu.png"
DESKTOP_FILE="${DESKTOP_DIR}/Ortu.desktop"
AUTOSTART_FILE="${AUTOSTART_DIR}/Ortu.desktop"

mkdir -p "${INSTALL_DIR}" "${DESKTOP_DIR}" "${AUTOSTART_DIR}" "${ICON_DIR}"

install -m 0755 "${APPIMAGE_PATH}" "${INSTALL_APPIMAGE}"
install -m 0644 "${REPO_ROOT}/src-tauri/icons/128x128.png" "${ICON_PATH}"

cat > "${DESKTOP_FILE}" <<EOF
[Desktop Entry]
Type=Application
Version=1.0
Name=Ortu
Comment=Local-first clipboard manager
Exec=${INSTALL_APPIMAGE}
Icon=${ICON_PATH}
Terminal=false
Categories=Utility;Productivity;
StartupNotify=false
EOF

cat > "${AUTOSTART_FILE}" <<EOF
[Desktop Entry]
Type=Application
Version=1.0
Name=Ortu
Comment=Start Ortu hidden in the tray
Exec=${INSTALL_APPIMAGE} --hidden
Icon=${ICON_PATH}
Terminal=false
StartupNotify=false
X-GNOME-Autostart-enabled=true
EOF

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "${DESKTOP_DIR}" >/dev/null 2>&1 || true
fi

echo "Installed AppImage to: ${INSTALL_APPIMAGE}"
echo "Desktop entry: ${DESKTOP_FILE}"
echo "Autostart entry: ${AUTOSTART_FILE}"
