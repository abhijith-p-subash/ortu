<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { ClipboardItem } from "$lib/types";
  import { listen } from "@tauri-apps/api/event";
  import { buildSearchQuery, clipPreview } from "$lib/filters";
  import "../../app.css";

  let history = $state<ClipboardItem[]>([]);
  let categories = $state<string[]>([]);
  let searchQuery = $state("");
  let selectedIndex = $state(0);
  let container = $state<HTMLDivElement | null>(null);
  let searchInput = $state<HTMLInputElement | null>(null);
  let shell = $state<HTMLDivElement | null>(null);

  let currentCategory = $state<string | null>(null);
  let showGroupSelector = $state<number | null>(null);
  let newGroupName = $state("");
  let hoverPreview = $state<{
    itemId: number; content: string; x: number; y: number; category: string | null;
  } | null>(null);
  const LONG_CONTENT_PREVIEW_THRESHOLD = 140;

  let filteredCategories = $derived(
    currentCategory || !searchQuery.trim()
      ? []
      : categories.filter(c => c.toLowerCase().includes(searchQuery.toLowerCase()))
  );

  async function loadData() {
    try {
      const search = currentCategory
        ? buildSearchQuery(currentCategory, searchQuery)
        : buildSearchQuery(null, searchQuery);
      const historyData = (await invoke("get_history", { search: search || null })) as ClipboardItem[];
      const catData = (await invoke("get_categories")) as string[];
      history = historyData;
      categories = catData;
      const total = history.length + (currentCategory ? 0 : filteredCategories.length);
      if (selectedIndex >= total) selectedIndex = Math.max(0, total - 1);
    } catch (e) { console.error("Failed to load data:", e); }
  }

  async function copyAndPaste(item: ClipboardItem) {
    try {
      hoverPreview = null;
      await invoke("copy_item_and_paste_from_popup", { id: item.id });
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      window.alert(message);
    }
  }

  async function addToGroup(itemId: number, groupName: string) {
    await invoke("add_to_group", { itemId, groupName });
    showGroupSelector = null;
    loadData();
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

  function handleKeydown(e: KeyboardEvent) {
    // ⌘1–9: instant copy by position
    const num = parseInt(e.key);
    if (!isNaN(num) && num >= 1 && num <= 9 && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      const item = history[num - 1];
      if (item) { selectedIndex = (currentCategory ? 0 : filteredCategories.length) + (num - 1); copyAndPaste(item); }
      return;
    }

    if (e.key === "Escape") {
      if (showGroupSelector !== null) { showGroupSelector = null; }
      else if (currentCategory) { currentCategory = null; }
      else { hoverPreview = null; invoke("close_window", { label: "popup" }); }
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      const total = history.length + (currentCategory ? 0 : filteredCategories.length);
      selectedIndex = (selectedIndex + 1) % (total || 1);
      scrollIntoView();
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      const total = history.length + (currentCategory ? 0 : filteredCategories.length);
      selectedIndex = (selectedIndex - 1 + (total || 1)) % (total || 1);
      scrollIntoView();
    } else if (e.key === "Enter") {
      const catsCount = currentCategory ? 0 : filteredCategories.length;
      if (selectedIndex < catsCount) {
        currentCategory = filteredCategories[selectedIndex];
        searchQuery = ""; selectedIndex = 0;
      } else {
        const item = history[selectedIndex - catsCount];
        if (item) copyAndPaste(item);
      }
    } else if (e.key === "Backspace" && searchQuery === "" && currentCategory) {
      currentCategory = null;
    } else if (e.key === "Delete" || (e.metaKey && e.key === "Backspace")) {
      const catsCount = currentCategory ? 0 : filteredCategories.length;
      const item = history[selectedIndex - catsCount];
      if (item) deleteItem(item);
    } else if (e.key === "p" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      const catsCount = currentCategory ? 0 : filteredCategories.length;
      const item = history[selectedIndex - catsCount];
      if (item) togglePermanent(item);
    }
  }

  function scrollIntoView() {
    tick().then(() => {
      container?.querySelector(`[data-index="${selectedIndex}"]`)?.scrollIntoView({ block: "nearest" });
    });
  }

  function fullPreviewContent(item: ClipboardItem): string {
    const raw = item.raw_content?.trim();
    return raw ? raw : clipPreview(item.raw_content, item.content_type);
  }

  function shouldShowHoverPreview(item: ClipboardItem): boolean {
    return fullPreviewContent(item).length > LONG_CONTENT_PREVIEW_THRESHOLD;
  }

  function updateHoverPreviewPosition(e: MouseEvent, item: ClipboardItem) {
    if (!shell) return;
    const rect = shell.getBoundingClientRect();
    const panelWidth = 300; const panelHeight = 220; const margin = 10;
    let x = e.clientX - rect.left + 14;
    if (x + panelWidth + margin > rect.width) x = Math.max(margin, e.clientX - rect.left - panelWidth - 14);
    let y = e.clientY - rect.top - 14;
    if (y + panelHeight + margin > rect.height) y = rect.height - panelHeight - margin;
    y = Math.max(margin, y);
    hoverPreview = { itemId: item.id, content: fullPreviewContent(item), x, y, category: item.category ?? null };
  }

  function handleItemHoverStart(e: MouseEvent, item: ClipboardItem) {
    if (showGroupSelector !== null || !shouldShowHoverPreview(item)) { hoverPreview = null; return; }
    updateHoverPreviewPosition(e, item);
  }

  function handleItemHoverMove(e: MouseEvent, item: ClipboardItem) {
    if (hoverPreview?.itemId !== item.id) return;
    updateHoverPreviewPosition(e, item);
  }

  function handleItemHoverEnd(item: ClipboardItem) {
    if (hoverPreview?.itemId === item.id) hoverPreview = null;
  }

  $effect(() => { loadData(); });

  onMount(() => {
    window.addEventListener("keydown", handleKeydown);
    const preventContextMenu = (e: MouseEvent) => e.preventDefault();
    window.addEventListener("contextmenu", preventContextMenu);

    const setupListeners = async () => {
      try {
        const unFocus = await listen("tauri://focus", () => {
          currentCategory = null; searchQuery = ""; selectedIndex = 0; hoverPreview = null;
          loadData();
          tick().then(() => {
            searchInput?.focus();
            container?.scrollTo({ top: 0, behavior: "instant" });
          });
        });
        const unClipboard = await listen("clipboard-updated", async () => { await loadData(); });
        return () => { unFocus(); unClipboard(); };
      } catch (err) { console.error("Failed to setup listeners:", err); return () => {}; }
    };

    const cleanup = setupListeners();
    return () => {
      window.removeEventListener("keydown", handleKeydown);
      window.removeEventListener("contextmenu", preventContextMenu);
      cleanup.then(c => c());
    };
  });

  function getTypeIcon(category: string | null): string {
    if (!category) return '<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="16" y1="13" x2="8" y2="13"/><line x1="16" y1="17" x2="8" y2="17"/>';
    const c = category.toLowerCase();
    if (c === "url") return '<circle cx="12" cy="12" r="10"/><line x1="2" y1="12" x2="22" y2="12"/><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/>';
    if (c.includes("docker") || c.includes("shell") || c.includes("terminal") || c.includes("cloud")) return '<polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/>';
    if (c.includes("git")) return '<line x1="6" y1="3" x2="6" y2="15"/><circle cx="18" cy="6" r="3"/><circle cx="6" cy="18" r="3"/><path d="M18 9a9 9 0 0 1-9 9"/>';
    if (c.includes("database") || c.includes("sql")) return '<ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"/><path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"/>';
    if (c.includes("code") || c.includes("ci")) return '<polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/>';
    return '<path d="M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82z"/><line x1="7" y1="7" x2="7.01" y2="7"/>';
  }
</script>

<!-- ─────────────────────────────────────────────────
     POPUP SHELL  —  Raycast-inspired compact launcher
───────────────────────────────────────────────── -->
<div
  bind:this={shell}
  class="popup-shell flex flex-col h-screen bg-[#0e1014]/[0.97] text-[#c8cdd4] overflow-hidden border border-white/[0.08] relative"
  style="backdrop-filter: blur(24px);"
>

  <!-- ── Search header ───────────────────────────── -->
  <div class="flex items-center gap-2.5 px-3.5 border-b border-white/[0.06] bg-[#09090c]/[0.6] shrink-0" style="min-height: 48px;">

    {#if currentCategory}
      <!-- Breadcrumb chip -->
      <button
        onclick={() => (currentCategory = null)}
        class="flex items-center gap-1.5 shrink-0 h-6 px-2.5 rounded-full bg-[#AEB291]/[0.12] border border-[#AEB291]/[0.2] text-[11px] font-medium text-[#AEB291] hover:bg-[#AEB291]/[0.18] transition-colors"
      >
        <svg xmlns="http://www.w3.org/2000/svg" width="9" height="9" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><polyline points="15 18 9 12 15 6"/></svg>
        {currentCategory}
      </button>
      <div class="w-px h-4 bg-white/[0.08] shrink-0"></div>
    {:else}
      <!-- Search icon -->
      <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" class="text-white/20 shrink-0 pointer-events-none">
        <circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/>
      </svg>
    {/if}

    <input
      type="text"
      bind:this={searchInput}
      bind:value={searchQuery}
      placeholder={currentCategory ? `Search in ${currentCategory}…` : "Search history, groups…"}
      class="flex-1 bg-transparent text-[14px] text-white/75 focus:outline-none placeholder:text-white/18 py-3"
    />

    {#if searchQuery}
      <button
        onclick={() => (searchQuery = "")}
        aria-label="Clear search"
        class="shrink-0 text-white/20 hover:text-white/50 transition-colors"
      >
        <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
      </button>
    {/if}
  </div>

  <!-- ── Item list ────────────────────────────────── -->
  <div class="flex-1 overflow-y-auto custom-scrollbar py-1.5 px-1.5" bind:this={container}>

    <!-- Group suggestions (while searching without category filter) -->
    {#if !currentCategory}
      {#each filteredCategories as cat, i}
        <div
          class="relative flex items-center justify-between px-3 py-2 rounded-xl cursor-default transition-all duration-75
            {i === selectedIndex ? 'bg-white/[0.07]' : 'hover:bg-white/[0.04]'}"
          onclick={() => { currentCategory = cat; searchQuery = ""; }}
          onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); currentCategory = cat; searchQuery = ""; } }}
          role="button" tabindex="0" data-index={i}
        >
          {#if i === selectedIndex}
            <div class="absolute left-0 top-1/2 -translate-y-1/2 w-[2px] h-5 bg-[#FF8A3D] rounded-r-full" aria-hidden="true"></div>
          {/if}
          <div class="flex items-center gap-2.5 min-w-0">
            <div class="w-[22px] h-[22px] flex items-center justify-center rounded-lg bg-[#FF8A3D]/[0.1] text-[#FF8A3D]/60 shrink-0">
              <svg xmlns="http://www.w3.org/2000/svg" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
              </svg>
            </div>
            <span class="text-[13px] font-medium text-white/60 truncate">{cat}</span>
          </div>
          <span class="text-[9px] font-semibold uppercase tracking-[0.1em] text-white/20 shrink-0 ml-2">Group</span>
        </div>
      {/each}
    {/if}

    <!-- Clip items -->
    {#each history as item, i}
      {@const idx = i + (currentCategory ? 0 : filteredCategories.length)}
      {@const isSelected = idx === selectedIndex}
      {@const itemUrl = (() => { try { if (!item.raw_content.trim().startsWith('http')) return null; return new URL(item.raw_content.trim()).hostname.replace(/^www\./, ''); } catch { return null; } })()}
      <div
        class="relative flex items-center gap-2.5 px-3 py-[8px] rounded-xl cursor-default transition-all duration-100 group
          {isSelected ? 'bg-white/[0.08]' : 'hover:bg-white/[0.04]'}"
        style="{isSelected ? 'box-shadow: inset 0 0 0 1px rgba(255,138,61,0.15)' : ''}"
        onclick={() => copyAndPaste(item)}
        onmouseenter={(e) => handleItemHoverStart(e, item)}
        onmousemove={(e) => handleItemHoverMove(e, item)}
        onmouseleave={() => handleItemHoverEnd(item)}
        onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); copyAndPaste(item); } }}
        role="button" tabindex="0" data-index={idx}
      >
        <!-- Left accent: orange when selected, type color otherwise -->
        {#if isSelected}
          <div class="absolute left-0 top-1/2 -translate-y-1/2 w-[2px] h-6 bg-gradient-to-b from-[#FF8A3D] to-[#ff6b1a] rounded-r-full" aria-hidden="true"></div>
        {:else if item.is_permanent}
          <div class="absolute left-0 top-1/2 -translate-y-1/2 w-[2px] h-4 bg-amber-400/40 rounded-r-full" aria-hidden="true"></div>
        {:else if itemUrl}
          <div class="absolute left-0 top-1/2 -translate-y-1/2 w-[2px] h-4 bg-sky-400/30 rounded-r-full" aria-hidden="true"></div>
        {/if}

        <!-- Type icon -->
        <div class="w-[22px] h-[22px] flex items-center justify-center rounded-lg bg-white/[0.05] text-white/25 shrink-0">
          <svg xmlns="http://www.w3.org/2000/svg" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
            {@html getTypeIcon(item.category)}
          </svg>
        </div>

        <!-- Content — description label + content -->
        <div class="min-w-0 flex-1 overflow-hidden">
          {#if isSelected}
            <!-- Expanded: description on its own line, content up to 3 lines -->
            {#if item.description}
              <p class="text-[10px] font-semibold text-[#AEB291]/70 truncate mb-0.5 tracking-tight">{item.description}</p>
            {/if}
            <p class="text-[13px] text-white/75 leading-snug break-words line-clamp-3 whitespace-pre-wrap">
              {clipPreview(item.raw_content, item.content_type)}
            </p>
          {:else}
            <!-- Compact: description · content on one line -->
            <div class="flex items-baseline gap-1.5 min-w-0">
              {#if item.description}
                <span class="text-[10px] font-semibold text-[#AEB291]/60 shrink-0 tracking-tight">{item.description}</span>
                <span class="text-white/18 text-[10px] shrink-0">·</span>
              {:else if itemUrl}
                <span class="text-[10px] font-semibold text-[#AEB291]/55 shrink-0 tracking-tight">{itemUrl}</span>
                <span class="text-white/18 text-[10px] shrink-0">·</span>
              {/if}
              <span class="text-[13px] text-white/55 truncate leading-snug">{clipPreview(item.raw_content, item.content_type)}</span>
            </div>
          {/if}
        </div>

        <!-- Right actions -->
        <div class="flex items-center gap-0.5 shrink-0">
          <!-- Pin: always visible if pinned, hover otherwise -->
          <button
            onclick={(e) => { e.stopPropagation(); togglePermanent(item); }}
            class="p-1 rounded-lg transition-all {item.is_permanent ? 'text-amber-400/80 opacity-100' : 'text-white/20 opacity-0 group-hover:opacity-100 hover:text-white/50'} hover:bg-white/[0.06]"
            title={item.is_permanent ? "Unpin" : "Pin"}
          >
            <svg xmlns="http://www.w3.org/2000/svg" width="11" height="11" viewBox="0 0 24 24" fill={item.is_permanent ? "currentColor" : "none"} stroke="currentColor" stroke-width="2.5" stroke-linecap="round">
              <line x1="12" y1="17" x2="12" y2="22"/><path d="M5 17h14v-1.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V6a3 3 0 0 0-3-3 3 3 0 0 0-3 3v4.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24Z"/>
            </svg>
          </button>
          <button
            onclick={(e) => { e.stopPropagation(); showGroupSelector = item.id; }}
            class="p-1 opacity-0 group-hover:opacity-100 hover:bg-white/[0.06] rounded-lg transition-all text-white/25 hover:text-white/60"
            title="Save to group"
          >
            <svg xmlns="http://www.w3.org/2000/svg" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><path d="M12 5v14M5 12h14"/></svg>
          </button>
          <button
            onclick={(e) => { e.stopPropagation(); deleteItem(item); }}
            class="p-1 opacity-0 group-hover:opacity-100 hover:bg-[#FF8A3D]/[0.08] rounded-lg transition-all text-white/25 hover:text-[#FF8A3D]/60"
            title="Delete"
          >
            <svg xmlns="http://www.w3.org/2000/svg" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round">
              <path d="M3 6h18"/><path d="M8 6V4h8v2"/><path d="M6 6l1 14h10l1-14"/>
            </svg>
          </button>
        </div>
      </div>
    {/each}

    <!-- Empty state -->
    {#if history.length === 0 && filteredCategories.length === 0}
      <div class="flex flex-col items-center justify-center py-10 text-center">
        <div class="w-10 h-10 rounded-2xl bg-white/[0.03] border border-white/[0.05] flex items-center justify-center mb-3 text-white/15">
          <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
            <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
          </svg>
        </div>
        <p class="text-[12px] text-white/25">{searchQuery ? "No results" : "Nothing here yet"}</p>
      </div>
    {/if}
  </div>

  <!-- ── Group selector overlay ───────────────────── -->
  {#if showGroupSelector !== null}
    <div class="absolute inset-0 bg-black/70 backdrop-blur-md flex items-center justify-center p-3 z-50">
      <div class="bg-[#13151b] w-full max-w-[240px] rounded-2xl border border-white/[0.08] shadow-2xl shadow-black/60 overflow-hidden flex flex-col max-h-[80%]">
        <div class="px-4 py-3 border-b border-white/[0.06] flex justify-between items-center">
          <span class="text-[11px] font-semibold text-white/35 uppercase tracking-widest">Save to Group</span>
          <button onclick={() => (showGroupSelector = null)} aria-label="Close" class="text-white/25 hover:text-white/70 transition-colors">
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
          </button>
        </div>
        <div class="overflow-y-auto flex-1 p-1.5 space-y-px">
          {#each categories as cat}
            <button
              onclick={() => addToGroup(showGroupSelector!, cat)}
              class="w-full text-left px-3 py-2 text-[12px] hover:bg-white/[0.05] rounded-xl transition-colors flex items-center gap-2 text-white/40 hover:text-white/80"
            >
              <svg xmlns="http://www.w3.org/2000/svg" class="w-3 h-3 text-[#FF8A3D]/40 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
              </svg>
              {cat}
            </button>
          {/each}
        </div>
        <div class="p-2 border-t border-white/[0.05]">
          <input type="text" bind:value={newGroupName} placeholder="New group…"
            class="w-full bg-white/[0.04] border border-white/[0.07] rounded-lg px-2.5 py-1.5 text-[12px] text-white/60 placeholder:text-white/20 focus:outline-none focus:border-white/[0.12] mb-1.5"
            onkeydown={(e) => e.key === "Enter" && createAndAddToGroup(showGroupSelector!)} />
          <button onclick={() => createAndAddToGroup(showGroupSelector!)}
            class="w-full bg-[#AEB291]/70 hover:bg-[#AEB291]/90 text-black text-[11px] font-semibold py-1.5 rounded-lg transition-colors">
            Create & Save
          </button>
        </div>
      </div>
    </div>
  {/if}

  <!-- ── Status bar ────────────────────────────────── -->
  <div class="px-4 py-2 border-t border-white/[0.05] bg-[#09090c]/[0.4] flex justify-between items-center shrink-0">
    <span class="text-[9px] font-medium text-white/20 tracking-wide">
      {history.length} clips{currentCategory ? ` in ${currentCategory}` : categories.length > 0 ? ` · ${categories.length} groups` : ""}
    </span>
    <div class="flex items-center gap-3">
      <span class="text-[9px] text-white/15 flex items-center gap-1">
        <kbd class="px-1 py-0.5 bg-white/[0.05] rounded text-[8px] border border-white/[0.07]">↵</kbd>
        paste
      </span>
      <span class="text-[9px] text-white/15 flex items-center gap-1">
        <kbd class="px-1 py-0.5 bg-white/[0.05] rounded text-[8px] border border-white/[0.07]">esc</kbd>
        hide
      </span>
    </div>
  </div>

</div>

<style>
  :global(html) {
    border-radius: 14px !important;
    overflow: hidden;
    background: transparent;
  }
  :global(body) {
    margin: 0;
    overflow: hidden;
    border-radius: 14px !important;
    background: transparent;
  }
  .popup-shell {
    border-radius: 14px !important;
  }
</style>
