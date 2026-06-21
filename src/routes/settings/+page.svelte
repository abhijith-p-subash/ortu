<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { goto } from "$app/navigation";
  import { platform } from "@tauri-apps/plugin-os";
  import { setTheme, getStoredTheme, type Theme } from "$lib/theme";
  import { SHORTCUT_ACTIONS, prettyAccelerator, acceleratorFromEvent, getKeyLabels } from "$lib/shortcuts";
  import "../../app.css";

  let currentPlatform = $state<string>("macos");
  let keyLabels = $derived(getKeyLabels(currentPlatform));

  // ── Toast ──────────────────────────────────────────────
  interface Toast { id: number; message: string; type: "success" | "error" | "info" }
  let toasts = $state<Toast[]>([]);
  let toastCounter = 0;
  function showToast(message: string, type: Toast["type"] = "info") {
    const id = ++toastCounter;
    toasts = [...toasts, { id, message, type }];
    setTimeout(() => { toasts = toasts.filter(t => t.id !== id); }, 3000);
  }

  // ── Theme ──────────────────────────────────────────────
  let currentTheme = $state<Theme>("dark");
  const THEME_OPTIONS: { value: Theme; label: string }[] = [
    { value: "system", label: "System" },
    { value: "light", label: "Light" },
    { value: "dark", label: "Dark" },
  ];
  function selectTheme(t: Theme) { currentTheme = t; setTheme(t); }

  // ── Privacy: auto-mask secrets ─────────────────────────
  let autoMaskSecrets = $state(false);
  async function loadAutoMask() {
    try {
      const v = (await invoke("get_setting", { key: "auto_mask_secrets" })) as string | null;
      autoMaskSecrets = v === "1";
    } catch { /* default off */ }
  }
  async function toggleAutoMask() {
    autoMaskSecrets = !autoMaskSecrets;
    try {
      await invoke("set_setting", { key: "auto_mask_secrets", value: autoMaskSecrets ? "1" : "0" });
    } catch { showToast("Failed to save setting", "error"); }
  }

  // ── History retention ──────────────────────────────────
  let retentionDays = $state(0);
  let retentionMax = $state(0);
  const RETENTION_DAYS = [
    { value: 0, label: "Forever" }, { value: 7, label: "7 days" },
    { value: 30, label: "30 days" }, { value: 90, label: "90 days" },
  ];
  const RETENTION_MAX = [
    { value: 0, label: "Unlimited" }, { value: 500, label: "500" },
    { value: 1000, label: "1,000" }, { value: 5000, label: "5,000" },
  ];
  async function loadRetention() {
    try {
      const d = (await invoke("get_setting", { key: "retention_days" })) as string | null;
      retentionDays = d ? parseInt(d) || 0 : 0;
      const m = (await invoke("get_setting", { key: "retention_max_items" })) as string | null;
      retentionMax = m ? parseInt(m) || 0 : 0;
    } catch { /* defaults */ }
  }
  async function applyRetention(days: number, max: number) {
    retentionDays = days; retentionMax = max;
    try {
      await invoke("set_setting", { key: "retention_days", value: String(days) });
      await invoke("set_setting", { key: "retention_max_items", value: String(max) });
      await invoke("manual_cleanup");
      showToast("Retention updated", "success");
    } catch (e) { showToast("Failed: " + e, "error"); }
  }

  // ── Global shortcuts (user-rebindable) ─────────────────
  let customShortcuts = $state<Record<string, string>>({});
  let capturingAction = $state<string | null>(null);
  async function loadShortcuts() {
    try { customShortcuts = (await invoke("get_shortcuts")) as Record<string, string>; }
    catch (e) { console.error("Failed to load shortcuts:", e); }
  }
  function startCapture(action: string) { capturingAction = action; }
  async function applyCapturedShortcut(action: string, accelerator: string) {
    capturingAction = null;
    try {
      await invoke("set_shortcut", { action, accelerator });
      await loadShortcuts();
      showToast("Shortcut updated", "success");
    } catch (e) { showToast(String(e), "error"); }
  }
  async function restoreDefaultShortcuts() {
    capturingAction = null;
    try {
      customShortcuts = (await invoke("reset_shortcuts")) as Record<string, string>;
      showToast("Shortcuts restored to defaults", "success");
    } catch (e) { showToast("Failed to restore: " + e, "error"); }
  }

  function goBack() { goto("/"); }

  function handleKeydown(e: KeyboardEvent) {
    if (capturingAction) {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape") { capturingAction = null; return; }
      const accel = acceleratorFromEvent(e);
      if (accel) applyCapturedShortcut(capturingAction, accel);
      return;
    }
    if (e.key === "Escape") goBack();
  }

  onMount(() => {
    currentTheme = getStoredTheme();
    (async () => {
      try { currentPlatform = await platform(); } catch { /* keep default */ }
      await Promise.all([loadAutoMask(), loadRetention(), loadShortcuts()]);
    })();
    window.addEventListener("keydown", handleKeydown);
    return () => window.removeEventListener("keydown", handleKeydown);
  });
</script>

<div class="h-screen flex flex-col bg-app text-fg overflow-hidden select-none pt-6">
  <!-- ── Header ───────────────────────────────────────── -->
  <header class="h-12 shrink-0 flex items-center gap-3 px-3 border-b border-overlay/[0.09] " data-tauri-drag-region>
    <button onclick={goBack} aria-label="Back"
      class="h-[28px] w-[28px] flex items-center justify-center rounded-lg text-fg/55 hover:text-fg/90 hover:bg-overlay/[0.09] transition-colors">
      <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="15 18 9 12 15 6"/></svg>
    </button>
    <div class="flex items-center gap-2">
      <svg xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="text-fg/55"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>
      <span class="text-[13px] font-semibold text-fg/88 tracking-tight">Settings</span>
    </div>
  </header>

  <!-- ── Body ─────────────────────────────────────────── -->
  <div class="flex-1 overflow-y-auto custom-scrollbar">
    <div class="max-w-2xl mx-auto p-5 space-y-6">

      <!-- Appearance -->
      <section class="space-y-3">
        <div class="text-[9px] font-semibold uppercase tracking-[0.1em] text-fg/30">Appearance</div>
        <div class="flex items-start justify-between gap-4 p-3.5 bg-surface rounded-xl border border-overlay/[0.08]">
          <p class="text-[12px] text-fg/50 leading-relaxed pt-1.5 max-w-[220px]">Choose how Ortu looks. <span class="text-fg/75 font-medium">System</span> follows your OS appearance.</p>
          <div role="radiogroup" aria-label="Theme" class="flex shrink-0 rounded-xl bg-overlay/[0.06] border border-overlay/[0.1] p-0.5">
            {#each THEME_OPTIONS as opt}
              <button
                role="radio"
                aria-checked={currentTheme === opt.value}
                onclick={() => selectTheme(opt.value)}
                class="px-3 py-1.5 rounded-lg text-[12px] font-medium transition-colors
                  {currentTheme === opt.value ? 'bg-[#FF8A3D] text-black' : 'text-fg/60 hover:text-fg/90'}"
              >{opt.label}</button>
            {/each}
          </div>
        </div>
      </section>

      <!-- Privacy -->
      <section class="space-y-3">
        <div class="text-[9px] font-semibold uppercase tracking-[0.1em] text-fg/30">Privacy</div>
        <div class="flex items-start justify-between gap-4 p-3.5 bg-surface rounded-xl border border-overlay/[0.08]">
          <div class="min-w-0">
            <div class="text-[13px] font-medium text-fg/80">Auto-mask detected secrets</div>
            <p class="text-[11px] text-fg/45 mt-0.5 leading-relaxed">Automatically encrypt &amp; mask copied passwords, API keys, tokens and SSH keys. You can always reveal and copy them back.</p>
          </div>
          <button
            role="switch"
            aria-checked={autoMaskSecrets}
            aria-label="Toggle auto-mask secrets"
            onclick={toggleAutoMask}
            class="relative shrink-0 mt-0.5 h-[22px] w-[38px] rounded-full transition-colors {autoMaskSecrets ? 'bg-[#FF8A3D]' : 'bg-overlay/[0.18]'}"
          >
            <span class="absolute top-[2px] left-[2px] h-[18px] w-[18px] rounded-full bg-white shadow transition-transform {autoMaskSecrets ? 'translate-x-[16px]' : ''}"></span>
          </button>
        </div>
      </section>

      <!-- History -->
      <section class="space-y-3">
        <div class="text-[9px] font-semibold uppercase tracking-[0.1em] text-fg/30">History</div>
        <div class="p-3.5 bg-surface rounded-xl border border-overlay/[0.08] space-y-3">
          <div class="flex items-center justify-between gap-3">
            <span class="text-[12px] text-fg/60">Keep history for</span>
            <div class="flex shrink-0 rounded-lg bg-overlay/[0.06] border border-overlay/[0.1] p-0.5">
              {#each RETENTION_DAYS as opt}
                <button onclick={() => applyRetention(opt.value, retentionMax)}
                  class="px-2 py-1 rounded-md text-[11px] font-medium transition-colors {retentionDays === opt.value ? 'bg-[#FF8A3D] text-black' : 'text-fg/60 hover:text-fg/90'}">{opt.label}</button>
              {/each}
            </div>
          </div>
          <div class="flex items-center justify-between gap-3">
            <span class="text-[12px] text-fg/60">Max items</span>
            <div class="flex shrink-0 rounded-lg bg-overlay/[0.06] border border-overlay/[0.1] p-0.5">
              {#each RETENTION_MAX as opt}
                <button onclick={() => applyRetention(retentionDays, opt.value)}
                  class="px-2 py-1 rounded-md text-[11px] font-medium transition-colors {retentionMax === opt.value ? 'bg-[#FF8A3D] text-black' : 'text-fg/60 hover:text-fg/90'}">{opt.label}</button>
              {/each}
            </div>
          </div>
          <p class="text-[10px] text-fg/40 leading-relaxed">Pinned items and items in your groups are always kept — retention only clears ungrouped history.</p>
        </div>
      </section>

      <!-- Global shortcuts -->
      <section class="space-y-3">
        <div class="flex items-center justify-between">
          <div class="text-[9px] font-semibold uppercase tracking-[0.1em] text-fg/30">Global Shortcuts</div>
          <button onclick={restoreDefaultShortcuts} class="text-[10px] font-medium text-fg/45 hover:text-[#FF8A3D] transition-colors">Restore defaults</button>
        </div>
        <div class="bg-surface rounded-xl border border-overlay/[0.08] divide-y divide-overlay/[0.06]">
          {#each SHORTCUT_ACTIONS as action}
            <div class="flex items-center justify-between gap-3 p-3.5">
              <div class="min-w-0">
                <div class="text-[13px] font-medium text-fg/80">{action.label}</div>
                <p class="text-[11px] text-fg/45 mt-0.5 leading-relaxed">{action.description}</p>
              </div>
              {#if capturingAction === action.id}
                <button onclick={() => (capturingAction = null)}
                  class="shrink-0 h-[28px] px-3 rounded-lg border border-[#FF8A3D]/40 bg-[#FF8A3D]/[0.1] text-[11px] font-semibold text-[#FF8A3D] animate-pulse">
                  Press keys… (Esc)
                </button>
              {:else}
                <button onclick={() => startCapture(action.id)} title="Click, then press the new key combination"
                  class="kbd shrink-0 h-[28px] min-w-[64px] px-3 text-[12px] font-semibold hover:border-[#FF8A3D]/50 hover:text-[#FF8A3D] transition-colors">
                  {prettyAccelerator(customShortcuts[action.id] ?? "", currentPlatform)}
                </button>
              {/if}
            </div>
          {/each}
        </div>
        <p class="text-[10px] text-fg/40 leading-relaxed">Click a shortcut, then press a key combination (must include {keyLabels.mod}, {keyLabels.alt}, or {keyLabels.shift}). If the OS or another app already uses it, the change is rejected.</p>
      </section>

    </div>
  </div>
</div>

<!-- Toasts -->
<div class="fixed bottom-4 left-1/2 -translate-x-1/2 z-50 flex flex-col items-center gap-2">
  {#each toasts as toast (toast.id)}
    <div class="px-3.5 py-2 rounded-lg text-[12px] font-medium shadow-lg border
      {toast.type === 'success' ? 'bg-[#AEB291] text-black border-transparent'
       : toast.type === 'error' ? 'bg-red-500/90 text-white border-transparent'
       : 'bg-raised text-fg/80 border-overlay/[0.1]'}">
      {toast.message}
    </div>
  {/each}
</div>
