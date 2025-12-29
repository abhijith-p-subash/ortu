<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { ClipboardItem } from "$lib/types";
  import { listen } from "@tauri-apps/api/event";
  import { save, open } from "@tauri-apps/plugin-dialog";
  import { platform } from "@tauri-apps/plugin-os";
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

  // Toast notification state
  let showCopiedToast = $state(false);
  let copiedToastTimer: number | null = null;

  // Detect platform
  onMount(async () => {
    try {
      currentPlatform = await platform();
    } catch (e) {
      console.error("Failed to detect platform:", e);
    }
  });

  // OS-specific key labels
  let modKey = $derived(currentPlatform === "macos" ? "Cmd" : "Ctrl");
  let deleteKey = $derived(currentPlatform === "macos" ? "⌫" : "Backspace");
  let altKey = $derived(currentPlatform === "macos" ? "Option" : "Alt");

  async function loadHistory() {
    try {
      let prefix = "category:";
      // Check if it's a known smart group
      if (["URL", "Images", "Text"].includes(selectedGroup || "")) {
        prefix = "group:";
      }

      const search = selectedGroup
        ? `${prefix}${selectedGroup} ${searchQuery}`
        : searchQuery;
      const data = (await invoke("get_history", {
        search: search || null,
      })) as ClipboardItem[];
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
    if (
      !confirm(
        `Are you sure you want to delete group "${name}"? Items will NOT be deleted.`
      )
    )
      return;
    try {
      await invoke("delete_group", { name });
      if (selectedGroup === name) selectedGroup = null;
      await loadGroups();
      await loadHistory();
    } catch (e) {
      console.error("Failed to delete group:", e);
    }
  }

  async function renameGroup() {
    if (!editingGroup || !editGroupName.trim()) return;
    try {
      await invoke("rename_group", {
        oldName: editingGroup,
        newName: editGroupName.trim(),
      });
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
    if (
      selectedGroup &&
      !["URL", "Dev", "Code", "Images", "Text"].includes(selectedGroup)
    ) {
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
      alert("Export successful!");
    } catch (e) {
      console.error("Export failed:", e);
      alert("Export failed: " + e);
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
      const path = await open({
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (path) {
        processingIO = true;
        // path is string, explicit cast to correct type if needed (open returns string|null|string[])
        // multiple: false by default so it returns string|null
        await invoke("restore_data", {
          path: path as string,
          mode: importMode,
        });
        showImportModal = false;
        await loadHistory();
        await loadGroups();
        alert("Import successful!");
      }
    } catch (e) {
      console.error(e);
      alert("Error: " + e);
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
        alert("Export successful!");
      }
    } catch (e) {
      console.error("Failed to export group:", e);
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
        alert("Full export successful!");
      }
    } catch (e) {
      console.error("Failed to export all:", e);
    }
  }

  async function importGroup() {
    try {
      const path = await open({
        filters: [{ name: "Text", extensions: ["txt"] }],
      });
      if (path && typeof path === "string") {
        const groupName = prompt("Enter name for the imported group:");
        if (groupName) {
          await invoke("import_group", { name: groupName, path });
          await loadGroups();
          await loadHistory();
          alert("Import successful!");
        }
      }
    } catch (e) {
      console.error("Failed to import group:", e);
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
      await invoke("add_to_group", {
        itemId: itemId,
        groupName: newGroupName.trim(),
      });
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
      await navigator.clipboard.writeText(item.raw_content);
      // await invoke("close_window");
      console.log("Item copied to clipboard");
      await invoke("paste_item");

      // Show copied toast
      if (copiedToastTimer) clearTimeout(copiedToastTimer);
      showCopiedToast = true;
      copiedToastTimer = window.setTimeout(() => {
        showCopiedToast = false;
        copiedToastTimer = null;
      }, 2000);
    } catch (err) {
      console.error("Failed to copy and paste: ", err);
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
      selectedIndex =
        (selectedIndex - 1 + (history.length || 1)) % (history.length || 1);
      scrollIntoView();
    } else if (e.key === "Enter") {
      if (history[selectedIndex]) {
        copyAndPaste(history[selectedIndex]);
      }
    } else if (e.key === "Delete" || (e.metaKey && e.key === "Backspace")) {
      if (history[selectedIndex]) {
        deleteItem(history[selectedIndex]);
      }
    } else if (e.key === "p" && (e.metaKey || e.ctrlKey)) {
      if (history[selectedIndex]) {
        togglePermanent(history[selectedIndex]);
      }
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
    const selectedElement = container.querySelector(
      `[data-index="${selectedIndex}"]`
    );
    selectedElement?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }

  // Auto-scroll when selection changes
  $effect(() => {
    if (selectedIndex !== undefined) {
      scrollIntoView();
    }
  });

  $effect(() => {
    if (searchQuery !== undefined || selectedGroup !== undefined) {
      loadHistory();
    }
  });

  onMount(() => {
    loadHistory();
    loadGroups();
    window.addEventListener("keydown", handleKeydown);

    let unlistenFocus: () => void;
    let unlistenClipboard: () => void;

    const setupListeners = async () => {
      try {
        const uFocus = await listen("tauri://focus", async () => {
          console.log("Main window focused - refreshing");
          await loadHistory();
          await loadGroups();
          // Reset select only if history was empty or search changed?
          // For now, keep selection if possible, but focus search.
          await tick();
          searchInput?.focus();
        });
        unlistenFocus = uFocus;

        const uClipboard = await listen("clipboard-updated", async () => {
          console.log("Clipboard update received in main window");
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
      if (unlistenFocus) unlistenFocus();
      if (unlistenClipboard) unlistenClipboard();
    };
  });

  function getCategoryIcon(category: string | null): string {
    if (!category)
      return '<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path><polyline points="14 2 14 8 20 8"></polyline><line x1="16" y1="13" x2="8" y2="13"></line><line x1="16" y1="17" x2="8" y2="17"></line><polyline points="10 9 9 9 8 9"></polyline>'; // FileText

    const c = category.toLowerCase();

    if (c === "url")
      return '<circle cx="12" cy="12" r="10"></circle><line x1="2" y1="12" x2="22" y2="12"></line><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"></path>'; // Globe

    if (
      c.includes("docker") ||
      c.includes("shell") ||
      c.includes("kubernetes") ||
      c.includes("cloud") ||
      c.includes("terminal")
    )
      return '<polyline points="4 17 10 11 4 5"></polyline><line x1="12" y1="19" x2="20" y2="19"></line>'; // Terminal

    if (c.includes("git") || c.includes("version"))
      return '<line x1="6" y1="3" x2="6" y2="15"></line><circle cx="18" cy="6" r="3"></circle><circle cx="6" cy="18" r="3"></circle><path d="M18 9a9 9 0 0 1-9 9"></path>'; // Git Branch

    if (c.includes("database") || c.includes("sql"))
      return '<ellipse cx="12" cy="5" rx="9" ry="3"></ellipse><path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"></path><path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"></path>'; // Database

    if (
      c.includes("code") ||
      c.includes("runtime") ||
      c.includes("package") ||
      c.includes("ci")
    )
      return '<polyline points="16 18 22 12 16 6"></polyline><polyline points="8 6 2 12 8 18"></polyline>'; // Code

    return '<path d="M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82z"></path><line x1="7" y1="7" x2="7.01" y2="7"></line>'; // Tag
  }
</script>

<div
  class="flex flex-col h-screen bg-[#1a1a1a] text-zinc-300 overflow-hidden font-sans selection:bg-red-500/30"
>
  <!-- Top Bar / Header -->
  <header
    class="px-4 py-3 border-b border-[#333] bg-[#1e1e1e] flex items-center justify-between shadow-sm"
  >
    <div class="flex items-center space-x-3">
      <div
        class="w-8 h-8 rounded-full flex items-center justify-center shadow-lg shadow-red-500/20"
      >
        <img src="/logo.png" alt="" srcset="" />
      </div>
      <div>
        <h1 class="text-sm font-bold text-white tracking-tight">Ortu</h1>
        <p
          class="text-[10px] text-zinc-500 font-medium uppercase tracking-widest"
        >
          Workspace History
        </p>
      </div>
    </div>
    <div class="flex items-center space-x-2">
      <button
        onclick={openExportModal}
        class="flex items-center space-x-2 px-3 py-1.5 bg-[#2a2a2a] hover:bg-[#333] rounded-md border border-[#333] transition-all text-xs font-semibold"
        title="Full Backup (.json)"
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          ><path
            d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"
          ></path><polyline points="17 21 17 13 7 13 7 21"></polyline><polyline
            points="7 3 7 8 15 8"
          ></polyline></svg
        >
        <span>Backup</span>
      </button>
      <button
        onclick={openImportModal}
        class="flex items-center space-x-2 px-3 py-1.5 bg-[#2a2a2a] hover:bg-[#333] rounded-md border border-[#333] transition-all text-xs font-semibold"
        title="Restore Data (.json)"
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          ><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path><polyline
            points="7 10 12 15 17 10"
          ></polyline><line x1="12" y1="15" x2="12" y2="3"></line></svg
        >
        <span>Restore</span>
      </button>
      <div class="w-px h-4 bg-[#333] mx-1"></div>
      <button
        onclick={exportAllTxt}
        class="flex items-center space-x-2 px-3 py-1.5 bg-[#2a2a2a] hover:bg-[#333] rounded-md border border-[#333] transition-all text-xs font-semibold"
        title="Export All to .TXT"
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          ><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" /><polyline
            points="17 8 12 3 7 8"
          /><line x1="12" y1="3" x2="12" y2="15" /></svg
        >
        <span>Export All</span>
      </button>
      <button
        onclick={() => (isViewingGroups = true)}
        class="flex items-center space-x-2 px-4 py-1.5 bg-red-500 hover:bg-red-600 rounded-md shadow-lg shadow-red-500/20 text-white transition-all text-xs font-bold"
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          ><path
            d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"
          ></path></svg
        >
        <span>Manage Groups</span>
      </button>
      <div class="w-px h-4 bg-[#333] mx-1"></div>
      <button
        onclick={() => (showHelpModal = true)}
        class="p-1.5 hover:bg-[#2a2a2a] rounded-md transition-all text-zinc-400 hover:text-white"
        title="Help"
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="18"
          height="18"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
          ><circle cx="12" cy="12" r="10"></circle><path
            d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3"
          ></path><line x1="12" y1="17" x2="12.01" y2="17"></line></svg
        >
      </button>
      <button
        onclick={() => (showAboutModal = true)}
        class="p-1.5 hover:bg-[#2a2a2a] rounded-md transition-all text-zinc-400 hover:text-white"
        title="About"
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="18"
          height="18"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
          ><circle cx="12" cy="12" r="10"></circle><line
            x1="12"
            y1="16"
            x2="12"
            y2="12"
          ></line><line x1="12" y1="8" x2="12.01" y2="8"></line></svg
        >
      </button>
    </div>
  </header>

  <div class="flex flex-1 overflow-hidden">
    <!-- Persistent Sidebar -->
    <aside class="w-64 bg-[#1e1e1e] border-r border-[#333] flex flex-col">
      <div class="p-4 border-b border-[#333]">
        <span
          class="text-[10px] font-bold uppercase tracking-widest text-zinc-500"
          >Navigation</span
        >
      </div>
      <!-- Fixed All History Button -->
      <div class="p-2 border-b border-[#333]">
        <button
          class="w-full text-left px-3 py-2 rounded-md text-sm transition-all {selectedGroup ===
          null
            ? 'bg-red-500/10 text-red-500 font-bold'
            : 'text-zinc-400 hover:bg-[#2a2a2a]'}"
          onclick={() => {
            selectedGroup = null;
          }}
        >
          <span class="flex items-center gap-2">
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
              ><path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"
              ></path><polyline points="9 22 9 12 15 12 15 22"></polyline></svg
            >
            All History
          </span>
        </button>
      </div>

      <!-- Static Smart Groups Section -->
      <div class="p-2 border-b border-[#333] space-y-1">
        <div class="pb-2 px-3">
          <span
            class="text-[10px] font-bold uppercase tracking-widest text-zinc-600"
            >Smart Groups</span
          >
        </div>

        <button
          class="w-full text-left px-3 py-2 rounded-md text-sm transition-all {selectedGroup ===
          'URL'
            ? 'bg-red-500/10 text-red-500 font-bold'
            : 'text-zinc-400 hover:bg-[#2a2a2a]'}"
          onclick={() => (selectedGroup = "URL")}
        >
          <span class="flex items-center gap-2">
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
              ><circle cx="12" cy="12" r="10"></circle><line
                x1="2"
                y1="12"
                x2="22"
                y2="12"
              ></line><path
                d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"
              ></path></svg
            >
            URLs
          </span>
        </button>

        <button
          class="w-full text-left px-3 py-2 rounded-md text-sm transition-all {selectedGroup ===
          'Images'
            ? 'bg-red-500/10 text-red-500 font-bold'
            : 'text-zinc-400 hover:bg-[#2a2a2a]'}"
          onclick={() => (selectedGroup = "Images")}
        >
          <span class="flex items-center gap-2">
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
              ><rect x="3" y="3" width="18" height="18" rx="2" ry="2"
              ></rect><circle cx="8.5" cy="8.5" r="1.5"></circle><polyline
                points="21 15 16 10 5 21"
              ></polyline></svg
            >
            Images
          </span>
        </button>

        <button
          class="w-full text-left px-3 py-2 rounded-md text-sm transition-all {selectedGroup ===
          'Text'
            ? 'bg-red-500/10 text-red-500 font-bold'
            : 'text-zinc-400 hover:bg-[#2a2a2a]'}"
          onclick={() => (selectedGroup = "Text")}
        >
          <span class="flex items-center gap-2">
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
              ><path
                d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"
              ></path><polyline points="14 2 14 8 20 8"></polyline><line
                x1="16"
                y1="13"
                x2="8"
                y2="13"
              ></line><line x1="16" y1="17" x2="8" y2="17"></line><polyline
                points="10 9 9 9 8 9"
              ></polyline></svg
            >
            Text
          </span>
        </button>
      </div>

      <!-- Static Groups Header -->
      <div class="pt-4 pb-2 px-4">
        <span
          class="text-[10px] font-bold uppercase tracking-widest text-zinc-600"
          >Groups</span
        >
      </div>

      <!-- Scrollable Groups List -->
      <div class="flex-1 overflow-y-auto custom-scrollbar px-2 space-y-1">
        {#each groups as group}
          <div
            class="group relative flex items-center rounded-md hover:bg-[#2a2a2a] transition-all {selectedGroup ===
            group
              ? 'bg-[#252525]'
              : ''}"
          >
            {#if editingGroup === group}
              <input
                type="text"
                bind:value={editGroupName}
                class="flex-1 bg-transparent text-sm px-3 py-2 focus:outline-none text-white border-b border-red-500/50"
                autofocus
                onblur={renameGroup}
                onkeydown={(e) => {
                  if (e.key === "Enter") renameGroup();
                  if (e.key === "Escape") editingGroup = null;
                }}
              />
            {:else}
              <button
                class="flex-1 text-left px-3 py-2 text-sm {selectedGroup ===
                group
                  ? 'text-red-500 font-bold'
                  : 'text-zinc-400'}"
                onclick={() => {
                  selectedGroup = group;
                }}
              >
                {group}
              </button>
              <div
                class="flex opacity-0 group-hover:opacity-100 px-1 space-x-1"
              >
                <button
                  onclick={() => {
                    editingGroup = group;
                    editGroupName = group;
                  }}
                  class="p-1 hover:text-white"
                  title="Rename"
                >
                  <svg
                    xmlns="http://www.w3.org/2000/svg"
                    width="12"
                    height="12"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                    ><path
                      d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"
                    /><path
                      d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"
                    /></svg
                  >
                </button>
                <button
                  onclick={() => exportGroup(group)}
                  class="p-1 hover:text-white"
                  title="Export"
                >
                  <svg
                    xmlns="http://www.w3.org/2000/svg"
                    width="12"
                    height="12"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                    ><path
                      d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"
                    /><polyline points="7 10 12 15 17 10" /><line
                      x1="12"
                      y1="15"
                      x2="12"
                      y2="3"
                    /></svg
                  >
                </button>
                <button
                  onclick={() => deleteGroup(group)}
                  class="p-1 hover:text-red-500"
                  title="Delete"
                >
                  <svg
                    xmlns="http://www.w3.org/2000/svg"
                    width="12"
                    height="12"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                    ><path d="M3 6h18"></path><path
                      d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"
                    ></path><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"
                    ></path></svg
                  >
                </button>
              </div>
            {/if}
          </div>
        {/each}
      </div>
    </aside>

    <!-- Main Content Area -->
    <main class="flex-1 flex flex-col bg-[#1a1a1a]">
      <!-- Search & Filter Area -->
      <div class="p-4 border-b border-[#333] flex items-center space-x-4">
        <div class="flex-1 relative">
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2.5"
            class="absolute left-3 top-1/2 -translate-y-1/2 text-zinc-600"
            ><circle cx="11" cy="11" r="8"></circle><line
              x1="21"
              y1="21"
              x2="16.65"
              y2="16.65"
            ></line></svg
          >
          <input
            type="text"
            bind:this={searchInput}
            bind:value={searchQuery}
            placeholder={selectedGroup
              ? `Search in ${selectedGroup}...`
              : "Search all clips..."}
            class="w-full bg-[#1e1e1e] border border-[#333] rounded-lg pl-10 pr-4 py-2 text-sm focus:outline-none focus:border-red-500/50 transition-all placeholder:text-zinc-600 font-medium"
          />
        </div>
        {#if selectedGroup}
          <div
            class="flex items-center bg-red-500/10 border border-red-500/20 rounded-md py-1.5 px-3"
          >
            <span
              class="text-[10px] font-bold text-red-500 uppercase tracking-widest"
              >{selectedGroup}</span
            >
            <button
              onclick={() => (selectedGroup = null)}
              class="ml-2 text-red-500/50 hover:text-red-500"
            >
              <svg
                xmlns="http://www.w3.org/2000/svg"
                width="12"
                height="12"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2.5"
                ><line x1="18" y1="6" x2="6" y2="18"></line><line
                  x1="6"
                  y1="6"
                  x2="18"
                  y2="18"
                ></line></svg
              >
            </button>
          </div>
        {/if}
      </div>

      <!-- History List -->
      <div
        class="flex-1 overflow-y-auto custom-scrollbar p-4"
        bind:this={container}
      >
        <div class="grid gap-2">
          {#each history as item, i (item.id)}
            <div
              class="w-full p-4 rounded-xl border border-[#333] bg-[#1e1e1e] hover:bg-[#252525] transition-all group flex flex-col space-y-3 cursor-default
                            {i === selectedIndex
                ? 'ring-2 ring-red-500/50 bg-[#252525]'
                : ''}"
              onclick={() => {
                selectedIndex = i;
              }}
              role="button"
              tabindex="0"
              data-index={i}
            >
              <div class="flex items-start justify-between">
                <div class="min-w-0 flex-1">
                  <div class="relative">
                    <p
                      class="text-[13px] text-zinc-100 font-normal leading-relaxed break-words whitespace-pre-wrap {expandedItems.includes(
                        item.id
                      )
                        ? ''
                        : 'line-clamp-4'}"
                    >
                      {item.raw_content}
                    </p>
                    {#if item.raw_content.split("\n").length > 4 || item.raw_content.length > 300}
                      <button
                        onclick={(e) => {
                          e.stopPropagation();
                          if (expandedItems.includes(item.id)) {
                            expandedItems = expandedItems.filter(
                              (id) => id !== item.id
                            );
                          } else {
                            expandedItems = [...expandedItems, item.id];
                          }
                        }}
                        class="text-[10px] font-bold text-red-500/80 hover:text-red-500 mt-1 uppercase tracking-widest"
                      >
                        {expandedItems.includes(item.id)
                          ? "Collapse"
                          : "Expand"}
                      </button>
                    {/if}
                  </div>
                </div>
                <div class="flex items-center space-x-1 ml-4 self-start">
                  <button
                    class="p-2 rounded-lg hover:bg-[#333] transition-colors {item.is_permanent
                      ? 'text-amber-500'
                      : 'text-zinc-600'}"
                    onclick={(e) => {
                      e.stopPropagation();
                      togglePermanent(item);
                    }}
                    title="Pin Item"
                  >
                    <svg
                      xmlns="http://www.w3.org/2000/svg"
                      width="14"
                      height="14"
                      viewBox="0 0 24 24"
                      fill={item.is_permanent ? "currentColor" : "none"}
                      stroke="currentColor"
                      stroke-width="2.5"
                      stroke-linecap="round"
                      stroke-linejoin="round"
                    >
                      <line x1="12" y1="17" x2="12" y2="22"></line>
                      <path
                        d="M5 17h14v-1.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V6a3 3 0 0 0-3-3 3 3 0 0 0-3 3v4.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24Z"
                      ></path>
                    </svg>
                  </button>
                  <button
                    class="p-2 rounded-lg hover:bg-[#333] hover:text-red-500 transition-colors text-zinc-600"
                    onclick={(e) => {
                      e.stopPropagation();
                      categorizingItemId = item.id;
                      newGroupName = "";
                      isCategorizing = true;
                    }}
                    title="Add to Group"
                  >
                    <svg
                      xmlns="http://www.w3.org/2000/svg"
                      width="14"
                      height="14"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      stroke-width="2"
                    >
                      <path
                        d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"
                      ></path>
                      <line x1="12" y1="11" x2="12" y2="17"></line>
                      <line x1="9" y1="14" x2="15" y2="14"></line>
                    </svg>
                  </button>
                  <button
                    class="p-2 rounded-lg hover:bg-red-500/10 hover:text-red-500 transition-colors text-zinc-600"
                    onclick={(e) => {
                      e.stopPropagation();
                      deleteItem(item);
                    }}
                    title="Delete Item"
                  >
                    <svg
                      xmlns="http://www.w3.org/2000/svg"
                      width="14"
                      height="14"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      stroke-width="2"
                      ><path d="M3 6h18"></path><path
                        d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"
                      ></path><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"
                      ></path></svg
                    >
                  </button>
                </div>
              </div>

              <div
                class="flex items-center justify-between border-t border-[#333]/50 pt-3"
              >
                <div class="flex items-center space-x-3">
                  <!-- Icon -->
                  <div
                    class="w-4 h-4 flex items-center justify-center text-zinc-500"
                  >
                    <svg
                      xmlns="http://www.w3.org/2000/svg"
                      width="14"
                      height="14"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      stroke-width="2"
                      stroke-linecap="round"
                      stroke-linejoin="round"
                    >
                      {@html getCategoryIcon(item.category)}
                    </svg>
                  </div>
                  {#if item.groups && item.groups.length > 0}
                    {#each item.groups as group}
                      <span
                        class="flex items-center gap-1 text-[9px] font-bold uppercase py-0.5 px-2 bg-red-500/10 text-red-500 rounded-full border border-red-500/20"
                      >
                        {group}
                        {#if selectedGroup === group}
                          <button
                            onclick={(e) => {
                              e.stopPropagation();
                              removeFromGroup(item, group);
                            }}
                            class="hover:text-white ml-1 px-1 rounded-full hover:bg-red-500/20"
                            title="Remove from group"
                          >
                            ×
                          </button>
                        {/if}
                      </span>
                    {/each}
                  {:else if item.category}
                    <span
                      class="text-[9px] font-bold uppercase py-0.5 px-2 bg-red-500/10 text-red-500 rounded-full border border-red-500/20"
                    >
                      {item.category}
                    </span>
                  {/if}
                  <span class="text-[10px] text-zinc-600 font-medium">
                    {new Date(item.created_at).toLocaleString([], {
                      dateStyle: "medium",
                      timeStyle: "short",
                    })}
                  </span>
                </div>
                <button
                  class="text-[10px] font-bold text-zinc-500 hover:text-red-500 uppercase tracking-widest flex items-center space-x-1"
                  onclick={(e) => {
                    e.stopPropagation();
                    copyAndPaste(item);
                  }}
                >
                  <span>Copy & Paste</span>
                  <svg
                    xmlns="http://www.w3.org/2000/svg"
                    width="12"
                    height="12"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2.5"
                    ><polyline points="9 18 15 12 9 6"></polyline></svg
                  >
                </button>
              </div>
            </div>
          {/each}
        </div>

        {#if history.length === 0}
          <div
            class="flex flex-col items-center justify-center h-full py-20 text-center"
          >
            <div
              class="w-16 h-16 bg-[#1e1e1e] rounded-2xl flex items-center justify-center mb-4 text-zinc-700"
            >
              <svg
                xmlns="http://www.w3.org/2000/svg"
                width="32"
                height="32"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                ><rect x="9" y="9" width="13" height="13" rx="2" ry="2"
                ></rect><path
                  d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"
                ></path></svg
              >
            </div>
            <h3 class="text-zinc-400 font-bold">No clips found</h3>
            <p class="text-zinc-600 text-xs mt-1">
              Copy something to see it here
            </p>
          </div>
        {/if}
      </div>
    </main>
  </div>

  <!-- Export Modal -->
  {#if showExportModal}
    <div
      class="fixed inset-0 bg-black/60 backdrop-blur-sm z-50 flex items-center justify-center p-4"
    >
      <div
        class="w-full max-w-sm bg-[#1e1e1e] border border-[#333] rounded-2xl shadow-2xl overflow-hidden p-6 animate-in zoom-in-95 duration-200"
      >
        <h3 class="text-sm font-bold text-white mb-4">Export Data</h3>

        <div
          class="max-h-60 overflow-y-auto mb-4 border border-[#333] rounded-lg p-2"
        >
          <label
            class="flex items-center space-x-2 text-xs text-zinc-400 p-2 hover:bg-[#2a2a2a] rounded cursor-pointer"
          >
            <input
              type="checkbox"
              checked={exportSelectedGroups.length === 0}
              onchange={() => (exportSelectedGroups = [])}
              class="rounded border-zinc-600 bg-[#2a2a2a] text-red-500 focus:ring-red-500"
            />
            <span class="font-bold">All Data</span>
          </label>
          <div class="h-px bg-[#333] my-1"></div>
          {#each groups as group}
            <label
              class="flex items-center space-x-2 text-xs text-zinc-400 p-2 hover:bg-[#2a2a2a] rounded cursor-pointer"
            >
              <input
                type="checkbox"
                checked={exportSelectedGroups.includes(group)}
                onchange={(e) => {
                  if (e.currentTarget.checked) {
                    exportSelectedGroups = [...exportSelectedGroups, group];
                  } else {
                    exportSelectedGroups = exportSelectedGroups.filter(
                      (g) => g !== group
                    );
                  }
                }}
                class="rounded border-zinc-600 bg-[#2a2a2a] text-red-500 focus:ring-red-500"
              />
              <span>{group}</span>
            </label>
          {/each}
        </div>

        <div class="flex justify-end space-x-3">
          <button
            class="px-4 py-2 text-xs text-zinc-500 font-bold hover:text-white transition-colors"
            onclick={() => (showExportModal = false)}>Cancel</button
          >
          <button
            class="px-6 py-2 bg-red-500 text-white rounded-xl text-xs font-bold shadow-lg shadow-red-500/20 hover:bg-red-600 transition-all"
            onclick={performExport}
            disabled={processingIO}
          >
            {processingIO ? "Exporting..." : "Export"}
          </button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Import Modal -->
  {#if showImportModal}
    <div
      class="fixed inset-0 bg-black/60 backdrop-blur-sm z-50 flex items-center justify-center p-4"
    >
      <div
        class="w-full max-w-sm bg-[#1e1e1e] border border-[#333] rounded-2xl shadow-2xl overflow-hidden p-6 animate-in zoom-in-95 duration-200"
      >
        <h3 class="text-sm font-bold text-white mb-4">Import Data</h3>

        <div class="mb-6 space-y-3">
          <label
            class="flex items-center space-x-3 p-3 border border-[#333] rounded-xl cursor-pointer hover:bg-[#2a2a2a] transition-colors {importMode ===
            'merge'
              ? 'bg-[#2a2a2a] border-red-500/50'
              : ''}"
          >
            <input
              type="radio"
              name="importMode"
              value="merge"
              bind:group={importMode}
              class="text-red-500 focus:ring-red-500 bg-[#1e1e1e] border-zinc-600"
            />
            <div>
              <div class="text-white text-xs font-bold">Merge</div>
              <div class="text-[10px] text-zinc-500">
                Combine with existing data
              </div>
            </div>
          </label>

          <label
            class="flex items-center space-x-3 p-3 border border-[#333] rounded-xl cursor-pointer hover:bg-[#2a2a2a] transition-colors {importMode ===
            'replace'
              ? 'bg-[#2a2a2a] border-red-500/50'
              : ''}"
          >
            <input
              type="radio"
              name="importMode"
              value="replace"
              bind:group={importMode}
              class="text-red-500 focus:ring-red-500 bg-[#1e1e1e] border-zinc-600"
            />
            <div>
              <div class="text-white text-xs font-bold">Replace</div>
              <div class="text-[10px] text-zinc-500">
                Overwrite all existing data
              </div>
            </div>
          </label>
        </div>

        <div class="flex justify-end space-x-3">
          <button
            class="px-4 py-2 text-xs text-zinc-500 font-bold hover:text-white transition-colors"
            onclick={() => (showImportModal = false)}>Cancel</button
          >
          <button
            class="px-6 py-2 bg-red-500 text-white rounded-xl text-xs font-bold shadow-lg shadow-red-500/20 hover:bg-red-600 transition-all"
            onclick={performImport}
            disabled={processingIO}
          >
            {processingIO ? "Importing..." : "Select File"}
          </button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Manage Groups Modal -->
  {#if isViewingGroups}
    <div
      class="fixed inset-0 bg-black/60 backdrop-blur-sm z-50 flex items-center justify-center p-4"
    >
      <div
        class="w-full max-w-2xl bg-[#1e1e1e] border border-[#333] rounded-2xl shadow-2xl overflow-hidden flex flex-col max-h-[80vh] animate-in zoom-in-95 duration-200"
      >
        <div
          class="px-6 py-4 border-b border-[#333] flex justify-between items-center bg-[#1e1e1e]"
        >
          <div>
            <h2 class="text-lg font-bold text-white">Manage Groups</h2>
            <p class="text-xs text-zinc-500">Create and organize your clips</p>
          </div>
          <button
            onclick={() => (isViewingGroups = false)}
            class="text-zinc-500 hover:text-white"
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="20"
              height="20"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              ><line x1="18" y1="6" x2="6" y2="18"></line><line
                x1="6"
                y1="6"
                x2="18"
                y2="18"
              ></line></svg
            >
          </button>
        </div>

        <div class="p-6 border-b border-[#333] bg-[#1a1a1a]">
          <div class="flex items-center space-x-3">
            <input
              type="text"
              bind:value={newGroupName}
              placeholder="New group name..."
              class="flex-1 bg-[#252525] border border-[#333] rounded-xl px-4 py-2.5 text-sm focus:outline-none focus:border-red-500/50 transition-all font-medium"
              onkeydown={(e) => {
                if (e.key === "Enter") createGroup();
              }}
            />
            <button
              onclick={createGroup}
              class="px-6 py-2.5 bg-red-500 text-white rounded-xl text-sm font-bold shadow-lg shadow-red-500/20 hover:bg-red-600 transition-all"
            >
              Create Group
            </button>
          </div>
        </div>

        <div class="flex-1 overflow-y-auto custom-scrollbar p-6 space-y-3">
          {#each groups as group}
            <div
              class="flex items-center justify-between p-4 bg-[#252525] rounded-xl border border-transparent hover:border-[#333] transition-all group"
            >
              <div class="flex items-center space-x-4 flex-1">
                <div
                  class="w-10 h-10 rounded-lg bg-[#1e1e1e] flex items-center justify-center text-red-500/70"
                >
                  <svg
                    xmlns="http://www.w3.org/2000/svg"
                    width="20"
                    height="20"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                    ><path
                      d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"
                    ></path></svg
                  >
                </div>
                {#if editingGroup === group}
                  <input
                    type="text"
                    bind:value={editGroupName}
                    class="flex-1 bg-[#1e1e1e] text-sm px-3 py-1.5 rounded-lg focus:outline-none text-white border border-red-500/50"
                    autofocus
                    onblur={renameGroup}
                    onkeydown={(e) => {
                      if (e.key === "Enter") renameGroup();
                      if (e.key === "Escape") editingGroup = null;
                    }}
                  />
                {:else}
                  <div>
                    <h3 class="text-sm font-bold text-white">{group}</h3>
                    <p
                      class="text-[10px] text-zinc-500 font-medium uppercase tracking-widest mt-0.5"
                    >
                      User Created Group
                    </p>
                  </div>
                {/if}
              </div>

              <div class="flex items-center space-x-2">
                <button
                  onclick={() => {
                    editingGroup = group;
                    editGroupName = group;
                  }}
                  class="p-2 text-zinc-500 hover:text-white hover:bg-[#333] rounded-lg transition-all"
                  title="Rename"
                >
                  <svg
                    xmlns="http://www.w3.org/2000/svg"
                    width="16"
                    height="16"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                    ><path
                      d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"
                    /><path
                      d="M18.5 2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"
                    /></svg
                  >
                </button>
                <button
                  onclick={() => exportGroup(group)}
                  class="p-2 text-zinc-500 hover:text-white hover:bg-[#333] rounded-lg transition-all"
                  title="Export Group to .TXT"
                >
                  <svg
                    xmlns="http://www.w3.org/2000/svg"
                    width="16"
                    height="16"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                    ><path
                      d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"
                    /><polyline points="7 10 12 15 17 10" /><line
                      x1="12"
                      y1="15"
                      x2="12"
                      y2="3"
                    /></svg
                  >
                </button>
                <button
                  onclick={() => deleteGroup(group)}
                  class="p-2 text-zinc-500 hover:text-red-500 hover:bg-red-500/10 rounded-lg transition-all"
                  title="Delete Group"
                >
                  <svg
                    xmlns="http://www.w3.org/2000/svg"
                    width="16"
                    height="16"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                    ><path d="M3 6h18"></path><path
                      d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"
                    ></path><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"
                    ></path></svg
                  >
                </button>
              </div>
            </div>
          {:else}
            <div
              class="flex flex-col items-center justify-center py-12 text-center"
            >
              <div
                class="w-16 h-16 bg-[#252525] rounded-2xl flex items-center justify-center mb-4 text-zinc-700"
              >
                <svg
                  xmlns="http://www.w3.org/2000/svg"
                  width="32"
                  height="32"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  ><path
                    d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"
                  ></path></svg
                >
              </div>
              <h3 class="text-zinc-400 font-bold">No custom groups</h3>
              <p class="text-zinc-600 text-xs mt-1">
                Create your first group above to organize your clips
              </p>
            </div>
          {/each}
        </div>
      </div>
    </div>
  {/if}

  <!-- Move to Group Popup (Absolute) -->
  <!-- Move to Group Popup (Absolute) -->
  {#if isCategorizing}
    <div
      class="fixed inset-0 bg-black/60 backdrop-blur-sm z-50 flex items-center justify-center p-4"
    >
      <div
        class="w-full max-w-[280px] bg-[#1e1e1e] border border-[#333] rounded-2xl shadow-2xl overflow-hidden flex flex-col max-h-[70vh] animate-in zoom-in-95 duration-200"
      >
        <div
          class="px-4 py-3 border-b border-[#333] flex justify-between items-center bg-[#1e1e1e]"
        >
          <span
            class="text-[10px] font-bold uppercase tracking-widest text-zinc-500"
            >Save to Group</span
          >
          <button
            onclick={() => (isCategorizing = false)}
            class="text-zinc-500 hover:text-white">✕</button
          >
        </div>

        <div class="p-2 border-b border-[#333] bg-[#1a1a1a]">
          <input
            type="text"
            bind:value={newGroupName}
            placeholder="New or search group..."
            class="w-full bg-[#252525] border border-[#333] rounded-lg px-3 py-1.5 text-xs focus:outline-none focus:border-red-500/50 transition-all font-medium"
            autofocus
            onkeydown={(e) => {
              if (e.key === "Enter") moveItemToGroup();
              if (e.key === "Escape") isCategorizing = false;
            }}
          />
        </div>

        <div class="flex-1 overflow-y-auto custom-scrollbar p-1">
          {#each groups.filter((g) => g
              .toLowerCase()
              .includes(newGroupName.toLowerCase())) as group}
            <button
              onclick={() => {
                newGroupName = group;
                moveItemToGroup();
              }}
              class="w-full text-left px-3 py-2 text-xs hover:bg-[#2a2a2a] rounded-lg transition-all flex items-center space-x-2 text-zinc-400 hover:text-white"
            >
              <svg
                xmlns="http://www.w3.org/2000/svg"
                width="12"
                height="12"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                class="text-red-500/50"
                ><path
                  d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"
                ></path></svg
              >
              <span>{group}</span>
            </button>
          {/each}
          {#if newGroupName && !groups.includes(newGroupName)}
            <button
              onclick={moveItemToGroup}
              class="w-full text-left px-3 py-2 text-xs hover:bg-red-500/10 rounded-lg transition-all flex items-center space-x-2 text-red-500"
            >
              <svg
                xmlns="http://www.w3.org/2000/svg"
                width="12"
                height="12"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="3"
                ><line x1="12" y1="5" x2="12" y2="19"></line><line
                  x1="5"
                  y1="12"
                  x2="19"
                  y2="12"
                ></line></svg
              >
              <span>Create "{newGroupName}"</span>
            </button>
          {/if}
        </div>

        <div class="p-2 border-t border-[#333] bg-[#1a1a1a] flex justify-end">
          <button
            onclick={moveItemToGroup}
            class="px-4 py-1.5 bg-red-500 text-white rounded-lg text-xs font-bold hover:bg-red-600 transition-all"
            disabled={!newGroupName.trim()}
          >
            Save
          </button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Help Modal -->
  {#if showHelpModal}
    <div
      class="fixed inset-0 bg-black/60 backdrop-blur-sm z-50 flex items-center justify-center p-4"
      onclick={(e) => {
        if (e.target === e.currentTarget) showHelpModal = false;
      }}
    >
      <div
        class="w-full max-w-2xl bg-[#1e1e1e] border border-[#333] rounded-2xl shadow-2xl overflow-hidden animate-in zoom-in-95 duration-200"
      >
        <div
          class="p-6 border-b border-[#333] flex items-center justify-between"
        >
          <div class="flex items-center gap-3">
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="20"
              height="20"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              class="text-red-500"
              ><circle cx="12" cy="12" r="10"></circle><path
                d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3"
              ></path><line x1="12" y1="17" x2="12.01" y2="17"></line></svg
            >
            <h3 class="text-lg font-bold text-white">Help & Shortcuts</h3>
          </div>
          <button
            onclick={() => (showHelpModal = false)}
            class="text-zinc-500 hover:text-white transition-colors"
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="20"
              height="20"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              ><line x1="18" y1="6" x2="6" y2="18"></line><line
                x1="6"
                y1="6"
                x2="18"
                y2="18"
              ></line></svg
            >
          </button>
        </div>

        <div
          class="p-6 max-h-[70vh] overflow-y-auto custom-scrollbar space-y-6"
        >
          <!-- How to Open -->
          <div>
            <h4
              class="text-sm font-bold text-red-500 mb-3 uppercase tracking-wider"
            >
              Opening Windows
            </h4>
            <div class="space-y-2 text-sm">
              <div class="flex items-start gap-3 p-3 bg-[#252525] rounded-lg">
                <div class="flex-shrink-0 w-32 text-zinc-400 font-mono text-xs">
                  <kbd
                    class="px-2 py-1 bg-[#1a1a1a] rounded border border-[#333]"
                    >{altKey}</kbd
                  >
                  +
                  <kbd
                    class="px-2 py-1 bg-[#1a1a1a] rounded border border-[#333]"
                    >V</kbd
                  >
                </div>
                <div>
                  <div class="text-white font-semibold">Quick Popup</div>
                  <div class="text-zinc-500 text-xs">
                    Opens the quick access popup window
                  </div>
                </div>
              </div>
              <div class="flex items-start gap-3 p-3 bg-[#252525] rounded-lg">
                <div class="flex-shrink-0 w-32 text-zinc-400 text-xs">
                  Tray Icon
                </div>
                <div>
                  <div class="text-white font-semibold">Main Window</div>
                  <div class="text-zinc-500 text-xs">
                    Click the tray icon to open the main window
                  </div>
                </div>
              </div>
            </div>
          </div>

          <!-- Navigation Shortcuts -->
          <div>
            <h4
              class="text-sm font-bold text-red-500 mb-3 uppercase tracking-wider"
            >
              Navigation
            </h4>
            <div class="space-y-2 text-sm">
              <div
                class="flex items-center justify-between p-2 hover:bg-[#252525] rounded"
              >
                <span class="text-zinc-300">Navigate Down</span>
                <kbd
                  class="px-2 py-1 bg-[#1a1a1a] rounded border border-[#333] text-xs"
                  >↓</kbd
                >
              </div>
              <div
                class="flex items-center justify-between p-2 hover:bg-[#252525] rounded"
              >
                <span class="text-zinc-300">Navigate Up</span>
                <kbd
                  class="px-2 py-1 bg-[#1a1a1a] rounded border border-[#333] text-xs"
                  >↑</kbd
                >
              </div>
              <div
                class="flex items-center justify-between p-2 hover:bg-[#252525] rounded"
              >
                <span class="text-zinc-300">Close Window</span>
                <kbd
                  class="px-2 py-1 bg-[#1a1a1a] rounded border border-[#333] text-xs"
                  >Esc</kbd
                >
              </div>
            </div>
          </div>

          <!-- Action Shortcuts -->
          <div>
            <h4
              class="text-sm font-bold text-red-500 mb-3 uppercase tracking-wider"
            >
              Actions
            </h4>
            <div class="space-y-2 text-sm">
              <div
                class="flex items-center justify-between p-2 hover:bg-[#252525] rounded"
              >
                <span class="text-zinc-300">Copy & Paste Selected</span>
                <kbd
                  class="px-2 py-1 bg-[#1a1a1a] rounded border border-[#333] text-xs"
                  >Enter</kbd
                >
              </div>
              <div
                class="flex items-center justify-between p-2 hover:bg-[#252525] rounded"
              >
                <span class="text-zinc-300">Delete Item</span>
                <div class="flex items-center gap-1">
                  <kbd
                    class="px-2 py-1 bg-[#1a1a1a] rounded border border-[#333] text-xs"
                    >Delete</kbd
                  >
                  /
                  <kbd
                    class="px-2 py-1 bg-[#1a1a1a] rounded border border-[#333] text-xs"
                    >{modKey}+{deleteKey}</kbd
                  >
                </div>
              </div>
              <div
                class="flex items-center justify-between p-2 hover:bg-[#252525] rounded"
              >
                <span class="text-zinc-300">Pin/Unpin Item</span>
                <kbd
                  class="px-2 py-1 bg-[#1a1a1a] rounded border border-[#333] text-xs"
                  >{modKey}+P</kbd
                >
              </div>
              <div
                class="flex items-center justify-between p-2 hover:bg-[#252525] rounded"
              >
                <span class="text-zinc-300">Categorize Item</span>
                <kbd
                  class="px-2 py-1 bg-[#1a1a1a] rounded border border-[#333] text-xs"
                  >{modKey}+C</kbd
                >
              </div>
              <div
                class="flex items-center justify-between p-2 hover:bg-[#252525] rounded"
              >
                <span class="text-zinc-300">Manage Groups</span>
                <kbd
                  class="px-2 py-1 bg-[#1a1a1a] rounded border border-[#333] text-xs"
                  >{modKey}+G</kbd
                >
              </div>
            </div>
          </div>

          <!-- Features -->
          <div>
            <h4
              class="text-sm font-bold text-red-500 mb-3 uppercase tracking-wider"
            >
              Features
            </h4>
            <ul class="space-y-2 text-sm text-zinc-300">
              <li class="flex items-start gap-2">
                <span class="text-red-500 mt-1">•</span>
                <span
                  ><strong class="text-white">Smart Groups:</strong> Automatically
                  categorizes URLs, Images, and Text</span
                >
              </li>
              <li class="flex items-start gap-2">
                <span class="text-red-500 mt-1">•</span>
                <span
                  ><strong class="text-white">Custom Groups:</strong> Create and
                  organize items into custom categories</span
                >
              </li>
              <li class="flex items-start gap-2">
                <span class="text-red-500 mt-1">•</span>
                <span
                  ><strong class="text-white">Pinned Items:</strong> Pin important
                  items to keep them permanently</span
                >
              </li>
              <li class="flex items-start gap-2">
                <span class="text-red-500 mt-1">•</span>
                <span
                  ><strong class="text-white">Search:</strong> Quickly find items
                  with real-time search</span
                >
              </li>
              <li class="flex items-start gap-2">
                <span class="text-red-500 mt-1">•</span>
                <span
                  ><strong class="text-white">Export/Import:</strong> Backup and
                  restore your clipboard history</span
                >
              </li>
              <li class="flex items-start gap-2">
                <span class="text-red-500 mt-1">•</span>
                <span
                  ><strong class="text-white">Privacy:</strong> All data is stored
                  locally on your machine</span
                >
              </li>
            </ul>
          </div>
        </div>

        <div class="p-4 border-t border-[#333] flex justify-end">
          <button
            class="px-6 py-2 bg-red-500 text-white rounded-xl text-xs font-bold shadow-lg shadow-red-500/20 hover:bg-red-600 transition-all"
            onclick={() => (showHelpModal = false)}
          >
            Got it!
          </button>
        </div>
      </div>
    </div>
  {/if}

  <!-- About Modal -->
  {#if showAboutModal}
    <div
      class="fixed inset-0 bg-black/60 backdrop-blur-sm z-50 flex items-center justify-center p-4"
      onclick={(e) => {
        if (e.target === e.currentTarget) showAboutModal = false;
      }}
    >
      <div
        class="w-full max-w-md bg-[#1e1e1e] border border-[#333] rounded-2xl shadow-2xl overflow-hidden animate-in zoom-in-95 duration-200"
      >
        <div
          class="p-6 border-b border-[#333] flex items-center justify-between"
        >
          <div class="flex items-center gap-3">
            <div
              class="w-10 h-10 rounded-full flex items-center justify-center shadow-lg shadow-red-500/20"
            >
              <img src="/logo.png" alt="Ortu" class="w-8 h-8" />
            </div>
            <div>
              <h3 class="text-lg font-bold text-white">Ortu</h3>
              <p class="text-xs text-zinc-500">Clipboard Manager</p>
            </div>
          </div>
          <button
            onclick={() => (showAboutModal = false)}
            class="text-zinc-500 hover:text-white transition-colors"
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="20"
              height="20"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              ><line x1="18" y1="6" x2="6" y2="18"></line><line
                x1="6"
                y1="6"
                x2="18"
                y2="18"
              ></line></svg
            >
          </button>
        </div>

        <div class="p-6 space-y-6">
          <div class="text-center space-y-2">
            <div class="text-2xl font-bold text-white">Version 1.0.1</div>
            <p class="text-sm text-zinc-400">
              A powerful, privacy-focused clipboard manager for macOS
            </p>
          </div>

          <div class="space-y-4">
            <div class="p-4 bg-[#252525] rounded-xl">
              <div class="text-xs text-zinc-500 uppercase tracking-wider mb-2">
                Developer
              </div>
              <div class="text-sm text-white font-semibold">
                Abhijith P Subash
              </div>
            </div>

            <a
              href="https://www.linkedin.com/in/abhijith-p-subash-the-engineer/"
              target="_blank"
              rel="noopener noreferrer"
              class="block p-4 bg-[#252525] hover:bg-[#2a2a2a] rounded-xl transition-all group"
            >
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-3">
                  <svg
                    xmlns="http://www.w3.org/2000/svg"
                    width="20"
                    height="20"
                    viewBox="0 0 24 24"
                    fill="currentColor"
                    class="text-[#0A66C2]"
                  >
                    <path
                      d="M20.447 20.452h-3.554v-5.569c0-1.328-.027-3.037-1.852-3.037-1.853 0-2.136 1.445-2.136 2.939v5.667H9.351V9h3.414v1.561h.046c.477-.9 1.637-1.85 3.37-1.85 3.601 0 4.267 2.37 4.267 5.455v6.286zM5.337 7.433c-1.144 0-2.063-.926-2.063-2.065 0-1.138.92-2.063 2.063-2.063 1.14 0 2.064.925 2.064 2.063 0 1.139-.925 2.065-2.064 2.065zm1.782 13.019H3.555V9h3.564v11.452zM22.225 0H1.771C.792 0 0 .774 0 1.729v20.542C0 23.227.792 24 1.771 24h20.451C23.2 24 24 23.227 24 22.271V1.729C24 .774 23.2 0 22.222 0h.003z"
                    />
                  </svg>
                  <div>
                    <div class="text-sm text-white font-semibold">
                      Connect on LinkedIn
                    </div>
                    <div class="text-xs text-zinc-500">
                      @abhijith-p-subash-the-engineer
                    </div>
                  </div>
                </div>
                <svg
                  xmlns="http://www.w3.org/2000/svg"
                  width="16"
                  height="16"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  class="text-zinc-600 group-hover:text-zinc-400 transition-colors"
                >
                  <path
                    d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"
                  ></path>
                  <polyline points="15 3 21 3 21 9"></polyline>
                  <line x1="10" y1="14" x2="21" y2="3"></line>
                </svg>
              </div>
            </a>

            <div
              class="p-4 bg-gradient-to-br from-red-500/10 to-red-500/5 rounded-xl border border-red-500/20"
            >
              <div class="text-xs text-red-400 uppercase tracking-wider mb-2">
                Privacy First
              </div>
              <p class="text-xs text-zinc-400">
                All your clipboard data is stored locally on your device. No
                cloud sync, no tracking, no external servers.
              </p>
            </div>
          </div>

          <div
            class="pt-4 border-t border-[#333] text-center text-xs text-zinc-500"
          >
            © 2025 Ortu. All rights reserved.
          </div>
        </div>

        <div class="p-4 border-t border-[#333] flex justify-end">
          <button
            class="px-6 py-2 bg-red-500 text-white rounded-xl text-xs font-bold shadow-lg shadow-red-500/20 hover:bg-red-600 transition-all"
            onclick={() => (showAboutModal = false)}
          >
            Close
          </button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Copied Toast Notification -->
  {#if showCopiedToast}
    <div
      class="fixed bottom-4 right-4 bg-[#1e1e1e] border border-green-500/50 rounded-xl shadow-2xl shadow-green-500/20 px-4 py-3 flex items-center gap-3 animate-in slide-in-from-bottom-5 duration-300 z-50"
    >
      <div
        class="flex items-center justify-center w-5 h-5 bg-green-500/20 rounded-full"
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="3"
          class="text-green-500"
        >
          <polyline points="20 6 9 17 4 12"></polyline>
        </svg>
      </div>
      <span class="text-sm font-semibold text-white">Copied!</span>
    </div>
  {/if}
</div>

<style>
  :global(body) {
    margin: 0;
    overflow: hidden;
    background: transparent;
  }

  .custom-scrollbar::-webkit-scrollbar {
    width: 3px;
  }
  .custom-scrollbar::-webkit-scrollbar-track {
    background: transparent;
  }
  .custom-scrollbar::-webkit-scrollbar-thumb {
    background: #333;
    border-radius: 10px;
  }
  .custom-scrollbar::-webkit-scrollbar-thumb:hover {
    background: #444;
  }
</style>
