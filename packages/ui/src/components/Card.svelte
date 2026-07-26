<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    elevation = 1,
    padding = "md",
    interactive = false,
    selected = false,
    neon = true,
    as = "div",
    class: className = "",
    children,
    ...rest
  }: {
    elevation?: 0 | 1 | 2 | 3;
    padding?: "none" | "sm" | "md" | "lg";
    /** Adds hover affordance. Does NOT make the card a button — put a real
        control inside and stretch it, so nested interactives stay legal. */
    interactive?: boolean;
    selected?: boolean;
    /** Opt out for surfaces that should stay flat (e.g. inside a modal). */
    neon?: boolean;
    as?: "div" | "article" | "section" | "li";
    class?: string;
    children: Snippet;
    [key: string]: unknown;
  } = $props();
</script>

<svelte:element
  this={as}
  class="card pad-{padding} elev-{elevation} {className}"
  class:neon
  class:interactive
  class:selected
  {...rest}
>
  {@render children()}
</svelte:element>

<style>
  .card {
    position: relative;
    background: var(--bg-surface);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-lg);
    transition:
      background-color var(--dur-fast) var(--ease-out),
      border-color var(--dur-base) var(--ease-out),
      box-shadow var(--dur-base) var(--ease-out);
  }

  .card.neon {
    border-color: var(--neon-border);
  }

  .card.elev-0 { box-shadow: var(--elev-0); }
  .card.elev-1 { box-shadow: var(--elev-1); }
  .card.elev-2 { box-shadow: var(--elev-2); }
  .card.elev-3 { box-shadow: var(--elev-3); }

  .card.pad-none { padding: 0; }
  .card.pad-sm { padding: var(--space-3); }
  .card.pad-md { padding: var(--space-4); }
  .card.pad-lg { padding: var(--space-6); }

  .card.interactive:hover {
    background: var(--bg-surface-2);
  }

  .card.interactive.neon:hover {
    border-color: var(--neon-border-hover);
    box-shadow: var(--neon-glow-md);
  }

  .card.selected {
    border-color: var(--accent-ink);
    box-shadow: inset 3px 0 0 var(--accent), var(--neon-glow-sm);
  }

  /* CRT scanlines at full neon intensity, matching .neon-card in tokens.css */
  :global(.neon-full) .card.neon::after {
    content: "";
    position: absolute;
    inset: 0;
    pointer-events: none;
    border-radius: inherit;
    z-index: 1;
    background: repeating-linear-gradient(
      to bottom,
      transparent 0px,
      transparent 3px,
      color-mix(in srgb, var(--accent) 3%, transparent) 3px,
      color-mix(in srgb, var(--accent) 3%, transparent) 4px
    );
  }
</style>
