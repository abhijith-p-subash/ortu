<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { ClipboardItem } from "$lib/types";
  import { listen } from "@tauri-apps/api/event";
  import { save, open } from "@tauri-apps/plugin-dialog";
  import { platform } from "@tauri-apps/plugin-os";
  import { getVersion } from "@tauri-apps/api/app";
  import { buildSearchQuery, clipPreview } from "$lib/filters";
  import "../app.css";

  let history = $state<ClipboardItem[]>([]);
  let groups = $state<string[]>([]);
  let searchQuery = $state("");
  let selectedIndex = $state(0);
  let container = $state<HTMLDivElement | null>(null);
  let searchInput = $state<HTMLInputElement | null>(null);
  let isCategorizing = $state(false);
  let categorizingItemId = $state<number | null>(null);
  let isViewingGroups = $state(false);
  let selectedGroup = $state<string | null>(null);
  let newGroupName = $state("");
  let editingGroup = $state<string | null>(null);
  let editGroupName = $state("");
  let expandedItems = $state<number[]>([]);
  let appVersion = $state("1.0.0");

  // Import/Export State
  let showExportModal = $state(false);
  let showImportModal = $state(false);
  let exportSelectedGroups = $state<string[]>([]);
  let importMode = $state<"merge" | "replace">("merge");
  let processingIO = $state(false);

  // Help & About State
  let showHelpModal = $state(false);
  let showAboutModal = $state(false);
  let currentPlatform = $state<string>("macos");
  let macAccessibilityGranted = $state(true);
  let checkingMacAccessibility = $state(false);

  // Toast notification state
  let showCopiedToast = $state(false);
  let copiedToastTimer: number | null = null;

  // Toasts
  interface Toast { id: number; message: string; type: "success" | "error" | "info" }
  let toasts = $state<Toast[]>([]);
  let toastCounter = 0;

  function showToast(message: string, type: Toast["type"] = "info") {
    const id = ++toastCounter;
    toasts = [...toasts, { id, message, type }];
    setTimeout(() => { toasts = toasts.filter(t => t.id !== id); }, 3000);
  }

  // Confirm modal
  let confirmModal = $state<{ message: string; onConfirm: () => void } | null>(null);
  function confirmAction(message: string, onConfirm: () => void) {
    confirmModal = { message, onConfirm };
  }

  // Add Item modal
  let showAddItemModal = $state(false);
  let newItemContent = $state("");
  let newItemDescription = $state("");
  let newItemGroupInput = $state("");

  // More menu
  let showMoreMenu = $state(false);

  // Update check
  let updateAvailable = $state(false);
  let latestVersion = $state("");
  let releaseUrl = $state("https://github.com/abhijith-p-subash/ortu/releases/latest");

  async function checkForUpdates() {
    try {
      const res = await fetch("https://api.github.com/repos/abhijith-p-subash/ortu/releases/latest");
      if (!res.ok) return;
      const data = await res.json();
      const latest = (data.tag_name as string)?.replace(/^v/, "");
      if (!latest || latest === appVersion) return;
      const cur = appVersion.split(".").map(Number);
      const rem = latest.split(".").map(Number);
      const isNewer =
        rem[0] > cur[0] ||
        (rem[0] === cur[0] && rem[1] > cur[1]) ||
        (rem[0] === cur[0] && rem[1] === cur[1] && (rem[2] ?? 0) > (cur[2] ?? 0));
      if (isNewer) {
        updateAvailable = true;
        latestVersion = latest;
        releaseUrl = (data.html_url as string) ?? releaseUrl;
      }
    } catch {
      // silently ignore
    }
  }

  async function addManualItem() {
    if (!newItemContent.trim()) return;
    try {
      await invoke("add_manual_item", {
        content: newItemContent.trim(),
        description: newItemDescription.trim() || null,
        groupName: newItemGroupInput.trim() || null,
      });
      newItemContent = "";
      newItemDescription = "";
      newItemGroupInput = selectedGroup || "";
      showAddItemModal = false;
      await loadHistory();
      await loadGroups();
      showToast("Item added", "success");
    } catch (e) {
      showToast("Failed to add item: " + e, "error");
    }
  }

  // Detect platform
  onMount(async () => {
    try {
      currentPlatform = await platform();
      appVersion = await getVersion();
      await refreshMacAccessibilityStatus();
      setTimeout(checkForUpdates, 2000);
    } catch (e) {
      console.error("Failed to detect platform/version:", e);
    }
  });

  // OS-specific key labels
  let modKey = $derived(currentPlatform === "macos" ? "Cmd" : "Ctrl");
  let deleteKey = $derived(currentPlatform === "macos" ? "⌫" : "Backspace");
  let altKey = $derived(currentPlatform === "macos" ? "Option" : "Alt");

  async function refreshMacAccessibilityStatus() {
    if (currentPlatform !== "macos") {
      macAccessibilityGranted = true;
      return;
    }
    try {
      checkingMacAccessibility = true;
      macAccessibilityGranted = (await invoke("get_macos_accessibility_status")) as boolean;
    } catch (e) {
      console.error("Failed to check macOS Accessibility status:", e);
      macAccessibilityGranted = false;
    } finally {
      checkingMacAccessibility = false;
    }
  }

  async function openMacAccessibilitySettings() {
    try {
      await invoke("open_macos_accessibility_settings");
    } catch (e) {
      console.error("Failed to open Accessibility settings:", e);
    }
  }

  async function loadHistory() {
    try {
      const search = selectedGroup
        ? buildSearchQuery(selectedGroup, searchQuery)
        : buildSearchQuery(null, searchQuery);
      const data = (await invoke("get_history", { search: search || null })) as ClipboardItem[];
      history = data;
      if (selectedIndex >= history.length) {
        selectedIndex = Math.max(0, history.length - 1);
      }
    } catch (e) {
      console.error("Failed to load history:", e);
    }
  }

  async function loadGroups() {
    try {
      groups = (await invoke("get_categories")) as string[];
    } catch (e) {
      console.error("Failed to load groups:", e);
    }
  }

  async function createGroup() {
    if (!newGroupName.trim()) return;
    try {
      await invoke("create_group", { name: newGroupName.trim() });
      newGroupName = "";
      await loadGroups();
    } catch (e) {
      console.error("Failed to create group:", e);
    }
  }

  async function deleteGroup(name: string) {
    confirmAction(`Delete group "${name}"? Items will NOT be deleted.`, async () => {
      try {
        await invoke("delete_group", { name });
        if (selectedGroup === name) selectedGroup = null;
        await loadGroups();
        await loadHistory();
        showToast(`Group "${name}" deleted`, "success");
      } catch (e) {
        showToast("Failed to delete group: " + e, "error");
      }
    });
  }

  async function renameGroup() {
    if (!editingGroup || !editGroupName.trim()) return;
    try {
      await invoke("rename_group", { oldName: editingGroup, newName: editGroupName.trim() });
      if (selectedGroup === editingGroup) selectedGroup = editGroupName.trim();
      editingGroup = null;
      editGroupName = "";
      await loadGroups();
      await loadHistory();
    } catch (e) {
      console.error("Failed to rename group:", e);
    }
  }

  async function openExportModal() {
    exportSelectedGroups = [];
    if (selectedGroup && !["URL", "Dev", "Code", "Images", "Text"].includes(selectedGroup)) {
      exportSelectedGroups = [selectedGroup];
    }
    showExportModal = true;
  }

  async function performExport() {
    try {
      const path = await save({
        filters: [{ name: "JSON", extensions: ["json"] }],
        defaultPath: `ortu_backup_${new Date().toISOString().split("T")[0]}.json`,
      });
      if (!path) return;
      processingIO = true;
      await invoke("backup_data", {
        path,
        groups: exportSelectedGroups.length > 0 ? exportSelectedGroups : [],
      });
      showExportModal = false;
      showToast("Export successful", "success");
    } catch (e) {
      showToast("Export failed: " + e, "error");
    } finally {
      processingIO = false;
    }
  }

  async function openImportModal() {
    importMode = "merge";
    showImportModal = true;
  }

  async function performImport() {
    try {
      const path = await open({ filters: [{ name: "JSON", extensions: ["json"] }] });
      if (path) {
        processingIO = true;
        await invoke("restore_data", { path: path as string, mode: importMode });
        showImportModal = false;
        await loadHistory();
        await loadGroups();
        showToast("Import successful", "success");
      }
    } catch (e) {
      showToast("Import failed: " + e, "error");
    } finally {
      processingIO = false;
    }
  }

  async function exportGroup(name: string) {
    try {
      const path = await save({
        filters: [{ name: "Text", extensions: ["txt"] }],
        defaultPath: `${name}_export.txt`,
      });
      if (path && typeof path === "string") {
        await invoke("export_group", { name, path });
        showToast("Group exported", "success");
      }
    } catch (e) {
      showToast("Export failed: " + e, "error");
    }
  }

  async function exportAllTxt() {
    try {
      const path = await save({
        filters: [{ name: "Text", extensions: ["txt"] }],
        defaultPath: `ortu_full_export.txt`,
      });
      if (path && typeof path === "string") {
        await invoke("export_all_txt", { path });
        showToast("Full export successful", "success");
      }
    } catch (e) {
      showToast("Export failed: " + e, "error");
    }
  }

  let importGroupNameInput = $state("");
  let showImportGroupModal = $state(false);
  let importGroupPath = $state("");

  async function importGroup() {
    try {
      const path = await open({ filters: [{ name: "Text", extensions: ["txt"] }] });
      if (path && typeof path === "string") {
        importGroupPath = path;
        importGroupNameInput = "";
        showImportGroupModal = true;
      }
    } catch (e) {
      showToast("Failed to open file: " + e, "error");
    }
  }

  async function confirmImportGroup() {
    if (!importGroupNameInput.trim()) return;
    try {
      await invoke("import_group", { name: importGroupNameInput.trim(), path: importGroupPath });
      await loadGroups();
      await loadHistory();
      showImportGroupModal = false;
      showToast("Group imported", "success");
    } catch (e) {
      showToast("Import failed: " + e, "error");
    }
  }

  async function togglePermanent(item: ClipboardItem) {
    await invoke("toggle_permanent", { id: item.id });
    await loadHistory();
  }

  async function deleteItem(item: ClipboardItem) {
    if (!item) return;
    await invoke("delete_entry", { id: item.id });
    await loadHistory();
  }

  async function moveItemToGroup() {
    const itemId =
      categorizingItemId ||
      (history[selectedIndex] ? history[selectedIndex].id : null);
    if (!itemId || !newGroupName.trim()) return;
    try {
      await invoke("add_to_group", { itemId, groupName: newGroupName.trim() });
      isCategorizing = false;
      newGroupName = "";
      await loadHistory();
      await loadGroups();
    } catch (e) {
      console.error("Failed to add item to group:", e);
    }
  }

  async function removeFromGroup(item: ClipboardItem, group: string) {
    try {
      await invoke("remove_from_group", { itemId: item.id, groupName: group });
      await loadHistory();
    } catch (e) {
      console.error("Failed to remove group:", e);
    }
  }

  async function copyAndPaste(item: ClipboardItem) {
    try {
      await invoke("copy_item_to_clipboard", { id: item.id });
      if (copiedToastTimer) clearTimeout(copiedToastTimer);
      showCopiedToast = true;
      copiedToastTimer = window.setTimeout(() => {
        showCopiedToast = false;
        copiedToastTimer = null;
      }, 2000);
    } catch (err) {
      console.error("Failed to copy: ", err);
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (isCategorizing) {
      if (e.key === "Enter") {
        e.preventDefault();
        moveItemToGroup();
      } else if (e.key === "Escape") {
        isCategorizing = false;
        newGroupName = "";
      }
      return;
    }

    if (e.key === "Escape") {
      invoke("close_window");
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      selectedIndex = (selectedIndex + 1) % (history.length || 1);
      scrollIntoView();
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      selectedIndex = (selectedIndex - 1 + (history.length || 1)) % (history.length || 1);
      scrollIntoView();
    } else if (e.key === "Enter") {
      if (history[selectedIndex]) copyAndPaste(history[selectedIndex]);
    } else if (e.key === "Delete" || (e.metaKey && e.key === "Backspace")) {
      if (history[selectedIndex]) deleteItem(history[selectedIndex]);
    } else if (e.key === "p" && (e.metaKey || e.ctrlKey)) {
      if (history[selectedIndex]) togglePermanent(history[selectedIndex]);
    } else if (e.key === "c" && (e.metaKey || e.ctrlKey)) {
      if (history[selectedIndex]) {
        e.preventDefault();
        isCategorizing = true;
      }
    } else if (e.key === "g" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      isViewingGroups = !isViewingGroups;
      if (isViewingGroups) loadGroups();
    }
  }

  function scrollIntoView() {
    if (!container) return;
    const selectedElement = container.querySelector(`[data-index="${selectedIndex}"]`);
    selectedElement?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }

  $effect(() => {
    if (selectedIndex !== undefined) scrollIntoView();
  });

  $effect(() => {
    if (searchQuery !== undefined || selectedGroup !== undefined) loadHistory();
  });

  onMount(() => {
    loadHistory();
    loadGroups();
    window.addEventListener("keydown", handleKeydown);
    const preventContextMenu = (e: MouseEvent) => e.preventDefault();
    window.addEventListener("contextmenu", preventContextMenu);

    let unlistenFocus: () => void;
    let unlistenClipboard: () => void;

    const setupListeners = async () => {
      try {
        const uFocus = await listen("tauri://focus", async () => {
          await loadHistory();
          await loadGroups();
          await refreshMacAccessibilityStatus();
          await tick();
          searchInput?.focus();
        });
        unlistenFocus = uFocus;

        const uClipboard = await listen("clipboard-updated", async () => {
          await loadHistory();
        });
        unlistenClipboard = uClipboard;
      } catch (e) {
        console.error("Failed to setup main listeners:", e);
      }
    };

    setupListeners();

    return () => {
      window.removeEventListener("keydown", handleKeydown);
      window.removeEventListener("contextmenu", preventContextMenu);
      if (unlistenFocus) unlistenFocus();
      if (unlistenClipboard) unlistenClipboard();
    };
  });

  function getCategoryIcon(category: string | null): string {
    if (!category)
      return '<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline><line x1="16" y1="13" x2="8" y2="13"></line><line x1="16" y1="17" x2="8" y2="17"></line><polyline points="10 9 9 9 8 9"></polyline>';

    const c = category.toLowerCase();

    if (c === "url")
      return '<circle cx="12" cy="12" r="10"></circle><line x1="2" y1="12" x2="22" y2="12"></line><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"></path>';

    if (c.includes("docker") || c.includes("shell") || c.includes("kubernetes") || c.includes("cloud") || c.includes("terminal"))
      return '<polyline points="4 17 10 11 4 5"></polyline><line x1="12" y1="19" x2="20" y2="19"></line>';

    if (c.includes("git") || c.includes("version"))
      return '<line x1="6" y1="3" x2="6" y2="15"></line><circle cx="18" cy="6" r="3"></circle><circle cx="6" cy="18" r="3"></circle><path d="M18 9a9 9 0 0 1-9 9"></path>';

    if (c.includes("database") || c.includes("sql"))
      return '<ellipse cx="12" cy="5" rx="9" ry="3"></ellipse><path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"></path><path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"></path>';

    if (c.includes("code") || c.includes("runtime") || c.includes("package") || c.includes("ci"))
      return '<polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline>';

    return '<path d="M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82z"></path><line x1="7" y1="7" x2="7.01" y2="7"></line>';
  }
</script>

<div class="flex flex-col h-screen bg-[#111214] text-zinc-300 overflow-hidden selection:bg-[#FF8A3D]/30 ">

  <!-- Header -->
  <header class="h-11 shrink-0 px-4 flex items-center justify-between border-b border-[#252a30] bg-[#111214] mt-6">
    <div class="flex items-center gap-2.5">
      <img src="/logo.png" alt="" class="w-5 h-5 shrink-0" />
      <span class="text-[13px] font-bold text-white tracking-tight">Ortu</span>
    </div>
    <div class="flex items-center gap-1.5">
      <button
        onclick={() => { newItemGroupInput = selectedGroup || ""; showAddItemModal = true; }}
        class="flex items-center gap-1.5 h-7 px-3 bg-[#FF8A3D] hover:bg-[#f07d34] rounded text-xs font-semibold text-[#111214] transition-colors"
        title="Add item manually"
      >
        <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
        Add
      </button>
      <div class="relative">
        <button
          onclick={() => (showMoreMenu = !showMoreMenu)}
          class="h-7 w-7 flex items-center justify-center hover:bg-[#1e2228] rounded text-zinc-500 hover:text-zinc-200 transition-colors"
          title="More options"
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 24 24" fill="currentColor" stroke="none"><circle cx="12" cy="5" r="1.5"/><circle cx="12" cy="12" r="1.5"/><circle cx="12" cy="19" r="1.5"/></svg>
        </button>
        {#if showMoreMenu}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            role="menu"
            tabindex="-1"
            class="absolute right-0 top-8 w-44 bg-[#17191c] border border-[#252a30] rounded shadow-2xl z-50 py-1"
            onmouseleave={() => (showMoreMenu = false)}
          >
            <button onclick={() => { showMoreMenu = false; openExportModal(); }} class="w-full text-left px-3 py-2 text-xs text-zinc-400 hover:bg-[#1e2228] hover:text-white flex items-center gap-2.5 transition-colors">
              <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"/><polyline points="17 21 17 13 7 13 7 21"/><polyline points="7 3 7 8 15 8"/></svg>
              Backup
            </button>
            <button onclick={() => { showMoreMenu = false; openImportModal(); }} class="w-full text-left px-3 py-2 text-xs text-zinc-400 hover:bg-[#1e2228] hover:text-white flex items-center gap-2.5 transition-colors">
              <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
              Restore
            </button>
            <button onclick={() => { showMoreMenu = false; exportAllTxt(); }} class="w-full text-left px-3 py-2 text-xs text-zinc-400 hover:bg-[#1e2228] hover:text-white flex items-center gap-2.5 transition-colors">
              <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" y1="3" x2="12" y2="15"/></svg>
              Export All (.txt)
            </button>
            <div class="h-px bg-[#252a30] my-1"></div>
            <button onclick={() => { showMoreMenu = false; showHelpModal = true; }} class="w-full text-left px-3 py-2 text-xs text-zinc-400 hover:bg-[#1e2228] hover:text-white flex items-center gap-2.5 transition-colors">
              <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>
              Help
            </button>
            <button onclick={() => { showMoreMenu = false; showAboutModal = true; }} class="w-full text-left px-3 py-2 text-xs text-zinc-400 hover:bg-[#1e2228] hover:text-white flex items-center gap-2.5 transition-colors">
              <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/></svg>
              About
            </button>
          </div>
        {/if}
      </div>
    </div>
  </header>

  <!-- Update banner -->
  {#if updateAvailable}
    <div class="mx-3 mt-2 flex items-center justify-between rounded bg-[#AEB291]/6 border border-[#AEB291]/15 px-3 py-2">
      <p class="text-xs text-zinc-400">
        <span class="font-semibold text-white">v{latestVersion}</span> is available
      </p>
      <div class="flex items-center gap-3 shrink-0">
        <a href={releaseUrl} target="_blank" rel="noopener noreferrer" class="text-xs font-semibold text-[#AEB291] hover:text-white transition-colors">Download →</a>
        <button onclick={() => (updateAvailable = false)} aria-label="Dismiss update notification" class="text-zinc-600 hover:text-zinc-400 transition-colors">
          <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
        </button>
      </div>
    </div>
  {/if}

  <!-- macOS Accessibility banner -->
  {#if currentPlatform === "macos" && !macAccessibilityGranted}
    <div class="mx-3 mt-2 rounded border border-[#FF8A3D]/20 bg-[#FF8A3D]/5 px-3 py-2.5">
      <div class="flex items-start justify-between gap-3">
        <div class="min-w-0">
          <p class="text-xs font-semibold text-white">Accessibility permission needed</p>
          <p class="mt-0.5 text-[11px] text-zinc-400 leading-relaxed">
            Enable <span class="text-white font-medium">Ortu</span> in System Settings → Privacy & Security → Accessibility to paste directly.
          </p>
        </div>
        <div class="flex shrink-0 items-center gap-1.5 mt-0.5">
          <button
            onclick={refreshMacAccessibilityStatus}
            class="h-6 px-2.5 rounded border border-[#252a30] bg-[#1e2228] text-[11px] font-medium text-zinc-300 hover:text-white transition-colors"
            disabled={checkingMacAccessibility}
          >{checkingMacAccessibility ? "..." : "Refresh"}</button>
          <button
            onclick={openMacAccessibilitySettings}
            class="h-6 px-2.5 rounded bg-[#FF8A3D] text-[11px] font-semibold text-[#111214] hover:bg-[#f07d34] transition-colors"
          >Open Settings</button>
        </div>
      </div>
    </div>
  {/if}

  <div class="flex flex-1 overflow-hidden min-w-0">

    <!-- Sidebar -->
    <aside class="w-52 shrink-0 flex flex-col border-r border-[#252a30] bg-[#0d1013]">

      <!-- Nav -->
      <nav class="p-2 space-y-0.5 pt-3">
        <button
          class="w-full flex items-center gap-2.5 px-2.5 py-2 rounded text-[13px] transition-colors {selectedGroup === null ? 'bg-[#1e2228] text-white font-medium' : 'text-zinc-500 hover:text-zinc-200 hover:bg-[#17191c]'}"
          onclick={() => { selectedGroup = null; }}
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="{selectedGroup === null ? 'text-[#FF8A3D]' : 'text-zinc-600'}">
            <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
          </svg>
          All History
        </button>

        <div class="h-px bg-[#1a1e24] my-2 mx-1"></div>

        <button
          class="w-full flex items-center gap-2.5 px-2.5 py-2 rounded text-[13px] transition-colors {selectedGroup === 'URL' ? 'bg-[#1e2228] text-white font-medium' : 'text-zinc-500 hover:text-zinc-200 hover:bg-[#17191c]'}"
          onclick={() => (selectedGroup = "URL")}
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="{selectedGroup === 'URL' ? 'text-[#AEB291]' : 'text-zinc-600'}">
            <circle cx="12" cy="12" r="10"/><line x1="2" y1="12" x2="22" y2="12"/><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/>
          </svg>
          URLs
        </button>
        <button
          class="w-full flex items-center gap-2.5 px-2.5 py-2 rounded text-[13px] transition-colors {selectedGroup === 'Text' ? 'bg-[#1e2228] text-white font-medium' : 'text-zinc-500 hover:text-zinc-200 hover:bg-[#17191c]'}"
          onclick={() => (selectedGroup = "Text")}
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="{selectedGroup === 'Text' ? 'text-[#AEB291]' : 'text-zinc-600'}">
            <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="16" y1="13" x2="8" y2="13"/><line x1="16" y1="17" x2="8" y2="17"/>
          </svg>
          Text
        </button>
      </nav>

      <!-- Groups header -->
      <div class="flex items-center justify-between px-3 pt-3 pb-1.5 mt-1">
        <span class="text-[10px] font-semibold uppercase tracking-wider text-zinc-600">Groups</span>
        <button onclick={() => (isViewingGroups = true)} class="text-zinc-600 hover:text-[#AEB291] transition-colors" title="Manage groups">
          <svg xmlns="http://www.w3.org/2000/svg" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
        </button>
      </div>

      <!-- Groups list -->
      <div class="flex-1 overflow-y-auto custom-scrollbar px-2 pb-2 space-y-0.5">
        {#each groups as group}
          <div class="group/g relative flex items-center rounded transition-colors {selectedGroup === group ? 'bg-[#1e2228]' : 'hover:bg-[#17191c]'}">
            {#if editingGroup === group}
              <input
                type="text"
                bind:value={editGroupName}
                class="flex-1 bg-transparent text-[13px] px-2.5 py-1.5 focus:outline-none text-white border-b border-[#AEB291]/25"
                onblur={renameGroup}
                onkeydown={(e) => { if (e.key === "Enter") renameGroup(); if (e.key === "Escape") editingGroup = null; }}
              />
            {:else}
              <button
                class="flex-1 min-w-0 text-left px-2.5 py-1.5 text-[13px] truncate transition-colors {selectedGroup === group ? 'text-white font-medium' : 'text-zinc-500'}"
                title={group}
                onclick={() => { selectedGroup = group; }}
              >
                <span class="block truncate">{group}</span>
              </button>
              <div class="flex opacity-0 group-hover/g:opacity-100 pr-1 gap-0.5 transition-opacity">
                <button
                  onclick={() => { editingGroup = group; editGroupName = group; }}
                  class="p-1 text-zinc-600 hover:text-zinc-200 rounded transition-colors" title="Rename"
                >
                  <svg xmlns="http://www.w3.org/2000/svg" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
                </button>
                <button onclick={() => exportGroup(group)} class="p-1 text-zinc-600 hover:text-zinc-200 rounded transition-colors" title="Export">
                  <svg xmlns="http://www.w3.org/2000/svg" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
                </button>
                <button onclick={() => deleteGroup(group)} class="p-1 text-zinc-600 hover:text-[#FF8A3D] rounded transition-colors" title="Delete">
                  <svg xmlns="http://www.w3.org/2000/svg" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/></svg>
                </button>
              </div>
            {/if}
          </div>
        {/each}
      </div>
    </aside>

    <!-- Main content -->
    <main class="flex-1 min-w-0 flex flex-col bg-[#111214]">

      <!-- Search -->
      <div class="px-3 py-2.5 border-b border-[#252a30] flex items-center gap-2.5">
        <div class="flex-1 relative">
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" class="absolute left-3 top-1/2 -translate-y-1/2 text-zinc-600 pointer-events-none">
            <circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/>
          </svg>
          <input
            type="text"
            bind:this={searchInput}
            bind:value={searchQuery}
            placeholder={selectedGroup ? `Search in ${selectedGroup}...` : "Search clips..."}
            class="w-full bg-[#17191c] border border-[#252a30] rounded pl-9 pr-3 py-2 text-sm focus:outline-none focus:border-[#AEB291]/30 transition-colors placeholder:text-zinc-600"
          />
        </div>
        {#if selectedGroup}
          <div class="flex items-center gap-1.5 shrink-0 bg-[#1e2228] rounded px-2 py-1.5 border border-[#AEB291]/12">
            <span class="text-[11px] font-medium text-[#AEB291]">{selectedGroup}</span>
            <button onclick={() => (selectedGroup = null)} class="text-zinc-600 hover:text-zinc-300 transition-colors" aria-label="Clear selected group">
              <svg xmlns="http://www.w3.org/2000/svg" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
            </button>
          </div>
        {/if}
      </div>

      <!-- History list -->
      <div class="flex-1 overflow-y-auto custom-scrollbar p-3" bind:this={container}>
        <div class="space-y-1.5">
          {#each history as item, i (item.id)}
            <div
              class="group/card p-3 rounded border transition-all cursor-default
                {i === selectedIndex
                  ? 'bg-[#1e2228] border-[#AEB291]/18'
                  : 'bg-[#17191c] border-[#252a30] hover:bg-[#1a1d22] hover:border-[#2f3540]'}"
              onclick={() => { selectedIndex = i; }}
              onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); selectedIndex = i; } }}
              role="button"
              tabindex="0"
              data-index={i}
            >
              <div class="flex items-start gap-2 min-w-0">
                <div class="min-w-0 flex-1">
                  {#if item.description}
                    <p class="text-[11px] font-medium text-[#AEB291] mb-1 truncate">{item.description}</p>
                  {/if}
                  <p class="text-[13px] text-zinc-200 leading-relaxed break-all whitespace-pre-wrap {expandedItems.includes(item.id) ? '' : 'line-clamp-3'}">
                    {clipPreview(item.raw_content, item.content_type)}
                  </p>
                  {#if item.raw_content.split('\n').length > 3 || item.raw_content.length > 250}
                    <button
                      onclick={(e) => {
                        e.stopPropagation();
                        if (expandedItems.includes(item.id)) {
                          expandedItems = expandedItems.filter(id => id !== item.id);
                        } else {
                          expandedItems = [...expandedItems, item.id];
                        }
                      }}
                      class="text-[10px] text-zinc-600 hover:text-zinc-400 mt-0.5 transition-colors"
                    >{expandedItems.includes(item.id) ? 'Show less' : 'Show more'}</button>
                  {/if}
                </div>
                <!-- Actions: visible on hover + selected -->
                <div class="flex items-center gap-0.5 shrink-0 self-start mt-0.5 transition-opacity {i === selectedIndex ? 'opacity-100' : 'opacity-0 group-hover/card:opacity-100'}">
                  <button
                    class="p-1.5 rounded transition-colors hover:bg-[#252a30] {item.is_permanent ? 'text-amber-400' : 'text-zinc-600 hover:text-zinc-200'}"
                    onclick={(e) => { e.stopPropagation(); togglePermanent(item); }}
                    title="Pin"
                  >
                    <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill={item.is_permanent ? "currentColor" : "none"} stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                      <line x1="12" y1="17" x2="12" y2="22"/><path d="M5 17h14v-1.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V6a3 3 0 0 0-3-3 3 3 0 0 0-3 3v4.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24Z"/>
                    </svg>
                  </button>
                  <button
                    class="p-1.5 rounded text-zinc-600 hover:text-zinc-200 hover:bg-[#252a30] transition-colors"
                    onclick={(e) => { e.stopPropagation(); categorizingItemId = item.id; newGroupName = ""; isCategorizing = true; }}
                    title="Add to group"
                  >
                    <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/><line x1="12" y1="11" x2="12" y2="17"/><line x1="9" y1="14" x2="15" y2="14"/>
                    </svg>
                  </button>
                  <button
                    class="p-1.5 rounded text-zinc-600 hover:text-[#FF8A3D] hover:bg-[#252a30] transition-colors"
                    onclick={(e) => { e.stopPropagation(); deleteItem(item); }}
                    title="Delete"
                  >
                    <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/>
                    </svg>
                  </button>
                </div>
              </div>

              <!-- Meta row -->
              <div class="flex items-center justify-between mt-2 pt-2 border-t border-[#1e2228]/60">
                <div class="flex items-center gap-1.5 flex-wrap min-w-0">
                  <div class="w-3.5 h-3.5 flex items-center justify-center text-zinc-600 shrink-0">
                    <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                      {@html getCategoryIcon(item.category)}
                    </svg>
                  </div>
                  {#if item.groups && item.groups.length > 0}
                    {#each item.groups as grp}
                      <div class="flex items-center gap-0.5 text-[9px] font-medium uppercase py-0.5 px-1.5 bg-[#1e2228] text-[#AEB291] rounded border border-[#AEB291]/10">
                        <button onclick={(e) => { e.stopPropagation(); selectedGroup = grp; }}>{grp}</button>
                        {#if selectedGroup === grp}
                          <button onclick={(e) => { e.stopPropagation(); removeFromGroup(item, grp); }} class="hover:text-white ml-0.5 hover:bg-red-500/15 rounded-full px-0.5" title="Remove from group">×</button>
                        {/if}
                      </div>
                    {/each}
                  {:else if item.category}
                    <button
                      class="text-[9px] font-medium uppercase py-0.5 px-1.5 bg-[#1e2228] text-[#AEB291] rounded border border-[#AEB291]/10 hover:border-[#AEB291]/22 transition-colors"
                      onclick={(e) => { e.stopPropagation(); selectedGroup = item.category; }}
                    >{item.category}</button>
                  {/if}
                  {#if item.is_manual}
                    <span class="text-[9px] font-medium px-1.5 py-0.5 bg-[#FF8A3D]/8 text-[#FF8A3D]/60 rounded border border-[#FF8A3D]/10">manual</span>
                  {/if}
                  <span class="text-[10px] text-zinc-600">
                    {new Date(item.created_at).toLocaleString([], { dateStyle: "medium", timeStyle: "short" })}
                  </span>
                </div>
                <button
                  class="text-[10px] font-medium text-zinc-600 hover:text-[#FF8A3D] transition-colors flex items-center gap-0.5 shrink-0 ml-2"
                  onclick={(e) => { e.stopPropagation(); copyAndPaste(item); }}
                >
                  Copy
                  <svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="9 18 15 12 9 6"/></svg>
                </button>
              </div>
            </div>
          {/each}
        </div>

        {#if history.length === 0}
          <div class="flex flex-col items-center justify-center h-full py-16 text-center">
            <div class="w-12 h-12 rounded bg-[#17191c] flex items-center justify-center mb-3 text-zinc-700">
              <svg xmlns="http://www.w3.org/2000/svg" width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
              </svg>
            </div>
            <p class="text-sm font-medium text-zinc-500">No clips found</p>
            <p class="text-xs text-zinc-700 mt-0.5">
              {selectedGroup ? `Nothing in "${selectedGroup}"` : "Copy something to see it here"}
            </p>
          </div>
        {/if}
      </div>
    </main>
  </div>

  <!-- Export Modal -->
  {#if showExportModal}
    <div class="fixed inset-0 bg-black/70 backdrop-blur-sm z-50 flex items-center justify-center p-4">
      <div class="w-full max-w-sm bg-[#17191c] border border-[#252a30] rounded shadow-2xl p-5">
        <h3 class="text-sm font-semibold text-white mb-4">Export Data</h3>
        <div class="max-h-52 overflow-y-auto mb-4 border border-[#252a30] rounded p-1.5 space-y-0.5">
          <label class="flex items-center gap-2.5 text-xs text-zinc-400 p-2 hover:bg-[#1e2228] rounded cursor-pointer">
            <input type="checkbox" checked={exportSelectedGroups.length === 0} onchange={() => (exportSelectedGroups = [])} class="rounded border-zinc-600 bg-[#1e2228] text-[#FF8A3D]" />
            <span class="font-medium">All Data</span>
          </label>
          <div class="h-px bg-[#252a30]"></div>
          {#each groups as group}
            <label class="flex items-center gap-2.5 text-xs text-zinc-400 p-2 hover:bg-[#1e2228] rounded cursor-pointer">
              <input type="checkbox" checked={exportSelectedGroups.includes(group)} onchange={(e) => { if (e.currentTarget.checked) { exportSelectedGroups = [...exportSelectedGroups, group]; } else { exportSelectedGroups = exportSelectedGroups.filter(g => g !== group); } }} class="rounded border-zinc-600 bg-[#1e2228] text-[#FF8A3D]" />
              <span>{group}</span>
            </label>
          {/each}
        </div>
        <div class="flex justify-end gap-2">
          <button class="h-8 px-3 text-xs text-zinc-500 hover:text-white transition-colors" onclick={() => (showExportModal = false)}>Cancel</button>
          <button class="h-8 px-4 bg-[#FF8A3D] text-[#111214] rounded text-xs font-semibold hover:bg-[#f07d34] transition-colors disabled:opacity-40" onclick={performExport} disabled={processingIO}>{processingIO ? "Exporting..." : "Export"}</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Import Modal -->
  {#if showImportModal}
    <div class="fixed inset-0 bg-black/70 backdrop-blur-sm z-50 flex items-center justify-center p-4">
      <div class="w-full max-w-sm bg-[#17191c] border border-[#252a30] rounded shadow-2xl p-5">
        <h3 class="text-sm font-semibold text-white mb-4">Import Data</h3>
        <div class="space-y-2 mb-5">
          <label class="flex items-center gap-3 p-3 border rounded cursor-pointer transition-colors {importMode === 'merge' ? 'border-[#AEB291]/25 bg-[#1e2228]' : 'border-[#252a30] hover:bg-[#1e2228]'}">
            <input type="radio" name="importMode" value="merge" bind:group={importMode} class="text-[#FF8A3D]" />
            <div>
              <div class="text-white text-xs font-semibold">Merge</div>
              <div class="text-[10px] text-zinc-500 mt-0.5">Combine with existing data</div>
            </div>
          </label>
          <label class="flex items-center gap-3 p-3 border rounded cursor-pointer transition-colors {importMode === 'replace' ? 'border-[#AEB291]/25 bg-[#1e2228]' : 'border-[#252a30] hover:bg-[#1e2228]'}">
            <input type="radio" name="importMode" value="replace" bind:group={importMode} class="text-[#FF8A3D]" />
            <div>
              <div class="text-white text-xs font-semibold">Replace</div>
              <div class="text-[10px] text-zinc-500 mt-0.5">Overwrite all existing data</div>
            </div>
          </label>
        </div>
        <div class="flex justify-end gap-2">
          <button class="h-8 px-3 text-xs text-zinc-500 hover:text-white transition-colors" onclick={() => (showImportModal = false)}>Cancel</button>
          <button class="h-8 px-4 bg-[#FF8A3D] text-[#111214] rounded text-xs font-semibold hover:bg-[#f07d34] transition-colors disabled:opacity-40" onclick={performImport} disabled={processingIO}>{processingIO ? "Importing..." : "Select File"}</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Manage Groups Modal -->
  {#if isViewingGroups}
    <div class="fixed inset-0 bg-black/70 backdrop-blur-sm z-50 flex items-center justify-center p-4">
      <div class="w-full max-w-lg bg-[#17191c] border border-[#252a30] rounded shadow-2xl overflow-hidden flex flex-col max-h-[80vh]">
        <div class="px-5 py-4 border-b border-[#252a30] flex justify-between items-center">
          <div>
            <h2 class="text-sm font-semibold text-white">Manage Groups</h2>
            <p class="text-[11px] text-zinc-500 mt-0.5">Create and organize your clips</p>
          </div>
          <button onclick={() => (isViewingGroups = false)} class="text-zinc-500 hover:text-white transition-colors" aria-label="Close manage groups dialog">
            <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
          </button>
        </div>
        <div class="p-5 border-b border-[#252a30]">
          <div class="flex items-center gap-2">
            <input
              type="text"
              bind:value={newGroupName}
              placeholder="New group name..."
              class="flex-1 bg-[#1e2228] border border-[#252a30] rounded px-3 py-2 text-sm focus:outline-none focus:border-[#AEB291]/30 transition-colors placeholder:text-zinc-600"
              onkeydown={(e) => { if (e.key === "Enter") createGroup(); }}
            />
            <button onclick={createGroup} class="h-9 px-4 bg-[#AEB291] hover:bg-[#9ea382] text-[#111214] rounded text-xs font-semibold transition-colors">
              Create
            </button>
          </div>
        </div>
        <div class="flex-1 overflow-y-auto custom-scrollbar p-4 space-y-1.5">
          {#each groups as group}
            <div class="flex items-center justify-between p-3 bg-[#1e2228] rounded border border-transparent hover:border-[#252a30] transition-all group/mg">
              <div class="flex items-center gap-3 flex-1 min-w-0">
                <div class="w-8 h-8 rounded bg-[#17191c] flex items-center justify-center text-[#FF8A3D]/50 shrink-0">
                  <svg xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
                </div>
                {#if editingGroup === group}
                  <input
                    type="text"
                    bind:value={editGroupName}
                    class="flex-1 bg-[#17191c] text-sm px-2.5 py-1.5 rounded focus:outline-none text-white border border-[#AEB291]/25"
                    onblur={renameGroup}
                    onkeydown={(e) => { if (e.key === "Enter") renameGroup(); if (e.key === "Escape") editingGroup = null; }}
                  />
                {:else}
                  <span class="text-sm font-medium text-white truncate">{group}</span>
                {/if}
              </div>
              <div class="flex items-center gap-1">
                <button onclick={() => { editingGroup = group; editGroupName = group; }} class="p-1.5 text-zinc-600 hover:text-white hover:bg-[#252a30] rounded transition-colors" title="Rename">
                  <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
                </button>
                <button onclick={() => exportGroup(group)} class="p-1.5 text-zinc-600 hover:text-white hover:bg-[#252a30] rounded transition-colors" title="Export">
                  <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
                </button>
                <button onclick={() => deleteGroup(group)} class="p-1.5 text-zinc-600 hover:text-[#FF8A3D] hover:bg-[#252a30] rounded transition-colors" title="Delete">
                  <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/></svg>
                </button>
              </div>
            </div>
          {:else}
            <div class="flex flex-col items-center justify-center py-10 text-center">
              <div class="w-12 h-12 bg-[#1e2228] rounded flex items-center justify-center mb-3 text-zinc-700">
                <svg xmlns="http://www.w3.org/2000/svg" width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
              </div>
              <p class="text-sm font-medium text-zinc-500">No groups yet</p>
              <p class="text-xs text-zinc-700 mt-0.5">Create your first group above</p>
            </div>
          {/each}
        </div>
      </div>
    </div>
  {/if}

  <!-- Move to Group popup -->
  {#if isCategorizing}
    <div class="fixed inset-0 bg-black/70 backdrop-blur-sm z-50 flex items-center justify-center p-4">
      <div class="w-full max-w-[260px] bg-[#17191c] border border-[#252a30] rounded shadow-2xl overflow-hidden flex flex-col max-h-[70vh]">
        <div class="px-3 py-2.5 border-b border-[#252a30] flex justify-between items-center">
          <span class="text-[11px] font-semibold text-zinc-400">Save to Group</span>
          <button onclick={() => (isCategorizing = false)} aria-label="Close" class="text-zinc-600 hover:text-white transition-colors">
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
          </button>
        </div>
        <div class="p-2 border-b border-[#252a30]">
          <input
            type="text"
            bind:value={newGroupName}
            placeholder="Type or search group..."
            class="w-full bg-[#1e2228] border border-[#252a30] rounded px-3 py-1.5 text-xs focus:outline-none focus:border-[#AEB291]/30 transition-colors placeholder:text-zinc-600"
            onkeydown={(e) => { if (e.key === "Enter") moveItemToGroup(); if (e.key === "Escape") isCategorizing = false; }}
          />
        </div>
        <div class="flex-1 overflow-y-auto custom-scrollbar p-1.5 space-y-0.5">
          {#each groups.filter(g => g.toLowerCase().includes(newGroupName.toLowerCase())) as group}
            <button
              onclick={() => { newGroupName = group; moveItemToGroup(); }}
              class="w-full text-left px-3 py-2 text-xs hover:bg-[#1e2228] rounded transition-colors flex items-center gap-2 text-zinc-400 hover:text-white"
            >
              <svg xmlns="http://www.w3.org/2000/svg" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="text-[#FF8A3D]/50 shrink-0"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
              {group}
            </button>
          {/each}
          {#if newGroupName && !groups.includes(newGroupName)}
            <button
              onclick={moveItemToGroup}
              class="w-full text-left px-3 py-2 text-xs hover:bg-[#FF8A3D]/8 rounded transition-colors flex items-center gap-2 text-[#FF8A3D]"
            >
              <svg xmlns="http://www.w3.org/2000/svg" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
              Create "{newGroupName}"
            </button>
          {/if}
        </div>
        <div class="p-2 border-t border-[#252a30] flex justify-end">
          <button onclick={moveItemToGroup} class="h-7 px-4 bg-[#AEB291] hover:bg-[#9ea382] text-[#111214] rounded text-xs font-semibold transition-colors disabled:opacity-40" disabled={!newGroupName.trim()}>
            Save
          </button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Help Modal -->
  {#if showHelpModal}
    <div
      class="fixed inset-0 bg-black/70 backdrop-blur-sm z-50 flex items-center justify-center p-4"
      onclick={(e) => { if (e.target === e.currentTarget) showHelpModal = false; }}
      onkeydown={(e) => { if (e.key === "Escape") showHelpModal = false; }}
      role="dialog" aria-modal="true" tabindex="-1"
    >
      <div class="w-full max-w-xl bg-[#17191c] border border-[#252a30] rounded shadow-2xl overflow-hidden">
        <div class="px-5 py-4 border-b border-[#252a30] flex items-center justify-between">
          <h3 class="text-sm font-semibold text-white">Help & Shortcuts</h3>
          <button onclick={() => (showHelpModal = false)} class="text-zinc-500 hover:text-white transition-colors" aria-label="Close help dialog">
            <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
          </button>
        </div>
        <div class="p-5 max-h-[65vh] overflow-y-auto custom-scrollbar space-y-5">
          <div>
            <h4 class="text-[10px] font-semibold uppercase tracking-wider text-[#AEB291] mb-2.5">Opening Windows</h4>
            <div class="space-y-1.5">
              <div class="flex items-start gap-3 p-2.5 bg-[#1e2228] rounded text-sm">
                <div class="text-zinc-400 font-mono text-[11px] shrink-0 mt-0.5">
                  <kbd class="px-1.5 py-0.5 bg-[#17191c] rounded border border-[#252a30] text-xs">{altKey}</kbd>+<kbd class="px-1.5 py-0.5 bg-[#17191c] rounded border border-[#252a30] text-xs">V</kbd>
                </div>
                <div>
                  <div class="text-white text-xs font-medium">Quick Popup</div>
                  <div class="text-zinc-500 text-[11px]">Opens the quick access popup</div>
                </div>
              </div>
              <div class="flex items-start gap-3 p-2.5 bg-[#1e2228] rounded text-sm">
                <div class="text-zinc-400 text-[11px] shrink-0 mt-0.5">Tray Icon</div>
                <div>
                  <div class="text-white text-xs font-medium">Main Window</div>
                  <div class="text-zinc-500 text-[11px]">Click the tray icon</div>
                </div>
              </div>
            </div>
          </div>
          <div>
            <h4 class="text-[10px] font-semibold uppercase tracking-wider text-[#AEB291] mb-2.5">Navigation</h4>
            <div class="space-y-0.5">
              <div class="flex items-center justify-between p-2 hover:bg-[#1e2228] rounded text-xs">
                <span class="text-zinc-300">Navigate items</span>
                <div class="flex gap-1">
                  <kbd class="px-1.5 py-0.5 bg-[#1e2228] rounded border border-[#252a30] text-zinc-400">↑</kbd>
                  <kbd class="px-1.5 py-0.5 bg-[#1e2228] rounded border border-[#252a30] text-zinc-400">↓</kbd>
                </div>
              </div>
              <div class="flex items-center justify-between p-2 hover:bg-[#1e2228] rounded text-xs">
                <span class="text-zinc-300">Close Window</span>
                <kbd class="px-1.5 py-0.5 bg-[#1e2228] rounded border border-[#252a30] text-zinc-400">Esc</kbd>
              </div>
            </div>
          </div>
          <div>
            <h4 class="text-[10px] font-semibold uppercase tracking-wider text-[#AEB291] mb-2.5">Actions</h4>
            <div class="space-y-0.5">
              <div class="flex items-center justify-between p-2 hover:bg-[#1e2228] rounded text-xs">
                <span class="text-zinc-300">Copy Selected</span>
                <kbd class="px-1.5 py-0.5 bg-[#1e2228] rounded border border-[#252a30] text-zinc-400">Enter</kbd>
              </div>
              <div class="flex items-center justify-between p-2 hover:bg-[#1e2228] rounded text-xs">
                <span class="text-zinc-300">Delete Item</span>
                <div class="flex items-center gap-1">
                  <kbd class="px-1.5 py-0.5 bg-[#1e2228] rounded border border-[#252a30] text-zinc-400">Delete</kbd>
                  <span class="text-zinc-600">/</span>
                  <kbd class="px-1.5 py-0.5 bg-[#1e2228] rounded border border-[#252a30] text-zinc-400">{modKey}+{deleteKey}</kbd>
                </div>
              </div>
              <div class="flex items-center justify-between p-2 hover:bg-[#1e2228] rounded text-xs">
                <span class="text-zinc-300">Pin / Unpin</span>
                <kbd class="px-1.5 py-0.5 bg-[#1e2228] rounded border border-[#252a30] text-zinc-400">{modKey}+P</kbd>
              </div>
              <div class="flex items-center justify-between p-2 hover:bg-[#1e2228] rounded text-xs">
                <span class="text-zinc-300">Add to Group</span>
                <kbd class="px-1.5 py-0.5 bg-[#1e2228] rounded border border-[#252a30] text-zinc-400">{modKey}+C</kbd>
              </div>
              <div class="flex items-center justify-between p-2 hover:bg-[#1e2228] rounded text-xs">
                <span class="text-zinc-300">Manage Groups</span>
                <kbd class="px-1.5 py-0.5 bg-[#1e2228] rounded border border-[#252a30] text-zinc-400">{modKey}+G</kbd>
              </div>
            </div>
          </div>
          <div>
            <h4 class="text-[10px] font-semibold uppercase tracking-wider text-[#AEB291] mb-2.5">Features</h4>
            <ul class="space-y-1.5 text-xs text-zinc-400">
              <li class="flex items-start gap-2"><span class="text-[#FF8A3D] mt-0.5 shrink-0">•</span><span><strong class="text-zinc-200">Smart Groups</strong> — auto-categorizes URLs and Text</span></li>
              <li class="flex items-start gap-2"><span class="text-[#FF8A3D] mt-0.5 shrink-0">•</span><span><strong class="text-zinc-200">Custom Groups</strong> — create and organize clips into categories</span></li>
              <li class="flex items-start gap-2"><span class="text-[#FF8A3D] mt-0.5 shrink-0">•</span><span><strong class="text-zinc-200">Pin Items</strong> — keep important items permanently</span></li>
              <li class="flex items-start gap-2"><span class="text-[#FF8A3D] mt-0.5 shrink-0">•</span><span><strong class="text-zinc-200">Add Manually</strong> — add text with a description to any group</span></li>
              <li class="flex items-start gap-2"><span class="text-[#FF8A3D] mt-0.5 shrink-0">•</span><span><strong class="text-zinc-200">Privacy First</strong> — all data stored locally, no cloud</span></li>
            </ul>
          </div>
        </div>
        <div class="px-5 py-3 border-t border-[#252a30] flex justify-end">
          <button class="h-8 px-4 bg-[#FF8A3D] text-[#111214] rounded text-xs font-semibold hover:bg-[#f07d34] transition-colors" onclick={() => (showHelpModal = false)}>Got it</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- About Modal -->
  {#if showAboutModal}
    <div
      class="fixed inset-0 bg-black/70 backdrop-blur-sm z-50 flex items-center justify-center p-4"
      onclick={(e) => { if (e.target === e.currentTarget) showAboutModal = false; }}
      onkeydown={(e) => { if (e.key === "Escape") showAboutModal = false; }}
      role="dialog" aria-modal="true" tabindex="-1"
    >
      <div class="w-full max-w-sm bg-[#17191c] border border-[#252a30] rounded shadow-2xl overflow-hidden">
        <div class="px-5 py-4 border-b border-[#252a30] flex items-center justify-between">
          <div class="flex items-center gap-3">
            <img src="/logo.png" alt="Ortu" class="w-8 h-8 rounded" />
            <div>
              <h3 class="text-sm font-semibold text-white">Ortu</h3>
              <p class="text-[11px] text-zinc-500">Clipboard Manager</p>
            </div>
          </div>
          <button onclick={() => (showAboutModal = false)} class="text-zinc-500 hover:text-white transition-colors" aria-label="Close about dialog">
            <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
          </button>
        </div>
        <div class="p-5 space-y-4">
          <div class="text-center space-y-1">
            <div class="text-2xl font-bold text-white">v{appVersion}</div>
            <p class="text-xs text-zinc-500">Privacy-focused clipboard manager</p>
          </div>
          <div class="p-3 bg-[#1e2228] rounded">
            <div class="text-[10px] text-zinc-600 uppercase tracking-wider mb-1">Developer</div>
            <div class="text-sm font-medium text-white">Abhijith P Subash</div>
          </div>
          <a href="https://www.linkedin.com/in/abhijith-p-subash-the-engineer/" target="_blank" rel="noopener noreferrer" class="flex items-center justify-between p-3 bg-[#1e2228] hover:bg-[#252b33] rounded transition-colors group">
            <div class="flex items-center gap-2.5">
              <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="currentColor" class="text-[#0A66C2] shrink-0">
                <path d="M20.447 20.452h-3.554v-5.569c0-1.328-.027-3.037-1.852-3.037-1.853 0-2.136 1.445-2.136 2.939v5.667H9.351V9h3.414v1.561h.046c.477-.9 1.637-1.85 3.37-1.85 3.601 0 4.267 2.37 4.267 5.455v6.286zM5.337 7.433c-1.144 0-2.063-.926-2.063-2.065 0-1.138.92-2.063 2.063-2.063 1.14 0 2.064.925 2.064 2.063 0 1.139-.925 2.065-2.064 2.065zm1.782 13.019H3.555V9h3.564v11.452zM22.225 0H1.771C.792 0 0 .774 0 1.729v20.542C0 23.227.792 24 1.771 24h20.451C23.2 24 24 23.227 24 22.271V1.729C24 .774 23.2 0 22.222 0h.003z"/>
              </svg>
              <div>
                <div class="text-xs font-medium text-white">LinkedIn</div>
                <div class="text-[10px] text-zinc-500">@abhijith-p-subash-the-engineer</div>
              </div>
            </div>
            <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="text-zinc-600 group-hover:text-zinc-400 transition-colors"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>
          </a>
          <div class="p-3 bg-[#FF8A3D]/6 rounded border border-[#FF8A3D]/10">
            <div class="text-[10px] text-[#FF8A3D]/60 uppercase tracking-wider mb-1">Privacy First</div>
            <p class="text-[11px] text-zinc-400">All data stored locally. No cloud, no tracking.</p>
          </div>
          <p class="text-center text-[10px] text-zinc-600">© 2025 Ortu. All rights reserved.</p>
        </div>
        <div class="px-5 py-3 border-t border-[#252a30] flex justify-end">
          <button class="h-8 px-4 bg-[#FF8A3D] text-[#111214] rounded text-xs font-semibold hover:bg-[#f07d34] transition-colors" onclick={() => (showAboutModal = false)}>Close</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Add Item Modal -->
  {#if showAddItemModal}
    <div
      class="fixed inset-0 bg-black/70 backdrop-blur-sm z-50 flex items-center justify-center p-4"
      onclick={(e) => { if (e.target === e.currentTarget) showAddItemModal = false; }}
      onkeydown={(e) => { if (e.key === "Escape") showAddItemModal = false; }}
      role="dialog" aria-modal="true" tabindex="-1"
    >
      <div class="w-full max-w-md bg-[#17191c] border border-[#252a30] rounded shadow-2xl overflow-hidden">
        <div class="px-5 py-4 border-b border-[#252a30] flex items-center justify-between">
          <div>
            <h3 class="text-sm font-semibold text-white">Add Item</h3>
            <p class="text-[11px] text-zinc-500 mt-0.5">Manually add text to your library</p>
          </div>
          <button onclick={() => (showAddItemModal = false)} class="text-zinc-500 hover:text-white transition-colors" aria-label="Close">
            <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
          </button>
        </div>
        <div class="p-5 space-y-3.5">
          <div>
            <label for="item-desc" class="block text-[10px] font-semibold uppercase tracking-wider text-zinc-500 mb-1.5">
              Description <span class="normal-case tracking-normal font-normal text-zinc-600">(optional)</span>
            </label>
            <input id="item-desc" type="text" bind:value={newItemDescription} placeholder="e.g. AWS prod key, API endpoint..." class="w-full bg-[#1e2228] border border-[#252a30] rounded px-3 py-2 text-sm focus:outline-none focus:border-[#AEB291]/30 transition-colors placeholder:text-zinc-600" maxlength="120" />
          </div>
          <div>
            <label for="item-content" class="block text-[10px] font-semibold uppercase tracking-wider text-zinc-500 mb-1.5">
              Content <span class="text-[#FF8A3D]">*</span>
            </label>
            <textarea id="item-content" bind:value={newItemContent} placeholder="Paste or type content here..." rows="5" class="w-full bg-[#1e2228] border border-[#252a30] rounded px-3 py-2 text-sm focus:outline-none focus:border-[#AEB291]/30 transition-colors placeholder:text-zinc-600 resize-none" onkeydown={(e) => { if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) addManualItem(); }}></textarea>
          </div>
          <div>
            <label for="item-group" class="block text-[10px] font-semibold uppercase tracking-wider text-zinc-500 mb-1.5">
              Group <span class="normal-case tracking-normal font-normal text-zinc-600">(optional)</span>
            </label>
            <input id="item-group" type="text" bind:value={newItemGroupInput} placeholder="Group name or leave empty" list="groups-datalist" class="w-full bg-[#1e2228] border border-[#252a30] rounded px-3 py-2 text-sm focus:outline-none focus:border-[#AEB291]/30 transition-colors placeholder:text-zinc-600" onkeydown={(e) => { if (e.key === "Enter") addManualItem(); }} />
            <datalist id="groups-datalist">{#each groups as g}<option value={g}></option>{/each}</datalist>
          </div>
        </div>
        <div class="px-5 pb-5 flex items-center justify-between">
          <span class="text-[10px] text-zinc-700">{modKey}+↵ to save</span>
          <div class="flex gap-2">
            <button onclick={() => (showAddItemModal = false)} class="h-8 px-3 text-xs text-zinc-500 hover:text-white transition-colors">Cancel</button>
            <button onclick={addManualItem} disabled={!newItemContent.trim()} class="h-8 px-4 bg-[#FF8A3D] text-[#111214] rounded text-xs font-semibold hover:bg-[#f07d34] transition-colors disabled:opacity-40 disabled:cursor-not-allowed">
              Add Item
            </button>
          </div>
        </div>
      </div>
    </div>
  {/if}

  <!-- Confirm Modal -->
  {#if confirmModal}
    <div
      class="fixed inset-0 bg-black/70 backdrop-blur-sm z-50 flex items-center justify-center p-4"
      onclick={(e) => { if (e.target === e.currentTarget) confirmModal = null; }}
      onkeydown={(e) => { if (e.key === "Escape") confirmModal = null; }}
      role="dialog" aria-modal="true" tabindex="-1"
    >
      <div class="w-full max-w-xs bg-[#17191c] border border-[#252a30] rounded shadow-2xl p-5">
        <p class="text-sm text-zinc-200 mb-4">{confirmModal.message}</p>
        <div class="flex justify-end gap-2">
          <button onclick={() => (confirmModal = null)} class="h-8 px-3 text-xs text-zinc-500 hover:text-white transition-colors">Cancel</button>
          <button
            onclick={() => { const cb = confirmModal?.onConfirm; confirmModal = null; cb?.(); }}
            class="h-8 px-4 bg-[#FF8A3D] text-[#111214] rounded text-xs font-semibold hover:bg-[#f07d34] transition-colors"
          >Confirm</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Import Group Name Modal -->
  {#if showImportGroupModal}
    <div class="fixed inset-0 bg-black/70 backdrop-blur-sm z-50 flex items-center justify-center p-4" role="dialog" aria-modal="true" tabindex="-1">
      <div class="w-full max-w-xs bg-[#17191c] border border-[#252a30] rounded shadow-2xl p-5">
        <h3 class="text-sm font-semibold text-white mb-3">Name imported group</h3>
        <input
          type="text"
          bind:value={importGroupNameInput}
          placeholder="Group name..."
          class="w-full bg-[#1e2228] border border-[#252a30] rounded px-3 py-2 text-sm focus:outline-none focus:border-[#AEB291]/30 transition-colors mb-3"
          onkeydown={(e) => { if (e.key === "Enter") confirmImportGroup(); if (e.key === "Escape") showImportGroupModal = false; }}
        />
        <div class="flex justify-end gap-2">
          <button onclick={() => (showImportGroupModal = false)} class="h-8 px-3 text-xs text-zinc-500 hover:text-white transition-colors">Cancel</button>
          <button onclick={confirmImportGroup} disabled={!importGroupNameInput.trim()} class="h-8 px-4 bg-[#AEB291] hover:bg-[#9ea382] text-[#111214] rounded text-xs font-semibold transition-colors disabled:opacity-40">
            Import
          </button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Copied Toast -->
  {#if showCopiedToast}
    <div class="fixed bottom-4 right-4 bg-[#17191c] border border-green-500/25 rounded shadow-xl px-3 py-2 flex items-center gap-2 z-50">
      <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" class="text-green-500"><polyline points="20 6 9 17 4 12"/></svg>
      <span class="text-xs font-medium text-white">Copied</span>
    </div>
  {/if}

  <!-- Toast stack -->
  <div class="fixed bottom-4 left-4 z-50 flex flex-col gap-1.5 pointer-events-none">
    {#each toasts as toast (toast.id)}
      <div class="pointer-events-auto px-3 py-2 rounded border shadow-xl flex items-center gap-2.5
        {toast.type === 'success' ? 'bg-[#17191c] border-green-500/20' :
         toast.type === 'error'   ? 'bg-[#17191c] border-red-500/20' :
                                    'bg-[#17191c] border-[#252a30]'}">
        {#if toast.type === 'success'}
          <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" class="text-green-500 shrink-0"><polyline points="20 6 9 17 4 12"/></svg>
        {:else if toast.type === 'error'}
          <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" class="text-red-400 shrink-0"><circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/></svg>
        {:else}
          <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" class="text-[#AEB291] shrink-0"><circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/></svg>
        {/if}
        <span class="text-xs text-zinc-200">{toast.message}</span>
      </div>
    {/each}
  </div>
</div>

<style>
  :global(body) {
    margin: 0;
    overflow: hidden;
    background: transparent;
  }
</style>
