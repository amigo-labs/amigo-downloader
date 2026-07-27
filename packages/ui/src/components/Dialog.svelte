<script lang="ts">
  import type { Snippet } from "svelte";
  import { fade, scale, fly } from "svelte/transition";
  import { focusTrap, lockScroll } from "../actions/focusTrap.js";
  import IconButton from "./IconButton.svelte";

  let {
    title,
    description,
    size = "md",
    align = "center",
    layer = "modal",
    dismissable = true,
    closeLabel = "Close",
    showClose = true,
    isTop = true,
    class: className = "",
    onclose,
    children,
    header,
    footer,
  }: {
    title: string;
    description?: string;
    size?: "sm" | "md" | "lg" | "full";
    /** center = classic modal, top = command palette, right = side sheet. */
    align?: "center" | "top" | "right";
    layer?: "panel" | "modal" | "command" | "overlay";
    /** false blocks Escape and backdrop click — for flows that must be answered. */
    dismissable?: boolean;
    closeLabel?: string;
    showClose?: boolean;
    /** Only the topmost dialog may hold the focus trap and react to Escape. */
    isTop?: boolean;
    class?: string;
    onclose?: () => void;
    children: Snippet;
    header?: Snippet;
    footer?: Snippet;
  } = $props();

  // $props.id() may only be called once per component; derive the rest.
  const uid = $props.id();
  const titleId = `${uid}-title`;
  const descId = `${uid}-desc`;

  // Scroll lock is refcounted in the action module, so a dialog opened from
  // inside a side panel does not unlock the page when only it closes.
  $effect(() => lockScroll());

  function dismiss() {
    if (dismissable) onclose?.();
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && isTop && dismissable) {
      e.stopPropagation();
      onclose?.();
    }
  }

  const motion = $derived(
    align === "right"
      ? { fn: fly, params: { x: 320, duration: 200 } }
      : { fn: scale, params: { start: 0.97, duration: 150 } },
  );
</script>

<svelte:window onkeydown={onkeydown} />

<div class="root {align} layer-{layer}">
  <!-- Backdrop is a div, not a button: a full-screen button adds a focus stop
       to every overlay's tab ring. Escape and the close button are the
       keyboard routes out; the click here is a pointer-only convenience. -->
  <div
    class="scrim"
    transition:fade={{ duration: 120 }}
    onclick={dismiss}
    aria-hidden="true"
  ></div>

  <div
    class="panel {size} {className}"
    role="dialog"
    aria-modal="true"
    aria-labelledby={titleId}
    aria-describedby={description ? descId : undefined}
    use:focusTrap={{ active: isTop }}
    transition:motion.fn={motion.params}
  >
    <div class="head">
      <div class="head-text">
        <h2 id={titleId}>{title}</h2>
        {#if description}<p id={descId} class="desc">{description}</p>{/if}
      </div>
      {#if header}{@render header()}{/if}
      {#if showClose && dismissable}
        <IconButton icon="x" label={closeLabel} onclick={() => onclose?.()} />
      {/if}
    </div>

    <div class="body">{@render children()}</div>

    {#if footer}
      <div class="foot">{@render footer()}</div>
    {/if}
  </div>
</div>

<style>
  .root {
    position: fixed;
    inset: 0;
    display: flex;
    padding: var(--space-4);
  }

  .layer-panel { z-index: var(--z-panel); }
  .layer-modal { z-index: var(--z-modal); }
  .layer-command { z-index: var(--z-command); }
  .layer-overlay { z-index: var(--z-overlay); }

  .root.center { align-items: center; justify-content: center; }
  .root.top { align-items: flex-start; justify-content: center; padding-top: 10vh; }
  .root.right { align-items: stretch; justify-content: flex-end; padding: 0; }

  .scrim {
    position: absolute;
    inset: 0;
    background: rgb(0 0 0 / 60%);
    backdrop-filter: blur(2px);
  }

  .panel {
    position: relative;
    display: flex;
    flex-direction: column;
    width: 100%;
    background: var(--bg-surface);
    border: 1px solid var(--neon-border);
    border-radius: var(--radius-xl);
    box-shadow: var(--elev-3);
    /* dvh, not vh: the mobile keyboard shrinks the visual viewport and vh
       would push the footer buttons off-screen. */
    max-height: calc(100dvh - 2 * var(--space-4));
    overflow: hidden;
  }

  .panel.sm { max-width: 24rem; }
  .panel.md { max-width: 32rem; }
  .panel.lg { max-width: 48rem; }
  .panel.full { max-width: none; }

  .root.right .panel {
    max-width: 24rem;
    height: 100dvh;
    max-height: 100dvh;
    border-radius: 0;
    border-width: 0 0 0 1px;
    padding-bottom: env(safe-area-inset-bottom, 0px);
  }

  .head {
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
    padding: var(--space-4);
    border-bottom: 1px solid var(--border-color);
  }

  .head-text {
    flex: 1;
    min-width: 0;
  }

  h2 {
    margin: 0;
    font-size: var(--font-base);
    font-weight: 700;
    color: var(--text-primary);
  }

  .desc {
    margin: 0.25rem 0 0;
    font-size: var(--font-xs);
    color: var(--text-secondary);
  }

  /* The body scrolls, not the page — otherwise the footer is unreachable on a
     short viewport with the software keyboard open. */
  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: var(--space-4);
  }

  .foot {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--space-2);
    padding: var(--space-3) var(--space-4);
    border-top: 1px solid var(--border-color);
  }
</style>
