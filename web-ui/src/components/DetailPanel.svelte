<script lang="ts">
  import { selectedDownload, selectedDownloadId } from "../lib/stores";
  import { formatBytes, formatSpeed, pauseDownload, resumeDownload, deleteDownload } from "../lib/api";
  import Icon from "./Icon.svelte";
  import ChunkViz from "./ChunkViz.svelte";

  let dl = $derived($selectedDownload);
  let isOpen = $derived(dl !== null);

  let progress = $derived(
    dl?.filesize ? Math.round((dl.bytes_downloaded / dl.filesize) * 100) : 0
  );

  function close() {
    selectedDownloadId.set(null);
  }
</script>

{#if isOpen && dl}
  <!-- Desktop panel -->
  <div
    role="complementary"
    aria-label="Download details"
    class="hidden md:flex flex-col w-80 shrink-0 border-l overflow-y-auto"
    style="background: var(--bg-surface); border-color: var(--border-color)"
  >
    <!-- Header -->
    <div class="flex items-center justify-between px-4 py-3 border-b" style="border-color: var(--border-color)">
      <h3 class="font-semibold text-sm truncate flex-1" style="color: var(--text-primary)">{dl.filename || "Download"}</h3>
      <button
        onclick={close}
        class="p-1.5 rounded-lg transition-colors min-w-[44px] min-h-[44px] flex items-center justify-center"
        style="color: var(--text-secondary)"
        aria-label="Close detail panel"
      >
        <Icon name="x" size={16} />
      </button>
    </div>

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
            <p class="truncate mt-0.5" style="font-family: 'Share Tech Mono', monospace; font-size: 11px; color: var(--text-primary)">{dl.url}</p>
          </div>
          <div class="flex justify-between">
            <span style="color: var(--text-secondary)">Protocol</span>
            <span class="uppercase text-xs font-semibold" style="color: var(--text-primary)">{dl.protocol}</span>
          </div>
          {#if dl.filesize}
            <div class="flex justify-between">
              <span style="color: var(--text-secondary)">Size</span>
              <span style="font-family: 'Share Tech Mono', monospace; color: var(--text-primary)">{formatBytes(dl.filesize)}</span>
            </div>
          {/if}
          <div class="flex justify-between">
            <span style="color: var(--text-secondary)">Progress</span>
            <span style="font-family: 'Share Tech Mono', monospace; color: var(--neon-primary)">{progress}%</span>
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
          <span class="text-lg font-bold" style="font-family: 'Share Tech Mono', monospace; color: var(--neon-primary)">{formatSpeed(dl.speed)}</span>
        </section>
      {/if}

      <!-- Error -->
      {#if dl.error}
        <section>
          <h4 class="text-xs font-semibold uppercase mb-2" style="color: var(--neon-accent)">Error</h4>
          <p class="text-xs p-3 rounded-lg" style="font-family: 'Share Tech Mono', monospace; background: var(--bg-surface-2); color: var(--neon-accent)">{dl.error}</p>
        </section>
      {/if}

      <!-- Actions -->
      <section>
        <h4 class="text-xs font-semibold uppercase mb-2" style="color: var(--text-secondary)">Actions</h4>
        <div class="flex gap-2">
          {#if dl.status === "downloading"}
            <button
              onclick={() => pauseDownload(dl.id)}
              class="flex items-center gap-1.5 px-3 py-2 rounded-lg text-xs font-semibold min-h-[44px] transition-colors"
              style="background: var(--bg-surface-2); color: var(--neon-warning)"
              aria-label="Pause download"
            >
              <Icon name="pause" size={14} /> Pause
            </button>
          {:else if dl.status === "paused" || dl.status === "queued"}
            <button
              onclick={() => resumeDownload(dl.id)}
              class="flex items-center gap-1.5 px-3 py-2 rounded-lg text-xs font-semibold min-h-[44px] transition-colors"
              style="background: var(--bg-surface-2); color: var(--neon-primary)"
              aria-label="Resume download"
            >
              <Icon name="play" size={14} /> Resume
            </button>
          {/if}
          <button
            onclick={() => { deleteDownload(dl.id); close(); }}
            class="flex items-center gap-1.5 px-3 py-2 rounded-lg text-xs font-semibold min-h-[44px] transition-colors"
            style="background: var(--bg-surface-2); color: var(--neon-accent)"
            aria-label="Delete download"
          >
            <Icon name="trash" size={14} /> Delete
          </button>
        </div>
      </section>
    </div>
  </div>

  <!-- Mobile: full overlay -->
  <div class="fixed inset-0 z-50 md:hidden">
    <div class="absolute inset-0 bg-black/60" onclick={close}></div>
    <div
      role="complementary"
      aria-label="Download details"
      class="absolute right-0 top-0 bottom-0 w-full max-w-sm flex flex-col overflow-y-auto"
      style="background: var(--bg-surface)"
    >
      <div class="flex items-center justify-between px-4 py-3 border-b" style="border-color: var(--border-color)">
        <h3 class="font-semibold text-sm truncate flex-1">{dl.filename || "Download"}</h3>
        <button
          onclick={close}
          class="p-2 rounded-lg min-w-[44px] min-h-[44px] flex items-center justify-center"
          style="color: var(--text-secondary)"
          aria-label="Close detail panel"
        >
          <Icon name="x" size={18} />
        </button>
      </div>
      <div class="p-4 text-sm" style="color: var(--text-secondary)">
        <p>URL: {dl.url}</p>
        <p class="mt-2">Status: {dl.status}</p>
        <p class="mt-2">Progress: {progress}%</p>
        {#if dl.speed > 0}
          <p class="mt-2">Speed: {formatSpeed(dl.speed)}</p>
        {/if}
        {#if dl.error}
          <p class="mt-2" style="color: var(--neon-accent)">Error: {dl.error}</p>
        {/if}
      </div>
    </div>
  </div>
{/if}
