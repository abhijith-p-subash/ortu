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
  document.documentElement.setAttribute("data-theme", resolveTheme(t));
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
