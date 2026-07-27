<script lang="ts">
  let { progress = 0, size = 20, stroke = 2, active = false, label, valueText }:
    {
      progress?: number;
      size?: number;
      stroke?: number;
      active?: boolean;
      /** Accessible name. Without it the ring is hidden from assistive tech,
          which is correct only when adjacent text already states the value. */
      label?: string;
      valueText?: string;
    } = $props();

  let radius = $derived((size - stroke) / 2);
  let circumference = $derived(2 * Math.PI * radius);
  let dashOffset = $derived(circumference - (Math.min(progress, 100) / 100) * circumference);
</script>

<svg
  width={size}
  height={size}
  class="shrink-0"
  class:ring-pulse={active}
  role={label ? "progressbar" : "presentation"}
  aria-label={label}
  aria-hidden={label ? undefined : "true"}
  aria-valuemin={label ? 0 : undefined}
  aria-valuemax={label ? 100 : undefined}
  aria-valuenow={label ? Math.round(progress) : undefined}
  aria-valuetext={label ? (valueText ?? `${Math.round(progress)}%`) : undefined}
>
  <!-- Background circle -->
  <circle
    cx={size / 2}
    cy={size / 2}
    r={radius}
    fill="none"
    stroke="var(--border-color)"
    stroke-width={stroke}
  />
  <!-- Progress arc -->
  {#if progress > 0}
    <circle
      cx={size / 2}
      cy={size / 2}
      r={radius}
      fill="none"
      stroke="var(--accent-solid)"
      stroke-width={stroke}
      stroke-linecap="round"
      stroke-dasharray={circumference}
      stroke-dashoffset={dashOffset}
      transform="rotate(-90 {size / 2} {size / 2})"
      style="transition: stroke-dashoffset 0.5s ease; filter: drop-shadow(0 0 2px var(--accent))"
    />
  {/if}
</svg>

<style>
  @keyframes ring-pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.7; }
  }

  .ring-pulse {
    animation: ring-pulse 2s ease-in-out infinite;
  }
</style>
