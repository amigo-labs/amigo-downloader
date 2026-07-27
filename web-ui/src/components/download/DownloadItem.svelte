<script lang="ts">
  import { formatBytes, formatSpeed } from "../../lib/api";
  import { openDetailPanel, selectedDownloadId, type Download } from "../../lib/stores";
  import { locale, tr } from "../../lib/i18n";
  import {
    fileIcon, progressPct, statusInk, statusFill, statusTone, statusLabelKey,
    isActive as isActiveFn, canPause, canResume, canRetry, etaSeconds, formatEta, displayName,
  } from "../../lib/download";
  import { createDownloadActions } from "../../lib/downloadActions.svelte";
  import { handleCheckboxClick } from "../../lib/selection";
  import { selectedIds } from "../../lib/stores";
  import { dragHandle, dropTarget } from "../../lib/dnd";
  import ContextMenu from "../ContextMenu.svelte";
  import Icon from "@amigo/ui/components/Icon.svelte";
  import IconButton from "@amigo/ui/components/IconButton.svelte";
  import ChunkViz from "@amigo/ui/components/ChunkViz.svelte";
  import ProgressBar from "@amigo/ui/components/ProgressBar.svelte";

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
  let statusText = $derived(tr($locale, statusLabelKey(download.status)));
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
-->
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
      <span class="dl-status">{statusText}</span>
    </div>

    {#if variant === "card"}
      <p class="dl-url">{download.url}</p>
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

{#if contextMenu}
  <ContextMenu
    x={contextMenu.x}
    y={contextMenu.y}
    items={actions.contextMenuItems()}
    onclose={() => (contextMenu = null)}
  />
{/if}

<style>
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
     it is always in the DOM and always in the tab order. */
  .dl-check {
    appearance: none;
    flex-shrink: 0;
    width: 18px;
    height: 18px;
    margin: 0;
    border: 2px solid var(--border-color);
    border-radius: var(--radius-xs);
    background: transparent;
    cursor: pointer;
    opacity: 0.35;
    transition: opacity var(--dur-fast) var(--ease-out),
                background-color var(--dur-fast) var(--ease-out);
  }

  .dl-item:hover .dl-check,
  .dl-check:focus-visible,
  .dl-check:checked {
    opacity: 1;
  }

  .dl-check:checked {
    background: var(--accent-solid);
    border-color: var(--accent-solid);
  }

  .dl-check:checked::after {
    content: "";
    display: block;
    width: 4px;
    height: 8px;
    margin: 1px auto 0;
    border: solid var(--on-accent);
    border-width: 0 2px 2px 0;
    transform: rotate(45deg);
  }

  /* draggable moves onto the grip, which is what cursor-grab already promised.
     Making the whole card draggable also made text selection impossible. */
  .dl-grip {
    display: flex;
    align-items: center;
    flex-shrink: 0;
    padding: 0.25rem;
    border: none;
    background: transparent;
    color: var(--text-muted);
    cursor: grab;
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

  .dl-body {
    flex: 1;
    min-width: 0;
  }

  .dl-head {
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

  .dl-open {
    display: block;
    width: 100%;
    padding: 0;
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

  .dl-status {
    flex-shrink: 0;
    padding: 0.05rem 0.5rem;
    border-radius: var(--radius-full);
    font-size: 0.625rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--status-ink);
    background: color-mix(in srgb, var(--status-fill) 14%, transparent);
  }

  .dl-url {
    margin: 0.125rem 0 0;
    font-family: var(--font-mono);
    font-size: 0.6875rem;
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .dl-progress {
    margin: 0.5rem 0;
  }

  .row .dl-progress {
    margin: 0.25rem 0 0;
  }

  .dl-meta {
    display: flex;
    gap: 0.75rem;
    font-family: var(--font-mono);
    font-size: 0.6875rem;
    font-variant-numeric: tabular-nums;
    color: var(--text-secondary);
  }

  .dl-speed {
    color: var(--accent-ink);
  }

  /* Below the phone breakpoint the row dropped to ~420px of fixed-width
     columns and pushed the filename to zero. Secondary figures give way
     first; the filename and the actions never do. */
  @media (max-width: 639px) {
    .row .dl-pct,
    .row .dl-eta,
    .row .dl-url {
      display: none;
    }

    .row .dl-bytes {
      max-width: 9rem;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
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
</style>
