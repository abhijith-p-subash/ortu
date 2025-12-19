<script lang="ts">
    import { onMount, tick } from 'svelte';
    import { invoke } from '@tauri-apps/api/core';
    import type { ClipboardItem } from '$lib/types';
    import { listen } from '@tauri-apps/api/event';
    import "../app.css";

    let history = $state<ClipboardItem[]>([]);
    let categories = $state<String[]>([]);
    let searchQuery = $state('');
    let selectedIndex = $state(0);
    let container = $state<HTMLDivElement | null>(null);
    let searchInput = $state<HTMLInputElement | null>(null);
    let isCategorizing = $state(false);
    let isViewingGroups = $state(false);
    let selectedCategory = $state<string | null>(null);
    let newCategory = $state('');

    async function loadHistory() {
        try {
            const search = selectedCategory ? `category:${selectedCategory} ${searchQuery}` : searchQuery;
            const data = await invoke('get_history', { search: search || null }) as ClipboardItem[];
            history = data;
            if (selectedIndex >= history.length) {
                selectedIndex = Math.max(0, history.length - 1);
            }
        } catch (e) {
            console.error('Failed to load history:', e);
        }
    }

    async function loadCategories() {
        try {
            categories = await invoke('get_categories') as String[];
        } catch (e) {
            console.error('Failed to load categories:', e);
        }
    }

    async function togglePermanent(item: ClipboardItem) {
        await invoke('toggle_permanent', { id: item.id });
        await loadHistory();
    }

    async function deleteItem(item: ClipboardItem) {
        if (!item) return;
        await invoke('delete_entry', { id: item.id });
        await loadHistory();
    }

    async function setCategory() {
        if (!history[selectedIndex] || !newCategory.trim()) return;
        await invoke('set_category', { id: history[selectedIndex].id, category: newCategory.trim() });
        isCategorizing = false;
        newCategory = '';
        await loadHistory();
        await loadCategories();
    }

    async function copyAndPaste(item: ClipboardItem) {
        try {
            // Copy to clipboard
            await navigator.clipboard.writeText(item.raw_content);
            
            // Close window using the close_window command
            await invoke('close_window');
            
            // Trigger paste
            await invoke('paste_item');
        } catch (err) {
            console.error('Failed to copy and paste: ', err);
        }
    }

    function handleKeydown(e: KeyboardEvent) {
        if (isCategorizing) {
            if (e.key === 'Enter') {
                e.preventDefault();
                setCategory();
            } else if (e.key === 'Escape') {
                isCategorizing = false;
                newCategory = '';
            }
            return;
        }

        if (e.key === 'Escape') {
            // Close window on Escape
            invoke('close_window');
        } else if (e.key === 'ArrowDown') {
            e.preventDefault();
            selectedIndex = (selectedIndex + 1) % (history.length || 1);
            scrollIntoView();
        } else if (e.key === 'ArrowUp') {
            e.preventDefault();
            selectedIndex = (selectedIndex - 1 + (history.length || 1)) % (history.length || 1);
            scrollIntoView();
        } else if (e.key === 'Enter') {
            if (history[selectedIndex]) {
                copyAndPaste(history[selectedIndex]);
            }
        } else if (e.key === 'Delete' || (e.metaKey && e.key === 'Backspace')) {
             if (history[selectedIndex]) {
                deleteItem(history[selectedIndex]);
            }
        } else if (e.key === 'p' && (e.metaKey || e.ctrlKey)) {
             if (history[selectedIndex]) {
                togglePermanent(history[selectedIndex]);
            }
        } else if (e.key === 'c' && (e.metaKey || e.ctrlKey)) {
             if (history[selectedIndex]) {
                 e.preventDefault();
                 isCategorizing = true;
             }
        } else if (e.key === 'g' && (e.metaKey || e.ctrlKey)) {
             e.preventDefault();
             isViewingGroups = !isViewingGroups;
             if (isViewingGroups) loadCategories();
        }
    }

    function scrollIntoView() {
        if (!container) return;
        const selectedElement = container.querySelector(`[data-index="${selectedIndex}"]`);
        selectedElement?.scrollIntoView({ block: 'nearest' });
    }

    $effect(() => {
        if (searchQuery !== undefined || selectedCategory !== undefined) {
             loadHistory();
        }
    });

    onMount(() => {
        loadHistory();
        loadCategories();
        window.addEventListener('keydown', handleKeydown);
        
        const unlistenFocus = listen('tauri://focus', async () => {
             await loadHistory();
             await loadCategories();
             selectedIndex = 0;
             await tick();
             searchInput?.focus();
        });

        // Note: The blur handler is now handled in Rust via on_window_event
        // But we can keep this for additional frontend logic if needed
        const unlistenBlur = listen('tauri://blur', async () => {
             // Window will auto-hide via Rust blur handler
             // You can add additional cleanup here if needed
        });

        return () => {
            window.removeEventListener('keydown', handleKeydown);
            unlistenFocus.then(f => f());
            unlistenBlur.then(f => f());
        };
    });
</script>

<div class="flex flex-col h-screen bg-[#1e1e1e] text-zinc-300 overflow-hidden border border-[#333] rounded-lg font-sans selection:bg-red-500/30 shadow-2xl relative">
    <!-- Groups Sidebar / Overlay -->
    {#if isViewingGroups}
        <div class="absolute left-0 top-0 bottom-0 w-48 bg-[#1a1a1a] border-r border-[#333] z-40 flex flex-col shadow-xl animate-in slide-in-from-left duration-200">
            <div class="p-3 border-b border-[#333] flex justify-between items-center">
                <span class="text-[10px] font-bold uppercase tracking-widest text-zinc-500">Groups</span>
                <button onclick={() => isViewingGroups = false} class="text-zinc-600 hover:text-zinc-300">
                    <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><path d="M18 6L6 18M6 6l12 12"/></svg>
                </button>
            </div>
            <div class="flex-1 overflow-y-auto custom-scrollbar">
                <button 
                    class="w-full text-left px-3 py-2 text-xs hover:bg-[#2a2a2a] transition-colors {selectedCategory === null ? 'text-red-500 font-bold bg-[#252525]' : 'text-zinc-400'}"
                    onclick={() => { selectedCategory = null; isViewingGroups = false; }}
                >
                    All History
                </button>
                {#each categories as category}
                    <button 
                        class="w-full text-left px-3 py-2 text-xs hover:bg-[#2a2a2a] transition-colors {selectedCategory === category ? 'text-red-500 font-bold bg-[#252525]' : 'text-zinc-400'}"
                        onclick={() => { selectedCategory = category as string; isViewingGroups = false; }}
                    >
                        {category}
                    </button>
                {/each}
            </div>
        </div>
    {/if}

    <!-- Search Bar -->
    <div class="px-3 py-2 border-b border-[#333] bg-[#1e1e1e] flex items-center space-x-2">
        <button 
            onclick={() => isViewingGroups = !isViewingGroups}
            class="p-1 rounded hover:bg-[#333] text-zinc-500 hover:text-red-500 transition-colors"
            title="View Groups (⌘G)"
        >
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="7" height="7"></rect><rect x="14" y="3" width="7" height="7"></rect><rect x="14" y="14" width="7" height="7"></rect><rect x="3" y="14" width="7" height="7"></rect></svg>
        </button>
        <div class="flex-1 flex items-center space-x-2">
            {#if selectedCategory}
                <span class="bg-red-500/10 text-red-500 text-[10px] font-bold px-1.5 py-0.5 rounded border border-red-500/20 max-w-[100px] truncate">
                    {selectedCategory}
                </span>
            {/if}
            <input
                type="text"
                bind:this={searchInput}
                bind:value={searchQuery}
                placeholder={selectedCategory ? "Search in Group..." : "Search history..."}
                class="flex-1 bg-transparent text-sm focus:outline-none placeholder:text-zinc-600 font-medium py-1"
                autofocus
            />
        </div>
    </div>

    <!-- History List -->
    <div class="flex-1 overflow-y-auto custom-scrollbar" bind:this={container}>
        {#each history as item, i (item.id)}
            <div
                class="w-full text-left px-3 py-2 flex items-center justify-between transition-none group cursor-default border-b border-[#2a2a2a]/50
                {i === selectedIndex ? 'bg-[#3d3d3d] text-white' : 'hover:bg-[#2a2a2a]'}"
                onclick={() => { selectedIndex = i; copyAndPaste(item); }}
                onkeydown={(e) => { if (e.key === 'Enter') { selectedIndex = i; copyAndPaste(item); } }}
                role="button"
                tabindex="0"
                data-index={i}
            >
                <div class="flex items-center space-x-3 min-w-0 flex-1">
                    <span class="text-[10px] text-zinc-600 font-bold w-4 text-center">
                        {i < 9 ? i + 1 : ''}
                    </span>
                    <div class="flex flex-col min-w-0 flex-1">
                        <p class="text-[13px] truncate font-normal leading-tight">
                            {item.raw_content.replace(/\s+/g, ' ')}
                        </p>
                        {#if item.category}
                            <div class="flex items-center space-x-1 mt-0.5">
                                <span class="text-[9px] font-bold uppercase tracking-tighter text-red-500/80">
                                    {item.category}
                                </span>
                            </div>
                        {/if}
                    </div>
                </div>
                
                <div class="flex items-center space-x-2 ml-2">
                    <button 
                        class="p-1 rounded hover:bg-[#4d4d4d] transition-colors {item.is_permanent ? 'text-amber-500' : 'text-zinc-600'} opacity-0 group-hover:opacity-100"
                        onclick={(e) => { e.stopPropagation(); togglePermanent(item); }}
                        title="Toggle Pin (⌘P)"
                    >
                         <svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 24 24" fill={item.is_permanent ? "currentColor" : "none"} stroke="currentColor" stroke-width="2"><path d="M12 2L15.09 8.26L22 9.27L17 14.14L18.18 21.02L12 17.77L5.82 21.02L7 14.14L2 9.27L8.91 8.26L12 2Z"/></svg>
                    </button>
                    <button 
                        class="p-1 rounded hover:bg-red-500/20 hover:text-red-500 transition-colors text-zinc-600 opacity-0 group-hover:opacity-100"
                        onclick={(e) => { e.stopPropagation(); deleteItem(item); }}
                        title="Delete (Del)"
                    >
                        <svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18"></path><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"></path><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"></path></svg>
                    </button>
                    <span class="text-[10px] text-zinc-600 font-mono opacity-0 group-hover:opacity-100 transition-opacity">
                        {new Date(item.created_at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                    </span>
                </div>
            </div>
        {/each}
        
        {#if history.length === 0}
            <div class="flex flex-col items-center justify-center h-full text-zinc-700 py-10">
                <p class="text-[11px] uppercase tracking-widest font-bold">Empty History</p>
            </div>
        {/if}
    </div>

    <!-- Categorize Popup Overlay -->
    {#if isCategorizing}
        <div class="absolute inset-x-0 top-[41px] bg-[#1e1e1e] border-b border-[#333] p-3 shadow-xl z-50 flex flex-col space-y-2">
            <p class="text-[10px] text-zinc-500 font-bold uppercase">Assign Category / Create Group</p>
            <input 
                type="text" 
                bind:value={newCategory} 
                placeholder="Enter group name..."
                class="w-full bg-[#2a2a2a] border border-[#444] rounded px-2 py-1.5 text-xs text-white focus:outline-none focus:border-red-500/50"
                autofocus
                onkeydown={(e) => { if (e.key === 'Escape') isCategorizing = false; }}
            />
            <div class="flex justify-end space-x-2">
                <button class="text-[10px] text-zinc-500 hover:text-zinc-300" onclick={() => isCategorizing = false}>Cancel</button>
                <button class="text-[10px] text-red-500 font-bold" onclick={setCategory}>Save</button>
            </div>
        </div>
    {/if}

    <!-- Footer Commands -->
    <div class="px-3 py-1.5 border-t border-[#333] bg-[#1a1a1a] flex justify-between items-center text-[9px] text-zinc-600 font-bold uppercase tracking-tighter">
        <div class="flex space-x-3">
             <span>{history.length} items</span>
             {#if selectedCategory}
                 <button class="text-red-500 hover:underline" onclick={() => selectedCategory = null}>Clear Filter</button>
             {/if}
        </div>
        <div class="flex space-x-3">
            <span><kbd class="bg-[#2a2a2a] px-1 rounded text-zinc-500">ESC</kbd> Close</span>
            <span><kbd class="bg-[#2a2a2a] px-1 rounded text-zinc-500">⌘G</kbd> Groups</span>
            <span><kbd class="bg-[#2a2a2a] px-1 rounded text-zinc-500">⌘C</kbd> Category</span>
            <span><kbd class="bg-[#2a2a2a] px-1 rounded text-zinc-500">⌘P</kbd> Pin</span>
            <span><kbd class="bg-[#2a2a2a] px-1 rounded text-zinc-500">⏎</kbd> Paste</span>
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