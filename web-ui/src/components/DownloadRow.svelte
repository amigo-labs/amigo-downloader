<script lang="ts">
  import { pauseDownload, resumeDownload, deleteDownload, formatBytes, formatSpeed } from "../lib/api";

  let { download }: { download: any } = $props();

  let progress = $derived(
    download.filesize ? Math.round((download.bytes_downloaded / download.filesize) * 100) : 0
  );

  async function handlePause() { await pauseDownload(download.id); }
  async function handleResume() { await resumeDownload(download.id); }
  async function handleDelete() { await deleteDownload(download.id); }
</script>

<tr class="border-t transition-colors hover:brightness-95" style="border-color: var(--border-color)">
  <td class="px-4 py-2.5">
    <div class="truncate max-w-xs font-medium">{download.filename || download.url}</div>
  </td>
  <td class="px-4 py-2.5 font-mono text-xs" style="color: var(--text-secondary-color)">
    {download.filesize ? formatBytes(download.filesize) : "—"}
  </td>
  <td class="px-4 py-2.5">
    <span class="text-xs font-semibold uppercase">{download.status}</span>
  </td>
  <td class="px-4 py-2.5">
    <div class="flex items-center gap-2">
      <div class="progress-bar flex-1">
        <div class="progress-bar-fill" style="width: {progress}%"></div>
      </div>
      <span class="text-xs font-mono w-10 text-right">{progress}%</span>
    </div>
  </td>
  <td class="px-4 py-2.5 font-mono text-xs" style="color: var(--accent-color)">
    {download.status === "downloading" ? formatSpeed(download.speed) : "—"}
  </td>
  <td class="px-4 py-2.5 text-right">
    <div class="flex justify-end gap-1">
      {#if download.status === "downloading"}
        <button onclick={handlePause} class="px-2 py-1 rounded text-xs hover:bg-black/10" title="Pause">||</button>
      {:else if download.status === "paused" || download.status === "queued"}
        <button onclick={handleResume} class="px-2 py-1 rounded text-xs hover:bg-black/10" title="Resume">></button>
      {/if}
      <button onclick={handleDelete} class="px-2 py-1 rounded text-xs hover:bg-red-500/20 text-red-400" title="Remove">x</button>
    </div>
  </td>
</tr>
