<script lang="ts">
  import { pauseDownload, resumeDownload, deleteDownload, formatBytes, formatSpeed } from "../lib/api";
  import { selectedDownloadId } from "../lib/stores";
  import Icon from "./Icon.svelte";

  let { download }: { download: any } = $props();

  let progress = $derived(
    download.filesize ? Math.round((download.bytes_downloaded / download.filesize) * 100) : 0
  );

  async function handlePause() { await pauseDownload(download.id); }
  async function handleResume() { await resumeDownload(download.id); }
  async function handleDelete() { await deleteDownload(download.id); }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<tr
  class="border-t cursor-pointer transition-colors"
  style="border-color: var(--border-color)"
  onclick={() => selectedDownloadId.set(download.id)}
  role="row"
>
  <td class="px-4 py-2.5">
    <div class="truncate max-w-xs font-medium text-sm" style="color: var(--text-primary)">{download.filename || download.url}</div>
  </td>
  <td class="px-4 py-2.5" style="font-family: 'Share Tech Mono', monospace; font-size: 12px; color: var(--text-secondary)">
    {download.filesize ? formatBytes(download.filesize) : "\u2014"}
  </td>
  <td class="px-4 py-2.5">
    <span class="text-xs font-semibold uppercase" style="color: {download.status === 'downloading' ? 'var(--neon-primary)' : download.status === 'completed' ? 'var(--neon-success)' : download.status === 'failed' ? 'var(--neon-accent)' : 'var(--text-secondary)'}">{download.status}</span>
  </td>
  <td class="px-4 py-2.5">
    <div class="flex items-center gap-2">
      <div class="progress-bar flex-1">
        <div class="progress-bar-fill" style="width: {progress}%"></div>
      </div>
      <span class="text-xs w-10 text-right" style="font-family: 'Share Tech Mono', monospace; color: var(--text-secondary)">{progress}%</span>
    </div>
  </td>
  <td class="px-4 py-2.5" style="font-family: 'Share Tech Mono', monospace; font-size: 12px; color: var(--neon-primary)">
    {download.status === "downloading" ? formatSpeed(download.speed) : "\u2014"}
  </td>
  <td class="px-4 py-2.5 text-right">
    <div class="flex justify-end gap-1" onclick={(e) => e.stopPropagation()}>
      {#if download.status === "downloading"}
        <button onclick={handlePause} class="min-w-[44px] min-h-[44px] flex items-center justify-center rounded-lg" style="color: var(--text-secondary)" aria-label="Pause download">
          <Icon name="pause" size={14} />
        </button>
      {:else if download.status === "paused" || download.status === "queued"}
        <button onclick={handleResume} class="min-w-[44px] min-h-[44px] flex items-center justify-center rounded-lg" style="color: var(--text-secondary)" aria-label="Resume download">
          <Icon name="play" size={14} />
        </button>
      {/if}
      <button onclick={handleDelete} class="min-w-[44px] min-h-[44px] flex items-center justify-center rounded-lg" style="color: var(--neon-accent)" aria-label="Delete download">
        <Icon name="trash" size={14} />
      </button>
    </div>
  </td>
</tr>
