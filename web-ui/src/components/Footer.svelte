<script lang="ts">
  import { stats, crashReport } from "../lib/stores";
  import { formatSpeed } from "../lib/api";
  import Sparkline from "./Sparkline.svelte";

  let { speedHistory = [] }: { speedHistory?: number[] } = $props();
</script>

<footer
  class="relative shrink-0 flex items-center gap-4 px-4 border-t neon-top-line"
  style="height: 36px; background: var(--bg-surface); border-color: var(--border-color); font-family: 'Share Tech Mono', monospace; font-size: 12px"
>
  <!-- Mini sparkline -->
  <div class="shrink-0">
    <Sparkline values={speedHistory} width={100} height={20} color="var(--neon-primary)" />
  </div>

  <div class="w-px h-4" style="background: var(--border-color)"></div>

  <!-- Speed -->
  <span style="color: var(--text-secondary)">Speed</span>
  <span style="color: var(--neon-primary)">{formatSpeed($stats.speed_bytes_per_sec)}</span>

  <div class="w-px h-4" style="background: var(--border-color)"></div>

  <!-- Active -->
  <span style="color: var(--text-secondary)">Active</span>
  <span style="color: var(--text-primary)">
    {$stats.active_downloads}
    {#if $stats.active_downloads > 0}
      <span class="inline-block w-1.5 h-1.5 rounded-full ml-0.5 status-pulse" style="background: var(--neon-primary)"></span>
    {/if}
  </span>

  <div class="w-px h-4" style="background: var(--border-color)"></div>

  <!-- Queued -->
  <span style="color: var(--text-secondary)">Queued</span>
  <span style="color: var(--text-primary)">{$stats.queued}</span>

  <div class="w-px h-4" style="background: var(--border-color)"></div>

  <!-- Completed -->
  <span style="color: var(--text-secondary)">Done</span>
  <span style="color: var(--neon-success)">{$stats.completed}</span>

  <!-- Feedback — right aligned -->
  <div class="flex-1"></div>
  <button
    onclick={() => crashReport.set(null)}
    class="transition-opacity hover:opacity-80"
    style="color: var(--text-secondary); opacity: 0.6; font-size: 11px; font-family: 'Rajdhani', sans-serif"
  >
    Feedback
  </button>
</footer>
