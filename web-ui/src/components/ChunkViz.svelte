<script lang="ts">
  // Visualizes parallel chunk download progress
  let { chunks = 8, progress = 0, active = false, size = "compact" }:
    { chunks?: number; progress?: number; active?: boolean; size?: "compact" | "detailed" } = $props();

  let chunkStates = $derived(
    Array.from({ length: chunks }, (_, i) => {
      if (!active || progress === 0) return 0;
      if (progress >= 100) return 100;
      const base = progress / 100;
      const offset = (i / chunks) * 0.3;
      const chunkProgress = Math.min(1, Math.max(0, (base - offset) / (1 - offset * 0.5)));
      return Math.round(chunkProgress * 100);
    })
  );
</script>

<div
  class="flex gap-0.5 rounded overflow-hidden"
  class:h-1={size === "compact"}
  class:h-3={size === "detailed"}
  style="background: rgba(255, 255, 255, 0.04)"
>
  {#each chunkStates as cp, i}
    <div class="flex-1 relative overflow-hidden rounded-sm">
      <div
        class="absolute inset-0"
        class:chunk-pulse={active && cp > 0 && cp < 100}
        style="
          background: var(--neon-primary);
          opacity: {cp > 0 ? 0.2 + (cp / 100) * 0.6 : 0.03};
          width: {cp}%;
        "
      ></div>
      {#if size === "detailed"}
        <span class="absolute inset-0 flex items-center justify-center text-[8px]" style="font-family: 'Share Tech Mono', monospace; color: var(--text-secondary)">
          {cp}%
        </span>
      {/if}
    </div>
  {/each}
</div>

<style>
  @keyframes chunk-pulse {
    0%, 100% { opacity: 0.6; }
    50% { opacity: 0.9; }
  }

  .chunk-pulse {
    animation: chunk-pulse 1s ease-in-out infinite;
  }
</style>
