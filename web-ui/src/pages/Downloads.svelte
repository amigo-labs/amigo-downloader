<script lang="ts">
  import { downloads, usenetDownloads, protocolFilter, selectedDownloadId } from "../lib/stores";
  import DownloadCard from "../components/DownloadCard.svelte";
  import SkeletonCard from "../components/SkeletonCard.svelte";

  let filter = $state<string>("all");
  let loading = $state(false);

  const statusOrder: Record<string, number> = {
    downloading: 0,
    paused: 1,
    queued: 2,
    completed: 3,
    failed: 4,
  };

  const filters = ["all", "downloading", "queued", "paused", "completed", "failed"];

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
      .sort((a, b) => (statusOrder[a.status] ?? 99) - (statusOrder[b.status] ?? 99))
  );

  function countByStatus(status: string): number {
    return allDownloads().filter((d) => d.status === status).length;
  }
</script>

<div class="space-y-4">
  <!-- Filter chips (audit M8: radiogroup) -->
  <div role="radiogroup" aria-label="Filter by status" class="flex gap-2 flex-wrap">
    {#each filters as f}
      <button
        role="radio"
        aria-checked={filter === f}
        onclick={() => (filter = f)}
        class="px-3 py-1.5 rounded-lg text-sm font-medium transition-colors capitalize"
        style={filter === f
          ? "background: color-mix(in srgb, var(--neon-primary) 15%, transparent); color: var(--neon-primary)"
          : "background: var(--bg-surface); color: var(--text-secondary)"}
      >
        {f}
        {#if f !== "all"}
          <span class="ml-1 opacity-50">{countByStatus(f)}</span>
        {/if}
      </button>
    {/each}
  </div>

  <!-- Download list -->
  {#if loading}
    <div class="grid gap-3">
      <SkeletonCard count={3} />
    </div>
  {:else if filtered.length === 0}
    <div class="flex flex-col items-center justify-center py-20">
      <img src="/amigo-logo.png" alt="" width="64" height="64" class="rounded-lg opacity-40" />
      <p class="mt-4 text-sm" style="color: var(--text-secondary)">No downloads yet. Click "Add Download" or drop files to start.</p>
      <p class="text-xs mt-2" style="color: var(--text-secondary); opacity: 0.6">Ctrl+N to add &middot; Drag & drop supported</p>
    </div>
  {:else}
    <div class="grid gap-3">
      {#each filtered as download, i (download.id)}
        <DownloadCard {download} index={i} />
      {/each}
    </div>
  {/if}
</div>
