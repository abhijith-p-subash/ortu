<script lang="ts">
  import { toasts, dismissToast, type ToastType } from "$lib/toast";
  import { fly, fade } from "svelte/transition";
  import { flip } from "svelte/animate";

  // Per-type accent color (icon + left rail).
  function accent(type: ToastType): string {
    if (type === "success") return "#AEB291";
    if (type === "error") return "#F87171";
    return "#FF8A3D";
  }
</script>

<div class="pointer-events-none fixed bottom-5 left-1/2 -translate-x-1/2 z-[100] flex flex-col items-center gap-2">
  {#each $toasts as toast (toast.id)}
    <div
      animate:flip={{ duration: 200 }}
      in:fly={{ y: 18, duration: 220 }}
      out:fade={{ duration: 150 }}
      role="status"
      class="pointer-events-auto relative flex items-center gap-2.5 min-w-[220px] max-w-[380px] overflow-hidden rounded-xl border border-overlay/[0.12] bg-raised/95 backdrop-blur-md pl-3.5 pr-3 py-2.5 shadow-lg shadow-black/30"
    >
      <!-- Accent rail -->
      <span class="absolute left-0 top-0 h-full w-[3px]" style="background-color: {accent(toast.type)}"></span>

      <!-- Icon -->
      <span class="shrink-0" style="color: {accent(toast.type)}">
        {#if toast.type === "success"}
          <svg xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="20 6 9 17 4 12"/></svg>
        {:else if toast.type === "error"}
          <svg xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4"><circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/></svg>
        {:else}
          <svg xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4"><circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/></svg>
        {/if}
      </span>

      <span class="flex-1 min-w-0 text-[12.5px] font-medium text-fg/80 leading-snug break-words">{toast.message}</span>

      <button
        onclick={() => dismissToast(toast.id)}
        aria-label="Dismiss"
        class="shrink-0 -mr-0.5 text-fg/30 hover:text-fg/70 transition-colors"
      >
        <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
      </button>
    </div>
  {/each}
</div>
