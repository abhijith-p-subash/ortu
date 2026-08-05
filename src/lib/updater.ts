import { check } from "@tauri-apps/plugin-updater";
import { openUrl } from "@tauri-apps/plugin-opener";
import { platform, arch } from "@tauri-apps/plugin-os";

const REPO = "abhijith-p-subash/ortu";

// How long a release check stays fresh. Both windows share this cache, and the
// popup opens dozens of times a day, so the TTL is what keeps the hotkey path
// off the network almost always.
const CHECK_TTL_MS = 6 * 60 * 60 * 1000;

// Shared between the main window and the popup: same origin, same localStorage.
const CACHE_KEY = "ortu-update-check";
const DISMISSED_KEY = "ortu-update-dismissed";

// Installer file extensions to look for, in preference order, per OS.
const INSTALLER_EXTENSIONS: Record<string, string[]> = {
  windows: ["-setup.exe", ".msi", ".exe"],
  macos: [".dmg", ".app.tar.gz"],
  linux: [".AppImage", ".deb", ".rpm"],
};

// Arch name aliases that show up in Tauri bundle filenames.
const ARCH_ALIASES: Record<string, string[]> = {
  x86_64: ["x64", "x86_64", "amd64"],
  aarch64: ["aarch64", "arm64"],
};

interface GithubAsset {
  name: string;
  browser_download_url: string;
}

export interface AvailableUpdate {
  version: string;
  notes: string;
}

interface CacheEntry {
  checkedAt: number;
  update: AvailableUpdate | null;
}

/**
 * Resolves the direct download URL of the latest release's installer that
 * matches the current OS and CPU architecture (e.g. `.dmg` on macOS arm64,
 * `-setup.exe` on Windows x64). Falls back to the releases page if nothing
 * matches.
 */
export async function getOsInstallerUrl(): Promise<string> {
  const fallback = `https://github.com/${REPO}/releases/latest`;
  try {
    const os = platform(); // "macos" | "windows" | "linux" | ...
    const cpu = arch(); // "x86_64" | "aarch64" | ...

    const res = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`, {
      headers: { Accept: "application/vnd.github+json" },
    });
    if (!res.ok) return fallback;

    const data = (await res.json()) as { html_url?: string; assets?: GithubAsset[] };
    const assets = data.assets ?? [];
    const extensions = INSTALLER_EXTENSIONS[os] ?? [];
    const archTokens = ARCH_ALIASES[cpu] ?? [cpu];

    for (const ext of extensions) {
      const matches = assets.filter((a) => a.name.toLowerCase().endsWith(ext.toLowerCase()));
      if (matches.length === 0) continue;
      // Prefer an asset whose name names our architecture; else take the first.
      const archMatch = matches.find((a) =>
        archTokens.some((token) => a.name.toLowerCase().includes(token)),
      );
      return (archMatch ?? matches[0]).browser_download_url;
    }

    return data.html_url ?? fallback;
  } catch {
    return fallback;
  }
}

/**
 * Whether a newer, signature-verified release exists.
 *
 * Detection only. Ortu deliberately does not download or install updates
 * in-app: the updater plugin's `downloadAndInstall` step failed consistently on
 * shipped builds, leaving people stuck on a banner that never worked. The one
 * update path is now {@link downloadUpdate}.
 *
 * Throws if the endpoint can't be reached; callers usually want
 * {@link getAvailableUpdate} instead.
 */
export async function checkForUpdate(): Promise<AvailableUpdate | null> {
  const update = await check();
  if (!update) return null;
  return { version: update.version, notes: update.body ?? "" };
}

/**
 * Cached {@link checkForUpdate}, so whichever window opens first pays for the
 * request and the other reads the result. Never throws — a check that fails is
 * reported as "nothing available" so a flaky network can't break the UI.
 */
export async function getAvailableUpdate(
  options: { force?: boolean } = {},
): Promise<AvailableUpdate | null> {
  if (!options.force) {
    const cached = readCache();
    if (cached) return cached.update;
  }
  try {
    const update = await checkForUpdate();
    writeCache(update);
    return update;
  } catch (error) {
    console.error("Update check failed:", error);
    return null;
  }
}

/**
 * Opens the installer for this OS and architecture from the latest release.
 * This is the whole update flow — the download and install happen in the
 * browser and the platform's own installer, not inside Ortu.
 */
export async function downloadUpdate(): Promise<void> {
  await openUrl(await getOsInstallerUrl());
}

/**
 * Silences the banner for one version. A later release is a different version,
 * so it surfaces again. Shared across both windows.
 */
export function dismissUpdate(version: string): void {
  try {
    localStorage.setItem(DISMISSED_KEY, version);
  } catch {
    /* storage unavailable — the banner simply stays dismissible per session */
  }
}

/** Whether the user already waved this version away. */
export function isUpdateDismissed(version: string): boolean {
  try {
    return localStorage.getItem(DISMISSED_KEY) === version;
  } catch {
    return false;
  }
}

function readCache(): CacheEntry | null {
  try {
    const raw = localStorage.getItem(CACHE_KEY);
    if (!raw) return null;
    const entry = JSON.parse(raw) as CacheEntry;
    if (typeof entry?.checkedAt !== "number") return null;
    if (Date.now() - entry.checkedAt > CHECK_TTL_MS) return null;
    return entry;
  } catch {
    return null;
  }
}

function writeCache(update: AvailableUpdate | null): void {
  try {
    localStorage.setItem(CACHE_KEY, JSON.stringify({ checkedAt: Date.now(), update }));
  } catch {
    /* storage unavailable — we just check again next time */
  }
}
