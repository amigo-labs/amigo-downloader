<script lang="ts">
  import { selectedDownload, closeSidePanel, crashReport } from "../lib/stores";
  import { formatBytes, formatSpeed, pauseDownload, resumeDownload, deleteDownload } from "../lib/api";
  import ChunkViz from "./ChunkViz.svelte";
  import Icon from "./Icon.svelte";

  let dl = $derived($selectedDownload);

  let progress = $derived(
    dl?.filesize ? Math.round((dl.bytes_downloaded / dl.filesize) * 100) : 0
  );
</script>

{#if dl}
  <div class="p-4 space-y-5">
    <!-- Status -->
    <div>
      <span
        class="px-2.5 py-1 rounded-full text-xs font-semibold uppercase"
        style="color: {dl.status === 'downloading' ? 'var(--neon-primary)' : dl.status === 'completed' ? 'var(--neon-success)' : dl.status === 'failed' ? 'var(--neon-accent)' : dl.status === 'paused' ? 'var(--neon-warning)' : 'var(--text-secondary)'}; background: color-mix(in srgb, {dl.status === 'downloading' ? 'var(--neon-primary)' : dl.status === 'completed' ? 'var(--neon-success)' : dl.status === 'failed' ? 'var(--neon-accent)' : dl.status === 'paused' ? 'var(--neon-warning)' : 'var(--text-secondary)'} 10%, transparent)"
      >
        {dl.status}
      </span>
    </div>

    <!-- File Info -->
    <section>
      <h4 class="text-xs font-semibold uppercase mb-2" style="color: var(--text-secondary)">File Info</h4>
      <div class="space-y-2 text-sm">
        <div>
          <span style="color: var(--text-secondary)">URL</span>
          <p class="truncate mt-0.5" style="font-family: var(--font-mono);font-size: 11px; color: var(--text-primary)">{dl.url}</p>
        </div>
        <div class="flex justify-between">
          <span style="color: var(--text-secondary)">Protocol</span>
          <span class="uppercase text-xs font-semibold" style="color: var(--text-primary)">{dl.protocol}</span>
        </div>
        {#if dl.filesize}
          <div class="flex justify-between">
            <span style="color: var(--text-secondary)">Size</span>
            <span style="font-family: var(--font-mono);color: var(--text-primary)">{formatBytes(dl.filesize)}</span>
          </div>
        {/if}
        <div class="flex justify-between">
          <span style="color: var(--text-secondary)">Progress</span>
          <span style="font-family: var(--font-mono);color: var(--neon-primary)">{progress}%</span>
        </div>
      </div>
    </section>

    <!-- Chunk visualization -->
    {#if dl.status === "downloading"}
      <section>
        <h4 class="text-xs font-semibold uppercase mb-2" style="color: var(--text-secondary)">Chunks</h4>
        <ChunkViz chunks={8} {progress} active={true} size="detailed" />
      </section>
    {/if}

    <!-- Speed -->
    {#if dl.status === "downloading" && dl.speed > 0}
      <section>
        <h4 class="text-xs font-semibold uppercase mb-2" style="color: var(--text-secondary)">Speed</h4>
        <span class="text-lg font-bold" style="font-family: var(--font-mono);color: var(--neon-primary)">{formatSpeed(dl.speed)}</span>
      </section>
    {/if}

    <!-- Error -->
    {#if dl.error}
      <section>
        <h4 class="text-xs font-semibold uppercase mb-2" style="color: var(--neon-accent)">Error</h4>
        <p class="text-xs p-3 rounded-lg" style="font-family: var(--font-mono);background: var(--bg-surface-2); color: var(--neon-accent)">{dl.error}</p>
      </section>
    {/if}

    <!-- Actions -->
    <section>
      <h4 class="text-xs font-semibold uppercase mb-2" style="color: var(--text-secondary)">Actions</h4>
      <div class="flex gap-2">
        {#if dl.status === "downloading"}
          <button
            onclick={() => pauseDownload(dl.id)}
            class="action-btn flex items-center gap-1.5 px-3 py-2 rounded-lg text-xs font-semibold min-h-[44px]"
            style="background: var(--bg-surface-2); color: var(--neon-warning)"
            aria-label="Pause download"
          >
            <Icon name="pause" size={14} /> Pause
          </button>
        {:else if dl.status === "paused" || dl.status === "queued"}
          <button
            onclick={() => resumeDownload(dl.id)}
            class="action-btn flex items-center gap-1.5 px-3 py-2 rounded-lg text-xs font-semibold min-h-[44px]"
            style="background: var(--bg-surface-2); color: var(--neon-primary)"
            aria-label="Resume download"
          >
            <Icon name="play" size={14} /> Resume
          </button>
        {/if}
        {#if dl.status === "failed" && dl.error}
          <button
            onclick={() => crashReport.set({ download_id: dl.id, error_message: dl.error ?? undefined })}
            class="action-btn flex items-center gap-1.5 px-3 py-2 rounded-lg text-xs font-semibold min-h-[44px]"
            style="background: var(--bg-surface-2); color: var(--neon-warning)"
            aria-label="Report error"
          >
            <Icon name="flag" size={14} /> Report
          </button>
        {/if}
        <button
          onclick={() => { deleteDownload(dl.id); closeSidePanel(); }}
          class="action-btn flex items-center gap-1.5 px-3 py-2 rounded-lg text-xs font-semibold min-h-[44px]"
          style="background: var(--bg-surface-2); color: var(--neon-accent)"
          aria-label="Delete download"
        >
          <Icon name="trash" size={14} /> Delete
        </button>
      </div>
    </section>
  </div>
{/if}
