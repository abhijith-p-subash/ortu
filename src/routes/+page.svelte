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

  // ── Core state ─────────────────────────────────────────
  let history = $state<ClipboardItem[]>([]);
  let allItems = $state<ClipboardItem[]>([]); // unfiltered, for sidebar counts
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
  let flashingItemId = $state<number | null>(null);
  let flashTimer: number | null = null;

  // ── Modal / IO state ───────────────────────────────────
  let showExportModal = $state(false);
  let showImportModal = $state(false);
  let exportSelectedGroups = $state<string[]>([]);
  let importMode = $state<"merge" | "replace">("merge");
  let processingIO = $state(false);
  let showHelpModal = $state(false);
  let showAboutModal = $state(false);
  let currentPlatform = $state<string>("macos");
  let macAccessibilityGranted = $state(true);
  let checkingMacAccessibility = $state(false);
  let showCopiedToast = $state(false);
  let copiedToastTimer: number | null = null;
  let showMoreMenu = $state(false);
  let updateAvailable = $state(false);
  let latestVersion = $state("");
  let releaseUrl = $state("https://github.com/abhijith-p-subash/ortu/releases/latest");
  let showAddItemModal = $state(false);
  let newItemContent = $state("");
  let newItemDescription = $state("");
  let newItemGroupInput = $state("");
  let importGroupNameInput = $state("");
  let showImportGroupModal = $state(false);
  let importGroupPath = $state("");

  // ── Edit item modal state ──────────────────────────────
  let showEditModal = $state(false);
  let editingItem = $state<ClipboardItem | null>(null);
  let editContent = $state("");
  let editDescription = $state("");

  interface Toast { id: number; message: string; type: "success" | "error" | "info" }
  let toasts = $state<Toast[]>([]);
  let toastCounter = 0;

  function showToast(message: string, type: Toast["type"] = "info") {
    const id = ++toastCounter;
    toasts = [...toasts, { id, message, type }];
    setTimeout(() => { toasts = toasts.filter(t => t.id !== id); }, 3000);
  }

  let confirmModal = $state<{ message: string; onConfirm: () => void } | null>(null);
  function confirmAction(message: string, onConfirm: () => void) {
    confirmModal = { message, onConfirm };
  }

  // ── Derived: pinned-first sort ─────────────────────────
  let displayHistory = $derived([
    ...history.filter(i => i.is_permanent),
    ...history.filter(i => !i.is_permanent),
  ]);

  // ── Derived: time-grouped display history ──────────────
  interface IndexedItem { item: ClipboardItem; index: number }
  interface GroupedHistory {
    pinned: IndexedItem[];
    today: IndexedItem[];
    yesterday: IndexedItem[];
    thisWeek: IndexedItem[];
    older: IndexedItem[];
  }

  let groupedHistory = $derived.by((): GroupedHistory => {
    const now = new Date();
    const todayStart    = new Date(now.getFullYear(), now.getMonth(), now.getDate());
    const yesterStart   = new Date(todayStart.getTime() - 86_400_000);
    const weekStart     = new Date(todayStart.getTime() - 7 * 86_400_000);
    const result: GroupedHistory = { pinned: [], today: [], yesterday: [], thisWeek: [], older: [] };
    displayHistory.forEach((item, index) => {
      if (item.is_permanent) { result.pinned.push({ item, index }); return; }
      const d = new Date(item.created_at);
      if      (d >= todayStart)  result.today.push({ item, index });
      else if (d >= yesterStart) result.yesterday.push({ item, index });
      else if (d >= weekStart)   result.thisWeek.push({ item, index });
      else                       result.older.push({ item, index });
    });
    return result;
  });

  // ── Derived: sidebar counts ────────────────────────────
  let sidebarCounts = $derived.by(() => {
    const all  = allItems.length;
    const url  = allItems.filter(i => i.category === "URL").length;
    const text = allItems.filter(i => i.category === "Text").length;
    const groupCounts: Record<string, number> = {};
    for (const g of groups) {
      groupCounts[g] = allItems.filter(i => i.groups?.includes(g)).length;
    }
    return { all, url, text, groups: groupCounts };
  });

  // ── Platform / version ─────────────────────────────────
  let modKey  = $derived(currentPlatform === "macos" ? "Cmd"    : "Ctrl");
  let deleteKey = $derived(currentPlatform === "macos" ? "⌫"  : "Backspace");
  let altKey  = $derived(currentPlatform === "macos" ? "Option" : "Alt");

  onMount(async () => {
    try {
      currentPlatform = await platform();
      appVersion = await getVersion();
      await refreshMacAccessibilityStatus();
      setTimeout(checkForUpdates, 2000);
    } catch (e) { console.error(e); }
  });

  // ── Helpers ────────────────────────────────────────────

  function relativeTime(dateStr: string): string {
    const diff = Date.now() - new Date(dateStr).getTime();
    const m = Math.floor(diff / 60_000);
    const h = Math.floor(diff / 3_600_000);
    const d = Math.floor(diff / 86_400_000);
    if (m < 1)  return "just now";
    if (m < 60) return `${m}m ago`;
    if (h < 24) return `${h}h ago`;
    if (d === 1) return "Yesterday";
    if (d < 7)  return new Date(dateStr).toLocaleDateString([], { weekday: "short" });
    return new Date(dateStr).toLocaleDateString([], { month: "short", day: "numeric" });
  }

  function parseUrl(content: string): { domain: string; path: string } | null {
    try {
      if (!content.trim().startsWith("http")) return null;
      const u = new URL(content.trim());
      const domain = u.hostname.replace(/^www\./, "");
      const path = u.pathname + u.search + u.hash;
      return { domain, path: path === "/" ? "" : path };
    } catch { return null; }
  }

  function detectCodeLang(content: string): string | null {
    const c = content.trim();
    if (/^(const|let|var|function\s|class\s|import\s|export\s|async\s|=>\s*{)/.test(c)) return "JS/TS";
    if (/^(def |class |import |from \w+ import|print\(|if __name__)/.test(c)) return "Python";
    if (/^(<[a-zA-Z]|<!DOCTYPE|<!--)/.test(c)) return "HTML";
    if (/^(SELECT|INSERT|UPDATE|DELETE|CREATE|DROP|ALTER)\s/i.test(c)) return "SQL";
    if (/^(git |npm |yarn |pip |brew |docker |kubectl |curl |wget |chmod |sudo )/.test(c)) return "Shell";
    if (/^\s*[{\[]/.test(c) && c.length > 10) {
      try { JSON.parse(c); return "JSON"; } catch { /* not json */ }
    }
    return null;
  }

  // ── Data loading ───────────────────────────────────────

  async function loadHistory() {
    try {
      const search = selectedGroup
        ? buildSearchQuery(selectedGroup, searchQuery)
        : buildSearchQuery(null, searchQuery);
      const data = (await invoke("get_history", { search: search || null })) as ClipboardItem[];
      history = data;
      if (selectedIndex >= history.length) selectedIndex = Math.max(0, history.length - 1);
    } catch (e) { console.error("Failed to load history:", e); }
  }

  async function loadAllItems() {
    try {
      const data = (await invoke("get_history", { search: null })) as ClipboardItem[];
      allItems = data;
    } catch { /* non-critical */ }
  }

  async function loadGroups() {
    try { groups = (await invoke("get_categories")) as string[]; }
    catch (e) { console.error("Failed to load groups:", e); }
  }

  async function refreshAll() {
    await Promise.all([loadHistory(), loadGroups(), loadAllItems()]);
  }

  async function openImportModal() {
    importMode = "merge";
    showImportModal = true;
  }

  // ── Version check ──────────────────────────────────────

  async function checkForUpdates() {
    try {
      const res = await fetch("https://api.github.com/repos/abhijith-p-subash/ortu/releases/latest");
      if (!res.ok) return;
      const data = await res.json();
      const latest = (data.tag_name as string)?.replace(/^v/, "");
      if (!latest || latest === appVersion) return;
      const cur = appVersion.split(".").map(Number);
      const rem = latest.split(".").map(Number);
      if (rem[0] > cur[0] || (rem[0]===cur[0] && rem[1]>cur[1]) || (rem[0]===cur[0] && rem[1]===cur[1] && (rem[2]??0)>(cur[2]??0))) {
        updateAvailable = true; latestVersion = latest;
        releaseUrl = (data.html_url as string) ?? releaseUrl;
      }
    } catch { /* silently ignore */ }
  }

  // ── Group management ───────────────────────────────────

  async function createGroup() {
    if (!newGroupName.trim()) return;
    try { await invoke("create_group", { name: newGroupName.trim() }); newGroupName = ""; await loadGroups(); }
    catch (e) { console.error(e); }
  }

  async function deleteGroup(name: string) {
    confirmAction(`Delete group "${name}"? Items will NOT be deleted.`, async () => {
      try {
        await invoke("delete_group", { name });
        if (selectedGroup === name) selectedGroup = null;
        await refreshAll();
        showToast(`Group "${name}" deleted`, "success");
      } catch (e) { showToast("Failed: " + e, "error"); }
    });
  }

  async function renameGroup() {
    if (!editingGroup || !editGroupName.trim()) return;
    try {
      await invoke("rename_group", { oldName: editingGroup, newName: editGroupName.trim() });
      if (selectedGroup === editingGroup) selectedGroup = editGroupName.trim();
      editingGroup = null; editGroupName = "";
      await refreshAll();
    } catch (e) { console.error(e); }
  }

  // ── Backup / restore ───────────────────────────────────

  async function openExportModal() {
    exportSelectedGroups = [];
    if (selectedGroup && !["URL","Dev","Code","Images","Text"].includes(selectedGroup))
      exportSelectedGroups = [selectedGroup];
    showExportModal = true;
  }

  async function performExport() {
    try {
      const path = await save({ filters: [{ name: "JSON", extensions: ["json"] }], defaultPath: `ortu_backup_${new Date().toISOString().split("T")[0]}.json` });
      if (!path) return;
      processingIO = true;
      await invoke("backup_data", { path, groups: exportSelectedGroups.length > 0 ? exportSelectedGroups : [] });
      showExportModal = false; showToast("Export successful", "success");
    } catch (e) { showToast("Export failed: " + e, "error"); }
    finally { processingIO = false; }
  }

  async function performImport() {
    try {
      const path = await open({ filters: [{ name: "JSON", extensions: ["json"] }] });
      if (path) {
        processingIO = true;
        await invoke("restore_data", { path: path as string, mode: importMode });
        showImportModal = false;
        await refreshAll();
        showToast("Import successful", "success");
      }
    } catch (e) { showToast("Import failed: " + e, "error"); }
    finally { processingIO = false; }
  }

  async function exportGroup(name: string) {
    try {
      const path = await save({ filters: [{ name: "Text", extensions: ["txt"] }], defaultPath: `${name}_export.txt` });
      if (path && typeof path === "string") { await invoke("export_group", { name, path }); showToast("Exported", "success"); }
    } catch (e) { showToast("Export failed: " + e, "error"); }
  }

  async function exportAllTxt() {
    try {
      const path = await save({ filters: [{ name: "Text", extensions: ["txt"] }], defaultPath: "ortu_full_export.txt" });
      if (path && typeof path === "string") { await invoke("export_all_txt", { path }); showToast("Export successful", "success"); }
    } catch (e) { showToast("Export failed: " + e, "error"); }
  }

  async function importGroup() {
    try {
      const path = await open({ filters: [{ name: "Text", extensions: ["txt"] }] });
      if (path && typeof path === "string") { importGroupPath = path; importGroupNameInput = ""; showImportGroupModal = true; }
    } catch (e) { showToast("Failed to open: " + e, "error"); }
  }

  async function confirmImportGroup() {
    if (!importGroupNameInput.trim()) return;
    try {
      await invoke("import_group", { name: importGroupNameInput.trim(), path: importGroupPath });
      await refreshAll(); showImportGroupModal = false; showToast("Group imported", "success");
    } catch (e) { showToast("Import failed: " + e, "error"); }
  }

  // ── Item actions ───────────────────────────────────────

  async function togglePermanent(item: ClipboardItem) {
    await invoke("toggle_permanent", { id: item.id }); await refreshAll();
  }

  async function deleteItem(item: ClipboardItem) {
    if (!item) return;
    await invoke("delete_entry", { id: item.id }); await loadHistory(); await loadAllItems();
  }

  async function moveItemToGroup() {
    const itemId = categorizingItemId || (displayHistory[selectedIndex] ? displayHistory[selectedIndex].id : null);
    if (!itemId || !newGroupName.trim()) return;
    try {
      await invoke("add_to_group", { itemId, groupName: newGroupName.trim() });
      isCategorizing = false; newGroupName = "";
      await refreshAll();
    } catch (e) { console.error(e); }
  }

  async function removeFromGroup(item: ClipboardItem, group: string) {
    try { await invoke("remove_from_group", { itemId: item.id, groupName: group }); await loadHistory(); await loadAllItems(); }
    catch (e) { console.error(e); }
  }

  // ── Copy — primary action ──────────────────────────────
  async function copyItem(item: ClipboardItem, index: number) {
    selectedIndex = index;
    try {
      await invoke("copy_item_to_clipboard", { id: item.id });
      // Flash the card
      if (flashTimer) clearTimeout(flashTimer);
      flashingItemId = item.id;
      flashTimer = window.setTimeout(() => { flashingItemId = null; flashTimer = null; }, 380);
      // Brief toast confirmation
      if (copiedToastTimer) clearTimeout(copiedToastTimer);
      showCopiedToast = true;
      copiedToastTimer = window.setTimeout(() => { showCopiedToast = false; copiedToastTimer = null; }, 1800);
    } catch (err) { console.error("Failed to copy:", err); }
  }

  async function addManualItem() {
    if (!newItemContent.trim()) return;
    try {
      await invoke("add_manual_item", {
        content: newItemContent.trim(),
        description: newItemDescription.trim() || null,
        groupName: newItemGroupInput.trim() || null,
      });
      newItemContent = ""; newItemDescription = ""; newItemGroupInput = selectedGroup || "";
      showAddItemModal = false;
      await refreshAll();
      showToast("Item added", "success");
    } catch (e) { showToast("Failed to add: " + e, "error"); }
  }

  function openEditModal(item: ClipboardItem) {
    editingItem = item;
    editContent = item.raw_content;
    editDescription = item.description || "";
    showEditModal = true;
  }

  async function saveEditItem() {
    if (!editingItem || !editContent.trim()) return;
    try {
      await invoke("update_item", {
        id: editingItem.id,
        content: editContent.trim(),
        description: editDescription.trim() || null,
      });
      showEditModal = false;
      editingItem = null;
      await refreshAll();
      showToast("Item updated", "success");
    } catch (e) { showToast("Failed to update: " + e, "error"); }
  }

  // ── Keyboard ───────────────────────────────────────────
  function handleKeydown(e: KeyboardEvent) {
    // ⌘1–9: instant copy
    const num = parseInt(e.key);
    if (!isNaN(num) && num >= 1 && num <= 9 && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      const item = displayHistory[num - 1];
      if (item) copyItem(item, num - 1);
      return;
    }

    if (isCategorizing) {
      if (e.key === "Enter") { e.preventDefault(); moveItemToGroup(); }
      else if (e.key === "Escape") { isCategorizing = false; newGroupName = ""; }
      return;
    }

    if (showEditModal) {
      if (e.key === "Escape") showEditModal = false;
      return;
    }

    if (e.key === "Escape") invoke("close_window");
    else if (e.key === "ArrowDown") {
      e.preventDefault();
      selectedIndex = (selectedIndex + 1) % (displayHistory.length || 1);
      scrollIntoView();
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      selectedIndex = (selectedIndex - 1 + (displayHistory.length || 1)) % (displayHistory.length || 1);
      scrollIntoView();
    } else if (e.key === "Enter") {
      const item = displayHistory[selectedIndex];
      if (item) copyItem(item, selectedIndex);
    } else if (e.key === "Delete" || (e.metaKey && e.key === "Backspace")) {
      const item = displayHistory[selectedIndex];
      if (item) deleteItem(item);
    } else if (e.key === "p" && (e.metaKey || e.ctrlKey)) {
      const item = displayHistory[selectedIndex];
      if (item) togglePermanent(item);
    } else if (e.key === "c" && (e.metaKey || e.ctrlKey)) {
      const item = displayHistory[selectedIndex];
      if (item) { e.preventDefault(); categorizingItemId = item.id; isCategorizing = true; }
    } else if (e.key === "g" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      isViewingGroups = !isViewingGroups;
      if (isViewingGroups) loadGroups();
    }
  }

  function scrollIntoView() {
    if (!container) return;
    container.querySelector(`[data-index="${selectedIndex}"]`)?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }

  // ── Effects ────────────────────────────────────────────
  $effect(() => { if (selectedIndex !== undefined) scrollIntoView(); });
  $effect(() => { if (searchQuery !== undefined || selectedGroup !== undefined) loadHistory(); });

  onMount(() => {
    refreshAll();
    window.addEventListener("keydown", handleKeydown);
    const noCtxMenu = (e: MouseEvent) => e.preventDefault();
    window.addEventListener("contextmenu", noCtxMenu);

    let unlistenFocus: () => void;
    let unlistenClipboard: () => void;

    const setup = async () => {
      try {
        const uF = await listen("tauri://focus", async () => {
          await refreshAll(); await tick(); searchInput?.focus();
        });
        unlistenFocus = uF;
        const uC = await listen("clipboard-updated", async () => { await loadHistory(); await loadAllItems(); });
        unlistenClipboard = uC;
      } catch (e) { console.error(e); }
    };
    setup();

    return () => {
      window.removeEventListener("keydown", handleKeydown);
      window.removeEventListener("contextmenu", noCtxMenu);
      if (unlistenFocus) unlistenFocus();
      if (unlistenClipboard) unlistenClipboard();
    };
  });

  async function refreshMacAccessibilityStatus() {
    if (currentPlatform !== "macos") { macAccessibilityGranted = true; return; }
    try {
      checkingMacAccessibility = true;
      macAccessibilityGranted = (await invoke("get_macos_accessibility_status")) as boolean;
    } catch { macAccessibilityGranted = false; }
    finally { checkingMacAccessibility = false; }
  }

  async function openMacAccessibilitySettings() {
    try { await invoke("open_macos_accessibility_settings"); } catch { /* ignore */ }
  }

  function getCategoryIcon(category: string | null): string {
    if (!category) return '<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="16" y1="13" x2="8" y2="13"/><line x1="16" y1="17" x2="8" y2="17"/>';
    const c = category.toLowerCase();
    if (c === "url") return '<circle cx="12" cy="12" r="10"/><line x1="2" y1="12" x2="22" y2="12"/><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/>';
    if (c.includes("docker")||c.includes("shell")||c.includes("kubernetes")||c.includes("cloud")||c.includes("terminal")) return '<polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/>';
    if (c.includes("git")||c.includes("version")) return '<line x1="6" y1="3" x2="6" y2="15"/><circle cx="18" cy="6" r="3"/><circle cx="6" cy="18" r="3"/><path d="M18 9a9 9 0 0 1-9 9"/>';
    if (c.includes("database")||c.includes("sql")) return '<ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"/><path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"/>';
    if (c.includes("code")||c.includes("runtime")||c.includes("package")||c.includes("ci")) return '<polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/>';
    return '<path d="M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82z"/><line x1="7" y1="7" x2="7.01" y2="7"/>';
  }
</script>

<!-- ═══════════════════════════════════════════════════════
     ROOT
════════════════════════════════════════════════════════ -->
<div class="flex flex-col h-screen bg-[#08090c] text-[#d2d6dd] overflow-hidden selection:bg-[#FF8A3D]/20">

  <!-- ── Header ─────────────────────────────────────── -->
  <header class="mt-6 h-[44px] shrink-0 px-4 flex items-center justify-between bg-[#08090c] border-b border-white/[0.09]">
    <div class="flex items-center gap-2">
      <img src="/logo.png" alt="" class="w-[18px] h-[18px] shrink-0 opacity-90" />
      <span class="text-[13px] font-semibold text-white/88 tracking-tight">Ortu</span>
    </div>
    <div class="flex items-center gap-1">
      <button
        onclick={() => { newItemGroupInput = selectedGroup || ""; showAddItemModal = true; }}
        class="flex items-center gap-1.5 h-[26px] px-2.5 rounded-md text-[11px] font-semibold text-black transition-all bg-[#FF8A3D] hover:bg-[#ff9a56] active:scale-95 shadow-sm shadow-[#FF8A3D]/25"
      >
        <svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
        Add
      </button>
      <div class="relative">
        <button
          onclick={() => (showMoreMenu = !showMoreMenu)}
          aria-label="More options"
          class="h-[26px] w-[26px] flex items-center justify-center rounded-md text-white/55 hover:text-white/90 hover:bg-white/[0.09] transition-all"
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="currentColor" stroke="none">
            <circle cx="12" cy="5" r="1.6"/><circle cx="12" cy="12" r="1.6"/><circle cx="12" cy="19" r="1.6"/>
          </svg>
        </button>
        {#if showMoreMenu}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div role="menu" tabindex="-1"
            class="absolute right-0 top-[30px] w-44 bg-[#1c1f28] border border-white/[0.14] rounded-xl shadow-2xl shadow-black/70 z-50 py-1.5"
            onmouseleave={() => (showMoreMenu = false)}>
            <button onclick={() => { showMoreMenu=false; openExportModal(); }}   class="menu-item">
              <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"/><polyline points="17 21 17 13 7 13 7 21"/><polyline points="7 3 7 8 15 8"/></svg>
              Backup
            </button>
            <button onclick={() => { showMoreMenu=false; openImportModal(); }}   class="menu-item">
              <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
              Restore
            </button>
            <button onclick={() => { showMoreMenu=false; exportAllTxt(); }}      class="menu-item">
              <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" y1="3" x2="12" y2="15"/></svg>
              Export All (.txt)
            </button>
            <div class="h-px bg-white/[0.05] my-1 mx-3"></div>
            <button onclick={() => { showMoreMenu=false; showHelpModal=true; }} class="menu-item">
              <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>
              Help
            </button>
            <button onclick={() => { showMoreMenu=false; showAboutModal=true; }} class="menu-item">
              <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/></svg>
              About
            </button>
          </div>
        {/if}
      </div>
    </div>
  </header>

  <!-- ── Banners ───────────────────────────────────────── -->
  {#if updateAvailable}
    <div class="mx-3 mt-2 flex items-center justify-between rounded-lg bg-[#AEB291]/[0.07] border border-[#AEB291]/[0.15] px-3 py-2">
      <p class="text-[11px] text-white/50"><span class="font-semibold text-white/80">v{latestVersion}</span> is available</p>
      <div class="flex items-center gap-3 shrink-0">
        <a href={releaseUrl} target="_blank" rel="noopener noreferrer" class="text-[11px] font-semibold text-[#AEB291] hover:text-white transition-colors">Download →</a>
        <button onclick={() => (updateAvailable = false)} aria-label="Dismiss" class="text-white/25 hover:text-white/60 transition-colors">
          <svg xmlns="http://www.w3.org/2000/svg" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
        </button>
      </div>
    </div>
  {/if}

  {#if currentPlatform === "macos" && !macAccessibilityGranted}
    <div class="mx-3 mt-2 rounded-lg border border-[#FF8A3D]/[0.18] bg-[#FF8A3D]/[0.05] px-3 py-2.5">
      <div class="flex items-start justify-between gap-3">
        <div>
          <p class="text-[12px] font-semibold text-white/75">Accessibility permission needed</p>
          <p class="mt-0.5 text-[11px] text-white/35 leading-relaxed">Enable <span class="text-white/60 font-medium">Ortu</span> in System Settings → Privacy & Security → Accessibility.</p>
        </div>
        <div class="flex shrink-0 items-center gap-1.5 mt-0.5">
          <button onclick={refreshMacAccessibilityStatus} disabled={checkingMacAccessibility}
            class="h-6 px-2.5 rounded-md border border-white/[0.1] bg-white/[0.05] text-[11px] font-medium text-white/50 hover:text-white/80 transition-colors">
            {checkingMacAccessibility ? "…" : "Refresh"}
          </button>
          <button onclick={openMacAccessibilitySettings}
            class="h-6 px-2.5 rounded-md bg-[#FF8A3D] text-[11px] font-semibold text-black hover:bg-[#ff9a56] transition-colors">
            Open Settings
          </button>
        </div>
      </div>
    </div>
  {/if}

  <!-- ── Body ──────────────────────────────────────────── -->
  <div class="flex flex-1 overflow-hidden min-w-0">

    <!-- ── Sidebar ──────────────────────────────────── -->
    <aside class="w-[192px] shrink-0 flex flex-col border-r border-white/[0.09] bg-[#0c0e13]">

      <nav class="py-3 px-2 space-y-px">
        <!-- All History -->
        <div class="relative flex items-center">
          {#if selectedGroup === null}
            <span class="absolute left-0 top-1/2 -translate-y-1/2 w-[2px] h-[18px] bg-[#FF8A3D] rounded-r-full pointer-events-none" aria-hidden="true"></span>
          {/if}
          <button
            class="w-full flex items-center gap-2.5 pl-3 pr-2 py-[7px] rounded-lg text-[13px] transition-all {selectedGroup === null ? 'text-white font-medium bg-white/[0.07]' : 'text-white/52 hover:text-white/80 hover:bg-white/[0.05]'}"
            onclick={() => { selectedGroup = null; }}
          >
            <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="{selectedGroup === null ? 'text-[#FF8A3D]' : 'text-white/38'}">
              <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
            </svg>
            <span class="flex-1 min-w-0 truncate">All History</span>
            {#if sidebarCounts.all > 0}
              <span class="text-[10px] text-white/32 tabular-nums shrink-0">{sidebarCounts.all}</span>
            {/if}
          </button>
        </div>

        <div class="h-px bg-white/[0.07] my-1.5 mx-2"></div>

        <!-- URLs -->
        <div class="relative flex items-center">
          {#if selectedGroup === 'URL'}
            <span class="absolute left-0 top-1/2 -translate-y-1/2 w-[2px] h-[18px] bg-[#AEB291] rounded-r-full pointer-events-none" aria-hidden="true"></span>
          {/if}
          <button
            class="w-full flex items-center gap-2.5 pl-3 pr-2 py-[7px] rounded-lg text-[13px] transition-all {selectedGroup === 'URL' ? 'text-white font-medium bg-white/[0.07]' : 'text-white/52 hover:text-white/80 hover:bg-white/[0.05]'}"
            onclick={() => (selectedGroup = "URL")}
          >
            <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="{selectedGroup === 'URL' ? 'text-[#AEB291]' : 'text-white/38'}">
              <circle cx="12" cy="12" r="10"/><line x1="2" y1="12" x2="22" y2="12"/><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/>
            </svg>
            <span class="flex-1 min-w-0 truncate">URLs</span>
            {#if sidebarCounts.url > 0}
              <span class="text-[10px] text-white/32 tabular-nums shrink-0">{sidebarCounts.url}</span>
            {/if}
          </button>
        </div>

        <!-- Text -->
        <div class="relative flex items-center">
          {#if selectedGroup === 'Text'}
            <span class="absolute left-0 top-1/2 -translate-y-1/2 w-[2px] h-[18px] bg-[#AEB291] rounded-r-full pointer-events-none" aria-hidden="true"></span>
          {/if}
          <button
            class="w-full flex items-center gap-2.5 pl-3 pr-2 py-[7px] rounded-lg text-[13px] transition-all {selectedGroup === 'Text' ? 'text-white font-medium bg-white/[0.07]' : 'text-white/52 hover:text-white/80 hover:bg-white/[0.05]'}"
            onclick={() => (selectedGroup = "Text")}
          >
            <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="{selectedGroup === 'Text' ? 'text-[#AEB291]' : 'text-white/38'}">
              <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="16" y1="13" x2="8" y2="13"/><line x1="16" y1="17" x2="8" y2="17"/>
            </svg>
            <span class="flex-1 min-w-0 truncate">Text</span>
            {#if sidebarCounts.text > 0}
              <span class="text-[10px] text-white/32 tabular-nums shrink-0">{sidebarCounts.text}</span>
            {/if}
          </button>
        </div>
      </nav>

      <!-- Groups section -->
      <div class="flex items-center justify-between px-3 pb-1.5 pt-1">
        <span class="text-[9px] font-semibold uppercase tracking-[0.12em] text-white/38">Groups</span>
        <button onclick={() => (isViewingGroups = true)} class="text-white/38 hover:text-[#AEB291] transition-colors" title="Manage groups">
          <svg xmlns="http://www.w3.org/2000/svg" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
        </button>
      </div>

      <div class="flex-1 overflow-y-auto custom-scrollbar px-2 pb-3 space-y-px">
        {#each groups as group}
          <div class="group/g relative flex items-center">
            {#if selectedGroup === group}
              <span class="absolute left-0 top-1/2 -translate-y-1/2 w-[2px] h-[14px] bg-[#AEB291]/70 rounded-r-full pointer-events-none" aria-hidden="true"></span>
            {/if}
            {#if editingGroup === group}
              <input type="text" bind:value={editGroupName}
                class="flex-1 bg-transparent text-[13px] pl-3 pr-2 py-1.5 focus:outline-none text-white border-b border-[#AEB291]/25"
                onblur={renameGroup}
                onkeydown={(e) => { if (e.key === "Enter") renameGroup(); if (e.key === "Escape") editingGroup = null; }} />
            {:else}
              <button
                class="flex-1 min-w-0 flex items-center pl-3 pr-1 py-1.5 text-[13px] truncate transition-colors {selectedGroup === group ? 'text-white font-medium' : 'text-white/52 hover:text-white/80'}"
                onclick={() => { selectedGroup = group; }}
              >
                <span class="block truncate flex-1">{group}</span>
                {#if sidebarCounts.groups[group] !== undefined && sidebarCounts.groups[group] > 0}
                  <span class="text-[10px] text-white/30 tabular-nums ml-1.5 shrink-0">{sidebarCounts.groups[group]}</span>
                {/if}
              </button>
              <div class="flex opacity-0 group-hover/g:opacity-100 pr-0.5 gap-px transition-opacity">
                <button onclick={() => { editingGroup = group; editGroupName = group; }} class="p-1 text-white/38 hover:text-white/80 rounded transition-colors" title="Rename">
                  <svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
                </button>
                <button onclick={() => exportGroup(group)} class="p-1 text-white/38 hover:text-white/80 rounded transition-colors" title="Export">
                  <svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
                </button>
                <button onclick={() => deleteGroup(group)} class="p-1 text-white/38 hover:text-[#FF8A3D] rounded transition-colors" title="Delete">
                  <svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/></svg>
                </button>
              </div>
            {/if}
          </div>
        {/each}
      </div>
    </aside>

    <!-- ── Main area ──────────────────────────────────── -->
    <main class="flex-1 min-w-0 flex flex-col bg-[#08090c]">

      <!-- Search -->
      <div class="px-3 pt-3 pb-2.5">
        <div class="relative flex items-center gap-2">
          <div class="flex-1 relative">
            <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" class="absolute left-3 top-1/2 -translate-y-1/2 text-white/35 pointer-events-none">
              <circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/>
            </svg>
            <input type="text" bind:this={searchInput} bind:value={searchQuery}
              placeholder={selectedGroup ? `Search in ${selectedGroup}…` : "Search clips…"}
              class="w-full bg-white/[0.05] border border-white/[0.1] rounded-xl pl-9 pr-3 py-2.5 text-[13px] text-white/85 focus:outline-none focus:bg-white/[0.07] focus:border-white/[0.18] transition-all placeholder:text-white/30" />
          </div>
          {#if selectedGroup}
            <div class="flex items-center gap-1.5 shrink-0 rounded-full bg-white/[0.06] border border-white/[0.08] px-2.5 py-1">
              <span class="text-[11px] font-medium text-[#AEB291]">{selectedGroup}</span>
              <button onclick={() => (selectedGroup = null)} aria-label="Clear filter" class="text-white/25 hover:text-white/60 transition-colors leading-none">
                <svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
              </button>
            </div>
          {/if}
        </div>
      </div>

      <!-- ── Item list with sections ─────────────────── -->
      <div class="flex-1 overflow-y-auto custom-scrollbar px-3 pb-3" bind:this={container}>

        {#if displayHistory.length === 0}
          <!-- ── Empty state / onboarding ── -->
          <div class="flex flex-col items-center justify-center h-full py-16 text-center max-w-xs mx-auto">
            <div class="w-16 h-16 rounded-2xl bg-white/[0.03] border border-white/[0.06] flex items-center justify-center mb-5 text-white/15">
              <svg xmlns="http://www.w3.org/2000/svg" width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
              </svg>
            </div>
            {#if searchQuery}
              <p class="text-[14px] font-medium text-white/30">No results for "{searchQuery}"</p>
              <p class="text-[12px] text-white/18 mt-1.5">Try a different search term</p>
            {:else if selectedGroup}
              <p class="text-[14px] font-medium text-white/30">Nothing in "{selectedGroup}"</p>
              <p class="text-[12px] text-white/18 mt-1.5">Copy something, then assign it to this group</p>
            {:else}
              <p class="text-[14px] font-medium text-white/35">Your clipboard is empty</p>
              <p class="text-[12px] text-white/20 mt-2 leading-relaxed">
                Start copying text anywhere and it'll appear here automatically.
              </p>
              <div class="mt-5 space-y-2 w-full text-left">
                <div class="flex items-center gap-3 p-3 bg-white/[0.03] rounded-xl border border-white/[0.05]">
                  <kbd class="px-2 py-1 bg-black/40 rounded-lg border border-white/[0.08] text-[10px] text-white/35 font-mono shrink-0">{altKey}+V</kbd>
                  <span class="text-[11px] text-white/30">Open quick popup anywhere</span>
                </div>
                <div class="flex items-center gap-3 p-3 bg-white/[0.03] rounded-xl border border-white/[0.05]">
                  <kbd class="px-2 py-1 bg-black/40 rounded-lg border border-white/[0.08] text-[10px] text-white/35 font-mono shrink-0">{modKey}+1-9</kbd>
                  <span class="text-[11px] text-white/30">Instantly copy by position</span>
                </div>
                <div class="flex items-center gap-3 p-3 bg-white/[0.03] rounded-xl border border-white/[0.05]">
                  <div class="w-[26px] h-[18px] bg-[#FF8A3D] rounded flex items-center justify-center shrink-0">
                    <svg xmlns="http://www.w3.org/2000/svg" width="9" height="9" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
                  </div>
                  <span class="text-[11px] text-white/30">Add items manually with Add</span>
                </div>
              </div>
            {/if}
          </div>

        {:else}

          <!-- ── PINNED section ── -->
          {#if groupedHistory.pinned.length > 0}
            <div class="section-header text-amber-400/50">
              <svg xmlns="http://www.w3.org/2000/svg" width="9" height="9" viewBox="0 0 24 24" fill="currentColor" stroke="none"><path d="M5 17h14v-1.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V6a3 3 0 0 0-3-3 3 3 0 0 0-3 3v4.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24Z"/></svg>
              Pinned
            </div>
            <div class="space-y-1.5 mb-4">
              {#each groupedHistory.pinned as {item, index}}
                {@const urlInfo = parseUrl(item.raw_content)}
                {@const codeLang = !urlInfo ? detectCodeLang(item.raw_content) : null}
                {@const isFlashing = flashingItemId === item.id}
                {@const isSelected = selectedIndex === index}
                <div
                  class="group/card relative rounded-xl border transition-all duration-150 cursor-pointer select-none overflow-hidden
                    {isFlashing ? 'bg-green-500/[0.07] border-green-500/[0.3] shadow-lg shadow-green-500/[0.08]' :
                     isSelected ? 'bg-[#FF8A3D]/[0.07] border-[#FF8A3D]/[0.22] shadow-md shadow-[#FF8A3D]/[0.08]' :
                                  'bg-amber-400/[0.03] border-amber-400/[0.12] hover:bg-amber-400/[0.05] hover:border-amber-400/[0.18] hover:shadow-lg hover:shadow-black/[0.25] hover:-translate-y-px'}
                    active:scale-[0.995] active:translate-y-0"
                  onclick={() => copyItem(item, index)}
                  onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); copyItem(item, index); } }}
                  role="button" tabindex="0" data-index={index}
                >
                  <!-- Pinned left bar (amber, always visible) -->
                  <div class="absolute left-0 top-0 bottom-0 w-[3px] bg-amber-400/{isSelected ? '70' : '45'} rounded-r-full" aria-hidden="true"></div>
                  {#if isSelected}
                    <div class="absolute left-0 top-0 bottom-0 w-[3px] bg-gradient-to-b from-[#FF8A3D] to-[#ff6b1a] rounded-r-full" aria-hidden="true"></div>
                  {/if}

                  <div class="pl-4 pr-3 pt-3 pb-2.5">
                    {@render cardContent(item, urlInfo, codeLang, isSelected)}
                    {@render cardMeta(item, isSelected, index)}
                  </div>
                </div>
              {/each}
            </div>
          {/if}

          <!-- ── Time sections ── -->
          {#each ([
            { key: "today",     label: "Today",     items: groupedHistory.today,     headerColor: "text-white/50" },
            { key: "yesterday", label: "Yesterday", items: groupedHistory.yesterday, headerColor: "text-white/40" },
            { key: "thisWeek",  label: "This Week",  items: groupedHistory.thisWeek,  headerColor: "text-white/32" },
            { key: "older",     label: "Earlier",   items: groupedHistory.older,     headerColor: "text-white/26" },
          ]) as section}
            {#if section.items.length > 0}
              <div class="section-header {section.headerColor}">
                {section.label}
              </div>
              <div class="space-y-1.5 mb-4">
                {#each section.items as {item, index}}
                  {@const urlInfo = parseUrl(item.raw_content)}
                  {@const codeLang = !urlInfo ? detectCodeLang(item.raw_content) : null}
                  {@const isFlashing = flashingItemId === item.id}
                  {@const isSelected = selectedIndex === index}
                  <div
                    class="group/card relative rounded-xl border transition-all duration-150 cursor-pointer select-none overflow-hidden
                      {isFlashing ? 'bg-green-500/[0.07] border-green-500/[0.3] shadow-lg shadow-green-500/[0.08]' :
                       isSelected ? 'bg-[#FF8A3D]/[0.08] border-[#FF8A3D]/[0.28] shadow-md shadow-[#FF8A3D]/[0.1]' :
                                    'bg-white/[0.04] border-white/[0.1] hover:bg-white/[0.06] hover:border-white/[0.16] hover:shadow-lg hover:shadow-black/[0.3] hover:-translate-y-px'}
                      active:scale-[0.995] active:translate-y-0"
                    onclick={() => copyItem(item, index)}
                    onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); copyItem(item, index); } }}
                    role="button" tabindex="0" data-index={index}
                  >
                    <!-- Content-type left accent (non-selected) -->
                    {#if isSelected}
                      <div class="absolute left-0 top-0 bottom-0 w-[3px] bg-gradient-to-b from-[#FF8A3D] to-[#ff6b1a] rounded-r-full" aria-hidden="true"></div>
                    {:else if urlInfo}
                      <div class="absolute left-0 top-0 bottom-0 w-[2px] bg-sky-400/30 rounded-r-full" aria-hidden="true"></div>
                    {:else if codeLang}
                      <div class="absolute left-0 top-0 bottom-0 w-[2px] bg-violet-400/30 rounded-r-full" aria-hidden="true"></div>
                    {:else if item.is_manual}
                      <div class="absolute left-0 top-0 bottom-0 w-[2px] bg-[#FF8A3D]/25 rounded-r-full" aria-hidden="true"></div>
                    {/if}

                    <div class="pl-4 pr-3 pt-3 pb-2.5">
                      {@render cardContent(item, urlInfo, codeLang, isSelected)}
                      {@render cardMeta(item, isSelected, index)}
                    </div>
                  </div>
                {/each}
              </div>
            {/if}
          {/each}
        {/if}
      </div>
    </main>
  </div><!-- end body -->

</div><!-- end root -->

<!-- ══════════════════════════════════════════════
     SNIPPETS (reusable card sections)
══════════════════════════════════════════════ -->

{#snippet cardContent(item: ClipboardItem, urlInfo: {domain:string;path:string}|null, codeLang: string|null, isSelected: boolean)}
  <div class="flex items-start gap-2 min-w-0">
    <div class="min-w-0 flex-1">

      {#if item.description}
        <!-- content is primary; description is a small secondary label -->
        <p class="text-[13px] text-white/80 {expandedItems.includes(item.id) ? '' : 'line-clamp-2'} leading-relaxed break-words mb-1">{item.raw_content}</p>
        <p class="text-[10px] text-white/30 truncate">{item.description}</p>

      {:else if urlInfo}
        <!-- URL item: domain = small context label, path = primary content -->
        <div class="flex items-center gap-1.5 mb-1.5">
          <svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="text-[#AEB291]/62 shrink-0">
            <circle cx="12" cy="12" r="10"/><line x1="2" y1="12" x2="22" y2="12"/><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/>
          </svg>
          <span class="text-[10px] font-semibold text-[#AEB291]/78 uppercase tracking-wider">{urlInfo.domain}</span>
        </div>
        <p class="text-[13px] text-white/75 {urlInfo.path ? 'break-all line-clamp-2' : 'font-medium truncate'} leading-relaxed">
          {urlInfo.path || urlInfo.domain}
        </p>

      {:else if codeLang}
        <!-- Code item: language badge + monospace content -->
        <div class="flex items-center gap-2 mb-1.5">
          <span class="text-[9px] font-bold uppercase tracking-widest px-1.5 py-0.5 rounded bg-[#AEB291]/[0.12] text-[#AEB291]/80 border border-[#AEB291]/[0.22]">{codeLang}</span>
        </div>
        <p class="text-[12px] text-white/60 font-mono leading-relaxed break-all {expandedItems.includes(item.id) ? '' : 'line-clamp-3'} bg-white/[0.02] rounded-lg px-2.5 py-2">
          {clipPreview(item.raw_content, item.content_type)}
        </p>

      {:else}
        <!-- Regular text -->
        <p class="text-[13px] text-white/70 leading-relaxed break-words whitespace-pre-wrap {expandedItems.includes(item.id) ? '' : 'line-clamp-3'}">
          {clipPreview(item.raw_content, item.content_type)}
        </p>
      {/if}

      {#if item.raw_content.split('\n').length > 3 || item.raw_content.length > 250}
        <button
          onclick={(e) => {
            e.stopPropagation();
            expandedItems = expandedItems.includes(item.id)
              ? expandedItems.filter(id => id !== item.id)
              : [...expandedItems, item.id];
          }}
          class="text-[10px] text-white/20 hover:text-white/45 mt-1 transition-colors"
        >{expandedItems.includes(item.id) ? 'Show less' : 'Show more'}</button>
      {/if}
    </div>

    <!-- Action trio (hover / selected) -->
    <div class="flex items-center gap-0.5 shrink-0 self-start transition-opacity {isSelected ? 'opacity-100' : 'opacity-0 group-hover/card:opacity-100'}">
      <button class="p-1.5 rounded-lg transition-all hover:bg-white/[0.07] {item.is_permanent ? 'text-amber-400' : 'text-white/40 hover:text-white/75'}"
        onclick={(e) => { e.stopPropagation(); togglePermanent(item); }} title="Pin">
        <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill={item.is_permanent ? "currentColor" : "none"} stroke="currentColor" stroke-width="2.5" stroke-linecap="round">
          <line x1="12" y1="17" x2="12" y2="22"/><path d="M5 17h14v-1.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V6a3 3 0 0 0-3-3 3 3 0 0 0-3 3v4.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24Z"/>
        </svg>
      </button>
      <button class="p-1.5 rounded-lg text-white/40 hover:text-[#AEB291] hover:bg-white/[0.07] transition-all"
        onclick={(e) => { e.stopPropagation(); openEditModal(item); }} title="Edit">
        <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/>
        </svg>
      </button>
      <button class="p-1.5 rounded-lg text-white/40 hover:text-white/75 hover:bg-white/[0.07] transition-all"
        onclick={(e) => { e.stopPropagation(); categorizingItemId = item.id; newGroupName = ""; isCategorizing = true; }} title="Add to group">
        <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/><line x1="12" y1="11" x2="12" y2="17"/><line x1="9" y1="14" x2="15" y2="14"/>
        </svg>
      </button>
      <button class="p-1.5 rounded-lg text-white/40 hover:text-[#FF8A3D] hover:bg-[#FF8A3D]/[0.1] transition-all"
        onclick={(e) => { e.stopPropagation(); deleteItem(item); }} title="Delete">
        <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/>
        </svg>
      </button>
    </div>
  </div>
{/snippet}

{#snippet cardMeta(item: ClipboardItem, isSelected: boolean, _index: number)}
  <div class="flex items-center justify-between mt-2 pt-2 border-t border-white/[0.07]">
    <div class="flex items-center gap-1.5 flex-wrap min-w-0">
      <!-- Category icon (only when no other signal) -->
      {#if !item.groups?.length && !item.category && !item.is_manual}
        <svg xmlns="http://www.w3.org/2000/svg" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" class="text-white/18 shrink-0">
          {@html getCategoryIcon(item.category)}
        </svg>
      {/if}
      <!-- Group / category pills -->
      {#if item.groups && item.groups.length > 0}
        {#each item.groups as grp}
          <div class="inline-flex items-center rounded-full bg-white/[0.07] border border-white/[0.12] hover:border-[#AEB291]/35 transition-colors overflow-hidden">
            <button
              class="text-[9px] font-semibold uppercase tracking-wide py-0.5 pl-2 {selectedGroup === grp ? 'pr-1' : 'pr-2'} text-white/48 hover:text-[#AEB291] transition-colors"
              onclick={(e) => { e.stopPropagation(); selectedGroup = grp; }}
            >{grp}</button>
            {#if selectedGroup === grp}
              <button
                onclick={(e) => { e.stopPropagation(); removeFromGroup(item, grp); }}
                class="pr-1.5 text-[11px] leading-none text-white/35 hover:text-[#FF8A3D] transition-colors"
                title="Remove from group">×</button>
            {/if}
          </div>
        {/each}
      {:else if item.category}
        <button
          class="text-[9px] font-semibold uppercase tracking-wide py-0.5 px-2 rounded-full bg-white/[0.07] text-white/48 border border-white/[0.12] hover:text-[#AEB291] hover:border-[#AEB291]/35 transition-colors"
          onclick={(e) => { e.stopPropagation(); selectedGroup = item.category; }}
        >{item.category}</button>
      {/if}
      {#if item.is_manual}
        <span class="text-[9px] font-semibold uppercase tracking-wide py-0.5 px-2 rounded-full bg-[#FF8A3D]/[0.1] text-[#FF8A3D]/68 border border-[#FF8A3D]/[0.2]">manual</span>
      {/if}
      <!-- Relative time -->
      <span class="text-[10px] text-white/32">{relativeTime(item.created_at)}</span>
    </div>
    <!-- Copy icon (appears on hover / selected) -->
    <div class="shrink-0 ml-2 transition-all {isSelected ? 'opacity-100 text-[#FF8A3D]/70' : 'opacity-0 group-hover/card:opacity-100 text-white/35 group-hover/card:text-white/58'}">
      <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
      </svg>
    </div>
  </div>
{/snippet}


<!-- ══════════════════════════════════════════════
     MODALS
══════════════════════════════════════════════ -->

<!-- Export -->
{#if showExportModal}
  <div class="modal-backdrop">
    <div class="modal-box w-full max-w-sm">
      <h3 class="modal-title">Export Data</h3>
      <div class="max-h-52 overflow-y-auto custom-scrollbar mb-4 border border-white/[0.06] rounded-xl p-1.5 space-y-px">
        <label class="modal-check-row"><input type="checkbox" checked={exportSelectedGroups.length === 0} onchange={() => (exportSelectedGroups = [])} class="accent-[#FF8A3D]" /><span class="font-medium">All Data</span></label>
        <div class="h-px bg-white/[0.05]"></div>
        {#each groups as group}
          <label class="modal-check-row">
            <input type="checkbox" checked={exportSelectedGroups.includes(group)}
              onchange={(e) => { if (e.currentTarget.checked) exportSelectedGroups = [...exportSelectedGroups, group]; else exportSelectedGroups = exportSelectedGroups.filter(g => g !== group); }}
              class="accent-[#FF8A3D]" />
            <span>{group}</span>
          </label>
        {/each}
      </div>
      <div class="modal-footer">
        <button class="btn-ghost" onclick={() => (showExportModal = false)}>Cancel</button>
        <button class="btn-primary" onclick={performExport} disabled={processingIO}>{processingIO ? "Exporting…" : "Export"}</button>
      </div>
    </div>
  </div>
{/if}

<!-- Import -->
{#if showImportModal}
  <div class="modal-backdrop">
    <div class="modal-box w-full max-w-sm">
      <h3 class="modal-title">Import Data</h3>
      <div class="space-y-2 mb-5">
        {#each ([["merge","Merge","Combine with existing data"], ["replace","Replace","Overwrite all existing data"]] as const) as [val, label, desc]}
          <label class="flex items-center gap-3 p-3 border rounded-xl cursor-pointer transition-colors {importMode === val ? 'border-white/[0.12] bg-white/[0.04]' : 'border-white/[0.05] hover:bg-white/[0.03]'}">
            <input type="radio" name="importMode" value={val} bind:group={importMode} class="accent-[#FF8A3D]" />
            <div>
              <div class="text-[12px] font-semibold text-white/70">{label}</div>
              <div class="text-[10px] text-white/30 mt-0.5">{desc}</div>
            </div>
          </label>
        {/each}
      </div>
      <div class="modal-footer">
        <button class="btn-ghost" onclick={() => (showImportModal = false)}>Cancel</button>
        <button class="btn-primary" onclick={performImport} disabled={processingIO}>{processingIO ? "Importing…" : "Select File"}</button>
      </div>
    </div>
  </div>
{/if}

<!-- Manage Groups -->
{#if isViewingGroups}
  <div class="modal-backdrop">
    <div class="modal-box w-full max-w-lg flex flex-col max-h-[80vh]">
      <div class="px-5 py-4 border-b border-white/[0.06] flex justify-between items-center shrink-0">
        <div>
          <h2 class="modal-title mb-0">Manage Groups</h2>
          <p class="text-[11px] text-white/30 mt-0.5">Create and organize your clips</p>
        </div>
        <button onclick={() => (isViewingGroups = false)} class="text-white/25 hover:text-white/70 transition-colors" aria-label="Close"><svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg></button>
      </div>
      <div class="p-5 border-b border-white/[0.05] shrink-0">
        <div class="flex items-center gap-2">
          <input type="text" bind:value={newGroupName} placeholder="New group name…" class="modal-input flex-1" onkeydown={(e) => { if (e.key === "Enter") createGroup(); }} />
          <button onclick={createGroup} class="h-9 px-4 bg-[#AEB291]/80 hover:bg-[#AEB291] text-black text-[12px] font-semibold rounded-lg transition-colors">Create</button>
        </div>
      </div>
      <div class="flex-1 overflow-y-auto custom-scrollbar p-4 space-y-1.5">
        {#each groups as group}
          <div class="flex items-center justify-between p-3 bg-white/[0.03] rounded-xl border border-white/[0.05] hover:border-white/[0.09] transition-all">
            <div class="flex items-center gap-3 flex-1 min-w-0">
              <div class="w-8 h-8 rounded-lg bg-white/[0.05] flex items-center justify-center text-[#FF8A3D]/40 shrink-0">
                <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
              </div>
              {#if editingGroup === group}
                <input type="text" bind:value={editGroupName} class="flex-1 bg-white/[0.04] text-[13px] px-2.5 py-1.5 rounded-lg focus:outline-none text-white/70 border border-white/[0.1]"
                  onblur={renameGroup} onkeydown={(e) => { if (e.key === "Enter") renameGroup(); if (e.key === "Escape") editingGroup = null; }} />
              {:else}
                <span class="text-[13px] font-medium text-white/65 truncate">{group}</span>
              {/if}
            </div>
            <div class="flex items-center gap-1">
              <button onclick={() => { editingGroup = group; editGroupName = group; }} class="p-1.5 text-white/25 hover:text-white/70 hover:bg-white/[0.05] rounded-lg transition-all" title="Rename"><svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg></button>
              <button onclick={() => exportGroup(group)} class="p-1.5 text-white/25 hover:text-white/70 hover:bg-white/[0.05] rounded-lg transition-all" title="Export"><svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg></button>
              <button onclick={() => deleteGroup(group)} class="p-1.5 text-white/25 hover:text-[#FF8A3D] hover:bg-[#FF8A3D]/[0.08] rounded-lg transition-all" title="Delete"><svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/></svg></button>
            </div>
          </div>
        {:else}
          <div class="flex flex-col items-center justify-center py-10 text-center">
            <p class="text-[13px] font-medium text-white/25">No groups yet</p>
            <p class="text-[11px] text-white/15 mt-1">Create your first group above</p>
          </div>
        {/each}
      </div>
    </div>
  </div>
{/if}

<!-- Save to Group -->
{#if isCategorizing}
  <div class="modal-backdrop">
    <div class="modal-box w-full max-w-[260px] flex flex-col max-h-[70vh]">
      <div class="px-4 py-3 border-b border-white/[0.06] flex justify-between items-center shrink-0">
        <span class="text-[11px] font-semibold text-white/40 uppercase tracking-widest">Save to Group</span>
        <button onclick={() => (isCategorizing = false)} aria-label="Close" class="text-white/25 hover:text-white/70 transition-colors"><svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg></button>
      </div>
      <div class="p-2 border-b border-white/[0.05] shrink-0">
        <input type="text" bind:value={newGroupName} placeholder="Type or search…" class="modal-input w-full"
          onkeydown={(e) => { if (e.key === "Enter") moveItemToGroup(); if (e.key === "Escape") isCategorizing = false; }} />
      </div>
      <div class="flex-1 overflow-y-auto custom-scrollbar p-1.5 space-y-px">
        {#each groups.filter(g => g.toLowerCase().includes(newGroupName.toLowerCase())) as group}
          <button onclick={() => { newGroupName = group; moveItemToGroup(); }} class="w-full text-left px-3 py-2 text-[12px] hover:bg-white/[0.05] rounded-lg transition-colors flex items-center gap-2 text-white/40 hover:text-white/80">
            <svg xmlns="http://www.w3.org/2000/svg" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="text-[#FF8A3D]/40 shrink-0"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
            {group}
          </button>
        {/each}
        {#if newGroupName && !groups.includes(newGroupName)}
          <button onclick={moveItemToGroup} class="w-full text-left px-3 py-2 text-[12px] hover:bg-[#FF8A3D]/[0.07] rounded-lg transition-colors flex items-center gap-2 text-[#FF8A3D]/70">
            <svg xmlns="http://www.w3.org/2000/svg" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
            Create "{newGroupName}"
          </button>
        {/if}
      </div>
      <div class="p-2 border-t border-white/[0.05] flex justify-end shrink-0">
        <button onclick={moveItemToGroup} disabled={!newGroupName.trim()} class="h-7 px-4 bg-[#AEB291]/80 hover:bg-[#AEB291] text-black rounded-lg text-[11px] font-semibold transition-colors disabled:opacity-30">Save</button>
      </div>
    </div>
  </div>
{/if}

<!-- Help -->
{#if showHelpModal}
  <div class="modal-backdrop" onclick={(e) => { if (e.target === e.currentTarget) showHelpModal = false; }} onkeydown={(e) => { if (e.key === "Escape") showHelpModal = false; }} role="dialog" aria-modal="true" tabindex="-1">
    <div class="modal-box w-full max-w-xl">
      <div class="px-5 py-4 border-b border-white/[0.06] flex items-center justify-between">
        <h3 class="modal-title mb-0">Help & Shortcuts</h3>
        <button onclick={() => (showHelpModal = false)} class="text-white/25 hover:text-white/70 transition-colors" aria-label="Close"><svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg></button>
      </div>
      <div class="p-5 max-h-[65vh] overflow-y-auto custom-scrollbar space-y-5">
        <div>
          <h4 class="section-label mb-3">Opening</h4>
          <div class="space-y-1.5">
            <div class="flex items-start gap-3 p-3 bg-white/[0.03] rounded-xl">
              <code class="text-[11px] text-white/30 shrink-0 mt-0.5">{altKey}+V</code>
              <div><div class="text-[12px] font-medium text-white/65">Quick Popup</div><div class="text-[11px] text-white/30 mt-0.5">Open quick access popup anywhere</div></div>
            </div>
          </div>
        </div>
        <div>
          <h4 class="section-label mb-2.5">Shortcuts</h4>
          <div class="space-y-px">
            {#each [
              ["Copy item", "Click or Enter"],
              [`Quick copy 1–9`, `${modKey}+1–9`],
              ["Navigate", "↑ / ↓"],
              ["Delete item", `Del / ${modKey}+${deleteKey}`],
              ["Pin / Unpin", `${modKey}+P`],
              ["Add to group", `${modKey}+C`],
              ["Manage groups", `${modKey}+G`],
              ["Close window", "Esc"],
            ] as [label, key]}
              <div class="flex items-center justify-between px-2 py-[7px] hover:bg-white/[0.03] rounded-lg">
                <span class="text-[12px] text-white/45">{label}</span>
                <kbd class="px-2 py-0.5 bg-black/40 rounded border border-white/[0.07] text-[10px] text-white/30 font-mono">{key}</kbd>
              </div>
            {/each}
          </div>
        </div>
      </div>
      <div class="px-5 py-3 border-t border-white/[0.05] flex justify-end">
        <button onclick={() => (showHelpModal = false)} class="btn-primary">Got it</button>
      </div>
    </div>
  </div>
{/if}

<!-- About -->
{#if showAboutModal}
  <div class="modal-backdrop" onclick={(e) => { if (e.target === e.currentTarget) showAboutModal = false; }} onkeydown={(e) => { if (e.key === "Escape") showAboutModal = false; }} role="dialog" aria-modal="true" tabindex="-1">
    <div class="modal-box w-full max-w-sm">
      <div class="px-5 py-4 border-b border-white/[0.06] flex items-center justify-between">
        <div class="flex items-center gap-3">
          <img src="/logo.png" alt="Ortu" class="w-9 h-9 rounded-xl" />
          <div><h3 class="text-[14px] font-semibold text-white/80">Ortu</h3><p class="text-[11px] text-white/30">Clipboard Manager</p></div>
        </div>
        <button onclick={() => (showAboutModal = false)} class="text-white/25 hover:text-white/70 transition-colors" aria-label="Close"><svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg></button>
      </div>
      <div class="p-5 space-y-4">
        <div class="text-center">
          <div class="text-[28px] font-bold text-white/70 tracking-tight">v{appVersion}</div>
          <p class="text-[11px] text-white/30 mt-1">Privacy-focused clipboard manager</p>
        </div>
        <div class="p-3 bg-white/[0.03] rounded-xl border border-white/[0.05]">
          <div class="text-[9px] font-semibold uppercase tracking-[0.1em] text-white/20 mb-1.5">Developer</div>
          <div class="text-[13px] font-medium text-white/60">Abhijith P Subash</div>
        </div>
        <a href="https://www.linkedin.com/in/abhijith-p-subash-the-engineer/" target="_blank" rel="noopener noreferrer"
          class="flex items-center justify-between p-3 bg-white/[0.03] hover:bg-white/[0.05] rounded-xl border border-white/[0.05] transition-colors group">
          <div class="flex items-center gap-2.5">
            <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="currentColor" class="text-[#0A66C2] shrink-0"><path d="M20.447 20.452h-3.554v-5.569c0-1.328-.027-3.037-1.852-3.037-1.853 0-2.136 1.445-2.136 2.939v5.667H9.351V9h3.414v1.561h.046c.477-.9 1.637-1.85 3.37-1.85 3.601 0 4.267 2.37 4.267 5.455v6.286zM5.337 7.433c-1.144 0-2.063-.926-2.063-2.065 0-1.138.92-2.063 2.063-2.063 1.14 0 2.064.925 2.064 2.063 0 1.139-.925 2.065-2.064 2.065zm1.782 13.019H3.555V9h3.564v11.452zM22.225 0H1.771C.792 0 0 .774 0 1.729v20.542C0 23.227.792 24 1.771 24h20.451C23.2 24 24 23.227 24 22.271V1.729C24 .774 23.2 0 22.222 0h.003z"/></svg>
            <div><div class="text-[12px] font-medium text-white/55">LinkedIn</div><div class="text-[10px] text-white/25">@abhijith-p-subash-the-engineer</div></div>
          </div>
          <svg xmlns="http://www.w3.org/2000/svg" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="text-white/20 group-hover:text-white/40 transition-colors"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>
        </a>
        <div class="p-3 bg-[#FF8A3D]/[0.05] rounded-xl border border-[#FF8A3D]/[0.09]">
          <div class="text-[9px] font-semibold uppercase tracking-[0.1em] text-[#FF8A3D]/40 mb-1.5">Privacy First</div>
          <p class="text-[11px] text-white/35">All data stored locally. No cloud, no tracking.</p>
        </div>
        <p class="text-center text-[10px] text-white/15">© 2025 Ortu. All rights reserved.</p>
      </div>
      <div class="px-5 py-3 border-t border-white/[0.05] flex justify-end">
        <button onclick={() => (showAboutModal = false)} class="btn-primary">Close</button>
      </div>
    </div>
  </div>
{/if}

<!-- Add Item -->
{#if showAddItemModal}
  <div class="modal-backdrop" onclick={(e) => { if (e.target === e.currentTarget) showAddItemModal = false; }} onkeydown={(e) => { if (e.key === "Escape") showAddItemModal = false; }} role="dialog" aria-modal="true" tabindex="-1">
    <div class="modal-box w-full max-w-md">
      <div class="px-5 py-4 border-b border-white/[0.06] flex items-center justify-between">
        <div><h3 class="modal-title mb-0">Add Item</h3><p class="text-[11px] text-white/30 mt-0.5">Manually add text to your library</p></div>
        <button onclick={() => (showAddItemModal = false)} class="text-white/25 hover:text-white/70 transition-colors" aria-label="Close"><svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg></button>
      </div>
      <div class="p-5 space-y-3.5">
        <div>
          <label for="item-content" class="modal-field-label">Content <span class="text-[#FF8A3D]">*</span></label>
          <textarea id="item-content" bind:value={newItemContent} placeholder="Paste or type content here…" rows="5"
            class="modal-input w-full resize-none"
            onkeydown={(e) => { if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) addManualItem(); }}></textarea>
        </div>
        <div>
          <div class="flex items-center justify-between mb-1">
            <label for="item-desc" class="modal-field-label mb-0">Description <span class="normal-case tracking-normal font-normal text-white/20">(optional)</span></label>
            <span class="text-[9px] tabular-nums {newItemDescription.length > 70 ? 'text-[#FF8A3D]/70' : 'text-white/20'}">{newItemDescription.length}/80</span>
          </div>
          <input id="item-desc" type="text" bind:value={newItemDescription} placeholder="Short label, e.g. AWS prod key…" class="modal-input w-full" maxlength="80" />
        </div>
        <div>
          <label for="item-group" class="modal-field-label">Group <span class="normal-case tracking-normal font-normal text-white/20">(optional)</span></label>
          <input id="item-group" type="text" bind:value={newItemGroupInput} placeholder="Group name or leave empty" list="groups-datalist" class="modal-input w-full"
            onkeydown={(e) => { if (e.key === "Enter") addManualItem(); }} />
          <datalist id="groups-datalist">{#each groups as g}<option value={g}></option>{/each}</datalist>
        </div>
      </div>
      <div class="px-5 pb-5 flex items-center justify-between">
        <span class="text-[10px] text-white/20">{modKey}+↵ to save</span>
        <div class="flex gap-2">
          <button onclick={() => (showAddItemModal = false)} class="btn-ghost">Cancel</button>
          <button onclick={addManualItem} disabled={!newItemContent.trim()} class="btn-primary disabled:opacity-35 disabled:cursor-not-allowed">Add Item</button>
        </div>
      </div>
    </div>
  </div>
{/if}

<!-- Edit Item -->
{#if showEditModal && editingItem}
  <div class="modal-backdrop"
    onclick={(e) => { if (e.target === e.currentTarget) showEditModal = false; }}
    onkeydown={(e) => { if (e.key === "Escape") showEditModal = false; }}
    role="dialog" aria-modal="true" tabindex="-1">
    <div class="modal-box w-full max-w-md">
      <div class="px-5 py-4 border-b border-white/[0.06] flex items-center justify-between">
        <div>
          <h3 class="modal-title mb-0">Edit Item</h3>
          <p class="text-[11px] text-white/30 mt-0.5">Changes apply immediately on save</p>
        </div>
        <button onclick={() => (showEditModal = false)} class="text-white/25 hover:text-white/70 transition-colors" aria-label="Close">
          <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
        </button>
      </div>
      <div class="p-5 space-y-3.5">
        <!-- Content — primary field, large -->
        <div>
          <label for="edit-content" class="modal-field-label">Content <span class="text-[#FF8A3D]">*</span></label>
          <textarea id="edit-content" bind:value={editContent} placeholder="Content…" rows="6"
            class="modal-input w-full resize-none"
            onkeydown={(e) => { if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) saveEditItem(); }}></textarea>
        </div>
        <!-- Description — secondary field, small cap -->
        <div>
          <div class="flex items-center justify-between mb-1">
            <label for="edit-desc" class="modal-field-label mb-0">Description <span class="normal-case tracking-normal font-normal text-white/20">(optional)</span></label>
            <span class="text-[9px] tabular-nums {editDescription.length > 70 ? 'text-[#FF8A3D]/70' : 'text-white/20'}">{editDescription.length}/80</span>
          </div>
          <input id="edit-desc" type="text" bind:value={editDescription} placeholder="Short label, e.g. AWS prod key…"
            class="modal-input w-full" maxlength="80"
            onkeydown={(e) => { if (e.key === "Enter") saveEditItem(); }} />
        </div>
      </div>
      <div class="px-5 pb-5 flex items-center justify-between">
        <span class="text-[10px] text-white/20">{modKey}+↵ to save</span>
        <div class="flex gap-2">
          <button onclick={() => (showEditModal = false)} class="btn-ghost">Cancel</button>
          <button onclick={saveEditItem} disabled={!editContent.trim()} class="btn-primary disabled:opacity-35 disabled:cursor-not-allowed">Save</button>
        </div>
      </div>
    </div>
  </div>
{/if}

<!-- Confirm -->
{#if confirmModal}
  <div class="modal-backdrop" onclick={(e) => { if (e.target === e.currentTarget) confirmModal = null; }} onkeydown={(e) => { if (e.key === "Escape") confirmModal = null; }} role="dialog" aria-modal="true" tabindex="-1">
    <div class="modal-box w-full max-w-xs">
      <p class="text-[13px] text-white/55 leading-relaxed mb-5">{confirmModal.message}</p>
      <div class="flex justify-end gap-2">
        <button onclick={() => (confirmModal = null)} class="btn-ghost">Cancel</button>
        <button onclick={() => { const cb = confirmModal?.onConfirm; confirmModal = null; cb?.(); }} class="btn-primary">Confirm</button>
      </div>
    </div>
  </div>
{/if}

<!-- Import Group Name -->
{#if showImportGroupModal}
  <div class="modal-backdrop" role="dialog" aria-modal="true" tabindex="-1">
    <div class="modal-box w-full max-w-xs">
      <h3 class="modal-title">Name imported group</h3>
      <input type="text" bind:value={importGroupNameInput} placeholder="Group name…" class="modal-input w-full mb-3"
        onkeydown={(e) => { if (e.key === "Enter") confirmImportGroup(); if (e.key === "Escape") showImportGroupModal = false; }} />
      <div class="flex justify-end gap-2">
        <button onclick={() => (showImportGroupModal = false)} class="btn-ghost">Cancel</button>
        <button onclick={confirmImportGroup} disabled={!importGroupNameInput.trim()}
          class="h-8 px-4 bg-[#AEB291]/80 hover:bg-[#AEB291] text-black rounded-lg text-[12px] font-semibold transition-colors disabled:opacity-35">Import</button>
      </div>
    </div>
  </div>
{/if}

<!-- ══════════════════════════════════════════════
     TOASTS
══════════════════════════════════════════════ -->

{#if showCopiedToast}
  <div class="fixed bottom-5 right-5 bg-[#13151b] border border-green-500/[0.2] rounded-xl shadow-xl shadow-black/40 px-3.5 py-2 flex items-center gap-2 z-50">
    <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" class="text-green-500/80"><polyline points="20 6 9 17 4 12"/></svg>
    <span class="text-[12px] font-medium text-white/60">Copied</span>
  </div>
{/if}

<div class="fixed bottom-5 left-5 z-50 flex flex-col gap-1.5 pointer-events-none">
  {#each toasts as toast (toast.id)}
    <div class="pointer-events-auto px-3.5 py-2 rounded-xl border shadow-xl shadow-black/40 flex items-center gap-2.5
      {toast.type === 'success' ? 'bg-[#13151b] border-green-500/[0.18]' :
       toast.type === 'error'   ? 'bg-[#13151b] border-red-500/[0.18]'   :
                                  'bg-[#13151b] border-white/[0.07]'}">
      {#if toast.type === 'success'}
        <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" class="text-green-500/70 shrink-0"><polyline points="20 6 9 17 4 12"/></svg>
      {:else if toast.type === 'error'}
        <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" class="text-red-400/70 shrink-0"><circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/></svg>
      {:else}
        <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" class="text-[#AEB291]/60 shrink-0"><circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/></svg>
      {/if}
      <span class="text-[12px] text-white/55">{toast.message}</span>
    </div>
  {/each}
</div>

<style>
  :global(body) { margin: 0; overflow: hidden; background: transparent; }
</style>
