<script lang="ts">
  import { selectedDownload, closeSidePanel } from "../lib/stores";
  import { formatBytes, formatSpeed } from "../lib/api";
  import { locale, tr } from "../lib/i18n";
  import {
    progressPct, statusInk, statusFill, statusTone, statusLabelKey,
    canPause, canResume, canRetry, etaSeconds, formatEta,
  } from "../lib/download";
  import { createDownloadActions } from "../lib/downloadActions.svelte";
  import ChunkViz from "@amigo/ui/components/ChunkViz.svelte";
  import ProgressBar from "@amigo/ui/components/ProgressBar.svelte";
  import Button from "@amigo/ui/components/Button.svelte";
  import IconButton from "@amigo/ui/components/IconButton.svelte";
  import EmptyState from "@amigo/ui/components/EmptyState.svelte";
  import Banner from "@amigo/ui/components/Banner.svelte";

  let dl = $derived($selectedDownload);

  // Shared controller instead of a third private copy of the busy guard,
  // confirm timer and toast wording. The panel also gains the Undo-on-delete
  // that only the card used to offer.
  const actions = createDownloadActions(
    () => dl!,
    () => $locale,
  );

  let progress = $derived(dl ? progressPct(dl) : 0);
  let eta = $derived(dl ? formatEta(etaSeconds(dl)) : "");
  let statusText = $derived(dl ? tr($locale, statusLabelKey(dl.status)) : "");

  async function deleteAndClose() {
    const wasConfirming = actions.confirmingDelete;
    await actions.requestDelete();
    if (wasConfirming) closeSidePanel();
  }
</script>

{#if dl}
  <div class="p-4 space-y-5">
    <div>
      <span
        class="px-2.5 py-1 rounded-full text-xs font-semibold uppercase"
        style="color: {statusInk(dl.status)};
               background: color-mix(in srgb, {statusFill(dl.status)} 12%, transparent)"
      >
        {statusText}
      </span>
    </div>

    <section>
      <h4 class="text-xs font-semibold uppercase mb-2" style="color: var(--text-secondary)">
        {tr($locale, "detail.file_info")}
      </h4>
      <div class="space-y-2 text-sm">
        <div>
          <div class="flex items-center justify-between gap-2">
            <span style="color: var(--text-secondary)">URL</span>
            <IconButton
              icon="copy"
              size="sm"
              label={tr($locale, "action.copy")}
              onclick={() => actions.copyUrl()}
            />
          </div>
          <p class="truncate mt-0.5" style="font-family: var(--font-mono); font-size: 11px; color: var(--text-primary)">
            {dl.url}
          </p>
        </div>
        <div class="flex justify-between">
          <span style="color: var(--text-secondary)">{tr($locale, "detail.protocol")}</span>
          <span class="uppercase text-xs font-semibold" style="color: var(--text-primary)">{dl.protocol}</span>
        </div>
        {#if dl.filesize}
          <div class="flex justify-between">
            <span style="color: var(--text-secondary)">{tr($locale, "detail.size")}</span>
            <span style="font-family: var(--font-mono); color: var(--text-primary)">{formatBytes(dl.filesize)}</span>
          </div>
        {/if}
        <div class="flex justify-between">
          <span style="color: var(--text-secondary)">{tr($locale, "detail.progress")}</span>
          <span style="font-family: var(--font-mono); color: var(--accent-ink)">{progress}%</span>
        </div>
        <ProgressBar
          value={progress}
          tone={statusTone(dl.status)}
          active={dl.status === "downloading"}
          size="md"
          label={tr($locale, "detail.progress")}
          valueText="{progress}%"
        />
      </div>
    </section>

    {#if dl.status === "downloading" || (dl.status === "paused" && progress > 0)}
      <section>
        <h4 class="text-xs font-semibold uppercase mb-2" style="color: var(--text-secondary)">
          {tr($locale, dl.status === "downloading" ? "detail.chunks" : "detail.chunks_paused")}
        </h4>
        <ChunkViz
          chunks={8}
          {progress}
          active={dl.status === "downloading"}
          size="detailed"
          label={tr($locale, "detail.chunks")}
        />
      </section>
    {/if}

    {#if dl.status === "downloading" && dl.speed > 0}
      <section>
        <h4 class="text-xs font-semibold uppercase mb-2" style="color: var(--text-secondary)">
          {tr($locale, "detail.speed")}
        </h4>
        <div class="flex items-baseline gap-3">
          <span class="text-lg font-bold" style="font-family: var(--font-mono); color: var(--accent-ink)">
            {formatSpeed(dl.speed)}
          </span>
          {#if eta}
            <span class="text-xs" style="font-family: var(--font-mono); color: var(--text-secondary)">ETA {eta}</span>
          {/if}
        </div>
      </section>
    {/if}

    {#if dl.error}
      <Banner tone="danger" role="alert" title={tr($locale, "detail.error")}>
        <span style="font-family: var(--font-mono)">{dl.error}</span>
      </Banner>
    {/if}

    <section>
      <h4 class="text-xs font-semibold uppercase mb-2" style="color: var(--text-secondary)">
        {tr($locale, "detail.actions")}
      </h4>
      <div class="flex gap-2 flex-wrap">
        {#if canPause(dl)}
          <Button variant="soft" tone="warning" size="sm" iconLeft="pause"
                  disabled={actions.busy} onclick={() => actions.pause()}>
            {tr($locale, "action.pause")}
          </Button>
        {:else if canResume(dl)}
          <Button variant="soft" tone="accent" size="sm" iconLeft="play"
                  disabled={actions.busy} onclick={() => actions.resume()}>
            {tr($locale, "action.resume")}
          </Button>
        {/if}
        {#if canRetry(dl)}
          <Button variant="soft" tone="accent" size="sm" iconLeft="refresh"
                  disabled={actions.busy} onclick={() => actions.retry()}>
            {tr($locale, "action.retry")}
          </Button>
          {#if dl.error}
            <Button variant="soft" tone="warning" size="sm" iconLeft="flag"
                    onclick={() => actions.reportCrash()}>
              {tr($locale, "action.report")}
            </Button>
          {/if}
        {/if}
        <Button variant="soft" tone="danger" size="sm" iconLeft="trash"
                disabled={actions.busy} onclick={deleteAndClose}>
          {actions.confirmingDelete ? tr($locale, "action.sure") : tr($locale, "action.delete")}
        </Button>
      </div>
    </section>
  </div>
{:else}
  <!-- The panel used to have no else-branch, so a download deleted from
       another client (or pruned on completion) left the body blank while the
       header stayed put. -->
  <EmptyState
    size="sm"
    icon="file"
    title={tr($locale, "detail.gone")}
    description={tr($locale, "detail.gone_hint")}
  >
    {#snippet action()}
      <Button variant="outline" size="sm" onclick={closeSidePanel}>
        {tr($locale, "common.close")}
      </Button>
    {/snippet}
  </EmptyState>
{/if}
