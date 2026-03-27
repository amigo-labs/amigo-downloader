<script lang="ts">
  import { pauseDownload, resumeDownload, deleteDownload, formatBytes, formatSpeed } from "../lib/api";
  import ChunkViz from "./ChunkViz.svelte";

  let { download, index = 0 }: { download: any; index?: number } = $props();

  let progress = $derived(
    download.filesize ? Math.round((download.bytes_downloaded / download.filesize) * 100) : 0
  );

  let statusColor = $derived(
    download.status === "downloading" ? "var(--accent-color)" :
    download.status === "completed" ? "var(--color-success)" :
    download.status === "failed" ? "var(--color-error)" :
    download.status === "paused" ? "var(--color-warning)" :
    "var(--text-secondary-color)"
  );

  let isActive = $derived(download.status === "downloading");

  async function handlePause() { await pauseDownload(download.id); }
  async function handleResume() { await resumeDownload(download.id); }
  async function handleDelete() { await deleteDownload(download.id); }
</script>

<div
  class="rounded-xl p-4 transition-all interactive card-enter"
  style="background: var(--surface-2-color); border: 1px solid var(--border-color); --i: {index}"
>
  <div class="flex items-start justify-between gap-3 mb-3">
    <div class="min-w-0 flex-1">
      <div class="flex items-center gap-2">
        {#if isActive}
          <span class="w-2 h-2 rounded-full shrink-0 status-pulse" style="background: var(--accent-color)"></span>
        {/if}
        <h3 class="font-semibold truncate">{download.filename || download.url}</h3>
      </div>
      <p class="text-xs truncate mt-0.5" style="color: var(--text-secondary-color)">
        {download.url}
      </p>
    </div>
    <span
      class="px-2 py-0.5 rounded-full text-xs font-semibold uppercase shrink-0 transition-all"
      style="color: {statusColor}; background: color-mix(in srgb, {statusColor} 15%, transparent)"
    >
      {download.status}
    </span>
  </div>

  <!-- Chunk visualization for active downloads -->
  {#if isActive}
    <div class="mb-2">
      <ChunkViz chunks={8} {progress} active={true} />
    </div>
  {:else}
    <!-- Standard progress bar -->
    <div class="progress-bar mb-2">
      <div
        class="progress-bar-fill"
        class:active={isActive}
        style="width: {progress}%"
      ></div>
    </div>
  {/if}

  <div class="flex items-center justify-between text-xs" style="color: var(--text-secondary-color)">
    <div class="flex gap-4">
      <span>{formatBytes(download.bytes_downloaded)}{download.filesize ? ` / ${formatBytes(download.filesize)}` : ""}</span>
      {#if isActive && download.speed > 0}
        <span class="font-mono font-semibold" style="color: var(--accent-color)">{formatSpeed(download.speed)}</span>
      {/if}
      {#if progress > 0}
        <span>{progress}%</span>
      {/if}
    </div>

    <!-- Actions -->
    <div class="flex gap-1">
      {#if download.status === "downloading"}
        <button onclick={handlePause} class="px-2.5 py-1 rounded-lg text-xs font-medium transition-all hover:scale-105" style="background: var(--surface-3-color)" title="Pause">
          ⏸
        </button>
      {:else if download.status === "paused" || download.status === "queued"}
        <button onclick={handleResume} class="px-2.5 py-1 rounded-lg text-xs font-medium transition-all hover:scale-105" style="background: var(--surface-3-color)" title="Resume">
          ▶
        </button>
      {/if}
      <button onclick={handleDelete} class="px-2.5 py-1 rounded-lg text-xs font-medium transition-all hover:scale-105 hover:bg-red-500/20 text-red-400" style="background: var(--surface-3-color)" title="Remove">
        ✕
      </button>
    </div>
  </div>
</div>
