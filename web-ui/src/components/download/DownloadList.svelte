<script lang="ts">
  import { flip } from "svelte/animate";
  import { flipConfig } from "../../lib/motion";
  import { locale, tr } from "../../lib/i18n";
  import { ariaAnnouncement, type Download } from "../../lib/stores";
  import { displayName } from "../../lib/download";
  import DownloadItem from "./DownloadItem.svelte";

  let {
    downloads,
    variant = "card",
    reorderEnabled = false,
    onReorder,
    onRestoreOrder,
  }: {
    downloads: Download[];
    variant?: "card" | "row";
    reorderEnabled?: boolean;
    /** Move `draggedId` to the position currently held by `targetId`. */
    onReorder?: (draggedId: string, targetId: string) => void;
    /** Restore an exact ordering, used to cancel a keyboard grab. */
    onRestoreOrder?: (ids: string[]) => void;
  } = $props();

  let orderedIds = $derived(downloads.map((d) => d.id));

  // Keyboard reorder. Dragging was the only way to change queue order, so
  // priority management was mouse-only. Space grabs, arrows move, Enter
  // commits, Escape restores the order captured at grab time.
  let grabbedId = $state<string | null>(null);
  let orderBeforeGrab: string[] = [];

  function announce(key: string, params?: Record<string, string | number>) {
    ariaAnnouncement.set(tr($locale, key, params));
  }

  function handleGrabKey(e: KeyboardEvent, id: string) {
    if (!reorderEnabled) return;
    const list = orderedIds;
    const at = list.indexOf(id);
    if (at === -1) return;

    if (e.key === " " || e.key === "Enter") {
      e.preventDefault();
      if (grabbedId === id) {
        grabbedId = null;
        announce("downloads.reorder_dropped", { pos: at + 1, total: list.length });
      } else {
        grabbedId = id;
        orderBeforeGrab = [...list];
        announce("downloads.reorder_grabbed", { pos: at + 1, total: list.length });
      }
      return;
    }

    if (grabbedId !== id) return;

    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      grabbedId = null;
      onRestoreOrder?.(orderBeforeGrab);
      announce("downloads.reorder_cancelled");
      return;
    }

    const delta = e.key === "ArrowUp" ? -1 : e.key === "ArrowDown" ? 1 : 0;
    if (delta === 0) return;
    e.preventDefault();
    const to = at + delta;
    if (to < 0 || to >= list.length) return;
    onReorder?.(id, list[to]);
    announce("downloads.reorder_moved", { pos: to + 1, total: list.length });
  }
</script>

<!--
  A real list. The grid of <div>s this replaces gave screen readers no
  "list of N items" announcement and no per-item position.
-->
<ul class="dl-list {variant}" aria-label={tr($locale, "downloads.list_label")}>
  {#each downloads as download, i (download.id)}
    <li
      class="dl-slot"
      aria-posinset={i + 1}
      aria-setsize={downloads.length}
      animate:flip={flipConfig}
    >
      <DownloadItem
        {download}
        index={i}
        setSize={downloads.length}
        {variant}
        {reorderEnabled}
        {orderedIds}
        grabbed={grabbedId === download.id}
        {onReorder}
        onGrabKey={handleGrabKey}
      />
    </li>
  {/each}
</ul>

{#if grabbedId}
  <p class="sr-only" aria-live="assertive">
    {tr($locale, "downloads.reorder_active", {
      name: displayName(downloads.find((d) => d.id === grabbedId) ?? { filename: null, url: "" }),
    })}
  </p>
{/if}

<style>
  .dl-list {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .dl-list.card {
    gap: var(--space-3);
  }

  .dl-list.row {
    gap: 0.25rem;
  }

  .dl-slot {
    list-style: none;
  }
</style>
