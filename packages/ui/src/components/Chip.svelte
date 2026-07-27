<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  let {
    selected = false,
    count,
    icon,
    size = "md",
    disabled = false,
    class: className = "",
    children,
    ...rest
  }: {
    selected?: boolean;
    /** Trailing count badge, e.g. the number of downloads in a status. */
    count?: number;
    icon?: string;
    size?: "sm" | "md";
    disabled?: boolean;
    class?: string;
    children?: Snippet;
    [key: string]: unknown;
  } = $props();
</script>

<button
  type="button"
  class="chip {size} {className}"
  class:selected
  {disabled}
  {...rest}
>
  {#if icon}<Icon name={icon} size={size === "sm" ? 12 : 14} />{/if}
  {#if children}{@render children()}{/if}
  {#if count !== undefined}
    <span class="count">{count}</span>
  {/if}
</button>

<style>
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 0.375rem;
    border: 1px solid transparent;
    border-radius: var(--radius-md);
    background: var(--bg-surface);
    color: var(--text-secondary);
    font-weight: 500;
    font-family: inherit;
    white-space: nowrap;
    cursor: pointer;
    transition:
      background-color var(--dur-fast) var(--ease-out),
      color var(--dur-fast) var(--ease-out);
  }

  .chip.sm {
    min-height: 28px;
    padding: 0 0.5rem;
    font-size: var(--font-xs);
  }

  .chip.md {
    min-height: 34px;
    padding: 0 0.75rem;
    font-size: var(--font-sm);
  }

  .chip:hover:not(:disabled) {
    background: var(--hover-bg);
    color: var(--text-primary);
  }

  .chip.selected {
    background: var(--accent-subtle);
    color: var(--accent-ink);
    border-color: color-mix(in srgb, var(--accent) 30%, transparent);
  }

  .chip:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* Tabular figures so the count doesn't jitter as it ticks. */
  .count {
    font-variant-numeric: tabular-nums;
    opacity: 0.65;
  }
</style>
