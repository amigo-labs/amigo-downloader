<script lang="ts">
  import { pauseDownload, resumeDownload, deleteDownload } from "../lib/api";
  import {
    downloads, usenetDownloads, protocolFilter, openAddPanel,
    selectedIds, toggleSelection, clearSelection, selectAll,
    searchQuery,
  } from "../lib/stores";
  import { addToast } from "../lib/toast";
  import DownloadCard from "../components/DownloadCard.svelte";
  import DownloadCompactRow from "../components/DownloadCompactRow.svelte";
  import Icon from "../components/Icon.svelte";
  import SkeletonCard from "../components/SkeletonCard.svelte";

  let filter = $state<string>("all");
  let loading = $state(false);
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
  const sortOptions = [
    { value: "status", label: "Status" },
    { value: "name", label: "Name" },
    { value: "size", label: "Size" },
    { value: "date", label: "Date" },
  ];

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
    for (const id of $selectedIds) { await pauseDownload(id); }
    addToast("info", `Paused ${$selectedIds.size} downloads`);
    clearSelection();
  }

  async function batchResume() {
    for (const id of $selectedIds) { await resumeDownload(id); }
    addToast("info", `Resumed ${$selectedIds.size} downloads`);
    clearSelection();
  }

  async function batchDelete() {
    for (const id of $selectedIds) { await deleteDownload(id); }
    addToast("info", `Deleted ${$selectedIds.size} downloads`);
    clearSelection();
  }

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
    return (e: DragEvent) => {
      e.preventDefault();
      if (!draggedId || draggedId === targetId) { draggedId = null; return; }
      // Reorder in the correct store based on protocol filter
      const store = $protocolFilter === "usenet" ? usenetDownloads : downloads;
      store.update((list) => {
        const items = [...list];
        const fromIdx = items.findIndex((d) => d.id === draggedId);
        const toIdx = items.findIndex((d) => d.id === targetId);
        if (fromIdx === -1 || toIdx === -1) return list;
        const [item] = items.splice(fromIdx, 1);
        items.splice(toIdx, 0, item);
        return items;
      });
      draggedId = null;
    };
  }
</script>

<div class="space-y-4">
  <!-- Search bar + sort + view toggle -->
  <div class="flex gap-2 items-center">
    <div class="flex-1 flex items-center gap-2 rounded-lg px-3 py-2" style="background: var(--bg-surface); border: 1px solid var(--border-color)">
      <Icon name="search" size={16} />
      <input
        type="text"
        placeholder="Search downloads..."
        bind:value={$searchQuery}
        class="flex-1 bg-transparent text-sm outline-none"
        style="color: var(--text-primary)"
        aria-label="Search downloads"
      />
      {#if $searchQuery}
        <button onclick={() => searchQuery.set("")} class="icon-btn p-0.5 rounded" style="color: var(--text-secondary)" aria-label="Clear search">
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
        aria-label="Sort by"
      >
        {#each sortOptions as opt}
          <option value={opt.value} style="background: var(--bg-surface-2)">{opt.label}</option>
        {/each}
      </select>
    </div>

    <!-- View toggle -->
    <div class="flex shrink-0 rounded-lg overflow-hidden" style="border: 1px solid var(--border-color)">
      <button
        onclick={() => setViewMode("grid")}
        class="icon-btn p-2"
        style="color: {viewMode === 'grid' ? 'var(--neon-primary)' : 'var(--text-secondary)'}; background: {viewMode === 'grid' ? 'var(--hover-bg)' : 'var(--bg-surface)'}"
        aria-label="Grid view"
      >
        <Icon name="grid" size={14} />
      </button>
      <button
        onclick={() => setViewMode("list")}
        class="icon-btn p-2"
        style="color: {viewMode === 'list' ? 'var(--neon-primary)' : 'var(--text-secondary)'}; background: {viewMode === 'list' ? 'var(--hover-bg)' : 'var(--bg-surface)'}"
        aria-label="List view"
      >
        <Icon name="list" size={14} />
      </button>
    </div>
  </div>

  <!-- Filter chips + batch toolbar -->
  <div class="flex items-center gap-2 flex-wrap">
    <div role="radiogroup" aria-label="Filter by status" class="flex gap-2 flex-wrap flex-1">
      {#each filters as f}
        <button
          role="radio"
          aria-checked={filter === f}
          onclick={() => (filter = f)}
          class="filter-chip px-3 py-1.5 rounded-lg text-sm font-medium capitalize"
          style={filter === f
            ? "background: color-mix(in srgb, var(--neon-primary) 15%, transparent); color: var(--neon-primary)"
            : "background: var(--bg-surface); color: var(--text-secondary)"}
        >
          {f}
          {#if f === "all"}
            <span class="ml-1 opacity-50">{allDownloads().length}</span>
          {:else}
            <span class="ml-1 opacity-50">{countByStatus(f)}</span>
          {/if}
        </button>
      {/each}
    </div>

    <!-- Batch select toggle -->
    {#if filtered.length > 0}
      <button
        onclick={handleSelectAll}
        class="filter-chip px-3 py-1.5 rounded-lg text-sm font-medium"
        style={batchMode
          ? "background: color-mix(in srgb, var(--neon-primary) 15%, transparent); color: var(--neon-primary)"
          : "background: var(--bg-surface); color: var(--text-secondary)"}
        aria-label="Select all"
      >
        <Icon name="check" size={14} />
        {#if batchMode}
          <span class="ml-1">{$selectedIds.size}</span>
        {/if}
      </button>
    {/if}
  </div>

  <!-- Batch actions bar -->
  {#if batchMode}
    <div class="flex items-center gap-2 px-3 py-2 rounded-lg" style="background: var(--bg-surface); border: 1px solid var(--border-color)">
      <span class="text-xs font-semibold" style="color: var(--text-secondary)">{$selectedIds.size} selected</span>
      <div class="flex-1"></div>
      <button onclick={batchPause} class="action-btn flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-semibold" style="background: var(--bg-surface-2); color: var(--neon-warning)">
        <Icon name="pause" size={14} /> Pause
      </button>
      <button onclick={batchResume} class="action-btn flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-semibold" style="background: var(--bg-surface-2); color: var(--neon-primary)">
        <Icon name="play" size={14} /> Resume
      </button>
      <button onclick={batchDelete} class="action-btn flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-semibold" style="background: var(--bg-surface-2); color: var(--neon-accent)">
        <Icon name="trash" size={14} /> Delete
      </button>
      <button onclick={clearSelection} class="icon-btn p-1.5 rounded-lg" style="color: var(--text-secondary)" aria-label="Clear selection">
        <Icon name="x" size={14} />
      </button>
    </div>
  {/if}

  <!-- Download list -->
  {#if loading}
    <div class="grid gap-3">
      <SkeletonCard count={3} />
    </div>
  {:else if filtered.length === 0}
    <div class="flex flex-col items-center justify-center py-20">
      <img src="/amigo-logo.png" alt="" width="64" height="64" class="rounded-lg opacity-30" />
      <p class="mt-4 text-sm" style="color: var(--text-secondary)">No downloads yet</p>
      <button
        onclick={() => openAddPanel()}
        class="mt-4 flex items-center gap-2 px-4 py-2.5 rounded-lg text-sm font-semibold transition-colors"
        style="background: var(--neon-primary); color: var(--bg-deep)"
      >
        <Icon name="plus" size={16} />
        Add your first download
      </button>
      <p class="text-xs mt-3" style="color: var(--text-secondary); opacity: 0.5">Ctrl+N to add &middot; Drag & drop supported</p>
    </div>
  {:else}
    {#if viewMode === "grid"}
      <div class="grid gap-3">
        {#each filtered as download, i (download.id)}
          <DownloadCard
          {download}
          index={i}
          ondragstart={handleDragStart(download.id)}
          ondragover={handleDragOver}
          ondrop={handleDrop(download.id)}
        />
        {/each}
      </div>
    {:else}
      <div class="flex flex-col gap-1">
        {#each filtered as download (download.id)}
          <DownloadCompactRow {download} />
        {/each}
      </div>
    {/if}
  {/if}
</div>
