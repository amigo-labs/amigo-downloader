<script lang="ts">
  // Visualizes parallel chunk download progress.
  //
  // The previous "detailed" mode painted an 8px percentage in --text-secondary
  // on top of the accent fill — too small to read and with unpredictable
  // contrast. Per-chunk numbers now live in the native tooltip, and the bar
  // itself is a labelled image rather than a silent block of colour.
  let {
    chunks = 8,
    progress = 0,
    active = false,
    size = "compact",
    label,
  }: {
    chunks?: number;
    progress?: number;
    active?: boolean;
    size?: "compact" | "detailed";
    /** Accessible name. Omitted -> hidden from AT, correct when a sibling
        ProgressBar already reports the same value. */
    label?: string;
  } = $props();

  let chunkStates = $derived(
    Array.from({ length: chunks }, (_, i) => {
      if (!active || progress === 0) return 0;
      if (progress >= 100) return 100;
      const base = progress / 100;
      const offset = (i / chunks) * 0.3;
      const chunkProgress = Math.min(1, Math.max(0, (base - offset) / (1 - offset * 0.5)));
      return Math.round(chunkProgress * 100);
    }),
  );
</script>

<div
  class="chunks {size}"
  role={label ? "img" : "presentation"}
  aria-label={label}
  aria-hidden={label ? undefined : "true"}
>
  {#each chunkStates as cp, i}
    <div class="cell" title="Chunk {i + 1}: {cp}%">
      <div
        class="fill"
        class:chunk-pulse={active && cp > 0 && cp < 100}
        style:opacity={cp > 0 ? 0.35 + (cp / 100) * 0.65 : 0.08}
        style:width="{cp}%"
      ></div>
    </div>
  {/each}
</div>

<style>
  .chunks {
    display: flex;
    gap: 1px;
    border-radius: var(--radius-xs);
    overflow: hidden;
    background: var(--border-color);
  }

  .chunks.compact { height: 4px; }
  .chunks.detailed { height: 10px; }

  .cell {
    position: relative;
    flex: 1;
    overflow: hidden;
  }

  .fill {
    position: absolute;
    inset: 0 auto 0 0;
    background: var(--accent-solid);
    transition: width var(--dur-base) var(--ease-out);
  }

  @keyframes chunk-pulse {
    0%, 100% { opacity: 0.6; }
    50% { opacity: 0.95; }
  }

  .chunk-pulse {
    animation: chunk-pulse 1s ease-in-out infinite;
  }
</style>
