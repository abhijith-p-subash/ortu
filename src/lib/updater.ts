import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { platform, arch } from "@tauri-apps/plugin-os";

const REPO = "abhijith-p-subash/ortu";

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

/**
 * Resolves the direct download URL of the latest release's installer that
 * matches the current OS and CPU architecture (e.g. `.dmg` on macOS arm64,
 * `-setup.exe` on Windows x64). Falls back to the releases page if nothing
 * matches. Use this for the "manual" download path; the updater plugin already
 * picks the right artifact for the silent in-app update.
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

export interface UpdaterHooks {
  /** Called right before contacting the release endpoint. */
  onChecking?: () => void;
  /** No newer release is available. */
  onUpToDate?: () => void;
  /** A newer release exists. Return true to download + install, false to skip. */
  onAvailable?: (version: string, notes: string) => boolean | Promise<boolean>;
  /** Download progress. `total` is null until the server reports a length. */
  onProgress?: (downloaded: number, total: number | null) => void;
  /** Install finished. Return true to relaunch now, false to defer to next launch. */
  onReadyToRestart?: () => boolean | Promise<boolean>;
  /** Any failure during the check/download/install. */
  onError?: (error: unknown) => void;
}

/**
 * Checks the configured GitHub release endpoint for a newer, signature-verified
 * build and—if the hooks approve—downloads, installs, and relaunches the app.
 *
 * Safe to call unconditionally: in `tauri dev` or a browser there is no updater
 * runtime, so it resolves quietly without throwing.
 */
export async function checkForUpdates(hooks: UpdaterHooks = {}): Promise<void> {
  try {
    hooks.onChecking?.();
    const update = await check();

    if (!update) {
      hooks.onUpToDate?.();
      return;
    }

    const proceed = hooks.onAvailable
      ? await hooks.onAvailable(update.version, update.body ?? "")
      : true;
    if (!proceed) return;

    let downloaded = 0;
    let total: number | null = null;

    await update.downloadAndInstall((event) => {
      switch (event.event) {
        case "Started":
          total = event.data.contentLength ?? null;
          break;
        case "Progress":
          downloaded += event.data.chunkLength;
          hooks.onProgress?.(downloaded, total);
          break;
        case "Finished":
          hooks.onProgress?.(total ?? downloaded, total);
          break;
      }
    });

    const restart = hooks.onReadyToRestart ? await hooks.onReadyToRestart() : true;
    if (restart) await relaunch();
  } catch (error) {
    hooks.onError?.(error);
  }
}
