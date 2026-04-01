<script lang="ts">
  import { pauseDownload, resumeDownload, deleteDownload, formatBytes, formatSpeed } from "../lib/api";
  import { openDetailPanel, selectedDownloadId, crashReport } from "../lib/stores";
  import ChunkViz from "./ChunkViz.svelte";
  import Icon from "./Icon.svelte";

  let { download, index = 0 }: { download: any; index?: number } = $props();

  let progress = $derived(
    download.filesize ? Math.round((download.bytes_downloaded / download.filesize) * 100) : 0
  );

  // Status colors — neon accent only for active state (10% rule)
  let statusColor = $derived(
    download.status === "downloading" ? "var(--neon-primary)" :
    download.status === "completed" ? "var(--neon-success)" :
    download.status === "failed" ? "var(--neon-accent)" :
    download.status === "paused" ? "var(--neon-warning)" :
    "var(--text-secondary)"
  );

  let isActive = $derived(download.status === "downloading");
  let isSelected = $derived($selectedDownloadId === download.id);

  async function handlePause() { await pauseDownload(download.id); }
  async function handleResume() { await resumeDownload(download.id); }
  async function handleDelete() { await deleteDownload(download.id); }

  function select() {
    openDetailPanel(download.id);
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
  class="rounded-xl p-4 card-enter cursor-pointer flex gap-3 transition-colors"
  style="
    background: var(--bg-surface);
    border: 1px solid {isSelected ? statusColor : 'var(--border-color)'};
    --i: {index};
  "
  onclick={select}
  role="button"
  tabindex="0"
>
  <!-- Drag handle -->
  <div class="flex items-center shrink-0 cursor-grab" style="color: var(--text-secondary); width: 16px">
    <Icon name="grip" size={14} />
  </div>

  <!-- Left accent bar for active downloads -->
  {#if isActive}
    <div class="w-0.5 self-stretch rounded-full shrink-0" style="background: var(--neon-primary)"></div>
  {/if}

  <!-- Content -->
  <div class="flex-1 min-w-0">
    <div class="flex items-start justify-between gap-3 mb-2">
      <div class="min-w-0 flex-1">
        <div class="flex items-center gap-2">
          {#if isActive}
            <span class="w-1.5 h-1.5 rounded-full shrink-0 status-pulse" style="background: var(--neon-primary)"></span>
          {/if}
          <h3 class="font-semibold truncate text-sm" style="color: var(--text-primary)">{download.filename || download.url}</h3>
        </div>
        <p class="text-xs truncate mt-0.5" style="font-family: var(--font-mono);color: var(--text-secondary); font-size: 11px">
          {download.url}
        </p>
      </div>
      <span
        class="px-2 py-0.5 rounded-full text-[10px] font-semibold uppercase shrink-0"
        style="color: {statusColor}; background: color-mix(in srgb, {statusColor} 10%, transparent)"
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
      <div class="progress-bar mb-2">
        <div
          class="progress-bar-fill"
          class:active={isActive}
          style="width: {progress}%"
        ></div>
      </div>
    {/if}

    <div class="flex items-center justify-between text-xs" style="color: var(--text-secondary)">
      <div class="flex gap-3" style="font-family: var(--font-mono);font-size: 11px">
        <span>{formatBytes(download.bytes_downloaded)}{download.filesize ? ` / ${formatBytes(download.filesize)}` : ""}</span>
        {#if isActive && download.speed > 0}
          <span style="color: var(--neon-primary)">{formatSpeed(download.speed)}</span>
        {/if}
        {#if progress > 0}
          <span>{progress}%</span>
        {/if}
      </div>

      <!-- Actions — 44px min touch targets (audit H1) -->
      <div class="flex gap-1 items-center" onclick={(e) => e.stopPropagation()}>
        {#if download.status === "failed" && download.error}
          <button
            onclick={() => crashReport.set({ download_id: download.id, error_message: download.error })}
            class="min-w-[44px] min-h-[44px] flex items-center justify-center rounded-lg text-[10px] font-medium transition-colors"
            style="color: var(--neon-warning)"
            aria-label="Report error for {download.filename || 'download'}"
          >
            <Icon name="flag" size={14} />
          </button>
        {/if}
        {#if download.status === "downloading"}
          <button onclick={handlePause} class="min-w-[44px] min-h-[44px] flex items-center justify-center rounded-lg transition-colors" style="color: var(--text-secondary)" aria-label="Pause {download.filename || 'download'}">
            <Icon name="pause" size={16} />
          </button>
        {:else if download.status === "paused" || download.status === "queued"}
          <button onclick={handleResume} class="min-w-[44px] min-h-[44px] flex items-center justify-center rounded-lg transition-colors" style="color: var(--text-secondary)" aria-label="Resume {download.filename || 'download'}">
            <Icon name="play" size={16} />
          </button>
        {/if}
        <button onclick={handleDelete} class="min-w-[44px] min-h-[44px] flex items-center justify-center rounded-lg transition-colors" style="color: var(--neon-accent)" aria-label="Delete {download.filename || 'download'}">
          <Icon name="trash" size={16} />
        </button>
      </div>
    </div>
  </div>
</div>
