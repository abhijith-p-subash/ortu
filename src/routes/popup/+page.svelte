<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { ClipboardItem } from "$lib/types";
  import { listen } from "@tauri-apps/api/event";
  import { buildSearchQuery, clipPreview } from "$lib/filters";
  import "../../app.css";

  // --- STATE ---
  let history = $state<ClipboardItem[]>([]);
  let categories = $state<string[]>([]);
  let searchQuery = $state("");
  let selectedIndex = $state(0);
  let container = $state<HTMLDivElement | null>(null);
  let searchInput = $state<HTMLInputElement | null>(null);
  let shell = $state<HTMLDivElement | null>(null);

  // Navigation & Group state
  let currentCategory = $state<string | null>(null);
  let showGroupSelector = $state<number | null>(null);
  let newGroupName = $state("");
  let hoverPreview = $state<{
    itemId: number;
    content: string;
    x: number;
    y: number;
    category: string | null;
  } | null>(null);
  const LONG_CONTENT_PREVIEW_THRESHOLD = 140;

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
        search = buildSearchQuery(currentCategory, searchQuery);
      } else {
        search = buildSearchQuery(null, searchQuery);
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
      hoverPreview = null;
      // Close popup and trigger paste in the previous app
      await invoke("close_window", { label: "popup" });
      await invoke("copy_item_and_paste", { id: item.id });
    } catch (err) {
      console.error("Failed to copy and paste:", err);
    }
  }

  async function addToGroup(itemId: number, groupName: string) {
    await invoke("add_to_group", { itemId, groupName });
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
        hoverPreview = null;
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

  function fullPreviewContent(item: ClipboardItem): string {
    const raw = item.raw_content?.trim();
    if (raw) return raw;
    return clipPreview(item.raw_content, item.content_type);
  }

  function shouldShowHoverPreview(item: ClipboardItem): boolean {
    return fullPreviewContent(item).length > LONG_CONTENT_PREVIEW_THRESHOLD;
  }

  function updateHoverPreviewPosition(e: MouseEvent, item: ClipboardItem) {
    if (!shell) return;
    const rect = shell.getBoundingClientRect();
    const panelWidth = 300;
    const panelHeight = 220;
    const margin = 10;

    let x = e.clientX - rect.left + 14;
    if (x + panelWidth + margin > rect.width) {
      x = Math.max(margin, e.clientX - rect.left - panelWidth - 14);
    }

    let y = e.clientY - rect.top - 14;
    if (y + panelHeight + margin > rect.height) {
      y = rect.height - panelHeight - margin;
    }
    y = Math.max(margin, y);

    hoverPreview = {
      itemId: item.id,
      content: fullPreviewContent(item),
      x,
      y,
      category: item.category ?? null,
    };
  }

  function handleItemHoverStart(e: MouseEvent, item: ClipboardItem) {
    if (showGroupSelector !== null || !shouldShowHoverPreview(item)) {
      hoverPreview = null;
      return;
    }
    updateHoverPreviewPosition(e, item);
  }

  function handleItemHoverMove(e: MouseEvent, item: ClipboardItem) {
    if (hoverPreview?.itemId !== item.id) return;
    updateHoverPreviewPosition(e, item);
  }

  function handleItemHoverEnd(item: ClipboardItem) {
    if (hoverPreview?.itemId === item.id) {
      hoverPreview = null;
    }
  }

  // --- EFFECTS & LIFECYCLE ---
  // This automatically handles searching and category switching
  $effect(() => {
    loadData();
    // Dependencies: searchQuery, currentCategory
  });

  onMount(() => {
    window.addEventListener("keydown", handleKeydown);
    const preventContextMenu = (e: MouseEvent) => e.preventDefault();
    window.addEventListener("contextmenu", preventContextMenu);

    // This is the important part
    const setupListeners = async () => {
      try {
        // 1. Handle window focus (auto-reset)
        const unFocus = await listen("tauri://focus", () => {
          currentCategory = null;
          searchQuery = "";
          selectedIndex = 0;
          hoverPreview = null;
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
      window.removeEventListener("contextmenu", preventContextMenu);
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
  bind:this={shell}
  class="popup-shell flex flex-col h-screen bg-[#171a1d] text-zinc-300 overflow-hidden border border-[#333] rounded-lg font-sans selection:bg-[#FF8A3D]/30 shadow-2xl relative"
>
  <div
    class="px-3 py-2 border-b border-[#333] bg-[#171a1d] flex items-center gap-2"
  >
    {#if currentCategory}
      <button
        onclick={() => (currentCategory = null)}
        class="bg-[#343a42] hover:bg-[#2a3038] text-[10px] px-2 py-0.5 rounded text-white flex items-center"
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
    />
  </div>

  <div class="flex-1 overflow-y-auto custom-scrollbar p-2 space-y-1.5" bind:this={container}>
    {#if !currentCategory}
      {#each filteredCategories as cat, i}
        <div
          class="w-full px-3 py-2.5 flex items-center justify-between group cursor-default border rounded-md border-[#333]/70 transition-all {i ===
          selectedIndex
            ? 'bg-[#2a2f35] text-white shadow-sm'
            : 'hover:bg-[#343a42]/90'}"
          onclick={() => {
            currentCategory = cat;
            searchQuery = "";
          }}
          onkeydown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              currentCategory = cat;
              searchQuery = "";
            }
          }}
          role="button"
          tabindex="0"
          data-index={i}
        >
          <div class="flex items-center space-x-3">
            <svg
              xmlns="http://www.w3.org/2000/svg"
              class="w-3.5 h-3.5 text-[#FF8A3D]/75"
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
        class="w-full px-3 py-2.5 flex items-center justify-between gap-3 group cursor-default border rounded-md border-[#333]/70 transition-all {idx ===
        selectedIndex
          ? 'bg-[#2a2f35] text-white shadow-sm'
          : 'hover:bg-[#343a42]/90'}"
        onclick={() => copyAndPaste(item)}
        onmouseenter={(e) => handleItemHoverStart(e, item)}
        onmousemove={(e) => handleItemHoverMove(e, item)}
        onmouseleave={() => handleItemHoverEnd(item)}
        onkeydown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            copyAndPaste(item);
          }
        }}
        role="button"
        tabindex="0"
        data-index={idx}
      >
        <div class="flex items-center space-x-3 min-w-0 flex-1">
          <!-- Icon Indicator -->
          <div
            class="flex-shrink-0 w-5 h-5 flex items-center justify-center rounded bg-[#2a3038] text-zinc-500"
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

          <p class="text-[13px] text-zinc-200 font-normal leading-relaxed flex-1 break-words whitespace-pre-wrap line-clamp-4">
            {clipPreview(item.raw_content, item.content_type)}
          </p>
        </div>

        <div class="flex items-center gap-2">
          <button
            onclick={(e) => {
              e.stopPropagation();
              togglePermanent(item);
            }}
            class="p-1 hover:bg-[#343a42] rounded transition-all {item.is_permanent
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
            onclick={(e) => {
              e.stopPropagation();
              showGroupSelector = item.id;
            }}
            class="opacity-0 group-hover:opacity-100 p-1 hover:bg-[#343a42] rounded transition-all"
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
          <button
            onclick={(e) => {
              e.stopPropagation();
              deleteItem(item);
            }}
            class="opacity-0 group-hover:opacity-100 p-1 hover:bg-[#343a42] rounded transition-all"
            title="Delete"
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              class="w-3 h-3 text-zinc-500"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2.5"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <path d="M3 6h18" />
              <path d="M8 6V4h8v2" />
              <path d="M6 6l1 14h10l1-14" />
              <path d="M10 11v6" />
              <path d="M14 11v6" />
            </svg>
          </button>
          <!-- <span
            class="text-[10px] text-zinc-600 font-mono opacity-0 group-hover:opacity-100"
          >
            {new Date(item.created_at).toLocaleTimeString([], {
              hour: "2-digit",
              minute: "2-digit",
            })}
          </span> -->
        </div>
      </div>
    {/each}
  </div>

  <!-- Floating preview panel on hover for long content items -->
  <!-- {#if hoverPreview && showGroupSelector === null}
    <div
      class="absolute z-40 pointer-events-none w-[300px] max-h-[400px] bg-[#171a1d] border border-[#333] rounded-md shadow-2xl shadow-black/40 overflow-hidden"
      style={`left:${hoverPreview.x}px; top:${hoverPreview.y}px;`}
    >
      <div
        class="px-3 py-1.5 border-b border-[#333] bg-[#2a2f35] flex items-center justify-between"
      >
        <span class="text-[9px] font-bold uppercase tracking-wider text-[#FF8A3D]"
          >Full Preview</span
        >
        {#if hoverPreview.category}
          <span class="text-[9px] font-bold uppercase tracking-wider text-zinc-500"
            >{hoverPreview.category}</span
          >
        {/if}
      </div>
      <pre
        class="p-3 text-[8px] leading-relaxed text-zinc-200 whitespace-pre-wrap max-h-[300px] overflow-y-auto "
      >{hoverPreview.content}</pre>
    </div>
  {/if} -->

  {#if showGroupSelector !== null}
    <div
      class="absolute inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4 z-50"
    >
      <div
        class="bg-[#171a1d] w-full max-w-[240px] rounded-lg border border-[#333] shadow-2xl overflow-hidden flex flex-col max-h-[80%]"
      >
        <div
          class="px-3 py-2 border-b border-[#333] flex justify-between items-center"
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
              class="w-full text-left px-3 py-1.5 text-[12px] hover:bg-[#343a42] rounded flex items-center gap-2"
            >
              <svg
                xmlns="http://www.w3.org/2000/svg"
                class="w-3 h-3 text-[#FF8A3D]/60"
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
        <div class="p-2 border-t border-[#333] bg-[#171a1d]">
          <input
            type="text"
            bind:value={newGroupName}
            placeholder="New group..."
            class="w-full bg-[#171a1d] border border-[#333] rounded px-2 py-1 text-[11px] text-white placeholder:text-zinc-600 focus:outline-none focus:border-[#AEB291]/45 mb-1.5"
            onkeydown={(e) =>
              e.key === "Enter" && createAndAddToGroup(showGroupSelector!)}
          />
          <button
            onclick={() => createAndAddToGroup(showGroupSelector!)}
            class="w-full bg-[#3F423C] hover:bg-[#4d514a] text-white text-[10px] font-bold py-1 rounded"
            >CREATE & SAVE</button
          >
        </div>
      </div>
    </div>
  {/if}

  <div
    class="px-3 py-1 border-t border-[#333] bg-[#171a1d] flex justify-between items-center text-[8px] text-zinc-600 font-bold uppercase tracking-tighter"
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
  :global(html) {
    border-radius: 0.175rem !important;
    overflow: hidden;
    background: transparent;
  }

  :global(body) {
    margin: 0;
    overflow: hidden;
    border-radius: 0.175rem !important;
    background: transparent;
  }

  .popup-shell {
    border-radius: 0.175rem !important;
  }
  .custom-scrollbar::-webkit-scrollbar {
    width: 3px;
  }
  .custom-scrollbar::-webkit-scrollbar-thumb {
    background: #333;
    border-radius: 10px;
  }
</style>
