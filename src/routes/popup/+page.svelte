<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { ClipboardItem } from "$lib/types";
  import { listen } from "@tauri-apps/api/event";
  import "../../app.css";

  // --- STATE ---
  let history = $state<ClipboardItem[]>([]);
  let categories = $state<string[]>([]);
  let searchQuery = $state("");
  let selectedIndex = $state(0);
  let container = $state<HTMLDivElement | null>(null);
  let searchInput = $state<HTMLInputElement | null>(null);

  // Navigation & Group state
  let currentCategory = $state<string | null>(null);
  let showGroupSelector = $state<number | null>(null);
  let newGroupName = $state("");

  // --- DERIVED ---
  let filteredCategories = $derived(
    currentCategory || !searchQuery.trim()
      ? []
      : categories.filter((c) =>
          c.toLowerCase().includes(searchQuery.toLowerCase())
        )
  );

  // --- CORE LOGIC ---
  async function loadData() {
    try {
      let search = searchQuery;
      if (currentCategory) {
        search = `category:${currentCategory} ${searchQuery}`.trim();
      }

      const historyData = (await invoke("get_history", {
        search: search || null,
      })) as ClipboardItem[];

      const catData = (await invoke("get_categories")) as string[];

      history = historyData;
      categories = catData;

      // Keep selection in bounds
      const totalItems =
        history.length + (currentCategory ? 0 : filteredCategories.length);
      if (selectedIndex >= totalItems) {
        selectedIndex = Math.max(0, totalItems - 1);
      }
    } catch (e) {
      console.error("❌ Failed to load data:", e);
    }
  }

  async function copyAndPaste(item: ClipboardItem) {
    try {
      await navigator.clipboard.writeText(item.raw_content);
      // Close popup and trigger paste in the previous app
      await invoke("close_window", { label: "popup" });
      await invoke("paste_item");
    } catch (err) {
      console.error("Failed to copy and paste:", err);
    }
  }

  async function addToGroup(itemId: number, category: string) {
    await invoke("set_category", { id: itemId, category });
    showGroupSelector = null;
    loadData(); // Manual refresh after DB change
  }

  async function createAndAddToGroup(itemId: number) {
    if (!newGroupName.trim()) return;
    await invoke("create_group", { name: newGroupName.trim() });
    await addToGroup(itemId, newGroupName.trim());
    newGroupName = "";
  }
  async function togglePermanent(item: ClipboardItem) {
    if (!item) return;
    await invoke("toggle_permanent", { id: item.id });
    await loadData();
  }

  async function deleteItem(item: ClipboardItem) {
    if (!item) return;
    await invoke("delete_entry", { id: item.id });
    await loadData();
  }

  // --- INTERACTION ---
  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      if (showGroupSelector !== null) {
        showGroupSelector = null;
      } else if (currentCategory) {
        currentCategory = null; // $effect will trigger loadData
      } else {
        invoke("close_window", { label: "popup" });
      }
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      const total =
        history.length + (currentCategory ? 0 : filteredCategories.length);
      selectedIndex = (selectedIndex + 1) % (total || 1);
      scrollIntoView();
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      const total =
        history.length + (currentCategory ? 0 : filteredCategories.length);
      selectedIndex = (selectedIndex - 1 + (total || 1)) % (total || 1);
      scrollIntoView();
    } else if (e.key === "Enter") {
      const catsCount = currentCategory ? 0 : filteredCategories.length;
      if (selectedIndex < catsCount) {
        currentCategory = filteredCategories[selectedIndex];
        searchQuery = "";
        selectedIndex = 0;
      } else {
        const item = history[selectedIndex - catsCount];
        if (item) copyAndPaste(item);
      }
    } else if (e.key === "Backspace" && searchQuery === "" && currentCategory) {
      currentCategory = null;
    } else if (e.key === "Delete" || (e.metaKey && e.key === "Backspace")) {
      const catsCount = currentCategory ? 0 : filteredCategories.length;
      const item = history[selectedIndex - catsCount];
      if (item) {
        deleteItem(item);
      }
    } else if (e.key === "p" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      const catsCount = currentCategory ? 0 : filteredCategories.length;
      const item = history[selectedIndex - catsCount];
      if (item) {
        togglePermanent(item);
      }
    }
  }

  function scrollIntoView() {
    tick().then(() => {
      const selectedElement = container?.querySelector(
        `[data-index="${selectedIndex}"]`
      );
      selectedElement?.scrollIntoView({ block: "nearest" });
    });
  }

  // --- EFFECTS & LIFECYCLE ---
  // This automatically handles searching and category switching
  $effect(() => {
    loadData();
    // Dependencies: searchQuery, currentCategory
  });

  onMount(() => {
    window.addEventListener("keydown", handleKeydown);

    // This is the important part
    const setupListeners = async () => {
      try {
        // 1. Handle window focus (auto-reset)
        const unFocus = await listen("tauri://focus", () => {
          currentCategory = null;
          searchQuery = "";
          selectedIndex = 0;
          loadData(); // Force reload when window pops up
          tick().then(() => {
            searchInput?.focus();
            container?.scrollTo({ top: 0, behavior: "instant" });
          });
        });

        // 2. Handle real-time updates while window is ALREADY open
        const unClipboard = await listen("clipboard-updated", async () => {
          console.log("Clipboard event received!"); // Check your console!
          await loadData(); // Manually trigger the fetch
        });

        return () => {
          unFocus();
          unClipboard();
        };
      } catch (err) {
        console.error("Failed to setup listeners:", err);
        return () => {};
      }
    };

    const cleanup = setupListeners();

    return () => {
      window.removeEventListener("keydown", handleKeydown);
      cleanup.then((c) => c());
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
  class="flex flex-col h-screen bg-[#1e1e1e] text-zinc-300 overflow-hidden border border-[#333] rounded-lg font-sans selection:bg-red-500/30 shadow-2xl relative"
>
  <div
    class="px-3 py-2 border-b border-[#333] bg-[#1e1e1e] flex items-center gap-2"
  >
    {#if currentCategory}
      <button
        onclick={() => (currentCategory = null)}
        class="bg-[#333] hover:bg-[#444] text-[10px] px-2 py-0.5 rounded text-white flex items-center"
      >
        ← {currentCategory}
      </button>
    {/if}
    <input
      type="text"
      bind:this={searchInput}
      bind:value={searchQuery}
      placeholder={currentCategory
        ? `Search in ${currentCategory}...`
        : "Search history & groups..."}
      class="flex-1 bg-transparent text-sm focus:outline-none placeholder:text-zinc-600 font-medium py-1"
      autofocus
    />
  </div>

  <div class="flex-1 overflow-y-auto custom-scrollbar" bind:this={container}>
    {#if !currentCategory}
      {#each filteredCategories as cat, i}
        <div
          class="w-full px-3 py-2 flex items-center justify-between group cursor-default border-b border-[#2a2a2a]/30 {i ===
          selectedIndex
            ? 'bg-[#333] text-white'
            : 'hover:bg-[#2a2a2a]'}"
          onclick={() => {
            currentCategory = cat;
            searchQuery = "";
          }}
          role="button"
          tabindex="0"
          data-index={i}
        >
          <div class="flex items-center space-x-3">
            <svg
              xmlns="http://www.w3.org/2000/svg"
              class="w-3.5 h-3.5 text-red-500/70"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              ><path
                d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"
              ></path></svg
            >
            <span class="text-[13px] font-medium">{cat}</span>
          </div>
          <span class="text-[10px] text-zinc-600 font-bold">GROUP</span>
        </div>
      {/each}
    {/if}

    {#each history as item, i}
      {@const idx = i + (currentCategory ? 0 : filteredCategories.length)}
      <div
        class="w-full px-3 py-2 flex items-center justify-between group cursor-default border-b border-[#2a2a2a]/50 {idx ===
        selectedIndex
          ? 'bg-[#3d3d3d] text-white'
          : 'hover:bg-[#2a2a2a]'}"
        onclick={() => copyAndPaste(item)}
        role="button"
        tabindex="0"
        data-index={idx}
      >
        <div class="flex items-center space-x-3 min-w-0 flex-1">
          <!-- Icon Indicator -->
          <div
            class="flex-shrink-0 w-5 h-5 flex items-center justify-center rounded bg-[#2a2a2a] text-zinc-500"
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="12"
              height="12"
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

          <p
            class="text-[13px] font-normal leading-tight flex-1 break-words whitespace-pre-wrap line-clamp-4"
          >
            {item.raw_content}
          </p>
        </div>

        <div class="flex items-center gap-2">
          <button
            onclick={(e) => {
              e.stopPropagation();
              togglePermanent(item);
            }}
            class="p-1 hover:bg-[#444] rounded transition-all {item.is_permanent
              ? 'text-amber-500 opacity-100'
              : 'text-zinc-500 opacity-0 group-hover:opacity-100'}"
            title={item.is_permanent ? "Unpin" : "Pin"}
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="12"
              height="12"
              viewBox="0 0 24 24"
              fill={item.is_permanent ? "currentColor" : "none"}
              stroke="currentColor"
              stroke-width="2.5"
              ><path
                d="M12 2L15.09 8.26L22 9.27L17 14.14L18.18 21.02L12 17.77L5.82 21.02L7 14.14L2 9.27L8.91 8.26L12 2Z"
              /></svg
            >
          </button>
          <button
            onclick={(e) => {
              e.stopPropagation();
              showGroupSelector = item.id;
            }}
            class="opacity-0 group-hover:opacity-100 p-1 hover:bg-[#444] rounded transition-all"
            title="Save to group"
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              class="w-3 h-3 text-zinc-500"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2.5"><path d="M12 5v14M5 12h14" /></svg
            >
          </button>
          <span
            class="text-[10px] text-zinc-600 font-mono opacity-0 group-hover:opacity-100"
          >
            {new Date(item.created_at).toLocaleTimeString([], {
              hour: "2-digit",
              minute: "2-digit",
            })}
          </span>
        </div>
      </div>
    {/each}
  </div>

  {#if showGroupSelector !== null}
    <div
      class="absolute inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4 z-50"
    >
      <div
        class="bg-[#2a2a2a] w-full max-w-[240px] rounded-lg border border-[#444] shadow-2xl overflow-hidden flex flex-col max-h-[80%]"
      >
        <div
          class="px-3 py-2 border-b border-[#444] flex justify-between items-center"
        >
          <span
            class="text-[10px] font-bold uppercase tracking-tight text-zinc-400"
            >Save to Group</span
          >
          <button
            onclick={() => (showGroupSelector = null)}
            class="text-zinc-500 hover:text-white">✕</button
          >
        </div>
        <div class="overflow-y-auto flex-1 p-1">
          {#each categories as cat}
            <button
              onclick={() => addToGroup(showGroupSelector!, cat)}
              class="w-full text-left px-3 py-1.5 text-[12px] hover:bg-[#3d3d3d] rounded flex items-center gap-2"
            >
              <svg
                xmlns="http://www.w3.org/2000/svg"
                class="w-3 h-3 text-red-500/50"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                ><path
                  d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"
                ></path></svg
              >
              {cat}
            </button>
          {/each}
        </div>
        <div class="p-2 border-t border-[#444] bg-[#222]">
          <input
            type="text"
            bind:value={newGroupName}
            placeholder="New group..."
            class="w-full bg-[#1a1a1a] border border-[#333] rounded px-2 py-1 text-[11px] focus:outline-none mb-1.5"
            onkeydown={(e) =>
              e.key === "Enter" && createAndAddToGroup(showGroupSelector!)}
          />
          <button
            onclick={() => createAndAddToGroup(showGroupSelector!)}
            class="w-full bg-red-600 hover:bg-red-700 text-white text-[10px] font-bold py-1 rounded"
            >CREATE & SAVE</button
          >
        </div>
      </div>
    </div>
  {/if}

  <div
    class="px-3 py-1 border-t border-[#333] bg-[#1a1a1a] flex justify-between items-center text-[8px] text-zinc-600 font-bold uppercase tracking-tighter"
  >
    <span
      >{history.length} Clips {currentCategory
        ? `in ${currentCategory}`
        : `/ ${categories.length} Groups`}</span
    >
    <div class="flex space-x-2">
      <span>⏎ Paste</span>
      <span>ESC Hide</span>
    </div>
  </div>
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
  .custom-scrollbar::-webkit-scrollbar-thumb {
    background: #333;
    border-radius: 10px;
  }
</style>
