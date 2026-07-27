<script lang="ts">
  import { onMount } from "svelte";
  import { pauseDownload, resumeDownload, deleteDownload, addBatch, reorderQueue } from "../lib/api";
  import {
    downloads, usenetDownloads, protocolFilter, openAddPanel,
    selectedIds, searchQuery, downloadsLoaded, wsConnected,
    features, type Download,
  } from "../lib/stores";
  import { addToast } from "../lib/toast";
  import { locale, tr } from "../lib/i18n";
  import { statusCounts } from "../lib/download";
  import { clear as clearSelection, toggleAll, clearIfAny } from "../lib/selection";
  import { registerBindings, registerEscapeFallback, type Binding } from "../lib/keymap";
  import { batchProgress, bulkUpdate, bulkDelete } from "../lib/batch";
  import { moveById } from "../lib/dnd";
  import DownloadList from "../components/download/DownloadList.svelte";
  import SkeletonCard from "../components/SkeletonCard.svelte";
  import Icon from "@amigo/ui/components/Icon.svelte";
  import Button from "@amigo/ui/components/Button.svelte";
  import IconButton from "@amigo/ui/components/IconButton.svelte";
  import Chip from "@amigo/ui/components/Chip.svelte";
  import SegmentedControl from "@amigo/ui/components/SegmentedControl.svelte";
  import EmptyState from "@amigo/ui/components/EmptyState.svelte";
  import ProgressBar from "@amigo/ui/components/ProgressBar.svelte";

  let filter = $state<string>("all");
  let confirmingBatchDelete = $state(false);
  let sortBy = $state<string>("status");
  let batchAbort: AbortController | null = null;
  let viewMode = $state<"card" | "row">(
    ((typeof localStorage !== "undefined" ? localStorage.getItem("dl-view") : null) as "card" | "row") || "card",
  );

  function setViewMode(mode: string) {
    viewMode = mode as "card" | "row";
    if (typeof localStorage !== "undefined") localStorage.setItem("dl-view", mode);
  }

  const statusOrder: Record<string, number> = {
    downloading: 0, paused: 1, queued: 2, completed: 3, failed: 4,
  };

  const filters = ["all", "downloading", "queued", "paused", "completed", "failed"];
  const sortOptions = ["status", "name", "size", "date"];

  let allDownloads = $derived.by(() => {
    const proto = $protocolFilter;
    if (proto === "http") return $downloads;
    if (proto === "usenet") return $usenetDownloads;
    return [...$downloads, ...$usenetDownloads];
  });

  // One pass instead of six: the old code called countByStatus() once per
  // filter chip, so every WebSocket progress tick walked the whole list six
  // times over.
  let counts = $derived(statusCounts(allDownloads));

  let filtered = $derived.by(() =>
    allDownloads
      .filter((d) => filter === "all" || d.status === filter)
      .filter((d) => {
        if (!$searchQuery) return true;
        const q = $searchQuery.toLowerCase();
        return d.filename?.toLowerCase().includes(q) || d.url.toLowerCase().includes(q);
      })
      .sort((a, b) => {
        if (sortBy === "name") return (a.filename || a.url).localeCompare(b.filename || b.url);
        if (sortBy === "size") return (b.filesize ?? 0) - (a.filesize ?? 0);
        if (sortBy === "date") return new Date(b.created_at).getTime() - new Date(a.created_at).getTime();
        return (statusOrder[a.status] ?? 99) - (statusOrder[b.status] ?? 99);
      }),
  );

  let batchMode = $derived($selectedIds.size > 0);
  let showSkeleton = $derived(!$downloadsLoaded && allDownloads.length === 0);

  // Reordering is only meaningful when the list is in queue order. It used to
  // be offered under every sort, where the drop landed somewhere unrelated to
  // what the user saw and the result looked random.
  let reorderEnabled = $derived(sortBy === "status" && filter === "all" && !$searchQuery);

  onMount(() => {
    const bindings: Binding[] = [
      {
        chord: { key: "a", mod: true },
        descKey: "shortcuts.select_all",
        run: () => toggleAll(filtered.map((d) => d.id)),
      },
    ];
    const unregister = registerBindings(bindings);
    // Escape clears the selection once every overlay has had its turn.
    const unescape = registerEscapeFallback(clearIfAny);
    return () => {
      unregister();
      unescape();
    };
  });

  function handleSelectAll() {
    toggleAll(filtered.map((d) => d.id));
  }

  async function runBatchAction(
    action: "pause" | "resume" | "retry",
    perId: (id: string) => Promise<unknown>,
    okKey: string,
    partialKey: string,
  ) {
    if ($batchProgress?.running) return;
    const ids = [...$selectedIds];
    batchAbort = new AbortController();
    const { ok, failed, cancelled } = await bulkUpdate(ids, action, perId, batchAbort.signal);
    batchAbort = null;

    if (failed.length) {
      addToast("error", tr($locale, partialKey, {
        done: ok.length, total: ids.length, failed: failed.length,
      }));
      // Keep the failures selected so a retry is one click away, instead of
      // clearing unconditionally and throwing away the recovery affordance.
      selectedIds.set(new Set(failed));
      return;
    }
    if (!cancelled) addToast("info", tr($locale, okKey, { count: ok.length }));
    clearSelection();
  }

  const batchPause = () =>
    runBatchAction("pause", pauseDownload, "batch.paused", "batch.paused_partial");
  const batchResume = () =>
    runBatchAction("resume", resumeDownload, "batch.resumed", "batch.resumed_partial");

  async function batchDelete() {
    if (!confirmingBatchDelete) {
      confirmingBatchDelete = true;
      setTimeout(() => { confirmingBatchDelete = false; }, 2500);
      return;
    }
    confirmingBatchDelete = false;

    const ids = [...$selectedIds];
    // Capture URLs up front so the deletion can be undone.
    const byId = new Map(allDownloads.map((d) => [d.id, d.url]));
    const urls = ids.map((id) => byId.get(id)).filter((u): u is string => !!u);

    batchAbort = new AbortController();
    const { ok, failed } = await bulkDelete(ids, deleteDownload, batchAbort.signal);
    batchAbort = null;

    if (failed.length) {
      addToast("error", tr($locale, "batch.deleted_partial", {
        done: ok.length, total: ids.length, failed: failed.length,
      }));
      selectedIds.set(new Set(failed));
      return;
    }
    addToast("info", tr($locale, "batch.deleted", { count: ok.length }), undefined,
      urls.length ? { action: { label: tr($locale, "action.undo"), onAction: () => addBatch(urls) } } : undefined);
    clearSelection();
  }

  let emptyKey = $derived(
    $searchQuery ? "empty.search" : filter !== "all" ? `empty.${filter}` : "",
  );

  /** Which store actually holds this id — the "all" view mixes both. */
  function storeFor(id: string) {
    return $usenetDownloads.some((d) => d.id === id) ? usenetDownloads : downloads;
  }

  async function persistOrder(previous: Download[][]) {
    // The server assigns priority purely by position, so it needs a complete
    // ordering of both stores, not just the moved subset.
    const newOrder = [...$downloads, ...$usenetDownloads].map((d) => d.id);
    try {
      await reorderQueue(newOrder);
    } catch {
      // Roll back rather than leave the UI showing an order the server
      // rejected — the old code only logged to the console.
      downloads.set(previous[0]);
      usenetDownloads.set(previous[1]);
      addToast("error", tr($locale, "downloads.reorder_failed"));
    }
  }

  async function handleReorder(draggedId: string, targetId: string) {
    if (draggedId === targetId) return;
    const snapshot: Download[][] = [[...$downloads], [...$usenetDownloads]];
    const store = storeFor(draggedId);
    let moved = false;
    store.update((list) => {
      const next = moveById(list, draggedId, targetId);
      moved = next !== list;
      return next;
    });
    if (!moved) return;
    await persistOrder(snapshot);
  }

  async function handleRestoreOrder(ids: string[]) {
    const snapshot: Download[][] = [[...$downloads], [...$usenetDownloads]];
    const rank = new Map(ids.map((id, i) => [id, i]));
    const bySnapshotOrder = (a: Download, b: Download) =>
      (rank.get(a.id) ?? Number.MAX_SAFE_INTEGER) - (rank.get(b.id) ?? Number.MAX_SAFE_INTEGER);
    downloads.update((l) => [...l].sort(bySnapshotOrder));
    usenetDownloads.update((l) => [...l].sort(bySnapshotOrder));
    await persistOrder(snapshot);
  }

  const protocolOptions = $derived([
    { value: "all", label: tr($locale, "protocol.all") },
    { value: "http", label: "HTTP" },
    { value: "usenet", label: "Usenet" },
  ]);
</script>

<div class="space-y-4">
  <div class="toolbar-sticky space-y-3 -mx-1 px-1 pt-1 pb-2">
    <div class="flex gap-2 items-center flex-wrap">
      <div
        class="flex-1 min-w-[12rem] flex items-center gap-2 rounded-lg px-3"
        style="background: var(--bg-surface); border: 1px solid var(--border-color); min-height: 40px"
      >
        <Icon name="search" size={16} />
        <input
          type="search"
          placeholder={tr($locale, "downloads.search")}
          bind:value={$searchQuery}
          class="flex-1 bg-transparent text-sm outline-none min-w-0"
          style="color: var(--text-primary)"
          aria-label={tr($locale, "downloads.search")}
        />
      </div>

      <div
        class="flex items-center gap-1 rounded-lg px-2 shrink-0"
        style="background: var(--bg-surface); border: 1px solid var(--border-color); min-height: 40px"
      >
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

      <SegmentedControl
        size="sm"
        options={[
          { value: "card", label: tr($locale, "downloads.comfortable"), icon: "grid" },
          { value: "row", label: tr($locale, "downloads.compact"), icon: "list" },
        ]}
        value={viewMode}
        ariaLabel={tr($locale, "downloads.density")}
        onchange={setViewMode}
      />
    </div>

    <div class="flex items-center gap-2 flex-wrap">
      <!-- Below the desktop breakpoint the header has no room for this, and
           there was previously no mobile equivalent at all: under 640px the
           HTTP/Usenet switch simply did not exist. -->
      {#if $features.usenet}
        <div class="lg:hidden">
          <SegmentedControl
            size="sm"
            mono
            options={protocolOptions}
            value={$protocolFilter}
            ariaLabel={tr($locale, "downloads.protocol_filter")}
            onchange={(v) => protocolFilter.set(v as "all" | "http" | "usenet")}
          />
        </div>
      {/if}

      <div role="group" aria-label={tr($locale, "downloads.filter_by_status")} class="chip-rail flex gap-2 flex-1 overflow-x-auto">
        {#each filters as f}
          <Chip
            size="sm"
            selected={filter === f}
            count={f === "all" ? allDownloads.length : (counts[f] ?? 0)}
            onclick={() => (filter = f)}
            aria-pressed={filter === f}
          >
            {tr($locale, `filter.${f}`)}
          </Chip>
        {/each}
      </div>

      {#if filtered.length > 0}
        <Chip
          size="sm"
          icon="check"
          selected={batchMode}
          count={batchMode ? $selectedIds.size : undefined}
          onclick={handleSelectAll}
          aria-pressed={batchMode}
          aria-label={tr($locale, "downloads.select_all")}
        />
      {/if}
    </div>

    {#if !reorderEnabled && sortBy !== "status" && filtered.length > 1}
      <p class="flex items-center gap-2 text-xs" style="color: var(--text-secondary)">
        {tr($locale, "downloads.reorder_locked")}
        <Button variant="ghost" size="sm" onclick={() => (sortBy = "status")}>
          {tr($locale, "downloads.reorder_switch")}
        </Button>
      </p>
    {/if}
  </div>

  {#if batchMode}
    <div
      class="flex items-center gap-2 px-3 py-2 rounded-lg flex-wrap"
      style="background: var(--bg-surface); border: 1px solid var(--border-color)"
    >
      <span class="text-xs font-semibold" style="color: var(--text-secondary)">
        {$selectedIds.size}
        {tr($locale, "downloads.selected")}
      </span>

      {#if $batchProgress?.running}
        <div class="flex-1 min-w-[8rem] flex items-center gap-2">
          <ProgressBar
            value={$batchProgress.done}
            max={$batchProgress.total}
            label={tr($locale, "batch.progress", { done: $batchProgress.done, total: $batchProgress.total })}
          />
          <span class="text-xs tabular-nums shrink-0" style="color: var(--text-secondary)">
            {tr($locale, "batch.progress", { done: $batchProgress.done, total: $batchProgress.total })}
          </span>
        </div>
        <Button variant="ghost" size="sm" onclick={() => batchAbort?.abort()}>
          {tr($locale, "batch.cancel")}
        </Button>
      {:else}
        <div class="flex-1"></div>
        <Button variant="soft" tone="warning" size="sm" iconLeft="pause" onclick={batchPause}>
          {tr($locale, "batch.pause")}
        </Button>
        <Button variant="soft" tone="accent" size="sm" iconLeft="play" onclick={batchResume}>
          {tr($locale, "batch.resume")}
        </Button>
        <Button variant="soft" tone="danger" size="sm" iconLeft="trash" onclick={batchDelete}>
          {confirmingBatchDelete ? tr($locale, "batch.confirm") : tr($locale, "batch.delete")}
        </Button>
        <IconButton
          icon="x"
          size="sm"
          label={tr($locale, "downloads.clear_selection")}
          onclick={clearSelection}
        />
      {/if}
    </div>
  {/if}

  {#if showSkeleton}
    <SkeletonCard count={5} />
  {:else if filtered.length === 0}
    {#if !$wsConnected}
      <EmptyState
        icon="wifi-off"
        title={tr($locale, "downloads.offline")}
        description={tr($locale, "downloads.offline_hint")}
      />
    {:else if emptyKey}
      <EmptyState icon="search" title={tr($locale, emptyKey)} />
    {:else}
      <EmptyState
        image="/amigo-logo.png"
        title={tr($locale, "downloads.no_downloads")}
        description={tr($locale, "downloads.add_hint")}
      >
        {#snippet action()}
          <Button variant="solid" iconLeft="plus" onclick={() => openAddPanel()}>
            {tr($locale, "downloads.add_first")}
          </Button>
        {/snippet}
      </EmptyState>
    {/if}
  {:else}
    <DownloadList
      downloads={filtered}
      variant={viewMode}
      {reorderEnabled}
      onReorder={handleReorder}
      onRestoreOrder={handleRestoreOrder}
    />
  {/if}
</div>

<style>
  /* Toolbar sticks to the top of the scroll container so search/filters stay
     reachable in long lists. Slightly translucent so cards scroll under it. */
  .toolbar-sticky {
    position: sticky;
    top: 0;
    z-index: var(--z-sticky);
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
