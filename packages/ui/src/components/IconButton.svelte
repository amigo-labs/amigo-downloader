<script lang="ts">
  import Icon from "./Icon.svelte";

  // `label` is deliberately required rather than optional: icon-only buttons
  // with no accessible name were the single most common a11y defect in this
  // codebase, and a required prop makes that mistake a type error.
  let {
    icon,
    label,
    size = "md",
    tone = "neutral",
    pressed,
    disabled = false,
    class: className = "",
    ...rest
  }: {
    icon: string;
    /** Accessible name. Required — never omit. */
    label: string;
    /** md guarantees a 44x44 touch target; sm is for dense rows only. */
    size?: "sm" | "md";
    tone?: "neutral" | "accent" | "success" | "warning" | "danger";
    /** Set for toggle buttons; renders aria-pressed. */
    pressed?: boolean;
    disabled?: boolean;
    class?: string;
    [key: string]: unknown;
  } = $props();

  const iconSize = $derived(size === "sm" ? 14 : 18);
</script>

<button
  type="button"
  class="icon-button {size} {tone} {className}"
  class:is-pressed={pressed}
  aria-label={label}
  aria-pressed={pressed}
  title={label}
  {disabled}
  {...rest}
>
  <Icon name={icon} size={iconSize} />
</button>

<style>
  .icon-button {
    --ib-ink: var(--text-secondary);

    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: none;
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--ib-ink);
    cursor: pointer;
    transition:
      background-color var(--dur-fast) var(--ease-out),
      color var(--dur-fast) var(--ease-out);
  }

  .icon-button.accent {
    --ib-ink: var(--accent-ink);
  }
  .icon-button.success {
    --ib-ink: var(--success-ink);
  }
  .icon-button.warning {
    --ib-ink: var(--warning-ink);
  }
  .icon-button.danger {
    --ib-ink: var(--danger-ink);
  }

  .icon-button.md {
    min-width: 44px;
    min-height: 44px;
  }

  /* sm still clears 32px; use only where a 44px target would break the row. */
  .icon-button.sm {
    min-width: 32px;
    min-height: 32px;
  }

  .icon-button:hover:not(:disabled) {
    background: var(--hover-bg);
    color: var(--accent-ink);
  }

  .icon-button.is-pressed {
    background: var(--accent-subtle);
    color: var(--accent-ink);
  }

  .icon-button:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
</style>
