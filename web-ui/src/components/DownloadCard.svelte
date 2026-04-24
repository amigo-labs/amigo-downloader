<script lang="ts">
  import { onDestroy } from "svelte";
  import { pauseDownload, resumeDownload, retryDownload, deleteDownload, formatBytes, formatSpeed } from "../lib/api";
  import { openDetailPanel, selectedDownloadId, selectedIds, toggleSelection, crashReport } from "../lib/stores";
  import { addToast } from "../lib/toast";
  import ChunkViz from "@amigo/ui/components/ChunkViz.svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import Icon from "@amigo/ui/components/Icon.svelte";

  let { download, index = 0, ondragstart, ondragover, ondrop }:
    { download: any; index?: number; ondragstart?: (e: DragEvent) => void; ondragover?: (e: DragEvent) => void; ondrop?: (e: DragEvent) => void } = $props();

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
  let batchMode = $derived($selectedIds.size > 0);
  let isBatchSelected = $derived($selectedIds.has(download.id));

  // ETA calculation
  let eta = $derived(() => {
    if (!isActive || !download.speed || download.speed <= 0 || !download.filesize) return "";
    const remaining = download.filesize - download.bytes_downloaded;
    if (remaining <= 0) return "";
    const secs = Math.round(remaining / download.speed);
    if (secs < 60) return `${secs}s`;
    if (secs < 3600) return `${Math.floor(secs / 60)}m ${secs % 60}s`;
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    return `${h}h ${m}m`;
  });

  let confirmingDelete = $state(false);
  let confirmTimer: ReturnType<typeof setTimeout> | undefined;

  onDestroy(() => clearTimeout(confirmTimer));

  async function handlePause() { await pauseDownload(download.id); }
  async function handleResume() { await resumeDownload(download.id); }
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

  function select() {
    openDetailPanel(download.id);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      select();
    }
  }

  // Context menu (desktop only)
  let contextMenu = $state<{ x: number; y: number } | null>(null);

  function handleContextMenu(e: MouseEvent) {
    // Only on non-touch devices
    if ("ontouchstart" in window) return;
    e.preventDefault();
    contextMenu = { x: e.clientX, y: e.clientY };
  }

  function getContextMenuItems() {
    const items: { label: string; icon: string; action: () => void; color?: string }[] = [
      { label: "Copy URL", icon: "copy", action: () => navigator.clipboard.writeText(download.url) },
      { label: "Open in Browser", icon: "globe", action: () => window.open(download.url, "_blank") },
    ];
    if (download.status === "downloading") {
      items.push({ label: "Pause", icon: "pause", action: () => pauseDownload(download.id) });
    } else if (download.status === "paused" || download.status === "queued") {
      items.push({ label: "Resume", icon: "play", action: () => resumeDownload(download.id) });
    } else if (download.status === "failed") {
      items.push({ label: "Retry", icon: "refresh", action: () => retryDownload(download.id) });
    }
    items.push({ label: "Delete", icon: "trash", action: () => deleteDownload(download.id), color: "var(--neon-accent)" });
    return items;
  }

  async function copyUrl(e: MouseEvent) {
    e.stopPropagation();
    try {
      await navigator.clipboard.writeText(download.url);
      addToast("info", "URL copied");
    } catch {
      addToast("error", "Failed to copy");
    }
  }
</script>

<div
  class="download-card rounded-xl p-4 card-enter cursor-pointer flex gap-3"
  style="
    border: 1px solid {isSelected ? statusColor : 'var(--border-color)'};
    --i: {index};
  "
  onclick={select}
  onkeydown={handleKeydown}
  oncontextmenu={handleContextMenu}
  ondragstart={ondragstart}
  ondragover={ondragover}
  ondrop={ondrop}
  draggable={!batchMode}
  role="button"
  tabindex="0"
>
  <!-- Drag handle / batch checkbox -->
  {#if batchMode}
    <div class="flex items-center shrink-0" style="width: 16px" onclick={(e) => { e.stopPropagation(); toggleSelection(download.id); }}>
      <div
        class="w-4 h-4 rounded border-2 flex items-center justify-center transition-colors"
        style={isBatchSelected
          ? "background: var(--neon-primary); border-color: var(--neon-primary)"
          : "border-color: var(--border-color)"}
      >
        {#if isBatchSelected}
          <Icon name="check" size={10} />
        {/if}
      </div>
    </div>
  {:else}
    <div class="flex items-center shrink-0 cursor-grab" style="color: var(--text-secondary); width: 16px">
      <Icon name="grip" size={14} />
    </div>
  {/if}

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
          <Icon name={fileIcon(download.filename)} size={14} />
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

    <!-- Chunk visualization for active/paused downloads, progress bar otherwise -->
    {#if isActive}
      <div class="mb-2">
        <ChunkViz chunks={8} {progress} active={true} />
      </div>
    {:else if download.status === "paused" && progress > 0}
      <div class="mb-2" style="opacity: 0.5">
        <ChunkViz chunks={8} {progress} active={false} />
      </div>
    {:else}
      <div class="progress-bar mb-2">
        <div
          class="progress-bar-fill"
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
        {#if eta()}
          <span style="color: var(--text-secondary)">{eta()}</span>
        {/if}
      </div>

      <!-- Actions — 44px min touch targets (audit H1) -->
      <div class="flex gap-1 items-center" onclick={(e) => e.stopPropagation()}>
        <button onclick={copyUrl} class="icon-btn min-w-[44px] min-h-[44px] flex items-center justify-center rounded-lg" style="color: var(--text-secondary)" aria-label="Copy URL">
          <Icon name="copy" size={14} />
        </button>
        {#if download.status === "failed"}
          <button onclick={(e: MouseEvent) => { e.stopPropagation(); retryDownload(download.id); }} class="icon-btn min-w-[44px] min-h-[44px] flex items-center justify-center rounded-lg" style="color: var(--neon-primary)" aria-label="Retry {download.filename || 'download'}">
            <Icon name="refresh" size={14} />
          </button>
          {#if download.error}
            <button
              onclick={() => crashReport.set({ download_id: download.id, error_message: download.error })}
              class="icon-btn min-w-[44px] min-h-[44px] flex items-center justify-center rounded-lg text-[10px] font-medium"
              style="color: var(--neon-warning)"
              aria-label="Report error for {download.filename || 'download'}"
            >
              <Icon name="flag" size={14} />
            </button>
          {/if}
        {/if}
        {#if download.status === "downloading"}
          <button onclick={handlePause} class="icon-btn min-w-[44px] min-h-[44px] flex items-center justify-center rounded-lg" style="color: var(--text-secondary)" aria-label="Pause {download.filename || 'download'}">
            <Icon name="pause" size={16} />
          </button>
        {:else if download.status === "paused" || download.status === "queued"}
          <button onclick={handleResume} class="icon-btn min-w-[44px] min-h-[44px] flex items-center justify-center rounded-lg" style="color: var(--text-secondary)" aria-label="Resume {download.filename || 'download'}">
            <Icon name="play" size={16} />
          </button>
        {/if}
        <button onclick={handleDelete} class="icon-btn min-w-[44px] min-h-[44px] flex items-center justify-center rounded-lg" style="color: var(--neon-accent)" aria-label="Delete {download.filename || 'download'}">
          {#if confirmingDelete}
            <span class="text-[10px] font-bold">Sure?</span>
          {:else}
            <Icon name="trash" size={16} />
          {/if}
        </button>
      </div>
    </div>
  </div>
</div>

{#if contextMenu}
  <ContextMenu x={contextMenu.x} y={contextMenu.y} items={getContextMenuItems()} onclose={() => (contextMenu = null)} />
{/if}
