<script lang="ts">
  import { formatBytes, formatSpeed } from "../../lib/api";
  import { openDetailPanel, selectedDownloadId, type Download } from "../../lib/stores";
  import { locale, tr } from "../../lib/i18n";
  import {
    fileIcon, progressPct, statusInk, statusFill, statusTone,
    isActive as isActiveFn, canPause, canResume, canRetry, etaSeconds, formatEta, displayName,
    downloadStatusKey, failureReason, urlHost,
  } from "../../lib/download";
  import { createDownloadActions } from "../../lib/downloadActions.svelte";
  import { handleCheckboxClick } from "../../lib/selection";
  import { selectedIds } from "../../lib/stores";
  import { dragHandle, dropTarget } from "../../lib/dnd";
  import ContextMenu from "../ContextMenu.svelte";
  import Icon from "@amigo-labs/krake-ui/components/Icon.svelte";
  import IconButton from "@amigo-labs/krake-ui/components/IconButton.svelte";
  import ChunkViz from "@amigo-labs/krake-ui/components/ChunkViz.svelte";
  import ProgressBar from "@amigo-labs/krake-ui/components/ProgressBar.svelte";

  // One component, two densities. Two components is exactly what let the
  // compact row silently lose the context menu, copy-URL, crash reporting,
  // ETA, byte counters, selection highlight, reorder and Undo-on-delete; a
  // variant prop makes that divergence impossible to reintroduce.
  let {
    download,
    index = 0,
    setSize = 0,
    variant = "card",
    reorderEnabled = false,
    orderedIds = [],
    grabbed = false,
    onReorder,
    onGrabKey,
  }: {
    download: Download;
    index?: number;
    setSize?: number;
    variant?: "card" | "row";
    reorderEnabled?: boolean;
    orderedIds?: string[];
    /** True while a keyboard reorder grab is active on this item. */
    grabbed?: boolean;
    onReorder?: (draggedId: string, targetId: string) => void;
    onGrabKey?: (e: KeyboardEvent, id: string) => void;
  } = $props();

  const actions = createDownloadActions(() => download, () => $locale);

  let progress = $derived(progressPct(download));
  let active = $derived(isActiveFn(download));
  let ink = $derived(statusInk(download.status));
  let label = $derived(displayName(download));
  let statusText = $derived(tr($locale, downloadStatusKey(download)));
  let failure = $derived(failureReason(download));
  let host = $derived(urlHost(download.url));
  let eta = $derived(formatEta(etaSeconds(download)));
  let isOpen = $derived($selectedDownloadId === download.id);
  let checked = $derived($selectedIds.has(download.id));

  let contextMenu = $state<{ x: number; y: number } | null>(null);

  function openMenu(e: MouseEvent) {
    e.preventDefault();
    contextMenu = { x: e.clientX, y: e.clientY };
  }

  // Long-press opens the same menu on touch, where `contextmenu` never fires.
  // The old code bailed out on `"ontouchstart" in window`, which also matched
  // Windows touchscreen laptops -- those lost the menu while still having a
  // right mouse button, and touch devices got no replacement at all.
  let pressTimer: ReturnType<typeof setTimeout> | undefined;
  function onPointerDown(e: PointerEvent) {
    if (e.pointerType !== "touch") return;
    const { clientX, clientY } = e;
    pressTimer = setTimeout(() => (contextMenu = { x: clientX, y: clientY }), 500);
  }
  function cancelPress() {
    clearTimeout(pressTimer);
  }
</script>

<!--
  A plain container with real controls inside, not a `<div role="button">`.
  The old root put an <h3> and five <button>s inside a button role: invalid
  ARIA, no heading in the document outline, and an accessible name that was
  the entire card read out as one string. The owning <li> lives in
  DownloadList, where animate:flip needs it.

  `.dl-fit` is the size container the row lays itself out by: the list is
  ~380px wide next to the docked side panel on a 1024px screen, so the
  viewport says nothing about the room a row has. The context menu stays
  outside it, because a size container may become the containing block of
  fixed-position descendants.
-->
<div class="dl-fit">
<div
  class="dl-item {variant}"
  class:is-open={isOpen}
  class:is-checked={checked}
  class:is-grabbed={grabbed}
  role="group"
  aria-label={label}
  style="--i: {index}; --status-ink: {ink}; --status-fill: {statusFill(download.status)}"
  oncontextmenu={openMenu}
  onpointerdown={onPointerDown}
  onpointerup={cancelPress}
  onpointercancel={cancelPress}
  onpointermove={cancelPress}
  use:dropTarget={{ id: download.id, enabled: reorderEnabled, onReorder: (a, b) => onReorder?.(a, b) }}
>
  <input
    type="checkbox"
    class="dl-check"
    {checked}
    aria-label={tr($locale, "downloads.select_item", { name: label })}
    onclick={(e) => handleCheckboxClick(download.id, e, orderedIds)}
  />

  <button
    type="button"
    class="dl-grip"
    disabled={!reorderEnabled}
    draggable={reorderEnabled}
    aria-pressed={reorderEnabled ? grabbed : undefined}
    aria-label={reorderEnabled
      ? tr($locale, "downloads.reorder_handle", { name: label, pos: index + 1, total: setSize })
      : tr($locale, "downloads.reorder_locked")}
    title={reorderEnabled ? undefined : tr($locale, "downloads.reorder_locked")}
    onkeydown={(e) => onGrabKey?.(e, download.id)}
    use:dragHandle={{ id: download.id, enabled: reorderEnabled, onReorder: () => {} }}
  >
    <Icon name="grip" size={14} />
  </button>

  {#if active}
    <span class="dl-bar" aria-hidden="true"></span>
  {/if}

  <div class="dl-body">
    <div class="dl-head">
      <Icon name={fileIcon(download.filename)} size={14} />
      <h3 class="dl-title">
        <!-- The stretched pseudo-element keeps the whole-row click target
             while leaving the accessible name as just the filename. -->
        <button type="button" class="dl-open" onclick={() => openDetailPanel(download.id)}>
          {label}
        </button>
      </h3>
    </div>

    <!-- Name and source are what identify a download; the full URL with
         its path is in the detail panel and in this tooltip. -->
    <div class="dl-sub">
      <span class="dl-status">{statusText}</span>
      <span class="dl-host" title={download.url}>{host}</span>
    </div>

    {#if failure}
      <p class="dl-error" title={failure}>{failure}</p>
    {/if}

    <div class="dl-progress">
      {#if variant === "card" && (active || (download.status === "paused" && progress > 0))}
        <ChunkViz chunks={8} {progress} {active} />
      {:else}
        <ProgressBar
          value={progress}
          tone={statusTone(download.status)}
          {active}
          label={tr($locale, "downloads.progress_of", { name: label })}
          valueText="{progress}%"
        />
      {/if}
    </div>

    <div class="dl-meta">
      <span class="dl-bytes">
        {formatBytes(download.bytes_downloaded)}{download.filesize ? ` / ${formatBytes(download.filesize)}` : ""}
      </span>
      {#if active && download.speed > 0}
        <span class="dl-speed">{formatSpeed(download.speed)}</span>
      {/if}
      {#if progress > 0}<span class="dl-pct">{progress}%</span>{/if}
      {#if eta}<span class="dl-eta">{eta}</span>{/if}
    </div>
  </div>

  <div class="dl-actions">
    <IconButton
      icon="copy"
      size={variant === "row" ? "sm" : "md"}
      label={tr($locale, "action.copy_named", { name: label })}
      onclick={() => actions.copyUrl()}
    />
    {#if canRetry(download)}
      <IconButton
        icon="refresh"
        tone="accent"
        size={variant === "row" ? "sm" : "md"}
        label={tr($locale, "action.retry_named", { name: label })}
        disabled={actions.busy}
        onclick={() => actions.retry()}
      />
      {#if download.error}
        <IconButton
          icon="flag"
          tone="warning"
          size={variant === "row" ? "sm" : "md"}
          label={tr($locale, "action.report_named", { name: label })}
          onclick={() => actions.reportCrash()}
        />
      {/if}
    {/if}
    {#if canPause(download)}
      <IconButton
        icon="pause"
        size={variant === "row" ? "sm" : "md"}
        label={tr($locale, "action.pause_named", { name: label })}
        disabled={actions.busy}
        onclick={() => actions.pause()}
      />
    {:else if canResume(download)}
      <IconButton
        icon="play"
        size={variant === "row" ? "sm" : "md"}
        label={tr($locale, "action.resume_named", { name: label })}
        disabled={actions.busy}
        onclick={() => actions.resume()}
      />
    {/if}
    <!-- The label tracks the two-step state; a static label would override
         the visible "Sure?" and leave screen-reader users with no signal. -->
    <IconButton
      icon="trash"
      tone="danger"
      size={variant === "row" ? "sm" : "md"}
      pressed={actions.confirmingDelete || undefined}
      label={actions.confirmingDelete
        ? tr($locale, "action.confirm_delete_named", { name: label })
        : tr($locale, "action.delete_named", { name: label })}
      disabled={actions.busy}
      onclick={() => actions.requestDelete()}
    />
  </div>
</div>
</div>

{#if contextMenu}
  <ContextMenu
    x={contextMenu.x}
    y={contextMenu.y}
    items={actions.contextMenuItems()}
    onclose={() => (contextMenu = null)}
  />
{/if}

<style>
  .dl-fit {
    container: dl-fit / inline-size;
  }

  .dl-item {
    position: relative;
    display: flex;
    align-items: center;
    gap: 0.75rem;
    background: var(--bg-surface);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-lg);
    transition:
      background-color var(--dur-fast) var(--ease-out),
      border-color var(--dur-fast) var(--ease-out);
  }

  .dl-item.card {
    align-items: flex-start;
    padding: var(--space-4);
    animation: card-enter 0.35s var(--ease-out) both;
    animation-delay: calc(var(--i, 0) * 40ms);
  }

  .dl-item.row {
    padding: 0.375rem 0.75rem;
    border-radius: var(--radius-md);
  }

  .dl-item:hover {
    background: var(--bg-surface-2);
  }

  .dl-item.is-open {
    border-color: var(--status-ink);
  }

  .dl-item.is-checked {
    background: var(--accent-subtle);
  }

  .dl-item.is-grabbed {
    border-color: var(--accent-ink);
    box-shadow: var(--neon-glow-md);
  }

  :global(.dl-item.drop-target-active) {
    border-color: var(--accent-ink);
    box-shadow: inset 0 2px 0 var(--accent);
  }

  /* Checkbox and grip stay above the stretched title link. */
  .dl-check,
  .dl-grip,
  .dl-actions {
    position: relative;
    z-index: 1;
  }

  /* A real checkbox: focusable, Space-togglable, aria-checked for free.
     It fades in on hover/focus/checked so the resting list stays calm, but
     it is always in the DOM and always in the tab order. The input is a
     24px target (WCAG 2.5.8); the 18px box is drawn inside it, and the
     negative margin keeps that box where an 18px input would put it. */
  .dl-check {
    appearance: none;
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 24px;
    height: 24px;
    margin: -3px;
    border: none;
    border-radius: var(--radius-xs);
    background: transparent;
    cursor: pointer;
    opacity: 0.35;
    transition: opacity var(--dur-fast) var(--ease-out);
  }

  .dl-check::before {
    content: "";
    grid-area: 1 / 1;
    box-sizing: border-box;
    width: 18px;
    height: 18px;
    border: 2px solid var(--border-color);
    border-radius: var(--radius-xs);
    transition: background-color var(--dur-fast) var(--ease-out);
  }

  .dl-item:hover .dl-check,
  .dl-check:focus-visible,
  .dl-check:checked {
    opacity: 1;
  }

  .dl-check:checked::before {
    background: var(--accent-solid);
    border-color: var(--accent-solid);
  }

  .dl-check:checked::after {
    content: "";
    grid-area: 1 / 1;
    width: 4px;
    height: 8px;
    margin-top: -2px;
    border: solid var(--on-accent);
    border-width: 0 2px 2px 0;
    transform: rotate(45deg);
  }

  /* draggable moves onto the grip, which is what cursor-grab already promised.
     Making the whole card draggable also made text selection impossible. */
  .dl-grip {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    min-width: 24px;
    min-height: 24px;
    padding: 0;
    border: none;
    background: transparent;
    color: var(--text-muted);
    cursor: grab;
  }

  .card .dl-grip {
    margin-top: -2px;
  }

  .dl-grip:disabled {
    cursor: default;
    opacity: 0.35;
  }

  .dl-grip:active:not(:disabled) {
    cursor: grabbing;
  }

  .dl-bar {
    align-self: stretch;
    width: 2px;
    flex-shrink: 0;
    border-radius: var(--radius-full);
    background: var(--status-fill);
  }

  /* One grid for both densities: the card stacks name, host, progress and
     figures; the compact row puts the host in front of the figures. */
  .dl-body {
    flex: 1;
    min-width: 0;
    display: grid;
    align-items: center;
    grid-template-columns: minmax(0, 1fr) auto;
    grid-template-areas:
      "head status"
      "host host"
      "error error"
      "progress progress"
      "meta meta";
  }

  .row .dl-body {
    grid-template-columns: auto minmax(0, 1fr) auto;
    grid-template-areas:
      "head head status"
      "error error error"
      "progress progress progress"
      "host meta meta";
  }

  .dl-head {
    grid-area: head;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    min-width: 0;
    color: var(--text-muted);
  }

  .dl-title {
    flex: 1;
    min-width: 0;
    margin: 0;
    font-size: var(--font-sm);
    font-weight: 600;
  }

  /* The stretched ::after is the real target; the padding (taken back by
     the margin, so the line keeps its height) makes the button's own box
     24px as well. */
  .dl-open {
    display: block;
    width: 100%;
    margin: -2px 0;
    padding: 2px 0;
    border: none;
    background: transparent;
    color: var(--text-primary);
    font: inherit;
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: pointer;
  }

  .dl-open::after {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: inherit;
  }

  .dl-open:focus-visible {
    outline: none;
  }

  .dl-open:focus-visible::after {
    outline: 2px solid var(--accent-ink);
    outline-offset: -2px;
  }

  /* Status and host are one group in the markup; in a wide row they are
     placed separately on the body grid. */
  .dl-sub {
    display: contents;
  }

  .dl-status {
    grid-area: status;
    flex-shrink: 0;
    margin-left: 0.5rem;
    padding: 0.05rem 0.5rem;
    border-radius: var(--radius-full);
    font-size: 0.625rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    white-space: nowrap;
    color: var(--status-ink);
    background: color-mix(in srgb, var(--status-fill) 14%, transparent);
  }

  .dl-host {
    grid-area: host;
    margin: 0.125rem 0 0;
    font-family: var(--font-mono);
    font-size: 0.6875rem;
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .row .dl-host {
    max-width: 16rem;
    margin: 0 0.75rem 0 0;
  }

  /* Why it failed, on the row itself: the reason used to be one click
     away in the detail panel, or nowhere on a phone. */
  .dl-error {
    grid-area: error;
    margin: 0.25rem 0 0;
    font-size: 0.6875rem;
    color: var(--danger-ink);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .dl-progress {
    grid-area: progress;
    margin: 0.5rem 0;
  }

  .row .dl-progress {
    margin: 0.25rem 0 0;
  }

  .dl-meta {
    grid-area: meta;
    display: flex;
    flex-wrap: wrap;
    gap: 0.125rem 0.75rem;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: 0.6875rem;
    font-variant-numeric: tabular-nums;
    color: var(--text-secondary);
  }

  .dl-speed {
    color: var(--accent-ink);
  }

  .dl-actions {
    display: flex;
    align-items: center;
    gap: 0.125rem;
    flex-shrink: 0;
  }

  .card .dl-actions {
    align-self: flex-end;
  }

  /* The narrow layout, keyed to the width the row really has (a phone, or
     the list next to the docked side panel). Beside the name, three or
     four buttons and the status badge left the filename one or two
     characters. Now the name owns the first line, and progress and figures
     run the full width. A card gives the host its own line under the name
     and puts grip, status and actions on the next; a compact row keeps
     grip, status, host and actions on one line, where only the host
     gives way. */
  @container dl-fit (max-width: 37.5rem) {
    .dl-item,
    .dl-item.card {
      flex-wrap: wrap;
      align-items: center;
      gap: 0.25rem 0.5rem;
    }

    .dl-item.card {
      padding: var(--space-3);
    }

    .dl-body {
      display: contents;
    }

    .dl-check {
      order: 1;
      margin: 0;
    }

    /* check (24px) + gap + name fill line one exactly enough that the
       grip cannot join it. */
    .dl-head {
      order: 2;
      flex: 1 1 calc(100% - 2.5rem);
    }

    .dl-host,
    .row .dl-host {
      order: 3;
      min-width: 0;
      max-width: none;
      margin: 0;
    }

    /* Indented to sit under the name, not under the checkbox. */
    .card .dl-host {
      flex-basis: 100%;
      padding-left: 2rem;
    }

    .dl-grip,
    .card .dl-grip {
      order: 4;
      margin: 0;
    }

    .dl-status {
      order: 5;
      margin: 0;
    }

    .row .dl-sub {
      order: 4;
      display: flex;
      align-items: center;
      gap: 0.5rem;
      flex: 1 1 0;
    }

    /* Zero-width basis: the host takes what the line has left and never
       pushes the actions off it. */
    .row .dl-host {
      flex: 1 1 0;
      width: 0;
    }

    /* The buttons keep their full target; the negative margin only stops
       them from making the line taller than it reads. */
    .dl-actions,
    .card .dl-actions {
      order: 6;
      align-self: auto;
      gap: 0;
      margin: -0.25rem 0 -0.25rem auto;
    }

    .dl-error {
      order: 7;
      flex-basis: 100%;
      margin: 0;
    }

    .dl-progress,
    .row .dl-progress {
      order: 8;
      flex-basis: 100%;
      margin: 0.125rem 0;
    }

    .dl-meta {
      order: 9;
      flex-basis: 100%;
    }

    /* The status bar becomes an edge marker instead of a column. */
    .dl-bar {
      position: absolute;
      left: -1px;
      top: 0.75rem;
      bottom: 0.75rem;
      align-self: auto;
    }
  }

  /* On the narrowest phones the figures would wrap, and grip, status and
     three actions would not share a line: the ETA gives way and the card
     gives up some padding. */
  @container dl-fit (max-width: 21rem) {
    .dl-item,
    .dl-item.card {
      column-gap: 0.375rem;
    }

    .dl-item.card {
      padding: var(--space-2);
    }

    .dl-eta {
      display: none;
    }
  }
</style>
