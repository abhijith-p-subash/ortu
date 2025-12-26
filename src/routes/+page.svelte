<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { ClipboardItem } from "$lib/types";
  import { listen } from "@tauri-apps/api/event";
  import { save, open } from "@tauri-apps/plugin-dialog";
  import "../app.css";

  let history = $state<ClipboardItem[]>([]);
  let groups = $state<string[]>([]);
  let searchQuery = $state("");
  let selectedIndex = $state(0);
  let container = $state<HTMLDivElement | null>(null);
  let searchInput = $state<HTMLInputElement | null>(null);
  let isCategorizing = $state(false);
  let isViewingGroups = $state(false);
  let selectedGroup = $state<string | null>(null);
  let newGroupName = $state("");
  let editingGroup = $state<string | null>(null);
  let editGroupName = $state("");
  let draggedItemId = $state<number | null>(null);

  async function loadHistory() {
    try {
      let prefix = "category:";
      // Check if it's a known smart group
      if (
        ["Dev", "Code", "URL", "Images", "Text"].includes(selectedGroup || "")
      ) {
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

  async function exportGroup(name: string) {
    try {
      const path = await save({
        filters: [{ name: "Text", extensions: ["txt"] }],
        defaultPath: `${name}_export.txt`,
      });
      if (path) {
        await invoke("export_group", { name, path });
        alert("Export successful!");
      }
    } catch (e) {
      console.error("Failed to export group:", e);
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

  async function backupData() {
    try {
      const path = await save({
        filters: [{ name: "JSON", extensions: ["json"] }],
        defaultPath: `ortu_backup_${new Date().toISOString().split("T")[0]}.json`,
      });
      if (path) {
        await invoke("backup_data", { path });
        alert("Backup successful!");
      }
    } catch (e) {
      console.error("Failed to backup data:", e);
      alert("Failed to backup data: " + e);
    }
  }

  async function restoreData() {
    if (
      !confirm(
        "WARNING: Restore will overwrite ALL current history and groups. Continue?"
      )
    )
      return;

    try {
      const path = await open({
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (path && typeof path === "string") {
        await invoke("restore_data", { path });
        await loadGroups();
        await loadHistory();
        alert("Restore successful!");
      }
    } catch (e) {
      console.error("Failed to restore data:", e);
      alert("Failed to restore data: " + e);
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
    if (!history[selectedIndex] || !newGroupName.trim()) return;
    try {
      await invoke("set_category", {
        id: history[selectedIndex].id,
        category: newGroupName.trim(),
      });
      isCategorizing = false;
      newGroupName = "";
      await loadHistory();
      await loadGroups();
    } catch (e) {
      console.error("Failed to move item to group:", e);
    }
  }

  function handleDragStart(e: DragEvent, id: number) {
    if (e.dataTransfer) {
      e.dataTransfer.setData("text/plain", id.toString());
      e.dataTransfer.effectAllowed = "move";
      draggedItemId = id;
    }
  }

  function handleDragOver(e: DragEvent) {
    e.preventDefault();
    if (e.dataTransfer) {
      e.dataTransfer.dropEffect = "move";
    }
  }

  async function handleDrop(e: DragEvent, targetGroup: string) {
    e.preventDefault();
    const idStr = e.dataTransfer?.getData("text/plain");
    if (idStr) {
      const id = parseInt(idStr);
      if (!isNaN(id)) {
        await invoke("set_category", { id, category: targetGroup });
        await loadHistory();
        draggedItemId = null;
      }
    }
  }

  async function copyAndPaste(item: ClipboardItem) {
    try {
      await navigator.clipboard.writeText(item.raw_content);
      // await invoke("close_window");
      console.log("Item copied to clipboard");
      await invoke("paste_item");
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
    selectedElement?.scrollIntoView({ block: "nearest" });
  }

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
          await loadHistory();
          await loadGroups();
          selectedIndex = 0;
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
        class="w-8 h-8 bg-red-500 rounded-lg flex items-center justify-center shadow-lg shadow-red-500/20"
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="18"
          height="18"
          viewBox="0 0 24 24"
          fill="none"
          stroke="white"
          stroke-width="2.5"
          ><rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect><path
            d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"
          ></path></svg
        >
      </div>
      <div>
        <h1 class="text-sm font-bold text-white tracking-tight">
          Ortu Manager
        </h1>
        <p
          class="text-[10px] text-zinc-500 font-medium uppercase tracking-widest"
        >
          Workspace History
        </p>
      </div>
    </div>
    <div class="flex items-center space-x-2">
      <button
        onclick={backupData}
        class="flex items-center space-x-2 px-3 py-1.5 bg-[#2a2a2a] hover:bg-[#333] rounded-md border border-[#333] transition-all text-xs font-semibold"
        title="Backup Full History"
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
        onclick={restoreData}
        class="flex items-center space-x-2 px-3 py-1.5 bg-[#2a2a2a] hover:bg-[#333] rounded-md border border-[#333] transition-all text-xs font-semibold"
        title="Restore Full History"
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
        onclick={importGroup}
        class="flex items-center space-x-2 px-3 py-1.5 bg-[#2a2a2a] hover:bg-[#333] rounded-md border border-[#333] transition-all text-xs font-semibold"
        title="Import Group (Legacy)"
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
        <span>Import Grp</span>
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
      <div class="flex-1 overflow-y-auto custom-scrollbar p-2 space-y-1">
        <button
          class="w-full text-left px-3 py-2 rounded-md text-sm transition-all {selectedGroup ===
          null
            ? 'bg-red-500/10 text-red-500 font-bold'
            : 'text-zinc-400 hover:bg-[#2a2a2a]'}"
          onclick={() => {
            selectedGroup = null;
          }}
        >
          All History
        </button>

        <div class="pt-4 pb-2 px-3">
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
          'Dev'
            ? 'bg-red-500/10 text-red-500 font-bold'
            : 'text-zinc-400 hover:bg-[#2a2a2a]'}"
          onclick={() => (selectedGroup = "Dev")}
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
              ><polyline points="4 17 10 11 4 5"></polyline><line
                x1="12"
                y1="19"
                x2="20"
                y2="19"
              ></line></svg
            >
            Dev
          </span>
        </button>

        <button
          class="w-full text-left px-3 py-2 rounded-md text-sm transition-all {selectedGroup ===
          'Code'
            ? 'bg-red-500/10 text-red-500 font-bold'
            : 'text-zinc-400 hover:bg-[#2a2a2a]'}"
          onclick={() => (selectedGroup = "Code")}
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
              ><polyline points="16 18 22 12 16 6"></polyline><polyline
                points="8 6 2 12 8 18"
              ></polyline></svg
            >
            Code
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

        <div class="pt-4 pb-2 px-3">
          <span
            class="text-[10px] font-bold uppercase tracking-widest text-zinc-600"
            >Groups</span
          >
        </div>

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
                ondragover={handleDragOver}
                ondrop={(e) => handleDrop(e, group)}
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

      <div class="p-4 border-t border-[#333]">
        <div class="flex items-center space-x-2">
          <input
            type="text"
            bind:value={newGroupName}
            placeholder="Create group..."
            class="flex-1 bg-[#252525] border border-[#333] rounded px-3 py-1.5 text-xs focus:outline-none focus:border-red-500/30 transition-all"
            onkeydown={(e) => {
              if (e.key === "Enter") createGroup();
            }}
          />
          <button
            onclick={createGroup}
            class="p-1.5 bg-[#2a2a2a] rounded hover:text-red-500 transition-colors"
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="16"
              height="16"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2.5"
              ><line x1="12" y1="5" x2="12" y2="19" /><line
                x1="5"
                y1="12"
                x2="19"
                y2="12"
              /></svg
            >
          </button>
        </div>
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
                : ''} {draggedItemId === item.id ? 'opacity-50' : ''}"
              onclick={() => {
                selectedIndex = i;
              }}
              role="button"
              tabindex="0"
              data-index={i}
              draggable="true"
              ondragstart={(e) => handleDragStart(e, item.id)}
            >
              <div class="flex items-start justify-between">
                <div class="min-w-0 flex-1">
                  <p
                    class="text-[13px] text-zinc-100 font-normal leading-relaxed break-words whitespace-pre-wrap"
                  >
                    {item.raw_content}
                  </p>
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
                      stroke-width="2"
                      ><path
                        d="M12 2L15.09 8.26L22 9.27L17 14.14L18.18 21.02L12 17.77L5.82 21.02L7 14.14L2 9.27L8.91 8.26L12 2Z"
                      /></svg
                    >
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
                  {#if item.category}
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

  <!-- Move to Group Popup (Absolute) -->
  {#if isCategorizing}
    <div
      class="fixed inset-0 bg-black/60 backdrop-blur-sm z-50 flex items-center justify-center p-4"
    >
      <div
        class="w-full max-w-sm bg-[#1e1e1e] border border-[#333] rounded-2xl shadow-2xl overflow-hidden p-6 animate-in zoom-in-95 duration-200"
      >
        <h3 class="text-sm font-bold text-white mb-1">Move to Group</h3>
        <p
          class="text-[10px] text-zinc-500 uppercase tracking-widest font-bold mb-4"
        >
          Select or create a new group
        </p>

        <input
          type="text"
          bind:value={newGroupName}
          placeholder="Enter group name..."
          class="w-full bg-[#2a2a2a] border border-[#333] rounded-xl px-4 py-3 text-sm text-white focus:outline-none focus:border-red-500/50 transition-all mb-4"
          autofocus
          onkeydown={(e) => {
            if (e.key === "Escape") isCategorizing = false;
            if (e.key === "Enter") moveItemToGroup();
          }}
        />

        <div class="flex justify-end space-x-3">
          <button
            class="px-4 py-2 text-xs text-zinc-500 font-bold hover:text-white transition-colors"
            onclick={() => (isCategorizing = false)}>Cancel</button
          >
          <button
            class="px-6 py-2 bg-red-500 text-white rounded-xl text-xs font-bold shadow-lg shadow-red-500/20 hover:bg-red-600 transition-all"
            onclick={moveItemToGroup}>Move Clip</button
          >
        </div>
      </div>
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
