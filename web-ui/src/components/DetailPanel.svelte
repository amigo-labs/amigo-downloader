<script lang="ts">
  import { onDestroy } from "svelte";
  import { selectedDownload, closeSidePanel, crashReport, showFeedbackDialog } from "../lib/stores";
  import { formatBytes, formatSpeed, pauseDownload, resumeDownload, retryDownload, deleteDownload } from "../lib/api";
  import { addToast } from "../lib/toast";
  import { locale, tr } from "../lib/i18n";
  import ChunkViz from "@amigo/ui/components/ChunkViz.svelte";
  import Icon from "@amigo/ui/components/Icon.svelte";

  let dl = $derived($selectedDownload);
  let confirmingDelete = $state(false);
  let confirmTimer: ReturnType<typeof setTimeout> | undefined;
  // Guards against rapid re-clicks firing the same request multiple times.
  let busy = $state(false);

  onDestroy(() => clearTimeout(confirmTimer));

  async function runAction(action: () => Promise<unknown>) {
    if (busy) return;
    busy = true;
    try {
      await action();
    } catch {
      addToast("error", tr($locale, "toast.action_failed"));
    } finally {
      busy = false;
    }
  }

  const KNOWN_STATUS = ["downloading", "completed", "failed", "paused", "queued"];
  let statusLabel = $derived(
    dl && KNOWN_STATUS.includes(dl.status) ? tr($locale, `status.${dl.status}`) : dl?.status
  );

  let progress = $derived(
    dl?.filesize ? Math.round((dl.bytes_downloaded / dl.filesize) * 100) : 0
  );

  let eta = $derived(() => {
    if (!dl || dl.status !== "downloading" || !dl.speed || dl.speed <= 0 || !dl.filesize) return "";
    const remaining = dl.filesize - dl.bytes_downloaded;
    if (remaining <= 0) return "";
    const secs = Math.round(remaining / dl.speed);
    if (secs < 60) return `${secs}s`;
    if (secs < 3600) return `${Math.floor(secs / 60)}m ${secs % 60}s`;
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    return `${h}h ${m}m`;
  });

  async function copyUrl() {
    if (!dl) return;
    try {
      await navigator.clipboard.writeText(dl.url);
      addToast("info", tr($locale, "action.copy_url"));
    } catch {
      addToast("error", tr($locale, "action.copy_failed"));
    }
  }
</script>

{#if dl}
  <div class="p-4 space-y-5">
    <!-- Status -->
    <div>
      <span
        class="px-2.5 py-1 rounded-full text-xs font-semibold uppercase"
        style="color: {dl.status === 'downloading' ? 'var(--neon-primary)' : dl.status === 'completed' ? 'var(--neon-success)' : dl.status === 'failed' ? 'var(--neon-accent)' : dl.status === 'paused' ? 'var(--neon-warning)' : 'var(--text-secondary)'}; background: color-mix(in srgb, {dl.status === 'downloading' ? 'var(--neon-primary)' : dl.status === 'completed' ? 'var(--neon-success)' : dl.status === 'failed' ? 'var(--neon-accent)' : dl.status === 'paused' ? 'var(--neon-warning)' : 'var(--text-secondary)'} 10%, transparent)"
      >
        {statusLabel}
      </span>
    </div>

    <!-- File Info -->
    <section>
      <h4 class="text-xs font-semibold uppercase mb-2" style="color: var(--text-secondary)">{tr($locale, "detail.file_info")}</h4>
      <div class="space-y-2 text-sm">
        <div>
          <div class="flex items-center justify-between">
            <span style="color: var(--text-secondary)">URL</span>
            <button onclick={copyUrl} class="icon-btn p-1 rounded" style="color: var(--text-secondary)" aria-label={tr($locale, "action.copy")}>
              <Icon name="copy" size={12} />
            </button>
          </div>
          <p class="truncate mt-0.5" style="font-family: var(--font-mono);font-size: 11px; color: var(--text-primary)">{dl.url}</p>
        </div>
        <div class="flex justify-between">
          <span style="color: var(--text-secondary)">{tr($locale, "detail.protocol")}</span>
          <span class="uppercase text-xs font-semibold" style="color: var(--text-primary)">{dl.protocol}</span>
        </div>
        {#if dl.filesize}
          <div class="flex justify-between">
            <span style="color: var(--text-secondary)">{tr($locale, "detail.size")}</span>
            <span style="font-family: var(--font-mono);color: var(--text-primary)">{formatBytes(dl.filesize)}</span>
          </div>
        {/if}
        <div class="flex justify-between">
          <span style="color: var(--text-secondary)">{tr($locale, "detail.progress")}</span>
          <span style="font-family: var(--font-mono);color: var(--neon-primary)">{progress}%</span>
        </div>
      </div>
    </section>

    <!-- Chunk visualization -->
    {#if dl.status === "downloading"}
      <section>
        <h4 class="text-xs font-semibold uppercase mb-2" style="color: var(--text-secondary)">{tr($locale, "detail.chunks")}</h4>
        <ChunkViz chunks={8} {progress} active={true} size="detailed" />
      </section>
    {:else if dl.status === "paused" && progress > 0}
      <section>
        <h4 class="text-xs font-semibold uppercase mb-2" style="color: var(--text-secondary)">{tr($locale, "detail.chunks_paused")}</h4>
        <div style="opacity: 0.5">
          <ChunkViz chunks={8} {progress} active={false} size="detailed" />
        </div>
      </section>
    {/if}

    <!-- Speed + ETA -->
    {#if dl.status === "downloading" && dl.speed > 0}
      <section>
        <h4 class="text-xs font-semibold uppercase mb-2" style="color: var(--text-secondary)">{tr($locale, "detail.speed")}</h4>
        <div class="flex items-baseline gap-3">
          <span class="text-lg font-bold" style="font-family: var(--font-mono);color: var(--neon-primary)">{formatSpeed(dl.speed)}</span>
          {#if eta()}
            <span class="text-xs" style="font-family: var(--font-mono);color: var(--text-secondary)">ETA {eta()}</span>
          {/if}
        </div>
      </section>
    {/if}

    <!-- Error -->
    {#if dl.error}
      <section>
        <h4 class="text-xs font-semibold uppercase mb-2" style="color: var(--neon-accent)">{tr($locale, "detail.error")}</h4>
        <p class="text-xs p-3 rounded-lg" style="font-family: var(--font-mono);background: var(--bg-surface-2); color: var(--neon-accent)">{dl.error}</p>
      </section>
    {/if}

    <!-- Actions -->
    <section>
      <h4 class="text-xs font-semibold uppercase mb-2" style="color: var(--text-secondary)">{tr($locale, "detail.actions")}</h4>
      <div class="flex gap-2">
        {#if dl.status === "downloading"}
          <button
            onclick={() => runAction(() => pauseDownload(dl.id))}
            disabled={busy}
            class="action-btn flex items-center gap-1.5 px-3 py-2 rounded-lg text-xs font-semibold min-h-[44px] disabled:opacity-50"
            style="background: var(--bg-surface-2); color: var(--neon-warning)"
            aria-label={tr($locale, "action.pause")}
          >
            <Icon name="pause" size={14} /> {tr($locale, "action.pause")}
          </button>
        {:else if dl.status === "paused" || dl.status === "queued"}
          <button
            onclick={() => runAction(() => resumeDownload(dl.id))}
            disabled={busy}
            class="action-btn flex items-center gap-1.5 px-3 py-2 rounded-lg text-xs font-semibold min-h-[44px] disabled:opacity-50"
            style="background: var(--bg-surface-2); color: var(--neon-primary)"
            aria-label={tr($locale, "action.resume")}
          >
            <Icon name="play" size={14} /> {tr($locale, "action.resume")}
          </button>
        {/if}
        {#if dl.status === "failed"}
          <button
            onclick={() => runAction(() => retryDownload(dl.id))}
            disabled={busy}
            class="action-btn flex items-center gap-1.5 px-3 py-2 rounded-lg text-xs font-semibold min-h-[44px] disabled:opacity-50"
            style="background: var(--bg-surface-2); color: var(--neon-primary)"
            aria-label={tr($locale, "action.retry")}
          >
            <Icon name="refresh" size={14} /> {tr($locale, "action.retry")}
          </button>
          {#if dl.error}
            <button
              onclick={() => { crashReport.set({ download_id: dl.id, error_message: dl.error ?? undefined }); showFeedbackDialog.set(true); }}
              class="action-btn flex items-center gap-1.5 px-3 py-2 rounded-lg text-xs font-semibold min-h-[44px]"
              style="background: var(--bg-surface-2); color: var(--neon-warning)"
              aria-label={tr($locale, "action.report")}
            >
              <Icon name="flag" size={14} /> {tr($locale, "action.report")}
            </button>
          {/if}
        {/if}
        <button
          onclick={() => {
            if (!confirmingDelete) {
              confirmingDelete = true;
              confirmTimer = setTimeout(() => { confirmingDelete = false; }, 2000);
            } else {
              clearTimeout(confirmTimer);
              confirmingDelete = false;
              runAction(async () => { await deleteDownload(dl.id); closeSidePanel(); });
            }
          }}
          disabled={busy}
          class="action-btn flex items-center gap-1.5 px-3 py-2 rounded-lg text-xs font-semibold min-h-[44px] disabled:opacity-50"
          style="background: var(--bg-surface-2); color: var(--neon-accent)"
          aria-label={tr($locale, "action.delete")}
        >
          <Icon name="trash" size={14} /> {confirmingDelete ? tr($locale, "action.sure") : tr($locale, "action.delete")}
        </button>
      </div>
    </section>
  </div>
{/if}
