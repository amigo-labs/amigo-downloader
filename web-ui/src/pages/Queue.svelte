<script lang="ts">
  import { downloads } from "../lib/stores";
  import { resumeDownload, deleteDownload, formatBytes } from "../lib/api";

  let queued = $derived($downloads.filter((d) => d.status === "queued"));
</script>

<div class="space-y-4">
  {#if queued.length === 0}
    <div class="flex flex-col items-center justify-center py-20 opacity-50">
      <p style="color: var(--text-secondary-color)">Queue is empty.</p>
    </div>
  {:else}
    <p class="text-sm" style="color: var(--text-secondary-color)">{queued.length} items in queue</p>
    <div class="space-y-2">
      {#each queued as download, i (download.id)}
        <div
          class="flex items-center gap-4 rounded-xl px-4 py-3 transition-all"
          style="background: var(--surface-2-color); border: 1px solid var(--border-color)"
        >
          <span class="text-sm font-mono w-6 text-center" style="color: var(--text-secondary-color)">
            {i + 1}
          </span>
          <div class="flex-1 min-w-0">
            <p class="font-medium truncate">{download.filename || download.url}</p>
            <p class="text-xs" style="color: var(--text-secondary-color)">
              {download.filesize ? formatBytes(download.filesize) : "Unknown size"}
            </p>
          </div>
          <div class="flex gap-2">
            <button
              onclick={() => resumeDownload(download.id)}
              class="px-3 py-1.5 rounded-lg text-xs font-medium text-white"
              style="background: var(--accent-color)"
            >
              Start
            </button>
            <button
              onclick={() => deleteDownload(download.id)}
              class="px-3 py-1.5 rounded-lg text-xs font-medium hover:bg-red-500/20 text-red-400"
              style="background: var(--surface-3-color)"
            >
              Remove
            </button>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>
