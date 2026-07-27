<script lang="ts">
  import Icon from "./Icon.svelte";

  // A radiogroup is a single tab stop with arrow-key navigation between the
  // options (WAI-ARIA APG). The hand-rolled versions this replaces declared
  // role="radiogroup" but left every option as its own tab stop with no key
  // handling — announcing a contract they did not honour.
  let {
    options,
    value = $bindable(),
    ariaLabel,
    size = "md",
    mono = false,
    class: className = "",
    onchange,
  }: {
    options: { value: string; label: string; icon?: string; count?: number }[];
    value: string;
    ariaLabel: string;
    size?: "sm" | "md";
    mono?: boolean;
    class?: string;
    onchange?: (value: string) => void;
  } = $props();

  let buttons: HTMLButtonElement[] = $state([]);

  function select(next: string, focusIt = false) {
    if (next === value) return;
    value = next;
    onchange?.(next);
    if (focusIt) {
      const i = options.findIndex((o) => o.value === next);
      queueMicrotask(() => buttons[i]?.focus());
    }
  }

  function onkeydown(e: KeyboardEvent) {
    const i = options.findIndex((o) => o.value === value);
    let next = -1;
    if (e.key === "ArrowRight" || e.key === "ArrowDown") next = (i + 1) % options.length;
    else if (e.key === "ArrowLeft" || e.key === "ArrowUp") next = (i - 1 + options.length) % options.length;
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = options.length - 1;
    else return;
    e.preventDefault();
    select(options[next].value, true);
  }
</script>

<div
  role="radiogroup"
  aria-label={ariaLabel}
  tabindex="-1"

  class="segmented {size} {className}"
  class:mono
  {onkeydown}
>
  {#each options as option, i (option.value)}
    <button
      bind:this={buttons[i]}
      type="button"
      role="radio"
      aria-checked={value === option.value}
      tabindex={value === option.value ? 0 : -1}
      class="segment"
      class:selected={value === option.value}
      onclick={() => select(option.value)}
    >
      {#if option.icon}<Icon name={option.icon} size={size === "sm" ? 12 : 14} />{/if}
      {option.label}
      {#if option.count !== undefined}<span class="count">{option.count}</span>{/if}
    </button>
  {/each}
</div>

<style>
  .segmented {
    display: inline-flex;
    align-items: stretch;
    background: var(--bg-surface-2);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-md);
    overflow: hidden;
  }

  .segment {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.375rem;
    border: none;
    background: transparent;
    color: var(--text-secondary);
    font-weight: 600;
    font-family: inherit;
    white-space: nowrap;
    cursor: pointer;
    transition:
      background-color var(--dur-fast) var(--ease-out),
      color var(--dur-fast) var(--ease-out);
  }

  .segmented.mono .segment {
    font-family: var(--font-mono);
  }

  .segmented.sm .segment {
    min-height: 30px;
    padding: 0 0.625rem;
    font-size: var(--font-xs);
  }

  .segmented.md .segment {
    min-height: 38px;
    padding: 0 0.875rem;
    font-size: var(--font-sm);
  }

  .segment:hover:not(.selected) {
    background: var(--hover-bg);
    color: var(--text-primary);
  }

  .segment.selected {
    background: var(--accent-subtle);
    color: var(--accent-ink);
  }

  /* The outline would be clipped by the container's overflow:hidden. */
  .segment:focus-visible {
    outline: none;
    box-shadow: inset 0 0 0 2px var(--accent-ink);
  }

  .count {
    font-variant-numeric: tabular-nums;
    opacity: 0.65;
  }
</style>
