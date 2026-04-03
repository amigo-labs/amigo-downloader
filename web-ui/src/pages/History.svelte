<script lang="ts">
  import { onMount } from "svelte";
  import { getHistory, formatBytes, formatRelativeTime } from "../lib/api";
  import SkeletonCard from "../components/SkeletonCard.svelte";

  let history = $state<any[]>([]);
  let loading = $state(true);

  onMount(async () => {
    try {
      history = await getHistory();
    } catch (e) { console.error("Failed to load history:", e); }
    loading = false;
  });
</script>

<div class="space-y-4">
  {#if loading}
    <SkeletonCard count={3} />
  {:else if history.length === 0}
    <div class="flex flex-col items-center justify-center py-20">
      <img src="/amigo-logo.png" alt="" width="48" height="48" class="rounded-lg opacity-30" />
      <p class="mt-4 text-sm" style="color: var(--text-secondary)">No download history yet</p>
      <p class="text-xs mt-1" style="color: var(--text-secondary); opacity: 0.5">Completed downloads will appear here</p>
    </div>
  {:else}
    <div class="space-y-2">
      {#each history as item}
        <div
          class="download-card flex items-center gap-4 rounded-xl px-4 py-3"
          style="border: 1px solid var(--border-color)"
        >
          <div class="flex-1 min-w-0">
            <p class="font-medium truncate text-sm" style="color: var(--text-primary)">{item.filename || item.url}</p>
            <p class="text-xs" style="color: var(--text-secondary)">
              {item.filesize ? formatBytes(item.filesize) : "\u2014"} &middot; {formatRelativeTime(item.created_at)}
            </p>
          </div>
          <!-- Fix M9: use neon-success instead of hardcoded text-green-500 -->
          <span class="text-xs font-semibold" style="color: var(--neon-success)">Completed</span>
        </div>
      {/each}
    </div>
  {/if}
</div>
