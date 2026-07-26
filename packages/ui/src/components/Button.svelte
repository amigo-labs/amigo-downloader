<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  let {
    variant = "solid",
    tone = "accent",
    size = "md",
    loading = false,
    disabled = false,
    href,
    type = "button",
    iconLeft,
    iconRight,
    full = false,
    class: className = "",
    children,
    ...rest
  }: {
    /** solid = filled CTA, soft = tinted, ghost = text-only, outline = bordered */
    variant?: "solid" | "soft" | "ghost" | "outline";
    tone?: "accent" | "success" | "warning" | "danger";
    size?: "sm" | "md" | "lg";
    loading?: boolean;
    disabled?: boolean;
    href?: string;
    type?: "button" | "submit" | "reset";
    iconLeft?: string;
    iconRight?: string;
    full?: boolean;
    class?: string;
    children: Snippet;
    [key: string]: unknown;
  } = $props();

  const iconSize = $derived(size === "sm" ? 14 : size === "lg" ? 20 : 16);
  const isDisabled = $derived(disabled || loading);
</script>

{#snippet body()}
  {#if loading}
    <span class="spinner" aria-hidden="true"></span>
  {:else if iconLeft}
    <Icon name={iconLeft} size={iconSize} />
  {/if}
  <span class="label"><!--
 -->{@render children()}<!--
--></span>
  {#if iconRight && !loading}
    <Icon name={iconRight} size={iconSize} />
  {/if}
{/snippet}

{#if href && !isDisabled}
  <a
    {href}
    class="btn {variant} {tone} {size} {className}"
    class:full
    {...rest}
  >
    {@render body()}
  </a>
{:else}
  <button
    {type}
    class="btn {variant} {tone} {size} {className}"
    class:full
    disabled={isDisabled}
    aria-busy={loading || undefined}
    {...rest}
  >
    {@render body()}
  </button>
{/if}

<style>
  .btn {
    --btn-tone: var(--accent);
    --btn-ink: var(--accent-ink);
    --btn-solid: var(--accent-solid);
    --btn-on-solid: var(--on-accent);
    --btn-subtle: var(--accent-subtle);

    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    border: 1px solid transparent;
    border-radius: var(--radius-md);
    font-weight: 600;
    font-family: inherit;
    line-height: 1.2;
    white-space: nowrap;
    cursor: pointer;
    text-decoration: none;
    transition:
      background-color var(--dur-fast) var(--ease-out),
      border-color var(--dur-fast) var(--ease-out),
      box-shadow var(--dur-fast) var(--ease-out),
      color var(--dur-fast) var(--ease-out);
  }

  .btn.full {
    width: 100%;
  }

  /* Tones remap the four accent roles; every variant below is written once
     against the role names, so adding a tone never touches variant CSS. */
  .btn.success {
    --btn-tone: var(--success);
    --btn-ink: var(--success-ink);
    --btn-solid: var(--success-ink);
    --btn-on-solid: var(--bg-deep);
    --btn-subtle: color-mix(in srgb, var(--success-ink) 12%, var(--bg-surface));
  }
  .btn.warning {
    --btn-tone: var(--warning);
    --btn-ink: var(--warning-ink);
    --btn-solid: var(--warning-ink);
    --btn-on-solid: var(--bg-deep);
    --btn-subtle: color-mix(in srgb, var(--warning-ink) 12%, var(--bg-surface));
  }
  .btn.danger {
    --btn-tone: var(--danger);
    --btn-ink: var(--danger-ink);
    --btn-solid: var(--danger-ink);
    --btn-on-solid: var(--bg-deep);
    --btn-subtle: color-mix(in srgb, var(--danger-ink) 12%, var(--bg-surface));
  }

  /* Sizes — md and lg clear the 44px touch target. */
  .btn.sm {
    min-height: 32px;
    padding: 0 0.625rem;
    font-size: var(--font-xs);
  }
  .btn.md {
    min-height: 44px;
    padding: 0 0.875rem;
    font-size: var(--font-sm);
  }
  .btn.lg {
    min-height: 48px;
    padding: 0 1.25rem;
    font-size: var(--font-base);
  }

  .btn.solid {
    background: var(--btn-solid);
    color: var(--btn-on-solid);
    box-shadow: var(--neon-glow-sm);
  }
  .btn.solid:hover:not(:disabled) {
    filter: brightness(1.1);
    box-shadow: var(--neon-glow-md);
  }

  .btn.soft {
    background: var(--btn-subtle);
    color: var(--btn-ink);
  }
  .btn.soft:hover:not(:disabled) {
    border-color: color-mix(in srgb, var(--btn-tone) 40%, transparent);
  }

  .btn.ghost {
    background: transparent;
    color: var(--text-secondary);
  }
  .btn.ghost:hover:not(:disabled) {
    background: var(--hover-bg);
    color: var(--btn-ink);
  }

  .btn.outline {
    background: transparent;
    color: var(--btn-ink);
    border-color: var(--border-color);
  }
  .btn.outline:hover:not(:disabled) {
    border-color: color-mix(in srgb, var(--btn-tone) 50%, transparent);
    background: var(--hover-bg);
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
    box-shadow: none;
    filter: none;
  }

  .spinner {
    width: 1em;
    height: 1em;
    border: 2px solid currentcolor;
    border-right-color: transparent;
    border-radius: var(--radius-full);
    animation: btn-spin 0.6s linear infinite;
  }

  @keyframes btn-spin {
    to {
      transform: rotate(360deg);
    }
  }

  /* The global prefers-reduced-motion rule collapses the duration to 0.01ms,
     which would freeze the spinner mid-rotation. Keep it turning, slowly. */
  @media (prefers-reduced-motion: reduce) {
    .spinner {
      animation-duration: 1.6s !important;
      animation-iteration-count: infinite !important;
    }
  }
</style>
