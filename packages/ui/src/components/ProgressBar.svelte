<script lang="ts">
  let {
    value,
    max = 100,
    size = "sm",
    indeterminate = false,
    active = false,
    tone = "accent",
    label,
    valueText,
    class: className = "",
  }: {
    value: number;
    max?: number;
    size?: "xs" | "sm" | "md";
    indeterminate?: boolean;
    /** Adds the shimmer used for in-flight downloads. */
    active?: boolean;
    tone?: "accent" | "success" | "warning" | "danger";
    /** Accessible name. Omit only when an adjacent element already labels it. */
    label?: string;
    /** Human-readable value, e.g. "1.2 GB of 4 GB". Falls back to a percentage. */
    valueText?: string;
    class?: string;
  } = $props();

  const pct = $derived(Math.max(0, Math.min(100, (value / max) * 100)));
</script>

<div
  class="track {size} {tone} {className}"
  role="progressbar"
  aria-label={label}
  aria-valuemin={indeterminate ? undefined : 0}
  aria-valuemax={indeterminate ? undefined : max}
  aria-valuenow={indeterminate ? undefined : Math.round(value)}
  aria-valuetext={indeterminate ? undefined : (valueText ?? `${Math.round(pct)}%`)}
>
  <div
    class="fill"
    class:active
    class:indeterminate
    style:width={indeterminate ? undefined : `${pct}%`}
  ></div>
</div>

<style>
  .track {
    --pb-fill: var(--accent-solid);
    width: 100%;
    border-radius: var(--radius-full);
    background: var(--border-color);
    overflow: hidden;
  }

  .track.success { --pb-fill: var(--success-ink); }
  .track.warning { --pb-fill: var(--warning-ink); }
  .track.danger { --pb-fill: var(--danger-ink); }

  .track.xs { height: 3px; }
  .track.sm { height: 4px; }
  .track.md { height: 8px; }

  .fill {
    height: 100%;
    border-radius: inherit;
    background: var(--pb-fill);
    transition: width var(--dur-base) var(--ease-out);
  }

  .fill.active {
    background: linear-gradient(
      90deg,
      var(--pb-fill) 0%,
      color-mix(in srgb, var(--pb-fill) 70%, white) 50%,
      var(--pb-fill) 100%
    );
    background-size: 200% 100%;
    animation: pb-shimmer 2s linear infinite;
  }

  .fill.indeterminate {
    width: 35%;
    animation: pb-slide 1.4s var(--ease-out) infinite;
  }

  @keyframes pb-shimmer {
    0% { background-position: -200% 0; }
    100% { background-position: 200% 0; }
  }

  @keyframes pb-slide {
    0% { transform: translateX(-100%); }
    100% { transform: translateX(320%); }
  }
</style>
