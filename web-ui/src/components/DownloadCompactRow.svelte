<script lang="ts">
  import { onDestroy } from "svelte";
  import { pauseDownload, resumeDownload, retryDownload, deleteDownload, formatBytes, formatSpeed } from "../lib/api";
  import { openDetailPanel, selectedIds, toggleSelection } from "../lib/stores";
  import Icon from "@amigo/ui/components/Icon.svelte";

  let { download }: { download: any } = $props();

  function fileIcon(filename: string | null): string {
    if (!filename) return "file";
    const ext = filename.split(".").pop()?.toLowerCase() ?? "";
    if (["zip", "rar", "7z", "tar", "gz", "bz2"].includes(ext)) return "archive";
    if (["mp4", "mkv", "avi", "mov", "webm", "flv"].includes(ext)) return "video";
    if (["mp3", "flac", "ogg", "wav", "aac", "m4a"].includes(ext)) return "music";
    if (["pdf", "doc", "docx", "txt", "rtf", "odt"].includes(ext)) return "file-text";
    if (["jpg", "jpeg", "png", "gif", "svg", "webp", "bmp"].includes(ext)) return "image";
    return "file";
  }

  let progress = $derived(
    download.filesize ? Math.round((download.bytes_downloaded / download.filesize) * 100) : 0
  );

  let statusColor = $derived(
    download.status === "downloading" ? "var(--neon-primary)" :
    download.status === "completed" ? "var(--neon-success)" :
    download.status === "failed" ? "var(--neon-accent)" :
    download.status === "paused" ? "var(--neon-warning)" :
    "var(--text-secondary)"
  );

  let isActive = $derived(download.status === "downloading");
  let batchMode = $derived($selectedIds.size > 0);
  let isBatchSelected = $derived($selectedIds.has(download.id));

  let confirmingDelete = $state(false);
  let confirmTimer: ReturnType<typeof setTimeout> | undefined;

  onDestroy(() => clearTimeout(confirmTimer));

  function handleDelete(e: MouseEvent) {
    e.stopPropagation();
    if (!confirmingDelete) {
      confirmingDelete = true;
      confirmTimer = setTimeout(() => { confirmingDelete = false; }, 2000);
    } else {
      clearTimeout(confirmTimer);
      confirmingDelete = false;
      deleteDownload(download.id);
    }
  }
</script>

<div
  class="download-card flex items-center gap-3 rounded-lg px-3 py-2 cursor-pointer"
  style="border: 1px solid var(--border-color)"
  onclick={() => openDetailPanel(download.id)}
  onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); openDetailPanel(download.id); } }}
  role="button"
  tabindex="0"
>
  <!-- Batch checkbox -->
  {#if batchMode}
    <div class="shrink-0" onclick={(e) => { e.stopPropagation(); toggleSelection(download.id); }}>
      <div
        class="w-3.5 h-3.5 rounded border-2 flex items-center justify-center"
        style={isBatchSelected
          ? "background: var(--neon-primary); border-color: var(--neon-primary)"
          : "border-color: var(--border-color)"}
      >
        {#if isBatchSelected}<Icon name="check" size={8} />{/if}
      </div>
    </div>
  {/if}

  <!-- Filename with type icon -->
  <span class="flex items-center gap-1.5 flex-1 truncate text-sm font-medium min-w-0" style="color: var(--text-primary)">
    <Icon name={fileIcon(download.filename)} size={14} />
    <span class="truncate">{download.filename || download.url}</span>
  </span>

  <!-- Progress bar (inline) -->
  <div class="w-20 shrink-0">
    <div class="progress-bar">
      <div class="progress-bar-fill" class:active={isActive} style="width: {progress}%"></div>
    </div>
  </div>

  <!-- Progress % -->
  <span class="w-10 text-right text-xs shrink-0" style="font-family: var(--font-mono); color: var(--text-secondary)">{progress}%</span>

  <!-- Speed -->
  <span class="w-20 text-right text-xs shrink-0" style="font-family: var(--font-mono); color: var(--neon-primary)">
    {isActive ? formatSpeed(download.speed) : "\u2014"}
  </span>

  <!-- Status -->
  <span
    class="w-16 text-center text-[10px] font-semibold uppercase shrink-0 px-1.5 py-0.5 rounded-full"
    style="color: {statusColor}; background: color-mix(in srgb, {statusColor} 10%, transparent)"
  >
    {download.status}
  </span>

  <!-- Actions -->
  <div class="flex gap-0.5 shrink-0" onclick={(e) => e.stopPropagation()}>
    {#if download.status === "downloading"}
      <button onclick={() => pauseDownload(download.id)} class="icon-btn p-2 rounded min-w-[36px] min-h-[36px] flex items-center justify-center" style="color: var(--text-secondary)" aria-label="Pause">
        <Icon name="pause" size={14} />
      </button>
    {:else if download.status === "paused" || download.status === "queued"}
      <button onclick={() => resumeDownload(download.id)} class="icon-btn p-2 rounded min-w-[36px] min-h-[36px] flex items-center justify-center" style="color: var(--text-secondary)" aria-label="Resume">
        <Icon name="play" size={14} />
      </button>
    {:else if download.status === "failed"}
      <button onclick={() => retryDownload(download.id)} class="icon-btn p-2 rounded min-w-[36px] min-h-[36px] flex items-center justify-center" style="color: var(--neon-primary)" aria-label="Retry">
        <Icon name="refresh" size={14} />
      </button>
    {/if}
    <button onclick={handleDelete} class="icon-btn p-2 rounded min-w-[36px] min-h-[36px] flex items-center justify-center" style="color: {confirmingDelete ? 'var(--neon-accent)' : 'var(--text-secondary)'}" aria-label="Delete">
      {#if confirmingDelete}
        <span class="text-[10px] font-semibold px-1">Sure?</span>
      {:else}
        <Icon name="trash" size={14} />
      {/if}
    </button>
  </div>
</div>
