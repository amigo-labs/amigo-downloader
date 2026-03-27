<script lang="ts">
  import { downloads, layout } from "../lib/stores";
  import { pauseDownload, resumeDownload, deleteDownload, formatBytes, formatSpeed } from "../lib/api";
  import DownloadCard from "../components/DownloadCard.svelte";
  import DownloadRow from "../components/DownloadRow.svelte";

  let filter = $state<string>("all");

  let filtered = $derived(
    $downloads.filter((d) => {
      if (filter === "all") return true;
      return d.status === filter;
    })
  );
</script>

<div class="space-y-4">
  <!-- Filters -->
  <div class="flex gap-2 flex-wrap">
    {#each ["all", "downloading", "queued", "paused", "completed", "failed"] as f}
      <button
        onclick={() => (filter = f)}
        class="px-3 py-1.5 rounded-lg text-sm font-medium transition-all capitalize"
        style={filter === f
          ? "background: var(--accent-color); color: white"
          : "background: var(--surface-3-color); color: var(--text-secondary-color)"}
      >
        {f}
        {#if f !== "all"}
          <span class="ml-1 opacity-70">
            {$downloads.filter((d) => d.status === f).length}
          </span>
        {/if}
      </button>
    {/each}
  </div>

  <!-- Download list -->
  {#if filtered.length === 0}
    <div class="flex flex-col items-center justify-center py-20 opacity-50">
      <div class="pixel-logo text-4xl mb-4" style="font-family: 'Press Start 2P'; color: var(--accent-color)">
        :)
      </div>
      <p style="color: var(--text-secondary-color)">No downloads yet. Click "Add Download" to start.</p>
    </div>
  {:else if $layout === "modern"}
    <div class="grid gap-3">
      {#each filtered as download (download.id)}
        <DownloadCard {download} />
      {/each}
    </div>
  {:else}
    <!-- Classic table view -->
    <div class="rounded-lg overflow-hidden border" style="border-color: var(--border-color)">
      <table class="w-full text-sm">
        <thead>
          <tr style="background: var(--surface-2-color)">
            <th class="text-left px-4 py-2.5 font-semibold">Name</th>
            <th class="text-left px-4 py-2.5 font-semibold w-24">Size</th>
            <th class="text-left px-4 py-2.5 font-semibold w-24">Status</th>
            <th class="text-left px-4 py-2.5 font-semibold w-32">Progress</th>
            <th class="text-left px-4 py-2.5 font-semibold w-24">Speed</th>
            <th class="text-right px-4 py-2.5 font-semibold w-28">Actions</th>
          </tr>
        </thead>
        <tbody>
          {#each filtered as download (download.id)}
            <DownloadRow {download} />
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>
