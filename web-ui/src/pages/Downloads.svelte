<script lang="ts">
  import { flip } from "svelte/animate";
  import { pauseDownload, resumeDownload, deleteDownload, addBatch, reorderQueue } from "../lib/api";
  import {
    downloads, usenetDownloads, protocolFilter, openAddPanel,
    selectedIds, toggleSelection, clearSelection, selectAll,
    searchQuery, downloadsLoaded, wsConnected,
  } from "../lib/stores";
  import { addToast } from "../lib/toast";
  import { locale, tr } from "../lib/i18n";
  import { flipConfig } from "../lib/motion";
  import DownloadCard from "../components/DownloadCard.svelte";
  import DownloadCompactRow from "../components/DownloadCompactRow.svelte";
  import SkeletonCard from "../components/SkeletonCard.svelte";
  import Icon from "@amigo/ui/components/Icon.svelte";

  let filter = $state<string>("all");
  let confirmingBatchDelete = $state(false);
  let sortBy = $state<string>("status");
  let draggedId = $state<string | null>(null);
  let viewMode = $state<"grid" | "list">(
    (typeof localStorage !== "undefined" ? localStorage.getItem("dl-view") : null) as "grid" | "list" || "grid"
  );

  function setViewMode(mode: "grid" | "list") {
    viewMode = mode;
    if (typeof localStorage !== "undefined") localStorage.setItem("dl-view", mode);
  }

  const statusOrder: Record<string, number> = {
    downloading: 0,
    paused: 1,
    queued: 2,
    completed: 3,
    failed: 4,
  };

  const filters = ["all", "downloading", "queued", "paused", "completed", "failed"];
  const sortOptions = ["status", "name", "size", "date"];

  // Merge HTTP + Usenet downloads based on protocol filter
  let allDownloads = $derived(() => {
    const proto = $protocolFilter;
    if (proto === "http") return $downloads;
    if (proto === "usenet") return $usenetDownloads;
    return [...$downloads, ...$usenetDownloads];
  });

  let filtered = $derived(
    allDownloads()
      .filter((d) => filter === "all" || d.status === filter)
      .filter((d) => {
        if (!$searchQuery) return true;
        const q = $searchQuery.toLowerCase();
        return (d.filename?.toLowerCase().includes(q)) || d.url.toLowerCase().includes(q);
      })
      .sort((a, b) => {
        if (sortBy === "name") return (a.filename || a.url).localeCompare(b.filename || b.url);
        if (sortBy === "size") return (b.filesize ?? 0) - (a.filesize ?? 0);
        if (sortBy === "date") return new Date(b.created_at).getTime() - new Date(a.created_at).getTime();
        return (statusOrder[a.status] ?? 99) - (statusOrder[b.status] ?? 99);
      })
  );

  let batchMode = $derived($selectedIds.size > 0);
  // Show skeletons only on the very first load, before any data has arrived.
  let showSkeleton = $derived(!$downloadsLoaded && allDownloads().length === 0);

  function countByStatus(status: string): number {
    return allDownloads().filter((d) => d.status === status).length;
  }

  function handleSelectAll() {
    if ($selectedIds.size === filtered.length) {
      clearSelection();
    } else {
      selectAll(filtered.map((d) => d.id));
    }
  }

  async function batchPause() {
    const total = $selectedIds.size;
    let failed = 0;
    for (const id of $selectedIds) {
      try { await pauseDownload(id); } catch { failed++; }
    }
    addToast(failed ? "error" : "info",
      failed
        ? tr($locale, "batch.paused_partial", { done: total - failed, total, failed })
        : tr($locale, "batch.paused", { count: total }));
    clearSelection();
  }

  async function batchResume() {
    const total = $selectedIds.size;
    let failed = 0;
    for (const id of $selectedIds) {
      try { await resumeDownload(id); } catch { failed++; }
    }
    addToast(failed ? "error" : "info",
      failed
        ? tr($locale, "batch.resumed_partial", { done: total - failed, total, failed })
        : tr($locale, "batch.resumed", { count: total }));
    clearSelection();
  }

  async function batchDelete() {
    if (!confirmingBatchDelete) {
      confirmingBatchDelete = true;
      setTimeout(() => { confirmingBatchDelete = false; }, 3000);
      return;
    }
    confirmingBatchDelete = false;
    const total = $selectedIds.size;
    // Capture URLs up front so deletion can be undone (re-queues them).
    const byId = new Map(allDownloads().map((d) => [d.id, d.url]));
    const urls = [...$selectedIds].map((id) => byId.get(id)).filter((u): u is string => !!u);
    let failed = 0;
    for (const id of $selectedIds) {
      try { await deleteDownload(id); } catch { failed++; }
    }
    addToast(failed ? "error" : "info",
      failed
        ? tr($locale, "batch.deleted_partial", { done: total - failed, total, failed })
        : tr($locale, "batch.deleted", { count: total }),
      undefined,
      urls.length > 0
        ? { action: { label: tr($locale, "action.undo"), onAction: () => addBatch(urls) } }
        : undefined);
    clearSelection();
  }

  // Empty-state copy depends on context: searching, a specific filter, or the
  // genuine first-run state (filter "all", nothing at all).
  let emptyKey = $derived(
    $searchQuery ? "empty.search" : filter !== "all" ? `empty.${filter}` : ""
  );

  function handleDragStart(id: string) {
    return (e: DragEvent) => {
      draggedId = id;
      if (e.dataTransfer) {
        e.dataTransfer.effectAllowed = "move";
        e.dataTransfer.setData("text/plain", id);
      }
    };
  }

  function handleDragOver(e: DragEvent) {
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
  }

  function handleDrop(targetId: string) {
    return async (e: DragEvent) => {
      e.preventDefault();
      const dragged = draggedId;
      draggedId = null;
      if (!dragged || dragged === targetId) return;
      // Reorder within whichever store actually holds the dragged item (not
      // the protocol filter — in the "all" view the item may be a usenet one).
      // Cross-store drops (target in the other store) are a no-op.
      const store = $usenetDownloads.some((d) => d.id === dragged)
        ? usenetDownloads
        : downloads;
      let moved = false;
      store.update((list) => {
        const items = [...list];
        const fromIdx = items.findIndex((d) => d.id === dragged);
        const toIdx = items.findIndex((d) => d.id === targetId);
        if (fromIdx === -1 || toIdx === -1) return list;
        const [item] = items.splice(fromIdx, 1);
        items.splice(toIdx, 0, item);
        moved = true;
        return items;
      });
      if (!moved) return;
      // Persist a COMPLETE ordering of both stores (matching the combined
      // "all" view: HTTP then usenet). The server assigns priority purely by
      // position, so sending only a subset would leave every omitted download
      // at the default priority and reshuffle the global queue.
      const newOrder = [...$downloads, ...$usenetDownloads].map((d) => d.id);
      try {
        await reorderQueue(newOrder);
      } catch (err) {
        console.error("Failed to persist reorder:", err);
      }
    };
  }
</script>

<div class="space-y-4">
  <!-- Sticky toolbar: search + sort + view + filters stay in reach while scrolling -->
  <div class="toolbar-sticky space-y-3 -mx-1 px-1 pt-1 pb-2">
    <div class="flex gap-2 items-center flex-wrap">
      <div class="flex-1 min-w-[12rem] flex items-center gap-2 rounded-lg px-3 py-2" style="background: var(--bg-surface); border: 1px solid var(--border-color)">
        <Icon name="search" size={16} />
        <input
          type="text"
          placeholder={tr($locale, "downloads.search")}
          bind:value={$searchQuery}
          class="flex-1 bg-transparent text-sm outline-none min-w-0"
          style="color: var(--text-primary)"
          aria-label={tr($locale, "downloads.search")}
        />
        {#if $searchQuery}
          <button onclick={() => searchQuery.set("")} class="icon-btn p-0.5 rounded" style="color: var(--text-secondary)" aria-label={tr($locale, "common.close")}>
            <Icon name="x" size={14} />
          </button>
        {/if}
      </div>

      <!-- Sort -->
      <div class="flex items-center gap-1 rounded-lg px-2 py-1.5 shrink-0" style="background: var(--bg-surface); border: 1px solid var(--border-color)">
        <Icon name="sort" size={14} />
        <select
          bind:value={sortBy}
          class="bg-transparent text-xs outline-none cursor-pointer"
          style="color: var(--text-primary)"
          aria-label={tr($locale, "downloads.sort_by")}
        >
          {#each sortOptions as opt}
            <option value={opt} style="background: var(--bg-surface-2)">{tr($locale, `sort.${opt}`)}</option>
          {/each}
        </select>
      </div>

      <!-- View toggle -->
      <div class="flex shrink-0 rounded-lg overflow-hidden" style="border: 1px solid var(--border-color)">
        <button
          onclick={() => setViewMode("grid")}
          class="icon-btn p-2"
          aria-pressed={viewMode === "grid"}
          style="color: {viewMode === 'grid' ? 'var(--neon-primary)' : 'var(--text-secondary)'}; background: {viewMode === 'grid' ? 'var(--hover-bg)' : 'var(--bg-surface)'}"
          aria-label={tr($locale, "downloads.grid_view")}
        >
          <Icon name="grid" size={14} />
        </button>
        <button
          onclick={() => setViewMode("list")}
          class="icon-btn p-2"
          aria-pressed={viewMode === "list"}
          style="color: {viewMode === 'list' ? 'var(--neon-primary)' : 'var(--text-secondary)'}; background: {viewMode === 'list' ? 'var(--hover-bg)' : 'var(--bg-surface)'}"
          aria-label={tr($locale, "downloads.list_view")}
        >
          <Icon name="list" size={14} />
        </button>
      </div>
    </div>

    <!-- Filter chips (horizontally scrollable on narrow screens) + batch toggle -->
    <div class="flex items-center gap-2">
      <div role="radiogroup" aria-label="Filter by status" class="chip-rail flex gap-2 flex-1 overflow-x-auto">
        {#each filters as f}
          <button
            role="radio"
            aria-checked={filter === f}
            onclick={() => (filter = f)}
            class="filter-chip px-3 py-1.5 rounded-lg text-sm font-medium shrink-0"
            style={filter === f
              ? "background: color-mix(in srgb, var(--neon-primary) 15%, transparent); color: var(--neon-primary)"
              : "background: var(--bg-surface); color: var(--text-secondary)"}
          >
            {tr($locale, `filter.${f}`)}
            <span class="ml-1 tabular-nums opacity-50">{f === "all" ? allDownloads().length : countByStatus(f)}</span>
          </button>
        {/each}
      </div>

      {#if filtered.length > 0}
        <button
          onclick={handleSelectAll}
          class="filter-chip px-3 py-1.5 rounded-lg text-sm font-medium shrink-0 flex items-center"
          style={batchMode
            ? "background: color-mix(in srgb, var(--neon-primary) 15%, transparent); color: var(--neon-primary)"
            : "background: var(--bg-surface); color: var(--text-secondary)"}
          aria-label={tr($locale, "downloads.select_all")}
        >
          <Icon name="check" size={14} />
          {#if batchMode}<span class="ml-1 tabular-nums">{$selectedIds.size}</span>{/if}
        </button>
      {/if}
    </div>
  </div>

  <!-- Batch actions bar -->
  {#if batchMode}
    <div class="flex items-center gap-2 px-3 py-2 rounded-lg flex-wrap" style="background: var(--bg-surface); border: 1px solid var(--border-color)">
      <span class="text-xs font-semibold" style="color: var(--text-secondary)">{$selectedIds.size} {tr($locale, "downloads.selected")}</span>
      <div class="flex-1"></div>
      <button onclick={batchPause} class="action-btn flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-semibold" style="background: var(--bg-surface-2); color: var(--neon-warning)">
        <Icon name="pause" size={14} /> {tr($locale, "batch.pause")}
      </button>
      <button onclick={batchResume} class="action-btn flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-semibold" style="background: var(--bg-surface-2); color: var(--neon-primary)">
        <Icon name="play" size={14} /> {tr($locale, "batch.resume")}
      </button>
      <button onclick={batchDelete} class="action-btn flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-semibold" style="background: var(--bg-surface-2); color: var(--neon-accent)">
        <Icon name="trash" size={14} /> {confirmingBatchDelete ? tr($locale, "batch.confirm") : tr($locale, "batch.delete")}
      </button>
      <button onclick={clearSelection} class="icon-btn p-1.5 rounded-lg" style="color: var(--text-secondary)" aria-label={tr($locale, "downloads.clear_selection")}>
        <Icon name="x" size={14} />
      </button>
    </div>
  {/if}

  <!-- Download list -->
  {#if showSkeleton}
    <SkeletonCard count={5} />
  {:else if filtered.length === 0}
    {#if !$wsConnected}
      <!-- The list being empty while disconnected usually means we couldn't
           load it, not that the queue is empty — say so instead. -->
      <div class="flex flex-col items-center justify-center py-20 text-center">
        <Icon name="wifi-off" size={32} />
        <p class="mt-4 text-sm font-semibold" style="color: var(--text-primary)">{tr($locale, "downloads.offline")}</p>
        <p class="text-xs mt-1" style="color: var(--text-secondary)">{tr($locale, "downloads.offline_hint")}</p>
      </div>
    {:else if emptyKey}
      <!-- Context-aware empty state (specific filter / search) -->
      <div class="flex flex-col items-center justify-center py-20 text-center">
        <Icon name="search" size={32} />
        <p class="mt-4 text-sm" style="color: var(--text-secondary)">{tr($locale, emptyKey)}</p>
      </div>
    {:else}
      <!-- First-run empty state -->
      <div class="flex flex-col items-center justify-center py-20 text-center">
        <img src="/amigo-logo.png" alt="" width="64" height="64" class="rounded-lg opacity-30" />
        <p class="mt-4 text-sm" style="color: var(--text-secondary)">{tr($locale, "downloads.no_downloads")}</p>
        <button
          onclick={() => openAddPanel()}
          class="action-btn mt-4 flex items-center gap-2 px-4 py-2.5 rounded-lg text-sm font-semibold"
          style="background: var(--neon-primary); color: var(--bg-deep); box-shadow: var(--neon-glow-sm)"
        >
          <Icon name="plus" size={16} />
          {tr($locale, "downloads.add_first")}
        </button>
        <p class="text-xs mt-3" style="color: var(--text-secondary); opacity: 0.5">{tr($locale, "downloads.add_hint")}</p>
      </div>
    {/if}
  {:else if viewMode === "grid"}
    <div class="grid gap-3">
      {#each filtered as download, i (download.id)}
        <div animate:flip={flipConfig}>
          <DownloadCard
            {download}
            index={i}
            ondragstart={handleDragStart(download.id)}
            ondragover={handleDragOver}
            ondrop={handleDrop(download.id)}
          />
        </div>
      {/each}
    </div>
  {:else}
    <div class="flex flex-col gap-1">
      {#each filtered as download (download.id)}
        <div animate:flip={flipConfig}>
          <DownloadCompactRow {download} />
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  /* Toolbar sticks to the top of the scroll container so search/filters stay
     reachable in long lists. Slightly translucent so cards scroll under it. */
  .toolbar-sticky {
    position: sticky;
    top: 0;
    z-index: 20;
    background: color-mix(in srgb, var(--bg-deep) 88%, transparent);
    backdrop-filter: blur(8px);
  }

  /* Hide the scrollbar on the filter rail — it scrolls by drag/swipe. */
  .chip-rail {
    scrollbar-width: none;
  }
  .chip-rail::-webkit-scrollbar {
    display: none;
  }
</style>
