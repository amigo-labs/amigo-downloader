<script lang="ts">
  let { progress = 0, size = 20, stroke = 2, active = false }:
    { progress?: number; size?: number; stroke?: number; active?: boolean } = $props();

  let radius = $derived((size - stroke) / 2);
  let circumference = $derived(2 * Math.PI * radius);
  let dashOffset = $derived(circumference - (Math.min(progress, 100) / 100) * circumference);
</script>

<svg width={size} height={size} class="shrink-0" class:ring-pulse={active}>
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
      stroke="var(--neon-primary)"
      stroke-width={stroke}
      stroke-linecap="round"
      stroke-dasharray={circumference}
      stroke-dashoffset={dashOffset}
      transform="rotate(-90 {size / 2} {size / 2})"
      style="transition: stroke-dashoffset 0.5s ease; filter: drop-shadow(0 0 2px var(--neon-primary))"
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
