export type Theme = "dark" | "light" | "system";

const KEY = "ortu-theme";

export function getStoredTheme(): Theme {
  if (typeof localStorage === "undefined") return "dark";
  const v = localStorage.getItem(KEY);
  return v === "light" || v === "system" ? v : "dark";
}

function systemPrefersDark(): boolean {
  return (
    typeof window !== "undefined" &&
    !!window.matchMedia &&
    window.matchMedia("(prefers-color-scheme: dark)").matches
  );
}

export function resolveTheme(t: Theme): "dark" | "light" {
  if (t === "system") return systemPrefersDark() ? "dark" : "light";
  return t;
}

export function applyTheme(t: Theme): void {
  if (typeof document === "undefined") return;
  const resolved = resolveTheme(t);
  document.documentElement.setAttribute("data-theme", resolved);
  // Keep the native window titlebar in sync so the title stays legible.
  syncTitlebar(resolved === "dark");
}

function syncTitlebar(dark: boolean): void {
  // Best-effort; only available inside the Tauri runtime. Dynamic import keeps
  // this module usable in any non-Tauri context.
  import("@tauri-apps/api/core")
    .then(({ invoke }) => invoke("set_titlebar_theme", { dark }))
    .catch(() => {
      /* not in Tauri or command unavailable — ignore */
    });
}

export function setTheme(t: Theme): void {
  if (typeof localStorage !== "undefined") localStorage.setItem(KEY, t);
  applyTheme(t);
}

let mediaListenerAttached = false;

/** Applies the stored theme and keeps "system" mode in sync with the OS. */
export function initTheme(): void {
  applyTheme(getStoredTheme());
  if (!mediaListenerAttached && typeof window !== "undefined" && window.matchMedia) {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    mq.addEventListener?.("change", () => {
      if (getStoredTheme() === "system") applyTheme("system");
    });
    mediaListenerAttached = true;
  }
}
