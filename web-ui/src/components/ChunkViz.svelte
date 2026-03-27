<script lang="ts">
  // Visualizes parallel chunk download progress
  // Each chunk is a segment of the bar with independent progress
  let { chunks = 8, progress = 0, active = false }:
    { chunks?: number; progress?: number; active?: boolean } = $props();

  // Simulate individual chunk progress based on overall progress
  let chunkStates = $derived(
    Array.from({ length: chunks }, (_, i) => {
      if (!active || progress === 0) return 0;
      if (progress >= 100) return 100;

      // Stagger: earlier chunks are further along
      const base = progress / 100;
      const offset = (i / chunks) * 0.3;
      const chunkProgress = Math.min(1, Math.max(0, (base - offset) / (1 - offset * 0.5)));
      return Math.round(chunkProgress * 100);
    })
  );
</script>

<div class="flex gap-0.5 h-2 rounded overflow-hidden" style="background: var(--surface-3-color)">
  {#each chunkStates as cp, i}
    <div class="flex-1 relative overflow-hidden rounded-sm">
      <div
        class="absolute inset-0 transition-all duration-500"
        class:chunk-pulse={active && cp > 0 && cp < 100}
        style="
          background: var(--accent-color);
          opacity: {cp > 0 ? 0.3 + (cp / 100) * 0.7 : 0.08};
          width: {cp}%;
        "
      ></div>
    </div>
  {/each}
</div>

<style>
  @keyframes chunk-pulse {
    0%, 100% { opacity: 0.7; }
    50% { opacity: 1; }
  }

  .chunk-pulse {
    animation: chunk-pulse 1s ease-in-out infinite;
  }
</style>
